/**
 * @file lib.rs
 * @description Kapi 应用装配入口：Tauri 插件注册、数据库迁移、命令注册
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 —— 注册 tauri-plugin-sql（SQLite 迁移）
 */

mod db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // SQLite 插件：迁移随插件加载自动执行（唯一入口，见 db.rs）
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:kapi.db", db::migrations())
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
