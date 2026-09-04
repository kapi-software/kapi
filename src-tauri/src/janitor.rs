// 后台清理任务：plugin_events / system_logs 保留策略，防止审计表无限增长
// Background janitor: retention for plugin_events / system_logs so audit tables
// never grow unbounded
use std::time::Duration;

use sqlx::SqlitePool;

// 保留窗口：各表只留最新 N 条（事件消费走进程内总线，旧行仅剩审计价值）
// Retention windows: keep only the newest N rows per table (events are consumed
// via the in-process bus; old rows are audit-only)
const PLUGIN_EVENTS_KEEP: i64 = 10_000;
const SYSTEM_LOGS_KEEP: i64 = 20_000;
const TRIM_INTERVAL: Duration = Duration::from_secs(3600);

// 单表裁剪：删除最新 N 条之外的旧行（表名来自常量，非用户输入）
// Trim one table: delete rows older than the newest N (table names are constants)
async fn trim_table(pool: &SqlitePool, table: &str, keep: i64) -> Result<u64, String> {
    let sql = format!(
        "DELETE FROM {table} WHERE id < \
         (SELECT MIN(id) FROM (SELECT id FROM {table} ORDER BY id DESC LIMIT ?))"
    );
    let res = sqlx::query(&sql)
        .bind(keep)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(res.rows_affected())
}

// 启动清理循环：每小时一轮，首轮立即执行（启动即瘦身一次）
// Start the janitor loop: hourly, first pass immediately (a startup trim)
pub fn start(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(TRIM_INTERVAL);
        loop {
            interval.tick().await;
            for (table, keep) in
                [("plugin_events", PLUGIN_EVENTS_KEEP), ("system_logs", SYSTEM_LOGS_KEEP)]
            {
                match trim_table(&pool, table, keep).await {
                    Ok(0) => {}
                    Ok(n) => eprintln!("[janitor] trimmed {n} rows from {table}"),
                    Err(e) => eprintln!("[janitor] {table} trim failed: {e}"),
                }
            }
        }
    });
}
