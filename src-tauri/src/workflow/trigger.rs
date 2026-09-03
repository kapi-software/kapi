// 触发器条目 + 句柄
// Trigger entries + handles
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::workflow::model::TriggerEntry;

/// 活跃触发器句柄（取消令牌 + 后台任务）
/// Active trigger handle (cancellation token + background task)
pub struct TriggerHandle {
    pub cancel: CancellationToken,
    pub task: JoinHandle<()>,
}

// ============================================================
// 内存注册表
// In-memory registry
// ============================================================

/// 注册触发器到内存表
/// Register a trigger in the in-memory table
#[allow(dead_code)]
pub async fn register_trigger(
    triggers: &tokio::sync::RwLock<std::collections::HashMap<String, TriggerEntry>>,
    trigger_id: String,
    entry: TriggerEntry,
) {
    let mut map = triggers.write().await;
    map.insert(trigger_id, entry);
}

/// 取消触发器并停止后台任务
/// Cancel a trigger and stop its background task
#[allow(dead_code)]
pub async fn unregister_trigger(
    triggers: &tokio::sync::RwLock<std::collections::HashMap<String, TriggerEntry>>,
    trigger_tasks: &tokio::sync::RwLock<std::collections::HashMap<String, TriggerHandle>>,
    trigger_id: &str,
) {
    use std::collections::HashMap;
    // 停止后台任务 / stop background task
    let mut tasks: tokio::sync::RwLockWriteGuard<'_, HashMap<String, TriggerHandle>> =
        trigger_tasks.write().await;
    if let Some(handle) = tasks.remove(trigger_id) {
        handle.cancel.cancel();
        let _ = handle.task.await;
    }
    drop(tasks);
    // 移除注册表 / remove from registry
    let mut map = triggers.write().await;
    map.remove(trigger_id);
}
