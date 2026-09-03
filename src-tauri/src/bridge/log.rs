// 日志写入
// Log writing
use serde_json::Value;

// 写 system_logs 的公共入口
// Shared system_logs writer
pub(crate) async fn write_system_log(
    pool: &sqlx::SqlitePool,
    level: &str,
    message: &str,
    source: &str,
    data: Option<Value>,
) -> Result<(), String> {
    let data = data
        .map(|d| serde_json::to_string(&d).map_err(|e| format!("StorageError: {e}")))
        .transpose()?;
    sqlx::query("INSERT INTO system_logs (level, message, source, data) VALUES (?, ?, ?, ?)")
        .bind(level)
        .bind(message)
        .bind(source)
        .bind(&data)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}
