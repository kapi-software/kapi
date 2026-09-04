// 工作流引擎主体：执行入口 / 触发器生命周期 / DAG 调度
// Workflow engine body: execute entry / trigger lifecycle / DAG scheduling
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use handlebars::Handlebars;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::bridge::log::write_system_log;
use crate::workflow::model::{
    DataBinding, NodeOutcome, RunOutcome, TriggerEntry, TriggerType, Workflow, WorkflowContext,
    WorkflowGraph, WorkflowRun, WorkflowTrigger,
};
use crate::workflow::node::run_node;
use crate::workflow::topo::topological_waves;
use crate::wasm::engine::WasmRuntime;

pub struct WorkflowEngine {
    pub wasm: Arc<WasmRuntime>,
    pub pool: SqlitePool,
    pub triggers: Arc<RwLock<HashMap<String, TriggerEntry>>>,
    pub trigger_tasks: Arc<RwLock<HashMap<String, crate::workflow::trigger::TriggerHandle>>>,
    pub handlebars: Arc<Handlebars<'static>>,
}

impl WorkflowEngine {
    pub fn new(wasm: Arc<WasmRuntime>, pool: SqlitePool) -> Self {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(true);
        Self {
            wasm,
            pool,
            triggers: Arc::new(RwLock::new(HashMap::new())),
            trigger_tasks: Arc::new(RwLock::new(HashMap::new())),
            handlebars: Arc::new(handlebars),
        }
    }

    pub fn from_app(app: &AppHandle) -> Result<Arc<Self>, String> {
        let engine = app.state::<Arc<Self>>().inner().clone();
        Ok(engine)
    }

    #[allow(dead_code)]
    pub async fn register_trigger(&self, trigger_id: String, entry: TriggerEntry) {
        let mut map = self.triggers.write().await;
        map.insert(trigger_id, entry);
    }

    #[allow(dead_code)]
    pub async fn unregister_trigger(&self, trigger_id: &str) {
        use crate::workflow::trigger::unregister_trigger;
        unregister_trigger(&self.triggers, &self.trigger_tasks, trigger_id).await;
    }

    #[allow(dead_code)]
    pub async fn trigger_count(&self) -> usize {
        self.triggers.read().await.len()
    }

    #[allow(dead_code)]
    pub async fn reload_trigger(&self, trigger_id: String) -> Result<(), String> {
        self.unregister_trigger(&trigger_id).await;
        let row = sqlx::query(
            "SELECT id, workflow_id, trigger_type, config, is_enabled FROM workflow_triggers WHERE id = ? AND is_enabled = 1",
        )
        .bind(&trigger_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;

        if let Some(row) = row {
            let config_text: String = row.try_get("config").map_err(|e| format!("StorageError: {e}"))?;
            let config: Value = serde_json::from_str(&config_text).map_err(|e| format!("InvalidConfig: {e}"))?;
            let trigger_type = TriggerType::from_str(&row.try_get::<String, _>("trigger_type").map_err(|e| format!("StorageError: {e}"))?)
                .ok_or_else(|| "UnknownTriggerType".to_string())?;
            let entry = TriggerEntry {
                trigger_type,
                config,
                workflow_id: row.try_get("workflow_id").map_err(|e| format!("StorageError: {e}"))?,
            };
            self.register_trigger(trigger_id, entry).await;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn load_triggers_from_db(&self) -> Result<Vec<WorkflowTrigger>, String> {
        let rows = sqlx::query("SELECT id, workflow_id, trigger_type, config, is_enabled FROM workflow_triggers WHERE is_enabled = 1")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
        let mut triggers = Vec::with_capacity(rows.len());
        for row in rows {
            let config_text: String = row.try_get("config").map_err(|e| format!("StorageError: {e}"))?;
            let config: Value = serde_json::from_str(&config_text).map_err(|e| format!("InvalidConfig: {e}"))?;
            let is_enabled: i64 = row.try_get("is_enabled").map_err(|e| format!("StorageError: {e}"))?;
            triggers.push(WorkflowTrigger {
                id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
                workflow_id: row.try_get("workflow_id").map_err(|e| format!("StorageError: {e}"))?,
                trigger_type: row.try_get("trigger_type").map_err(|e| format!("StorageError: {e}"))?,
                config,
                is_enabled: is_enabled != 0,
            });
        }
        Ok(triggers)
    }

    #[allow(dead_code)]
    pub async fn start_triggers(&self, app: AppHandle) -> Result<(), String> {
        let triggers = self.load_triggers_from_db().await?;
        for trigger in triggers {
            self.start_trigger(app.clone(), trigger).await?;
        }
        Ok(())
    }

    async fn start_trigger(&self, app: AppHandle, trigger: WorkflowTrigger) -> Result<(), String> {
        use crate::workflow::trigger::{register_trigger, TriggerHandle};

        let trigger_type = TriggerType::from_str(&trigger.trigger_type)
            .ok_or_else(|| format!("UnknownTriggerType: {}", trigger.trigger_type))?;
        let entry = TriggerEntry {
            trigger_type,
            config: trigger.config.clone(),
            workflow_id: trigger.workflow_id.clone(),
        };
        let trigger_id = trigger.id.clone();

        register_trigger(&self.triggers, trigger_id.clone(), entry.clone()).await;

        let cancel = CancellationToken::new();
        let task: JoinHandle<()> = match trigger_type {
            TriggerType::Schedule => spawn_schedule_trigger(
                app.clone(),
                trigger.workflow_id.clone(),
                trigger.config.clone(),
                cancel.clone(),
            ),
            TriggerType::PluginEvent => {
                spawn_plugin_event_trigger(
                    app.clone(),
                    trigger.workflow_id.clone(),
                    trigger.config.clone(),
                    cancel.clone(),
                )
                .await
            }
            _ => return Ok(()),
        };

        let mut tasks = self.trigger_tasks.write().await;
        tasks.insert(trigger_id, TriggerHandle { cancel, task });
        Ok(())
    }

    /// 从工作流 ID 加载 → 拓扑排序 → 执行 → 落库
    /// Load by workflow_id → topological sort → execute → persist
    pub async fn execute(
        &self,
        workflow_id: &str,
        trigger_type: TriggerType,
        trigger_data: Value,
    ) -> Result<WorkflowRun, String> {
        let workflow = self.load_workflow(workflow_id).await?;
        // 总闸：工作流被禁用时，触发器类来源一律拒绝；手动运行放行（让用户能测试）
        // Master switch: reject trigger-driven execution when the workflow is disabled;
        // manual runs pass through (lets the user still test a disabled workflow).
        if !workflow.is_enabled && !matches!(trigger_type, TriggerType::Manual) {
            let _ = write_system_log(
                &self.pool,
                "warn",
                &format!(
                    "workflow {} is disabled; ignored {} trigger",
                    workflow_id,
                    trigger_type.as_str()
                ),
                "workflow_engine",
                Some(json!({ "workflow_id": workflow_id, "trigger_type": trigger_type.as_str() })),
            )
            .await;
            return Err(format!(
                "WorkflowDisabled: workflow {workflow_id} is disabled (master switch off)"
            ));
        }
        let waves = topological_waves(&workflow.graph)?;

        let run_id = self.insert_run(workflow_id, trigger_type).await?;

        let mut ctx = WorkflowContext {
            trigger: trigger_data,
            outputs: HashMap::new(),
        };

        let mut result = RunOutcome { status: "success", error: None };

        for wave in waves {
            let mut join_set = tokio::task::JoinSet::new();
            for node_id in wave {
                let node = match workflow.graph.nodes.iter().find(|n| n.id == node_id) {
                    Some(n) => n.clone(),
                    None => continue,
                };
                let wasm = self.wasm.clone();
                let pool = self.pool.clone();
                let handlebars = self.handlebars.clone();
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
                    run_node(run_id_clone, node, ctx_outputs, bindings, trigger_data, &wasm, &pool, &handlebars).await
                });
            }
            while let Some(joined) = join_set.join_next().await {
                match joined {
                    Ok(NodeOutcome::Success { node_id, output }) => {
                        ctx.outputs.insert(node_id, output);
                    }
                    Ok(NodeOutcome::Failure { node_id, error }) => {
                        result.status = "failed";
                        result.error = Some(error.clone());
                        let _ = node_id;
                    }
                    Ok(NodeOutcome::Skipped { .. }) => {}
                    Err(e) => {
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
            if result.status == "failed" {
                self.skip_remaining(&workflow, run_id, &ctx).await?;
                break;
            }
        }

        let run = self.finalize_run(run_id, &result).await?;
        let _ = write_system_log(
            &self.pool,
            if run.status == "success" { "info" } else { "error" },
            &format!("workflow {} run #{} → {}", workflow_id, run.id, run.status),
            "workflow_engine",
            Some(json!({ "workflow_id": workflow_id, "run_id": run.id, "status": run.status, "error": run.error })),
        )
        .await;

        Ok(run)
    }

    async fn load_workflow(&self, workflow_id: &str) -> Result<Workflow, String> {
        let row = sqlx::query(
            "SELECT id, name, description, graph, is_enabled, created_at, updated_at FROM workflows WHERE id = ?",
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?
        .ok_or_else(|| format!("WorkflowNotFound: {workflow_id}"))?;

        let graph_text: String = row.try_get("graph").map_err(|e| format!("StorageError: {e}"))?;
        let graph: WorkflowGraph = serde_json::from_str(&graph_text).map_err(|e| format!("InvalidGraph: {e}"))?;

        Ok(Workflow {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            name: row.try_get("name").map_err(|e| format!("StorageError: {e}"))?,
            description: row.try_get("description").ok(),
            graph,
            is_enabled: {
                let v: i64 = row.try_get("is_enabled").map_err(|e| format!("StorageError: {e}"))?;
                v != 0
            },
            created_at: row.try_get("created_at").ok(),
            updated_at: row.try_get("updated_at").ok(),
        })
    }

    async fn insert_run(&self, workflow_id: &str, trigger_type: TriggerType) -> Result<i64, String> {
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

    async fn finalize_run(&self, run_id: i64, outcome: &RunOutcome) -> Result<WorkflowRun, String> {
        sqlx::query(
            "UPDATE workflow_runs SET status = ?, error = ?, finished_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(outcome.status)
        .bind(outcome.error.as_deref())
        .bind(run_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;

        self.fetch_run(run_id).await
    }

    async fn fetch_run(&self, run_id: i64) -> Result<WorkflowRun, String> {
        let row = sqlx::query("SELECT * FROM workflow_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;

        Ok(WorkflowRun {
            id: row.try_get("id").map_err(|e| format!("StorageError: {e}"))?,
            workflow_id: row.try_get("workflow_id").map_err(|e| format!("StorageError: {e}"))?,
            trigger_type: row.try_get("trigger_type").ok(),
            status: row.try_get("status").map_err(|e| format!("StorageError: {e}"))?,
            error: row.try_get("error").ok(),
            started_at: row.try_get("started_at").map_err(|e| format!("StorageError: {e}"))?,
            finished_at: row.try_get("finished_at").ok(),
        })
    }

    async fn skip_remaining(&self, workflow: &Workflow, run_id: i64, ctx: &WorkflowContext) -> Result<(), String> {
        for node in &workflow.graph.nodes {
            if ctx.outputs.contains_key(&node.id) {
                continue;
            }
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
                "INSERT INTO workflow_step_logs (run_id, step_id, plugin_id, action, status) VALUES (?, ?, ?, ?, 'skipped')",
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

/// outputs 快照：跨 await 转移所有权
/// Snapshot outputs: move ownership across await
fn snapshot_outputs(src: &HashMap<String, Value>) -> HashMap<String, Value> {
    src.clone()
}

// ============================================================
// 触发器 starter：调度循环放在引擎中以避免 trigger↔engine 循环
// Trigger starters: the scheduling loop lives in the engine to avoid
// a trigger↔engine cycle
// ============================================================

/// Schedule 触发器：定时执行工作流
/// Schedule trigger: execute workflow periodically
fn spawn_schedule_trigger(
    app: AppHandle,
    workflow_id: String,
    config: Value,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let interval_secs = config
        .get("interval_seconds")
        .and_then(|v| v.as_i64())
        .unwrap_or(60) as u64;
    let workflow_id = workflow_id;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        // 跳过首次立即 tick：tokio::interval 的第一个 tick 总是立即触发，需要显式跳过
        // Skip first immediate tick: tokio::interval fires immediately on the first tick
        interval.tick().await;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    let engine = match WorkflowEngine::from_app(&app) {
                        Ok(e) => e,
                        Err(_) => break,
                    };
                    let _ = engine
                        .execute(&workflow_id, TriggerType::Schedule, json!({}))
                        .await;
                }
            }
        }
    })
}

/// PluginEvent 触发器：轮询 plugin_events 表
/// PluginEvent trigger: poll the plugin_events table
async fn spawn_plugin_event_trigger(
    app: AppHandle,
    workflow_id: String,
    config: Value,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let event_type = config
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let pool = match crate::plugin::pool::sqlite_pool(&app).await {
        Ok(p) => p,
        Err(_) => {
            return tokio::spawn(async move {});
        }
    };
    tokio::spawn(async move {
        // 200ms 轮询：突发时延迟从 1s 降到 0.2s；不再有 LIMIT 1，事件批量消费
        // 200ms poll: cuts burst latency from 1s to 0.2s; no LIMIT 1, batch consume
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // 跳过首次立即 tick：避免应用启动瞬间把已存在事件当成"新事件"全量重放一次
        // Skip first immediate tick: prevents treating all existing events as new on startup
        interval.tick().await;
        let mut last_event_id: i64 = 0;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    // 一次拉所有未消费的事件（按 id 升序），不再 LIMIT 1
                    // Pull all unconsumed events (id ASC) — no LIMIT 1
                    let rows = match sqlx::query(
                        "SELECT id, data FROM plugin_events WHERE event_type = ? AND id > ? ORDER BY id",
                    )
                    .bind(&event_type)
                    .bind(last_event_id)
                    .fetch_all(&pool)
                    .await {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    if rows.is_empty() {
                        continue;
                    }

                    let engine = match WorkflowEngine::from_app(&app) {
                        Ok(e) => e,
                        Err(_) => continue,
                    };

                    for row in rows {
                        use sqlx::Row;
                        let event_id: i64 = row.try_get("id").unwrap_or(0);
                        let data_text: Option<String> = row.try_get::<Option<String>, _>("data").unwrap_or(None);
                        let event_data: Value = match data_text.as_deref() {
                            Some(s) => serde_json::from_str(s).unwrap_or(Value::Null),
                            None => Value::Null,
                        };

                        // 推进游标：即使 execute 失败也推进，避免同一条事件反复重试
                        // Advance cursor even on failure to avoid retrying the same row forever
                        if event_id > last_event_id {
                            last_event_id = event_id;
                        }

                        let _ = engine
                            .execute(&workflow_id, TriggerType::PluginEvent, event_data)
                            .await;
                    }
                }
            }
        }
    })
}
