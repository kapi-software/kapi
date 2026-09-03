// 插件共享 SQLite 连接池
// Plugin shared SQLite connection pool
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

// 数据库连接键：与前端 Database.load(...) 保持一致
// DB connection key: must match the frontend Database.load(...)
const DB_KEY: &str = "sqlite:kapi.db";

// 共享 SQLite 连接池：前端 Database.load 创建（tauri-plugin-sql 状态），此处只取用
// Shared SQLite pool: created by the frontend Database.load (tauri-plugin-sql state)
// 插件未导出 sqlite() 访问器，直接匹配枚举变体；SqlitePool 为 Arc 句柄，克隆廉价
// The plugin exports no sqlite() accessor, so match the variant; SqlitePool is an Arc handle
pub(crate) async fn sqlite_pool(app: &AppHandle) -> Result<sqlx::SqlitePool, String> {
    let instances = app.state::<DbInstances>();
    // tokio RwLock：await 取读锁（无中毒语义）；守卫显式绑定，确保 await 后即可释放
    // tokio RwLock: await the read lock (no poisoning); the named guard drops right after the clone
    let guard = instances.0.read().await;
    match guard.get(DB_KEY) {
        Some(DbPool::Sqlite(pool)) => Ok(pool.clone()),
        _ => Err("数据库尚未初始化 / database not initialized yet".to_string()),
    }
}

// headless 默认动作：workflow.actions 中名为 run 的优先，否则首个 action，缺省字面量 "run"
// The headless default action: workflow.actions named "run" wins, then the first action, else "run"
pub fn pick_headless_action(manifest_json: &str) -> String {
    // 嵌套 fn 规避闭包引用生命周期推断 / a nested fn sidesteps closure lifetime inference
    fn name_of(v: &Value) -> Option<&str> {
        v.get("name").and_then(Value::as_str)
    }
    let value: Value = serde_json::from_str(manifest_json).unwrap_or(Value::Null);
    let Some(actions) = value
        .get("workflow")
        .and_then(|w| w.get("actions"))
        .and_then(Value::as_array)
    else {
        return "run".into();
    };
    if actions.iter().any(|v| name_of(v) == Some("run")) {
        return "run".into();
    }
    actions.iter().find_map(name_of).unwrap_or("run").to_string()
}
