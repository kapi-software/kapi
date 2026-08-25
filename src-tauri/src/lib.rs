// Kapi 应用装配入口：插件注册、数据库迁移、命令注册、Dock 服务、系统托盘
// Kapi app entry: plugin registration, DB migrations, commands, dock service, system tray
mod db;
mod dock;
mod tray;

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
        // Dock 轮询配置 + 托盘语言（前端经命令推送变更）
        // Dock polling config + tray language (frontend pushes changes via commands)
        .manage(Mutex::new(dock::DockConfig::default()))
        .manage(tray::TrayState::default())
        .invoke_handler(tauri::generate_handler![
            dock::dock_set_config,
            dock::launch_plugin,
            tray::tray_set_language
        ])
        // 主窗口关闭 = 隐藏驻留托盘，退出仅走托盘菜单
        // Closing the main window hides it to the tray; quit lives in the tray menu only
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // Dock 服务：启动定位 + 热区轮询线程（docs/DOCK.md）
            // Dock service: startup positioning + hotzone polling thread (docs/DOCK.md)
            dock::start(app.handle().clone());
            // 系统托盘：驻留运行 + 主面板/设置/退出菜单
            // System tray: resident app + panel/settings/quit menu
            tray::init(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
