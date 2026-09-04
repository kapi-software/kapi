// 工作流 Tauri 命令入口
// Workflow Tauri command entry
use std::sync::Arc;

use serde_json::json;
use tauri::AppHandle;

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
) -> Result<WorkflowRun, String> {
    let (engine, _) = engine_from_app(&app).await?;
    engine.execute(&workflow_id, TriggerType::Manual, json!({})).await
}

#[tauri::command]
pub async fn workflow_get(app: AppHandle, workflow_id: String) -> Result<Option<Workflow>, String> {
    let pool = sqlite_pool(&app).await?;
    db::workflow_get(&pool, &workflow_id).await
}

#[tauri::command]
pub async fn workflow_list(app: AppHandle) -> Result<Vec<Workflow>, String> {
    let pool = sqlite_pool(&app).await?;
    db::workflow_list(&pool).await
}

#[tauri::command]
pub async fn workflow_save(app: AppHandle, workflow: Workflow) -> Result<(), String> {
    // 权威校验：保存时再次跑 validate_graph，fatal 错误直接拒绝入库
    // Authoritative validation: re-run validate_graph on save; reject fatal errors
    let report = validate_graph(&workflow.graph);
    if let Some(fatal) = report.iter().find(|e| matches!(e.kind, crate::workflow::model::GraphErrorKind::Fatal)) {
        return Err(format!("InvalidGraph: {}", fatal.message));
    }
    let pool = sqlite_pool(&app).await?;
    db::workflow_save(&pool, &workflow).await
}

#[tauri::command]
pub async fn workflow_delete(app: AppHandle, workflow_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    // 删除工作流前，先 unregister 所有属于该工作流的触发器（避免定时器/监听器泄漏）
    // Unregister all triggers belonging to this workflow before deletion
    let engine = WorkflowEngine::from_app(&app)?;
    let triggers = db::trigger_list(&pool, Some(&workflow_id)).await?;
    for trigger in triggers {
        engine.unregister_trigger(&trigger.id).await;
    }
    db::workflow_delete(&pool, &workflow_id).await
}

#[tauri::command]
pub async fn workflow_runs(
    app: AppHandle,
    workflow_id: String,
    limit: Option<i32>,
) -> Result<Vec<WorkflowRun>, String> {
    let pool = sqlite_pool(&app).await?;
    db::workflow_runs(&pool, &workflow_id, limit.unwrap_or(20).clamp(1, 200)).await
}

/// 校验工作流图（环/悬空/重复 id 等）
/// Validate a workflow graph (cycles, dangling edges, duplicate ids, etc.)
/// 实时调用返回完整错误列表；空 Vec = 有效
/// Call live to get a full report; empty Vec = valid
#[tauri::command]
pub async fn workflow_validate(graph: crate::workflow::model::WorkflowGraph) -> Result<ValidationReport, String> {
    Ok(validate_graph(&graph))
}

#[tauri::command]
pub async fn workflow_run_steps(
    app: AppHandle,
    run_id: i64,
) -> Result<Vec<WorkflowStepLog>, String> {
    let pool = sqlite_pool(&app).await?;
    db::workflow_run_steps(&pool, run_id).await
}

#[tauri::command]
pub async fn trigger_save(app: AppHandle, trigger: WorkflowTrigger) -> Result<(), String> {
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
pub async fn trigger_delete(app: AppHandle, trigger_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let engine = WorkflowEngine::from_app(&app)?;
    engine.unregister_trigger(&trigger_id).await;
    db::trigger_delete(&pool, &trigger_id).await
}

#[tauri::command]
pub async fn trigger_list(
    app: AppHandle,
    workflow_id: Option<String>,
) -> Result<Vec<WorkflowTrigger>, String> {
    let pool = sqlite_pool(&app).await?;
    db::trigger_list(&pool, workflow_id.as_deref()).await
}
