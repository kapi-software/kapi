// 单节点执行：plugin 节点调用 WASM / transform 节点 Handlebars 渲染
// Single-node execution: plugin nodes invoke WASM / transform nodes render Handlebars
use std::collections::HashMap;
use std::sync::Arc;

use handlebars::Handlebars;
use serde_json::Value;
use sqlx::SqlitePool;

use crate::workflow::model::{DataBinding, NodeOutcome, WorkflowNode};
use crate::wasm::engine::WasmRuntime;

/// 执行单个节点（plugin 或 transform）
/// Execute a single node (plugin or transform)
pub async fn run_node(
    run_id: i64,
    node: WorkflowNode,
    prior_outputs: HashMap<String, Value>,
    bindings: Vec<DataBinding>,
    trigger_data: Value,
    wasm: &WasmRuntime,
    pool: &SqlitePool,
    handlebars: &Arc<Handlebars<'_>>,
) -> NodeOutcome {
    let step_id = node.id.clone();

    // Transform 节点：Handlebars 模板渲染
    // Transform node: Handlebars template rendering
    if node.node_type == "transform" {
        return run_transform_node(run_id, &node, &bindings, &prior_outputs, &trigger_data, pool, handlebars).await;
    }

    // Plugin 节点
    let plugin_id = match &node.plugin_id {
        Some(p) => p.clone(),
        None => {
            record_step_failure(pool, run_id, &node, "InvalidNode: missing plugin_id").await;
            return NodeOutcome::Failure { node_id: step_id, error: "InvalidNode: missing plugin_id".into() };
        }
    };
    let action = match &node.action {
        Some(a) => a.clone(),
        None => {
            record_step_failure(pool, run_id, &node, "InvalidNode: missing action").await;
            return NodeOutcome::Failure { node_id: step_id, error: "InvalidNode: missing action".into() };
        }
    };

    let input = assemble_input(&bindings, &prior_outputs, &trigger_data, &node.config);

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
    bindings: &[DataBinding],
    prior_outputs: &HashMap<String, Value>,
    trigger_data: &Value,
    pool: &SqlitePool,
    handlebars: &Arc<Handlebars<'_>>,
) -> NodeOutcome {
    let step_id = &node.id;
    let template = node
        .config
        .as_ref()
        .and_then(|c| c.get("template"))
        .and_then(|t| t.as_str())
        .unwrap_or("{}");

    let context = assemble_input(bindings, prior_outputs, trigger_data, &node.config);

    match handlebars.render_template(template, &context) {
        Ok(rendered) => {
            match serde_json::from_str::<Value>(&rendered) {
                Ok(output) => {
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
                    NodeOutcome::Success { node_id: step_id.clone(), output }
                }
                Err(e) => {
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
                    NodeOutcome::Failure {
                        node_id: step_id.clone(),
                        error: format!("TransformError: template output is not valid JSON: {e}"),
                    }
                }
            }
        }
        Err(e) => {
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
            NodeOutcome::Failure { node_id: step_id.clone(), error: format!("TransformError: {e}") }
        }
    }
}

/// 拼装节点输入：bindings 优先；node.config 作为默认值
/// Build node input: bindings win; node.config fields are the fallback
pub fn assemble_input(
    bindings: &[DataBinding],
    prior_outputs: &HashMap<String, Value>,
    trigger_data: &Value,
    node_config: &Option<Value>,
) -> Value {
    let mut obj = serde_json::Map::new();
    for b in bindings {
        let source_value = if b.from == "__trigger__" {
            trigger_data.get(&b.output).cloned().unwrap_or(Value::Null)
        } else {
            prior_outputs
                .get(&b.from)
                .and_then(|v| v.get(&b.output))
                .cloned()
                .unwrap_or(Value::Null)
        };
        obj.insert(b.input.clone(), source_value);
    }
    if let Some(Value::Object(cfg)) = node_config {
        for (k, v) in cfg {
            obj.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    Value::Object(obj)
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
