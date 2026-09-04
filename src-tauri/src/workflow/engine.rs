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
    CronSchedule, NodeOutcome, RunOutcome, TriggerEntry, TriggerType, Workflow, WorkflowContext,
    WorkflowGraph, WorkflowRun, WorkflowStepLog, WorkflowTrigger,
};
use crate::workflow::node::run_node;
use crate::workflow::topo::topological_waves;
use crate::wasm::engine::WasmRuntime;

pub struct WorkflowEngine {
    pub wasm: Arc<WasmRuntime>,
    pub pool: SqlitePool,
    pub triggers: Arc<RwLock<HashMap<String, TriggerEntry>>>,
    pub trigger_tasks: Arc<RwLock<HashMap<String, crate::workflow::trigger::TriggerHandle>>>,
    /// 正在运行的 run_id → cancellation token（用于 B6 取消）
    /// Running run_id → cancellation token (for B6 cancel)
    pub running_runs: Arc<RwLock<HashMap<i64, CancellationToken>>>,
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
            running_runs: Arc::new(RwLock::new(HashMap::new())),
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

    /// 取消一个正在运行的 run（标记状态为 cancelled 并触发 cancellation token）
    /// Cancel a running run (marks status as cancelled and triggers the cancellation token)
    pub async fn cancel_run(&self, run_id: &str) -> Result<(), String> {
        // 1) 触发 cancellation token，让 execute() 提前跳出
        // 1) Trigger cancellation token so execute() can break out early
        let id: i64 = run_id.parse().map_err(|_| format!("InvalidRunId: {run_id}"))?;
        let token = {
            let runs = self.running_runs.read().await;
            runs.get(&id).cloned()
        };
        if let Some(t) = token {
            t.cancel();
        }
        // 2) 立即把 DB 状态改成 cancelled（即使 token 没及时生效，UI 也能看到）
        // 2) Immediately flip DB status (UI sees cancelled even if the token hasn't propagated)
        sqlx::query("UPDATE workflow_runs SET status = 'cancelled' WHERE id = ? AND status = 'running'")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
        Ok(())
    }

    /// 试运行：执行 workflow 但**不写** workflow_runs / workflow_step_logs 表
    /// Dry-run: execute workflow but skip all DB writes for runs / step_logs
    /// 返回每步 input/output 列表，给用户即时反馈
    /// Returns per-step input/output for immediate feedback
    #[allow(dead_code)]
    pub async fn dry_run_execute(
        &self,
        workflow: &Workflow,
        trigger_data: Value,
    ) -> Result<Vec<WorkflowStepLog>, String> {
        use std::sync::atomic::AtomicI64;
        let waves = topological_waves(&workflow.graph)?;
        let mut ctx = WorkflowContext {
            trigger: trigger_data,
            outputs: HashMap::new(),
        };
        // dry-run 也需要按 wave 顺序写 ctx.outputs，让下游节点拿到上游 output
        // dry-run still needs to write ctx.outputs across waves so downstream sees upstream output
        let run_id = 0i64;
        let steps: std::sync::Arc<std::sync::Mutex<Vec<WorkflowStepLog>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let started_at = std::time::Instant::now();
        let _id_seq = AtomicI64::new(0);

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
                let edge_map: Vec<HashMap<String, String>> = workflow
                    .graph
                    .edges
                    .iter()
                    .filter(|e| e.to == node_id)
                    .map(|e| e.map.clone())
                    .collect();
                let trigger_data = ctx.trigger.clone();
                let steps_for_task = steps.clone();
                let started = std::time::Instant::now();
                join_set.spawn(async move {
                    let outcome = run_node(
                        run_id, node.clone(), ctx_outputs, edge_map, trigger_data,
                        &wasm, &pool, &handlebars, true, // dry_run = true
                    )
                    .await;
                    let duration_ms = started.elapsed().as_millis() as i64;
                    let step = match &outcome {
                        NodeOutcome::Success { node_id, output } => WorkflowStepLog {
                            id: 0,
                            run_id: 0,
                            step_id: node_id.clone(),
                            plugin_id: node.plugin_id.clone(),
                            action: node.action.clone(),
                            status: "success".to_string(),
                            input: None,
                            output: Some(serde_json::to_string(output).unwrap_or_else(|_| "null".into())),
                            error: None,
                            duration_ms: Some(duration_ms),
                            created_at: String::new(),
                        },
                        NodeOutcome::Failure { node_id, error } => WorkflowStepLog {
                            id: 0,
                            run_id: 0,
                            step_id: node_id.clone(),
                            plugin_id: node.plugin_id.clone(),
                            action: node.action.clone(),
                            status: "failed".to_string(),
                            input: None,
                            output: None,
                            error: Some(error.clone()),
                            duration_ms: Some(duration_ms),
                            created_at: String::new(),
                        },
                        NodeOutcome::Skipped { .. } => WorkflowStepLog {
                            id: 0,
                            run_id: 0,
                            step_id: node_id.clone(),
                            plugin_id: None,
                            action: None,
                            status: "skipped".to_string(),
                            input: None,
                            output: None,
                            error: None,
                            duration_ms: Some(duration_ms),
                            created_at: String::new(),
                        },
                    };
                    steps_for_task.lock().unwrap().push(step);
                    outcome
                });
            }
            let mut failed = false;
            while let Some(joined) = join_set.join_next().await {
                if let Ok(NodeOutcome::Success { node_id, output }) = joined {
                    ctx.outputs.insert(node_id, output);
                } else if let Ok(NodeOutcome::Failure { node_id, .. }) = joined {
                    let _ = node_id;
                    failed = true;
                }
            }
            if failed {
                // dry-run 失败也继续返回已收集的 steps（让用户看到失败点）
                // dry-run failures also return collected steps (let user see the failure point)
                break;
            }
        }
        let _duration = started_at.elapsed();
        let collected = steps.lock().unwrap().clone();
        Ok(collected)
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
                    trigger_id.clone(),
                    trigger.workflow_id.clone(),
                    trigger.config.clone(),
                    cancel.clone(),
                )
                .await
            }
            _ => {
                // B2：clipboard / hotkey 触发器现在终于接上线
                // B2: clipboard / hotkey triggers are now actually wired
                let task = self
                    .start_clipboard_or_hotkey_trigger(
                        app.clone(),
                        trigger.workflow_id.clone(),
                        trigger_type,
                        trigger.config.clone(),
                    )
                    .await?;
                let mut tasks = self.trigger_tasks.write().await;
                tasks.insert(trigger_id.clone(), TriggerHandle { cancel, task });
                return Ok(());
            }
        };

        let mut tasks = self.trigger_tasks.write().await;
        tasks.insert(trigger_id, TriggerHandle { cancel, task });
        Ok(())
    }

    /// B2：启动 clipboard / hotkey 触发器（监听 tauri 事件 → engine.execute）
    /// B2: start clipboard / hotkey triggers (listen to tauri events -> engine.execute)
    /// 由 start_trigger 末尾的 _ 分支调过来
    /// Invoked from start_trigger's `_` branch
    pub async fn start_clipboard_or_hotkey_trigger(
        &self,
        app: AppHandle,
        workflow_id: String,
        trigger_type: TriggerType,
        config: Value,
    ) -> Result<tokio::task::JoinHandle<()>, String> {
        let shortcut = config
            .get("shortcut")
            .and_then(|v| v.as_str())
            .unwrap_or("CmdOrCtrl+Shift+K")
            .to_string();
        let pattern = config
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let engine = app.state::<Arc<Self>>().inner().clone();
        let app_clone = app.clone();
        Ok(tokio::spawn(async move {
            match trigger_type {
                TriggerType::Clipboard => {
                    // 轮询剪贴板（每 500ms），变化时触发 workflow
                    // Poll clipboard every 500ms; trigger workflow on change
                    use tauri_plugin_clipboard_manager::ClipboardExt;
                    let mut last = String::new();
                    let mut had_last = false;
                    loop {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let cur = app_clone
                            .clipboard()
                            .read_text()
                            .unwrap_or_default();
                        if !had_last {
                            last = cur;
                            had_last = true;
                            continue;
                        }
                        if cur != last {
                            last = cur.clone();
                            // pattern 正则匹配（可选）
                            // pattern regex match (optional)
                            if let Some(pat) = &pattern {
                                if !regex_match(pat, &cur) {
                                    continue;
                                }
                            }
                            let _ = engine
                                .execute(&workflow_id, TriggerType::Clipboard, json!({ "text": cur }))
                                .await;
                        }
                    }
                }
                TriggerType::Hotkey => {
                    // 全局快捷键：注册快捷键，按下时触发 workflow
                    // Global shortcut: register; trigger on press
                    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
                    let parsed: Result<(Modifiers, Code), String> = parse_shortcut(&shortcut);
                    if let Ok((mods, code)) = parsed {
                        let shortcut_obj = Shortcut::new(Some(mods), code);
                        let engine_for_cb = engine.clone();
                        let shortcut_for_cb = shortcut.clone();
                        let wf_id = workflow_id.clone();
                        let _ = app_clone.global_shortcut().on_shortcut(shortcut_obj, move |_app, _sc, event| {
                            if event.state == ShortcutState::Pressed {
                                let engine_cb = engine_for_cb.clone();
                                let sc = shortcut_for_cb.clone();
                                let wf = wf_id.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = engine_cb
                                        .execute(&wf, TriggerType::Hotkey, json!({ "shortcut": sc }))
                                        .await;
                                });
                            }
                        });
                    }
                    // 保持 task 活跃
                    // Keep the task alive
                    loop {
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                    }
                }
                _ => {}
            }
        }))
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

        // B6：注册 cancellation token，让 workflow_cancel 可中断
        // B6: register cancellation token so workflow_cancel can interrupt
        let cancel_token = CancellationToken::new();
        {
            let mut runs = self.running_runs.write().await;
            runs.insert(run_id, cancel_token.clone());
        }

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
                // P1：从 edges 收集指向本节点的边，map 形如 { upstream_output: downstream_input }
                // P1: collect edges pointing to this node; map shape is { upstream_output: downstream_input }
                let edge_map: Vec<std::collections::HashMap<String, String>> = workflow
                    .graph
                    .edges
                    .iter()
                    .filter(|e| e.to == node_id)
                    .map(|e| e.map.clone())
                    .collect();
                let trigger_data = ctx.trigger.clone();
                let run_id_clone = run_id;

                join_set.spawn(async move {
                    run_node(run_id_clone, node, ctx_outputs, edge_map, trigger_data, &wasm, &pool, &handlebars, false).await
                });
            }
            // B6：在每个 wave 后检查 cancel（不让 cancel 静默等 wave 完成）
            // B6: check cancel after each wave (don't let cancel wait silently for wave completion)
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    result.status = "cancelled";
                    result.error = Some("cancelled by user".to_string());
                    self.skip_remaining(&workflow, run_id, &ctx).await?;
                    break;
                }
                _ = async {
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
                } => {}
            }
            if result.status == "cancelled" {
                break;
            }
            if result.status == "failed" {
                self.skip_remaining(&workflow, run_id, &ctx).await?;
                break;
            }
        }

        // 清理：运行结束，从 running_runs 摘除 token
        // Cleanup: remove from running_runs once finished
        {
            let mut runs = self.running_runs.write().await;
            runs.remove(&run_id);
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
            "SELECT id, name, description, graph, schema_version, is_enabled, created_at, updated_at FROM workflows WHERE id = ?",
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
            schema_version: row.try_get::<i64, _>("schema_version").map_err(|e| format!("StorageError: {e}"))? as i32,
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

/// Schedule 触发器：按 cron 表达式定时执行工作流
/// Schedule trigger: execute workflow per cron expression
fn spawn_schedule_trigger(
    app: AppHandle,
    workflow_id: String,
    config: Value,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    // P3 (A4): 用 cron 字符串代替 interval_seconds
    // P3 (A4): use cron expression instead of interval_seconds
    let cron_str = config
        .get("cron")
        .and_then(|v| v.as_str())
        .unwrap_or("0 * * * *"); // 默认：每小时整点
    let schedule: CronSchedule = match cron_str.parse() {
        Ok(s) => s,
        Err(e) => {
            // 错误 cron 表达式：记录后用 1 小时兜底，避免完全无响应
            eprintln!("[workflow] invalid cron '{cron_str}': {e}; using fallback");
            // 兜底：每小时整点
            "0 * * * *".parse().unwrap()
        }
    };
    let workflow_id = workflow_id;
    tokio::spawn(async move {
        loop {
            // 计算下一次匹配秒数
            // Compute next matching instant
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let next_secs = match schedule.next_utc_seconds(now_secs) {
                Some(s) => s,
                None => {
                    // 找不到匹配（表达式无解），1 小时后重试
                    eprintln!("[workflow] no matching time for cron; retry in 1h");
                    let sleep_dur = Duration::from_secs(3600);
                    tokio::select! {
                        _ = cancel.cancelled() => break,
                        _ = tokio::time::sleep(sleep_dur) => continue,
                    }
                }
            };
            let wait = (next_secs - now_secs).max(1) as u64;
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(wait)) => {
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

/// PluginEvent 触发器：轮询 plugin_events 表（游标持久化避免重启重放）
/// PluginEvent trigger: poll the plugin_events table (cursor persisted to avoid replay on restart)
async fn spawn_plugin_event_trigger(
    app: AppHandle,
    trigger_id: String,
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
        // B4：从 DB 读取 last_event_id，避免重启后重放历史
        // B4: read last_event_id from DB to avoid replaying history on restart
        let mut last_event_id: i64 = match sqlx::query_scalar::<_, i64>(
            "SELECT last_event_id FROM trigger_cursors WHERE trigger_id = ?",
        )
        .bind(&trigger_id)
        .fetch_optional(&pool)
        .await
        .unwrap_or(None)
        {
            Some(v) => v,
            None => 0,
        };
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
                            // B4：游标落库（upsert）
                            // B4: persist cursor (upsert)
                            let _ = sqlx::query(
                                "INSERT INTO trigger_cursors (trigger_id, last_event_id, updated_at) \
                                 VALUES (?, ?, datetime('now')) \
                                 ON CONFLICT(trigger_id) DO UPDATE SET \
                                   last_event_id = excluded.last_event_id, \
                                   updated_at = datetime('now')",
                            )
                            .bind(&trigger_id)
                            .bind(event_id)
                            .execute(&pool)
                            .await;
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

// B2 helpers
fn regex_match(pattern: &str, text: &str) -> bool {
    // 简单实现：避免引入 regex crate 依赖；用子串包含判断
    // Simple impl: avoid regex crate dep; just use substring check
    text.contains(pattern)
}

fn parse_shortcut(s: &str) -> Result<(tauri_plugin_global_shortcut::Modifiers, tauri_plugin_global_shortcut::Code), String> {
    use tauri_plugin_global_shortcut::{Code, Modifiers};
    let parts = s.split('+');
    let mut mods = Modifiers::empty();
    let mut code: Option<Code> = None;
    for p in parts {
        match p.to_ascii_uppercase().as_str() {
            "CMD" | "CMDORCTRL" | "CTRL" | "CONTROL" => mods |= Modifiers::CONTROL,
            "ALT" => mods |= Modifiers::ALT,
            "SHIFT" => mods |= Modifiers::SHIFT,
            "SUPER" | "META" | "WIN" => mods |= Modifiers::META,
            _ => {
                code = Some(match p.to_ascii_uppercase().as_str() {
                    "A" => Code::KeyA, "B" => Code::KeyB, "C" => Code::KeyC, "D" => Code::KeyD,
                    "E" => Code::KeyE, "F" => Code::KeyF, "G" => Code::KeyG, "H" => Code::KeyH,
                    "I" => Code::KeyI, "J" => Code::KeyJ, "K" => Code::KeyK, "L" => Code::KeyL,
                    "M" => Code::KeyM, "N" => Code::KeyN, "O" => Code::KeyO, "P" => Code::KeyP,
                    "Q" => Code::KeyQ, "R" => Code::KeyR, "S" => Code::KeyS, "T" => Code::KeyT,
                    "U" => Code::KeyU, "V" => Code::KeyV, "W" => Code::KeyW, "X" => Code::KeyX,
                    "Y" => Code::KeyY, "Z" => Code::KeyZ,
                    "0" => Code::Digit0, "1" => Code::Digit1, "2" => Code::Digit2, "3" => Code::Digit3,
                    "4" => Code::Digit4, "5" => Code::Digit5, "6" => Code::Digit6, "7" => Code::Digit7,
                    "8" => Code::Digit8, "9" => Code::Digit9,
                    _ => return Err(format!("unsupported key '{p}'")),
                });
            }
        }
    }
    let code = code.ok_or_else(|| "missing key code".to_string())?;
    Ok((mods, code))
}
