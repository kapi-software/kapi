// Kapi 应用装配入口：Tauri 插件注册、数据库迁移、命令注册、Dock 服务
// Kapi app entry: Tauri plugin registration, DB migrations, commands, dock service
mod db;
mod dock;

use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // SQLite 插件：迁移随插件加载自动执行（唯一入口，见 db.rs）
        // SQLite plugin: migrations run automatically on load (single entry, see db.rs)
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:kapi.db", db::migrations())
                .build(),
        )
        // Dock 轮询配置（前端经 dock_set_config 推送 settings 变更）
        // Dock polling config (frontend pushes settings changes via dock_set_config)
        .manage(Mutex::new(dock::DockConfig::default()))
        .invoke_handler(tauri::generate_handler![
            dock::dock_set_config,
            dock::launch_plugin
        ])
        .setup(|app| {
            // Dock 服务：启动定位 + 热区轮询线程（docs/DOCK.md）
            // Dock service: startup positioning + hotzone polling thread (docs/DOCK.md)
            dock::start(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
