// 设置写命令：settings 表的前端唯一写入口（市场缓存仍由 store.rs 内部写入）
// Settings write command: the sole frontend write path for settings (the store
// cache is still written internally by store.rs)
use std::collections::HashMap;

use tauri::AppHandle;

use crate::plugin::pool::sqlite_pool;

// 批量 upsert 设置：单事务提交（单键更新与整体重置共用同一命令）
// Batch-upsert settings in one transaction (single-key updates and full resets share it)
// value 由前端序列化为 JSON 字符串，与既有 settingsDb.set 存储格式一致
// The frontend serializes values to JSON strings, matching the existing settingsDb.set format
#[tauri::command]
pub async fn settings_set(app: AppHandle, entries: HashMap<String, String>) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    for (key, value) in &entries {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}
