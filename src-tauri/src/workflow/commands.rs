// 工作流 Tauri 命令入口
// Workflow Tauri command entry
//
// 错误处理：所有命令返回 `Result<T, AppError>`（Tauri 自动序列化为
// {code, message, kind} 给前端）；从内部 `Result<_, String>` 用 `?` 自动转换
// Error: all commands return `Result<T, AppError>` (Tauri serializes to
// {code, message, kind} for the frontend); `?` converts internal `String` errors.
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::{AppError, CmdResult};
use crate::plugin::pool::sqlite_pool;
use crate::workflow::db::{self};
use crate::workflow::engine::WorkflowEngine;
use crate::workflow::model::{TriggerType, ValidationReport, Workflow, WorkflowRun, WorkflowStepLog, WorkflowTrigger};
use crate::workflow::topo::validate_graph;

pub async fn engine_from_app(app: &AppHandle) -> Result<(Arc<WorkflowEngine>, sqlx::SqlitePool), String> {
    let pool = sqlite_pool(app).await?;
    let engine = WorkflowEngine::from_app(app)?;
    Ok((engine, pool))
}

#[tauri::command]
pub async fn workflow_execute(
    app: AppHandle,
    workflow_id: String,
) -> CmdResult<WorkflowRun> {
    let (engine, _) = engine_from_app(&app).await?;
    engine.execute(&workflow_id, TriggerType::Manual, json!({}))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn workflow_get(app: AppHandle, workflow_id: String) -> CmdResult<Option<Workflow>> {
    let pool = sqlite_pool(&app).await?;
    Ok(db::workflow_get(&pool, &workflow_id).await?)
}

#[tauri::command]
pub async fn workflow_list(app: AppHandle) -> CmdResult<Vec<Workflow>> {
    let pool = sqlite_pool(&app).await?;
    Ok(db::workflow_list(&pool).await?)
}

#[tauri::command]
pub async fn workflow_save(app: AppHandle, workflow: Workflow) -> CmdResult<()> {
    // 权威校验：保存时再次跑 validate_graph，fatal 错误直接拒绝入库
    // Authoritative validation: re-run validate_graph on save; reject fatal errors
    let report = validate_graph(&workflow.graph);
    if let Some(fatal) = report.iter().find(|e| matches!(e.kind, crate::workflow::model::GraphErrorKind::Fatal)) {
        return Err(AppError::business("InvalidGraph", fatal.message.clone()));
    }
    let pool = sqlite_pool(&app).await?;
    db::workflow_save(&pool, &workflow).await?;
    Ok(())
}

#[tauri::command]
pub async fn workflow_delete(app: AppHandle, workflow_id: String) -> CmdResult<()> {
    let pool = sqlite_pool(&app).await?;
    // 删除工作流前，先 unregister 所有属于该工作流的触发器（避免定时器/监听器泄漏）
    // Unregister all triggers belonging to this workflow before deletion
    let engine = WorkflowEngine::from_app(&app)?;
    let triggers = db::trigger_list(&pool, Some(&workflow_id)).await?;
    for trigger in triggers {
        engine.unregister_trigger(&trigger.id).await;
    }
    db::workflow_delete(&pool, &workflow_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn workflow_runs(
    app: AppHandle,
    workflow_id: String,
    limit: Option<i32>,
) -> CmdResult<Vec<WorkflowRun>> {
    let pool = sqlite_pool(&app).await?;
    Ok(db::workflow_runs(&pool, &workflow_id, limit.unwrap_or(20).clamp(1, 200)).await?)
}

/// 校验工作流图（环/悬空/重复 id 等）
/// Validate a workflow graph (cycles, dangling edges, duplicate ids, etc.)
/// 实时调用返回完整错误列表；空 Vec = 有效
/// Call live to get a full report; empty Vec = valid
#[tauri::command]
pub async fn workflow_validate(graph: crate::workflow::model::WorkflowGraph) -> CmdResult<ValidationReport> {
    Ok(validate_graph(&graph))
}

#[tauri::command]
pub async fn workflow_run_steps(
    app: AppHandle,
    run_id: i64,
) -> CmdResult<Vec<WorkflowStepLog>> {
    let pool = sqlite_pool(&app).await?;
    Ok(db::workflow_run_steps(&pool, run_id).await?)
}

#[tauri::command]
pub async fn trigger_save(app: AppHandle, trigger: WorkflowTrigger) -> CmdResult<()> {
    let pool = sqlite_pool(&app).await?;
    db::trigger_save(&pool, &trigger).await?;

    let engine = WorkflowEngine::from_app(&app)?;
    if trigger.is_enabled {
        engine.reload_trigger(trigger.id.clone()).await?;
    } else {
        engine.unregister_trigger(&trigger.id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn trigger_delete(app: AppHandle, trigger_id: String) -> CmdResult<()> {
    let pool = sqlite_pool(&app).await?;
    let engine = WorkflowEngine::from_app(&app)?;
    engine.unregister_trigger(&trigger_id).await;
    db::trigger_delete(&pool, &trigger_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn trigger_list(
    app: AppHandle,
    workflow_id: Option<String>,
) -> CmdResult<Vec<WorkflowTrigger>> {
    let pool = sqlite_pool(&app).await?;
    Ok(db::trigger_list(&pool, workflow_id.as_deref()).await?)
}

/// B6：取消正在运行的 run
/// B6: cancel a running run
#[tauri::command]
pub async fn workflow_cancel(app: AppHandle, run_id: String) -> CmdResult<()> {
    let engine = WorkflowEngine::from_app(&app)?;
    engine.cancel_run(&run_id).await?;
    Ok(())
}

/// P4：试运行 — 跑一次但**不落库**；返回每步 input/output
/// P4: dry-run — execute but skip DB writes; return per-step input/output
#[tauri::command]
pub async fn workflow_dry_run(
    app: AppHandle,
    workflow: Workflow,
    trigger_data: Option<Value>,
) -> CmdResult<Vec<WorkflowStepLog>> {
    let (engine, _) = engine_from_app(&app).await?;
    Ok(engine
        .dry_run_execute(&workflow, trigger_data.unwrap_or_else(|| json!({})))
        .await?)
}
