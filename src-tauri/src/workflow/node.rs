// 单节点执行：plugin 节点调用 WASM / transform 节点 Handlebars 渲染
// Single-node execution: plugin nodes invoke WASM / transform nodes render Handlebars
use std::collections::HashMap;
use std::sync::Arc;

use handlebars::Handlebars;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::workflow::model::{NodeOutcome, WorkflowNode};
use crate::wasm::engine::WasmRuntime;

/// 执行单个节点（plugin 或 transform）
/// Execute a single node (plugin or transform)
/// `edge_map` 是从 graph 中筛出指向本节点的边，map 字段形如 { upstream_output: downstream_input }
/// `edge_map` is the subset of edges that point to this node; map fields are { upstream_output: downstream_input }
/// `dry_run: true` 时跳过 step_log 的所有 SQL 写入（仅返回 NodeOutcome）
/// `dry_run: true` skips all step_log SQL writes (returns NodeOutcome only)
pub async fn run_node(
    run_id: i64,
    node: WorkflowNode,
    prior_outputs: HashMap<String, Value>,
    edge_map: Vec<HashMap<String, String>>,
    trigger_data: Value,
    wasm: &WasmRuntime,
    pool: &SqlitePool,
    handlebars: &Arc<Handlebars<'_>>,
    dry_run: bool,
) -> NodeOutcome {
    let step_id = node.id.clone();

    // Transform 节点：Handlebars 模板渲染
    // Transform node: Handlebars template rendering
    if node.node_type == "transform" {
        return run_transform_node(
            run_id, &node, &edge_map, &prior_outputs, &trigger_data, pool, handlebars, dry_run,
        )
        .await;
    }

    // Plugin 节点
    let plugin_id = match &node.plugin_id {
        Some(p) => p.clone(),
        None => {
            if !dry_run {
                record_step_failure(pool, run_id, &node, "InvalidNode: missing plugin_id").await;
            }
            return NodeOutcome::Failure { node_id: step_id, error: "InvalidNode: missing plugin_id".into() };
        }
    };
    let action = match &node.action {
        Some(a) => a.clone(),
        None => {
            if !dry_run {
                record_step_failure(pool, run_id, &node, "InvalidNode: missing action").await;
            }
            return NodeOutcome::Failure { node_id: step_id, error: "InvalidNode: missing action".into() };
        }
    };

    let input = assemble_input(&edge_map, &prior_outputs, &trigger_data, &node.config);

    if dry_run {
        // Dry-run: invoke WASM 但不写 step_log，直接返回 outcome
        // Dry-run: invoke WASM but skip step_log writes
        let started = std::time::Instant::now();
        let outcome = wasm.invoke_action(pool, &plugin_id, &action, &input).await;
        let _duration_ms = started.elapsed().as_millis() as i64;
        return match outcome {
            Ok(output) => NodeOutcome::Success { node_id: step_id, output },
            Err(e) => NodeOutcome::Failure { node_id: step_id, error: e },
        };
    }

    // INSERT step_log(running)
    let step_log_id = match sqlx::query(
        "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, input)
         VALUES (?, ?, ?, ?, 'running', ?)",
    )
    .bind(run_id)
    .bind(&step_id)
    .bind(&plugin_id)
    .bind(&action)
    .bind(serde_json::to_string(&input).unwrap_or_else(|_| "null".into()))
    .execute(pool)
    .await
    {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => return NodeOutcome::Failure { node_id: step_id, error: format!("StorageError: {e}") },
    };

    let started = std::time::Instant::now();
    let outcome = wasm.invoke_action(pool, &plugin_id, &action, &input).await;
    let duration_ms = started.elapsed().as_millis() as i64;

    match outcome {
        Ok(output) => {
            let output_json = serde_json::to_string(&output).unwrap_or_else(|_| "null".into());
            let _ = sqlx::query(
                "UPDATE workflow_step_logs SET status = 'success', output = ?, duration_ms = ? WHERE id = ?",
            )
            .bind(&output_json)
            .bind(duration_ms)
            .bind(step_log_id)
            .execute(pool)
            .await;
            NodeOutcome::Success { node_id: step_id, output }
        }
        Err(e) => {
            let _ = sqlx::query(
                "UPDATE workflow_step_logs SET status = 'failed', error = ?, duration_ms = ? WHERE id = ?",
            )
            .bind(&e)
            .bind(duration_ms)
            .bind(step_log_id)
            .execute(pool)
            .await;
            NodeOutcome::Failure { node_id: step_id, error: e }
        }
    }
}

/// Transform 节点执行
async fn run_transform_node(
    run_id: i64,
    node: &WorkflowNode,
    edge_map: &[HashMap<String, String>],
    prior_outputs: &HashMap<String, Value>,
    trigger_data: &Value,
    pool: &SqlitePool,
    handlebars: &Arc<Handlebars<'_>>,
    dry_run: bool,
) -> NodeOutcome {
    let step_id = &node.id;
    let template = node
        .config
        .as_ref()
        .and_then(|c| c.get("template"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");

    let context = assemble_input(edge_map, prior_outputs, trigger_data, &node.config);

    match handlebars.render_template(template, &context) {
        Ok(rendered) => match serde_json::from_str::<Value>(&rendered) {
            Ok(output) => {
                if !dry_run {
                    let output_str = serde_json::to_string(&output).unwrap_or_else(|_| "null".into());
                    let _ = sqlx::query(
                        "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, output)
                         VALUES (?, ?, ?, ?, 'success', ?)",
                    )
                    .bind(run_id)
                    .bind(step_id)
                    .bind(node.plugin_id.as_deref())
                    .bind(node.action.as_deref())
                    .bind(&output_str)
                    .execute(pool)
                    .await;
                }
                NodeOutcome::Success { node_id: step_id.clone(), output }
            }
            Err(e) => {
                if !dry_run {
                    let _ = sqlx::query(
                        "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, error)
                         VALUES (?, ?, ?, ?, 'failed', ?)",
                    )
                    .bind(run_id)
                    .bind(step_id)
                    .bind(node.plugin_id.as_deref())
                    .bind(node.action.as_deref())
                    .bind(format!("TemplateRenderError: {e}"))
                    .execute(pool)
                    .await;
                }
                NodeOutcome::Failure {
                    node_id: step_id.clone(),
                    error: format!("TransformError: template output is not valid JSON: {e}"),
                }
            }
        },
        Err(e) => {
            if !dry_run {
                let _ = sqlx::query(
                    "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, error)
                     VALUES (?, ?, ?, ?, 'failed', ?)",
                )
                .bind(run_id)
                .bind(step_id)
                .bind(node.plugin_id.as_deref())
                .bind(node.action.as_deref())
                .bind(format!("HandlebarsError: {e}"))
                .execute(pool)
                .await;
            }
            NodeOutcome::Failure { node_id: step_id.clone(), error: format!("TransformError: {e}") }
        }
    }
}

/// 拼装节点输入：从上游 outputs（按 edges.map）取值；node.config 作为缺省
/// Build node input: read upstream outputs via edges.map; node.config fields are fallback defaults
///
/// 每个 edge_map 形如 `{ upstream_output: downstream_input }`：表示把上游 outputs[upstream_output]
/// 喂给本节点 inputs[downstream_input]
/// Each edge_map is { upstream_output: downstream_input }: upstream outputs[upstream_output]
/// → this node inputs[downstream_input]
///
/// 特殊 key `__trigger__` 表示从 trigger_data 取值（保留旧 binding 行为）
/// Special key `__trigger__` reads from trigger_data (legacy binding behavior)
pub fn assemble_input(
    edge_map: &[HashMap<String, String>],
    prior_outputs: &HashMap<String, Value>,
    trigger_data: &Value,
    node_config: &Option<Value>,
) -> Value {
    let mut obj = serde_json::Map::new();
    for map in edge_map {
        for (upstream_output, downstream_input) in map {
            // __trigger__ 是保留的伪 source id；上游字段是 upstream_output
            // __trigger__ is the reserved pseudo-source id; upstream field is upstream_output
            let source_value = if upstream_output == "__trigger__" {
                trigger_data.get(downstream_input).cloned().unwrap_or(Value::Null)
            } else if let Some((source_id, field)) = upstream_output.split_once(':') {
                // 新形式 "<source_node_id>:<field>" —— 把来源 id 编码进 key，避免上游与下游字段名同名歧义
                // New form "<source_node_id>:<field>" — encode source id in the key to disambiguate
                prior_outputs
                    .get(source_id)
                    .and_then(|v| v.get(field))
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                // 旧形式：只写上游字段名，从 prior_outputs 找（单上游默认）
                // Legacy: just upstream field name; find in prior_outputs (single-upstream default)
                // 多个上游同名字段无法区分——在数据层已被 P1 提示避免
                // Multiple upstreams with the same field name are ambiguous; P1 surfaces this
                find_field_in_outputs(prior_outputs, upstream_output)
            };
            obj.insert(downstream_input.clone(), source_value);
        }
    }
    if let Some(Value::Object(cfg)) = node_config {
        for (k, v) in cfg {
            obj.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    Value::Object(obj)
}

/// 在所有上游 outputs 中查找第一个出现指定字段名的值
/// Find the first upstream output that contains the given field name
fn find_field_in_outputs(prior_outputs: &HashMap<String, Value>, field: &str) -> Value {
    for v in prior_outputs.values() {
        if let Some(inner) = v.get(field) {
            return inner.clone();
        }
    }
    Value::Null
}

async fn record_step_failure(pool: &SqlitePool, run_id: i64, node: &WorkflowNode, err: &str) {
    let _ = sqlx::query(
        "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, error)
         VALUES (?, ?, ?, ?, 'failed', ?)",
    )
    .bind(run_id)
    .bind(&node.id)
    .bind(node.plugin_id.as_deref())
    .bind(node.action.as_deref())
    .bind(err)
    .execute(pool)
    .await;
}
