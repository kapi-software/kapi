// 数据库迁移装配：SQLite schema 版本化迁移的唯一入口
// Migration assembly: the single entry point for versioned SQLite schema migrations
// 表结构见 docs/DATABASE.md
use tauri_plugin_sql::{Migration, MigrationKind};

// 迁移清单：由 tauri-plugin-sql 在加载 `sqlite:kapi.db` 时自动执行且只执行一次
// （内部按版本号记录，幂等）；前端 Database.load 只做读写，不执行任何 DDL
// Migration list: executed automatically and exactly once by tauri-plugin-sql on load,
// versioned and idempotent; the frontend only reads/writes, never runs DDL
pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "init tables",
            sql: include_str!("../migrations/001_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "seed default settings",
            sql: include_str!("../migrations/002_defaults.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "enable WAL journal mode",
            sql: include_str!("../migrations/003_wal.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "workflow trigger registry",
            sql: include_str!("../migrations/004_workflow_triggers.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "trigger cursors (plugin_event last_event_id persistence)",
            sql: include_str!("../migrations/005_trigger_cursors.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "workflow graph schema_version column",
            sql: include_str!("../migrations/006_workflow_schema_version.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "drop trigger cursors (event bus replaced table polling)",
            sql: include_str!("../migrations/007_drop_trigger_cursors.sql"),
            kind: MigrationKind::Up,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // 全部迁移脚本可执行（含 003 的带返回行 PRAGMA）：内存库逐条跑通即可
    // Every migration script executes (incl. the row-returning PRAGMA in 003)
    #[tokio::test]
    async fn all_migrations_execute_cleanly() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        for m in migrations() {
            sqlx::raw_sql(&m.sql).execute(&pool).await.unwrap();
        }
        // 种子设置存在 / seeded settings exist
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM settings").fetch_one(&pool).await.unwrap();
        assert!(count > 0);
    }
}
