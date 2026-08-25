/**
 * @file db.rs
 * @description 数据库迁移装配：SQLite schema 版本化迁移的唯一入口
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始实现（001 建表 + 002 默认设置）
 */

use tauri_plugin_sql::{Migration, MigrationKind};

/// 迁移清单 / Migration list
///
/// 迁移由 tauri-plugin-sql 在插件加载 `sqlite:kapi.db` 时自动执行且只执行一次
/// （内部按版本号记录，幂等）。前端 `Database.load` 只做读写，不执行任何 DDL。
///
/// # Returns / 返回值
/// * `Vec<Migration>` - 按版本升序排列的迁移列表 / Migrations in ascending version order
///
/// # Example / 示例
/// ```
/// let migrations = migrations();
/// assert_eq!(migrations.len(), 2);
/// ```
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
    ]
}
