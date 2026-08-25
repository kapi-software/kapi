// 阻止 release 构建在 Windows 上弹出额外的控制台窗口，严禁删除
// Prevents an extra console window on Windows in release builds, DO NOT REMOVE
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri_app_lib::run()
}
