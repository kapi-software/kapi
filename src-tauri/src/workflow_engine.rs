// 工作流引擎：DAG 调度 + 触发器注册表 + 两级日志（docs/WORKFLOW.md §3）
// Workflow engine: DAG scheduler + trigger registry + two-level logs (docs/WORKFLOW.md §3)
// v1：仅 manual 触发 + plugin 节点类型；transform 类型保留为占位跳过
// v1: only manual trigger + plugin node type; transform is reserved as a placeholder (skipped)
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

use crate::plugin_bridge::write_system_log;
use crate::plugin_manager::sqlite_pool;
use crate::wasm_runtime::WasmRuntime;

// ============================================================
// 触发器类型（v1 仅 Manual 有运行时实现；其余为扩展占位）
// Trigger types (v1 only Manual has runtime; others are placeholder extensions)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerType {
    Clipboard,
    Hotkey,
    Schedule,
    Manual,
    PluginEvent,
}

impl TriggerType {
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerType::Clipboard => "clipboard",
            TriggerType::Hotkey => "hotkey",
            TriggerType::Schedule => "schedule",
            TriggerType::Manual => "manual",
            TriggerType::PluginEvent => "plugin_event",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "clipboard" => Some(Self::Clipboard),
            "hotkey" => Some(Self::Hotkey),
            "schedule" => Some(Self::Schedule),
            "manual" => Some(Self::Manual),
            "plugin_event" => Some(Self::PluginEvent),
            _ => None,
        }
    }
}

// 触发器条目（v1 占位：内存注册表，后续按 trigger_type 分派）
// Trigger entry (v1 placeholder: in-memory registry; dispatch by trigger_type later)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TriggerEntry {
    pub trigger_type: TriggerType,
    pub config: Value,
    pub workflow_id: String,
}

// ============================================================
// 工作流 DAG 数据模型（与前端 src/types/index.ts 对齐）
// Workflow DAG data model (mirrors src/types/index.ts)
// ============================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowGraph {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    #[serde(default)]
    pub bindings: Vec<DataBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String, // 'plugin' | 'transform' (v1 only 'plugin' is executed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DataBinding {
    pub from: String, // source node_id
    pub output: String,
    pub to: String, // target node_id
    pub input: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub graph: WorkflowGraph,
    #[serde(rename = "is_enabled")]
    pub is_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

// ============================================================
// 执行结果与状态（与数据库列对齐）
// Execution results and statuses (match DB columns)
// ============================================================

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowRun {
    pub id: i64,
    pub workflow_id: String,
    pub trigger_type: Option<String>,
    pub status: String, // 'running' | 'success' | 'failed' | 'cancelled'
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStepLog {
    pub id: i64,
    pub run_id: i64,
    pub step_id: String,
    pub plugin_id: Option<String>,
    pub action: Option<String>,
    pub status: String, // 'running' | 'success' | 'failed' | 'skipped'
    pub input: Option<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

// ============================================================
// 引擎主体
// Engine body
// ============================================================

pub struct WorkflowEngine {
    // WASM 运行时（与 kapi:plugin.invoke 共用 invoke_action 入口）
    // The WASM runtime (shares invoke_action with kapi:plugin.invoke)
    wasm: Arc<WasmRuntime>,
    // SQLite 池（直接持有便于单元测试；Tauri 命令路径经 sqlite_pool(&app) 重建）
    // SQLite pool (held for unit tests; Tauri command path rebuilds via sqlite_pool(&app))
    pool: SqlitePool,
    // 触发器注册表：trigger_id → TriggerEntry（v1 占位，路由分发留待后续）
    // Trigger registry: trigger_id → TriggerEntry (v1 placeholder; dispatch later)
    triggers: tokio::sync::RwLock<HashMap<String, TriggerEntry>>,
}

impl WorkflowEngine {
    pub fn new(wasm: Arc<WasmRuntime>, pool: SqlitePool) -> Self {
        Self {
            wasm,
            pool,
            triggers: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    // 从 AppHandle 取实例（依赖 .manage(WorkflowEngine::new(...))）
    // Fetch from AppHandle (relies on .manage(WorkflowEngine::new(...)))
    pub fn from_app(app: &AppHandle) -> Result<Arc<Self>, String> {
        let engine = app
            .state::<Arc<WorkflowEngine>>()
            .inner()
            .clone();
        Ok(engine)
    }

    // 触发器注册（占位 API；v1 仅 Manual 路径真正调用 execute）
    // Trigger registration (placeholder API; v1 only the Manual path calls execute)
    #[allow(dead_code)]
    pub async fn register_trigger(&self, trigger_id: String, entry: TriggerEntry) {
        let mut map = self.triggers.write().await;
        map.insert(trigger_id, entry);
    }

    #[allow(dead_code)]
    pub async fn unregister_trigger(&self, trigger_id: &str) {
        let mut map = self.triggers.write().await;
        map.remove(trigger_id);
    }

    #[allow(dead_code)]
    pub async fn trigger_count(&self) -> usize {
        self.triggers.read().await.len()
    }

    // ============================================================
    // 执行入口：从工作流 ID 加载 → 调度 → 落库
    // Execute: load by id → schedule → persist
    // ============================================================

    pub async fn execute(
        &self,
        workflow_id: &str,
        trigger_type: TriggerType,
        trigger_data: Value,
    ) -> Result<WorkflowRun, String> {
        // 1. 加载工作流（graph JSON 同步解析）
        // 1. Load the workflow (parse the graph JSON inline)
        let workflow = self.load_workflow(workflow_id).await?;

        // 2. 拓扑排序（检测环 / 入度映射 / 执行波次）
        // 2. Topological sort (cycle detection / in-degree map / execution waves)
        let waves = topological_waves(&workflow.graph)?;

        // 3. INSERT run (running) → 拿到 run_id
        // 3. INSERT run (running) → fetch run_id
        let run_id = self.insert_run(workflow_id, trigger_type).await?;

        // 4. 执行 + 两级日志 + 失败立即 fail_fast
        // 4. Execute + two-level logs + fail_fast on first error
        let mut ctx = WorkflowContext {
            trigger: trigger_data,
            outputs: HashMap::new(),
        };

        let mut result = RunOutcome {
            status: "success",
            error: None,
        };

        for wave in waves {
            // 一波内并发执行：每节点独立 step_log + 共享 ctx.outputs
            // Concurrent execution per wave: each node gets its own step_log; ctx.outputs is shared
            let mut join_set = tokio::task::JoinSet::new();
            for node_id in wave {
                let node = match workflow.graph.nodes.iter().find(|n| n.id == node_id) {
                    Some(n) => n.clone(),
                    None => continue,
                };
                let wasm = self.wasm.clone();
                let pool = self.pool.clone();
                let ctx_outputs = snapshot_outputs(&ctx.outputs);
                let bindings: Vec<DataBinding> = workflow
                    .graph
                    .bindings
                    .iter()
                    .filter(|b| b.to == node_id)
                    .cloned()
                    .collect();
                let trigger_data = ctx.trigger.clone();
                let run_id_clone = run_id;

                join_set.spawn(async move {
                    run_node(
                        run_id_clone,
                        node,
                        ctx_outputs,
                        bindings,
                        trigger_data,
                        &wasm,
                        &pool,
                    )
                    .await
                });
            }
            while let Some(joined) = join_set.join_next().await {
                match joined {
                    Ok(NodeOutcome::Success { node_id, output }) => {
                        // 写 outputs 必须在持锁外完成（避免 await 跨 Mutex 持有）
                        // The write to outputs must happen without holding a lock across an await
                        ctx.outputs.insert(node_id, output);
                    }
                    Ok(NodeOutcome::Failure { node_id, error }) => {
                        result.status = "failed";
                        result.error = Some(error.clone());
                        // 当前 wave 仍有并发未结束，继续 join 收尾（避免丢日志）
                        // The current wave still has unfinished tasks; keep joining to avoid losing logs
                        let _ = node_id;
                    }
                    Ok(NodeOutcome::Skipped { .. }) => {}
                    Err(e) => {
                        // JoinSet 自身 panic；记系统日志后继续
                        // JoinSet panic; record a system log and continue
                        let _ = write_system_log(
                            &self.pool,
                            "error",
                            &format!("workflow node task panicked: {e}"),
                            "workflow_engine",
                            Some(json!({ "run_id": run_id })),
                        )
                        .await;
                    }
                }
            }
            // fail_fast：本波次内有失败 → 标记未开始节点为 skipped，整 run 落 failed
            // fail_fast: a failure this wave → mark unstarted nodes as skipped, finalize the run as failed
            if result.status == "failed" {
                self.skip_remaining(&workflow, run_id, &ctx).await?;
                break;
            }
        }

        // 5. UPDATE run 终态
        // 5. UPDATE run to its terminal status
        let run = self.finalize_run(run_id, &result).await?;

        // 6. 写系统日志（便于 Logs 页追溯）
        // 6. Append a system log (for the Logs page to trace)
        let _ = write_system_log(
            &self.pool,
            if run.status == "success" { "info" } else { "error" },
            &format!(
                "workflow {} run #{} → {}",
                workflow_id, run.id, run.status
            ),
            "workflow_engine",
            Some(json!({
                "workflow_id": workflow_id,
                "run_id": run.id,
                "status": run.status,
                "error": run.error,
            })),
        )
        .await;

        Ok(run)
    }

    // 加载工作流（含 graph JSON 解析）
    // Load the workflow (incl. graph JSON parsing)
    async fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, String> {
        let row = sqlx::query(
            "SELECT id, name, description, graph, is_enabled, created_at, updated_at
             FROM workflows WHERE id = ?",
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?
        .ok_or_else(|| format!("WorkflowNotFound: {workflow_id}"))?;

        let graph_text: String = row
            .try_get("graph")
            .map_err(|e| format!("StorageError: {e}"))?;
        let graph: WorkflowGraph =
            serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;

        Ok(Workflow {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
            description: row.try_get("description").ok(),
            graph,
            is_enabled: {
                let v: i64 = row
                    .try_get("is_enabled")
                    .map_err(|e| format!("StorageError: {e}"))?;
                v != 0
            },
            created_at: row.try_get("created_at").ok(),
            updated_at: row.try_get("updated_at").ok(),
        })
    }

    // INSERT workflow_runs（status='running'）
    // INSERT workflow_runs (status='running')
    async fn insert_run(
        &self,
        workflow_id: &str,
        trigger_type: TriggerType,
    ) -> Result<i64, String> {
        let result = sqlx::query(
            "INSERT INTO workflow_runs (workflow_id, trigger_type, status) VALUES (?, ?, 'running')",
        )
        .bind(workflow_id)
        .bind(trigger_type.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
        Ok(result.last_insert_rowid())
    }

    // UPDATE workflow_runs 至终态（success / failed）
    // UPDATE workflow_runs to its terminal status (success / failed)
    async fn finalize_run(&self, run_id: i64, outcome: &RunOutcome) -> Result<WorkflowRun, String> {
        sqlx::query(
            "UPDATE workflow_runs
             SET status = ?, error = ?, finished_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(outcome.status)
        .bind(outcome.error.as_deref())
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;

        self.fetch_run(run_id).await
    }

    // 单条 run 拉取（无步骤日志；前端按需走 workflowDb.getRuns）
    // Fetch a single run (no step logs; the frontend fetches them via workflowDb.getRuns)
    async fn fetch_run(&self, run_id: i64) -> Result<WorkflowRun, String> {
        let row = sqlx::query("SELECT * FROM workflow_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;

        Ok(WorkflowRun {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            workflow_id: row
                .try_get("workflow_id")
                .map_err(|e| format!("StorageError: {e}"))?,
            trigger_type: row.try_get("trigger_type").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            error: row.try_get("error").ok(),
            started_at: row
                .try_get("started_at")
                .map_err(|e| format!("StorageError: {e}"))?,
            finished_at: row.try_get("finished_at").ok(),
        })
    }

    // fail_fast：把所有「未出现在 outputs / 尚未落库为 success/failed」的节点写 skipped
    // fail_fast: write all nodes not yet recorded as success/failed as skipped
    async fn skip_remaining(
        &self,
        workflow: &Workflow,
        run_id: i64,
        ctx: &WorkflowContext,
    ) -> Result<(), String> {
        for node in &workflow.graph.nodes {
            // 已成功执行的节点：outputs 里有记录 → 不重复
            // Nodes already executed: have an entry in outputs → skip
            if ctx.outputs.contains_key(&node.id) {
                continue;
            }
            // 已写过 step_log 的（running/success/failed）：靠 SQL 兜底跳过
            // Nodes already logged (running/success/failed): rely on SQL no-ops
            let existing: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM workflow_step_logs WHERE run_id = ? AND step_id = ?",
            )
            .bind(run_id)
            .bind(&node.id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
            if existing.is_some() {
                continue;
            }
            sqlx::query(
                "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status)
                 VALUES (?, ?, ?, ?, 'skipped')",
            )
            .bind(run_id)
            .bind(&node.id)
            .bind(node.plugin_id.as_deref())
            .bind(node.action.as_deref())
            .execute(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
        }
        Ok(())
    }
}

// ============================================================
// 内部辅助结构
// Internal helpers
// ============================================================

// 执行上下文：trigger + 各节点已产出输出（HashMap 跨 await 安全）
// Execution context: trigger + per-node outputs (HashMap is safe across await)
struct WorkflowContext {
    trigger: Value,
    outputs: HashMap<String, Value>,
}

// 终止态结果 / Terminal outcome
struct RunOutcome {
    status: &'static str,
    error: Option<String>,
}

// 单节点执行结果 / Per-node outcome
enum NodeOutcome {
    Success { node_id: String, output: Value },
    Failure { node_id: String, error: String },
    Skipped { #[allow(dead_code)] node_id: String },
}

// outputs 快照：跨 await 转移所有权（避免 HashMap 引用进 JoinSet）
// Snapshot of outputs: move the ownership across await (no HashMap ref into JoinSet)
fn snapshot_outputs(src: &HashMap<String, Value>) -> HashMap<String, Value> {
    src.clone()
}

// ============================================================
// 单节点执行：写 step_log(running) → 调 invoke_action → 落 success/failed
// Single node: write step_log(running) → invoke_action → record success/failed
// ============================================================

async fn run_node(
    run_id: i64,
    node: WorkflowNode,
    prior_outputs: HashMap<String, Value>,
    bindings: Vec<DataBinding>,
    trigger_data: Value,
    wasm: &WasmRuntime,
    pool: &SqlitePool,
) -> NodeOutcome {
    let step_id = node.id.clone();

    // transform 节点 v1 跳过（保留类型，便于前端编辑期保存合法 graph）
    // transform nodes are skipped in v1 (type is reserved so the editor can save valid graphs)
    if node.node_type != "plugin" {
        let _ = sqlx::query(
            "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status, error)
             VALUES (?, ?, ?, ?, 'skipped', ?)",
        )
        .bind(run_id)
        .bind(&step_id)
        .bind(node.plugin_id.as_deref())
        .bind(node.action.as_deref())
        .bind(format!(
            "TransformNotImplemented: node type '{}' is reserved (v1 only 'plugin' executes)",
            node.node_type
        ))
        .execute(pool)
        .await;
        return NodeOutcome::Skipped { node_id: step_id };
    }

    let plugin_id = match &node.plugin_id {
        Some(p) => p.clone(),
        None => {
            record_step_failure(pool, run_id, &node, "InvalidNode: missing plugin_id").await;
            return NodeOutcome::Failure {
                node_id: step_id,
                error: "InvalidNode: missing plugin_id".into(),
            };
        }
    };
    let action = match &node.action {
        Some(a) => a.clone(),
        None => {
            record_step_failure(pool, run_id, &node, "InvalidNode: missing action").await;
            return NodeOutcome::Failure {
                node_id: step_id,
                error: "InvalidNode: missing action".into(),
            };
        }
    };

    // 拼装输入：bindings[].output → prior_outputs[from][output] | trigger → input
    // Build input: bindings[].output → prior_outputs[from][output] | trigger → input
    let input = assemble_input(&bindings, &prior_outputs, &trigger_data, &node.config);

    // INSERT step_log(running) → step_id 由 SQL 自增；这里先插一条 placeholder
    // INSERT step_log(running) → step_id auto-increments; insert a placeholder row first
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
        Err(e) => {
            return NodeOutcome::Failure {
                node_id: step_id,
                error: format!("StorageError: {e}"),
            };
        }
    };

    // 执行 WASM action
    // Invoke the WASM action
    let started = std::time::Instant::now();
    let outcome = wasm.invoke_action(pool, &plugin_id, &action, &input).await;
    let duration_ms = started.elapsed().as_millis() as i64;

    match outcome {
        Ok(output) => {
            let output_json = serde_json::to_string(&output).unwrap_or_else(|_| "null".into());
            let _ = sqlx::query(
                "UPDATE workflow_step_logs
                 SET status = 'success', output = ?, duration_ms = ?
                 WHERE id = ?",
            )
            .bind(&output_json)
            .bind(duration_ms)
            .bind(step_log_id)
            .execute(pool)
            .await;
            NodeOutcome::Success {
                node_id: step_id,
                output,
            }
        }
        Err(e) => {
            let _ = sqlx::query(
                "UPDATE workflow_step_logs
                 SET status = 'failed', error = ?, duration_ms = ?
                 WHERE id = ?",
            )
            .bind(&e)
            .bind(duration_ms)
            .bind(step_log_id)
            .execute(pool)
            .await;
            NodeOutcome::Failure {
                node_id: step_id,
                error: e,
            }
        }
    }
}

// 拼装节点输入：bindings 优先；未命中 bindings 的字段尝试从 node.config 取
// Build the node input: bindings win; unmatched fields fall back to node.config
fn assemble_input(
    bindings: &[DataBinding],
    prior_outputs: &HashMap<String, Value>,
    trigger_data: &Value,
    node_config: &Option<Value>,
) -> Value {
    // 先按 bindings 拼装，缺失项允许保留 null（plugin action 应容忍）
    // First assemble from bindings; missing keys stay null (plugin actions should tolerate them)
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
    // node.config 的字段作为默认值（仅当 bindings 未指定同名 input）
    // node.config fields act as defaults (only when no binding names the same input)
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

// ============================================================
// 拓扑排序：返回执行波次（每波内的节点并发执行；波次间按序）
// Topological sort: returns execution waves (concurrent within a wave; ordered between waves)
// 抛出 CycleDetected 错误 / throws CycleDetected on cycles
// ============================================================

fn topological_waves(graph: &WorkflowGraph) -> Result<Vec<Vec<String>>, String> {
    // 节点集合（不存在的 edge 端点忽略；构建时仅校验已知节点）
    // The node set (unknown edge endpoints are ignored; only known nodes are validated at build time)
    let mut indeg: HashMap<&str, usize> = HashMap::new();
    let mut succs: HashMap<&str, Vec<&str>> = HashMap::new();
    let node_ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    for n in &graph.nodes {
        indeg.entry(n.id.as_str()).or_insert(0);
        succs.entry(n.id.as_str()).or_default();
    }
    for e in &graph.edges {
        if !node_ids.contains(e.from.as_str()) || !node_ids.contains(e.to.as_str()) {
            continue;
        }
        *indeg.entry(e.to.as_str()).or_insert(0) += 1;
        succs.entry(e.from.as_str()).or_default().push(e.to.as_str());
    }

    let mut waves: Vec<Vec<String>> = Vec::new();
    let mut frontier: Vec<&str> = indeg
        .iter()
        .filter_map(|(id, d)| if *d == 0 { Some(*id) } else { None })
        .collect();
    frontier.sort();

    let mut visited = 0usize;
    while !frontier.is_empty() {
        let wave: Vec<String> = frontier.iter().map(|s| s.to_string()).collect();
        let mut next: Vec<&str> = Vec::new();
        for id in &frontier {
            visited += 1;
            if let Some(children) = succs.get(id) {
                for c in children {
                    if let Some(d) = indeg.get_mut(c) {
                        *d -= 1;
                        if *d == 0 {
                            next.push(c);
                        }
                    }
                }
            }
        }
        next.sort();
        waves.push(wave);
        frontier = next;
    }

    if visited != node_ids.len() {
        return Err("CycleDetected: workflow graph has a cycle".into());
    }
    Ok(waves)
}

// ============================================================
// Tauri 命令层（插件调用 + 前端 invoke 共用入口）
// Tauri command layer (plugin + frontend invoke share these)
// ============================================================

// 取 SQLite 池 + 引擎（命令侧统一入口，便于失败信息一致）
// Fetch the SQLite pool + engine (single command-side entry; consistent error messages)
pub async fn engine_from_app(app: &AppHandle) -> Result<(Arc<WorkflowEngine>, SqlitePool), String> {
    let pool = sqlite_pool(app).await?;
    let engine = WorkflowEngine::from_app(app)?;
    Ok((engine, pool))
}

// 直接打开一份 SQLite 池（与 plugin-sql 共用同一 DB 文件）
// Open a dedicated SQLite pool (shares the same DB file as plugin-sql)
// plugin-sql 的 DbInstances 是 lazy 模式（前端调用 load 才填池 + 跑迁移）
// plugin-sql's DbInstances is lazy (pool filled + migrations run on the frontend's load command)
// 工作流命令路径由用户主动触发，此时前端早已 load，故直接用 app_config_dir 打开同一 DB 即可
// The workflow command path is triggered by user action, by which time the frontend has already loaded.
// Just open the same DB file via app_config_dir.
pub async fn open_pool_with_migrations(app: &AppHandle) -> Result<SqlitePool, String> {
    use tauri::Manager;
    // 与 plugin-sql wrapper.rs 保持一致：app_config_dir + sqlite:kapi.db
    // Match plugin-sql wrapper.rs: app_config_dir + sqlite:kapi.db
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("PathError: {e}"))?;
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("kapi.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = sqlx::SqlitePool::connect(&url)
        .await
        .map_err(|e| format!("StorageError: cannot open {} ({e})", db_path.display()))?;
    Ok(pool)
}

#[tauri::command]
pub async fn workflow_execute(
    app: AppHandle,
    workflow_id: String,
) -> Result<WorkflowRun, String> {
    let (engine, _) = engine_from_app(&app).await?;
    engine
        .execute(&workflow_id, TriggerType::Manual, json!({}))
        .await
}

#[tauri::command]
pub async fn workflow_get(app: AppHandle, workflow_id: String) -> Result<Option<Workflow>, String> {
    let pool = sqlite_pool(&app).await?;
    let row = sqlx::query(
        "SELECT id, name, description, graph, is_enabled, created_at, updated_at
         FROM workflows WHERE id = ?",
    )
    .bind(&workflow_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let Some(row) = row else { return Ok(None) };
    let graph_text: String = row
        .try_get("graph")
        .map_err(|e| format!("StorageError: {e}"))?;
    let graph: WorkflowGraph =
        serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;

    Ok(Some(Workflow {
        id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
        name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
        description: row.try_get("description").ok(),
        graph,
        is_enabled: {
            let v: i64 = row
                .try_get("is_enabled")
                .map_err(|e| format!("StorageError: {e}"))?;
            v != 0
        },
        created_at: row.try_get("created_at").ok(),
        updated_at: row.try_get("updated_at").ok(),
    }))
}

#[tauri::command]
pub async fn workflow_list(app: AppHandle) -> Result<Vec<Workflow>, String> {
    let pool = sqlite_pool(&app).await?;
    let rows = sqlx::query(
        "SELECT id, name, description, graph, is_enabled, created_at, updated_at
         FROM workflows ORDER BY updated_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let graph_text: String = row
            .try_get("graph")
            .map_err(|e| format!("StorageError: {e}"))?;
        let graph: WorkflowGraph =
            serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;
        out.push(Workflow {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
            description: row.try_get("description").ok(),
            graph,
            is_enabled: {
                let v: i64 = row
                    .try_get("is_enabled")
                    .map_err(|e| format!("StorageError: {e}"))?;
                v != 0
            },
            created_at: row.try_get("created_at").ok(),
            updated_at: row.try_get("updated_at").ok(),
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn workflow_save(app: AppHandle, workflow: Workflow) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let graph_json = serde_json::to_string(&workflow.graph)
        .map_err(|e| format!("InvalidGraph: {e}"))?;
    sqlx::query(
        "INSERT INTO workflows (id, name, description, graph, is_enabled, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           graph = excluded.graph,
           is_enabled = excluded.is_enabled,
           updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&workflow.id)
    .bind(&workflow.name)
    .bind(workflow.description.as_deref())
    .bind(&graph_json)
    .bind(if workflow.is_enabled { 1 } else { 0 })
    .execute(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn workflow_delete(app: AppHandle, workflow_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    // 外键 ON DELETE CASCADE 自动清 workflow_runs + workflow_step_logs
    // FK ON DELETE CASCADE wipes workflow_runs + workflow_step_logs
    sqlx::query("DELETE FROM workflows WHERE id = ?")
        .bind(&workflow_id)
        .execute(&pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn workflow_runs(
    app: AppHandle,
    workflow_id: String,
    limit: Option<i32>,
) -> Result<Vec<WorkflowRun>, String> {
    let pool = sqlite_pool(&app).await?;
    let limit = limit.unwrap_or(20).clamp(1, 200);
    let rows = sqlx::query(
        "SELECT * FROM workflow_runs WHERE workflow_id = ?
         ORDER BY started_at DESC LIMIT ?",
    )
    .bind(&workflow_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WorkflowRun {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            workflow_id: row
                .try_get("workflow_id")
                .map_err(|e| format!("StorageError: {e}"))?,
            trigger_type: row.try_get("trigger_type").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            error: row.try_get("error").ok(),
            started_at: row
                .try_get("started_at")
                .map_err(|e| format!("StorageError: {e}"))?,
            finished_at: row.try_get("finished_at").ok(),
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn workflow_run_steps(
    app: AppHandle,
    run_id: i64,
) -> Result<Vec<WorkflowStepLog>, String> {
    let pool = sqlite_pool(&app).await?;
    let rows = sqlx::query(
        "SELECT * FROM workflow_step_logs WHERE run_id = ? ORDER BY id",
    )
    .bind(run_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(WorkflowStepLog {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            run_id: row.try_get("run_id").map_err(|e| format!("StorageError: {e}"))?,
            step_id: row.try_get("step_id").map_err(|e| format!("StorageError: {e}"))?,
            plugin_id: row.try_get("plugin_id").ok(),
            action: row.try_get("action").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            input: row.try_get("input").ok(),
            output: row.try_get("output").ok(),
            error: row.try_get("error").ok(),
            duration_ms: row.try_get("duration_ms").ok(),
            created_at: row
                .try_get("created_at")
                .map_err(|e| format!("StorageError: {e}"))?,
        });
    }
    Ok(out)
}

// ============================================================
// 单元测试：拓扑排序
// Unit tests: topological sort
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_graph(nodes: Vec<(&str, &str)>, edges: Vec<(&str, &str)>) -> WorkflowGraph {
        WorkflowGraph {
            nodes: nodes
                .into_iter()
                .map(|(id, ty)| WorkflowNode {
                    id: id.into(),
                    node_type: ty.into(),
                    plugin_id: None,
                    action: None,
                    config: None,
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(f, t)| WorkflowEdge {
                    from: f.into(),
                    to: t.into(),
                })
                .collect(),
            bindings: vec![],
        }
    }

    // 单节点图：一个波次
    // Single-node graph: one wave
    #[test]
    fn topo_single_node() {
        let g = make_graph(vec![("a", "plugin")], vec![]);
        let waves = topological_waves(&g).unwrap();
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0], vec!["a"]);
    }

    // 链式 A→B→C：三个有序波次
    // Chain A->B->C: three ordered waves
    #[test]
    fn topo_chain() {
        let g = make_graph(
            vec![("a", "plugin"), ("b", "plugin"), ("c", "plugin")],
            vec![("a", "b"), ("b", "c")],
        );
        let waves = topological_waves(&g).unwrap();
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec!["a"]);
        assert_eq!(waves[1], vec!["b"]);
        assert_eq!(waves[2], vec!["c"]);
    }

    // 并行 A,B,C → D：第一波 [A,B,C] 并发，第二波 [D]
    // Parallel A,B,C -> D: first wave [A,B,C] concurrent, second wave [D]
    #[test]
    fn topo_diamond_waves() {
        let g = make_graph(
            vec![("a", "plugin"), ("b", "plugin"), ("c", "plugin"), ("d", "plugin")],
            vec![("a", "d"), ("b", "d"), ("c", "d")],
        );
        let waves = topological_waves(&g).unwrap();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0].len(), 3);
        assert!(waves[0].contains(&"a".to_string()));
        assert!(waves[0].contains(&"b".to_string()));
        assert!(waves[0].contains(&"c".to_string()));
        assert_eq!(waves[1], vec!["d"]);
    }

    // 环：A→B→A → CycleDetected
    // Cycle A->B->A → CycleDetected
    #[test]
    fn topo_cycle_detected() {
        let g = make_graph(
            vec![("a", "plugin"), ("b", "plugin")],
            vec![("a", "b"), ("b", "a")],
        );
        let err = topological_waves(&g).unwrap_err();
        assert!(err.starts_with("CycleDetected"), "got {err}");
    }

    // 空图（无节点）→ 零波次
    // Empty graph (no nodes) → zero waves
    #[test]
    fn topo_empty() {
        let g = make_graph(vec![], vec![]);
        let waves = topological_waves(&g).unwrap();
        assert!(waves.is_empty());
    }

    // TriggerType 字符串往返
    // TriggerType string round-trip
    #[test]
    fn trigger_type_roundtrip() {
        for s in ["clipboard", "hotkey", "schedule", "manual", "plugin_event"] {
            let t = TriggerType::from_str(s).unwrap();
            assert_eq!(t.as_str(), s);
        }
        assert!(TriggerType::from_str("bogus").is_none());
    }

    // 输入拼装：bindings 命中 prior_outputs，node.config 兜底
    // Input assembly: bindings hit prior_outputs; node.config is the fallback
    #[test]
    fn assemble_input_basic() {
        let mut prior = HashMap::new();
        prior.insert(
            "src".into(),
            json!({ "formatted": "hello", "length": 5 }),
        );
        let bindings = vec![DataBinding {
            from: "src".into(),
            output: "formatted".into(),
            to: "dst".into(),
            input: "content".into(),
        }];
        let cfg = Some(json!({ "extra": "config-value" }));
        let input = assemble_input(&bindings, &prior, &json!({}), &cfg);
        assert_eq!(input["content"], "hello");
        assert_eq!(input["extra"], "config-value");
    }

    // 输入拼装：trigger 数据作为 from='__trigger__'
    // Input assembly: trigger data is the source for from='__trigger__'
    #[test]
    fn assemble_input_trigger_source() {
        let prior = HashMap::new();
        let bindings = vec![DataBinding {
            from: "__trigger__".into(),
            output: "text".into(),
            to: "node".into(),
            input: "content".into(),
        }];
        let input = assemble_input(
            &bindings,
            &prior,
            &json!({ "text": "clipboard data" }),
            &None,
        );
        assert_eq!(input["content"], "clipboard data");
    }
}