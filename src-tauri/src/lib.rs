// Kapi 应用装配入口：插件注册、数据库迁移、协议注册、命令注册、Dock 服务、系统托盘
// Kapi app entry: plugin registration, DB migrations, protocol, commands, dock service, system tray
mod db;
mod dock;
mod plugin_bridge;
mod plugin_manager;
mod plugin_protocol;
mod store;
mod tray;
mod wasm_runtime;
mod workflow_engine;

use std::sync::{Arc, Mutex};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 目录选择对话框：插件本地导入（plugins 页）
        // Directory picker dialog: local plugin import (plugins page)
        .plugin(tauri_plugin_dialog::init())
        // 剪贴板：Clipboard 触发器使用
        // Clipboard: used by Clipboard trigger
        .plugin(tauri_plugin_clipboard_manager::init())
        // 全局快捷键：Hotkey 触发器使用
        // Global shortcut: used by Hotkey trigger
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
        // WASM 运行时：wasmtime 沙箱 + 模块缓存 + epoch ticker（docs/PLUGINS.md §5）
        // WASM runtime: the wasmtime sandbox, module cache and epoch ticker
        .manage(wasm_runtime::WasmRuntime::new())
        // kapi-plugin:// 协议：插件 web/ 静态资源服务（docs/ARCHITECTURE.md §3.4）
        // kapi-plugin:// protocol: plugin web/ static asset serving (docs/ARCHITECTURE.md §3.4)
        .register_uri_scheme_protocol(plugin_protocol::SCHEME, |ctx, request| {
            plugin_protocol::handle(ctx.app_handle(), request)
        })
        .invoke_handler(tauri::generate_handler![
            dock::dock_set_config,
            plugin_bridge::plugin_bridge,
            plugin_manager::plugin_install,
            plugin_manager::plugin_uninstall,
            plugin_manager::launch_plugin,
            store::store_list,
            store::store_install,
            tray::tray_set_language,
            workflow_engine::workflow_execute,
            workflow_engine::workflow_get,
            workflow_engine::workflow_list,
            workflow_engine::workflow_save,
            workflow_engine::workflow_delete,
            workflow_engine::workflow_runs,
            workflow_engine::workflow_run_steps,
            workflow_engine::trigger_save,
            workflow_engine::trigger_delete,
            workflow_engine::trigger_list
        ])
        // 主窗口关闭 = 隐藏驻留托盘，退出仅走托盘菜单；独立插件窗口销毁时清理事件订阅
        // Closing the main window hides it to the tray; a destroyed plugin window purges
        // its event subscriptions
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if window.label() == "main" {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    plugin_bridge::event_purge_window(window.label());
                }
                _ => {}
            }
        })
        .setup(|app| {
            // 事件推送总线：注入宿主句柄（events.emit 扇出用）
            // Event push bus: inject the host handle (used by the events.emit fan-out)
            plugin_bridge::init_event_bus(app.handle().clone());
            // 工作流引擎：setup 阶段直接新建一个 SQLite 池，与 plugin-sql 共用同一 DB 文件
            // Workflow engine: open a dedicated SQLite pool during setup (same DB file as plugin-sql)
            // plugin-sql 的 DbInstances 仅在 load 命令被前端调用后才填池（lazy 模式）
            // plugin-sql's DbInstances only fills in on the load command (lazy)
            // 工作流命令路径无需等前端 load，因此这里直接连 + apply migrations
            // The workflow command path doesn't wait for the frontend load; connect + migrate here
            let wasm = Arc::new(
                app.state::<wasm_runtime::WasmRuntime>()
                    .inner()
                    .clone(),
            );
            let app_handle = app.handle().clone();
            let pool_for_engine = tauri::async_runtime::block_on(async move {
                workflow_engine::open_pool_with_migrations(&app_handle).await
            })?;
            app.manage(Arc::new(workflow_engine::WorkflowEngine::new(
                wasm, // Arc<WasmRuntime>
                pool_for_engine,
            )));
            // 启动已持久化的触发器（schedule / plugin_event / clipboard / hotkey）
            // Start persisted triggers (schedule / plugin_event / clipboard / hotkey)
            let engine = app
                .state::<Arc<workflow_engine::WorkflowEngine>>()
                .inner()
                .clone();
            let app_for_triggers = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(_e) = engine.start_triggers(app_for_triggers).await {
                    eprintln!("failed to start triggers: {_e}");
                }
            });
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
