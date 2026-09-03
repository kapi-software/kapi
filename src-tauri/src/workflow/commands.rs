// 工作流 Tauri 命令入口
// Workflow Tauri command entry
use std::sync::Arc;

use serde_json::json;
use tauri::AppHandle;

use crate::plugin::pool::sqlite_pool;
use crate::workflow::db::{self};
use crate::workflow::engine::WorkflowEngine;
use crate::workflow::model::{TriggerType, Workflow, WorkflowRun, WorkflowStepLog, WorkflowTrigger};

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
    let pool = sqlite_pool(&app).await?;
    db::workflow_save(&pool, &workflow).await
}

#[tauri::command]
pub async fn workflow_delete(app: AppHandle, workflow_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
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
