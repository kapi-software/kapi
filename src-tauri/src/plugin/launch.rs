// 插件启动与窗口管理
// Plugin launch and window management
use serde_json::json;
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::plugin::install::plugin_window_label;
use crate::plugin::pool::sqlite_pool;
use crate::plugin::resolve::resolve_supported_windows;
use crate::bridge::log::write_system_log;
use crate::wasm::engine::WasmRuntime;

// 创建插件独立窗口：入口与参数取自该形态的声明（对齐 Tauri 窗口选项），缺省 800×600 可缩放居中
// Create the independent window: entry + params from the shape's declaration; defaults 800x600
fn create_plugin_window(
    app: &AppHandle,
    plugin_id: &str,
    resolved: &crate::plugin::manifest::ResolvedWindow,
) -> Result<(), String> {
    let cfg = &resolved.params;

    let label = plugin_window_label(plugin_id);
    let title = cfg.title.clone().unwrap_or_else(|| plugin_id.to_string());
    // 入口随查询参数传给壳页（entry 已经 is_safe_entry 校验，URL 安全）
    // The entry rides a query param into the shell (kept URL-safe by is_safe_entry)
    let url = WebviewUrl::App(format!("/plugin-window/{plugin_id}?entry={}", resolved.entry).into());

    let mut builder = WebviewWindowBuilder::new(app, &label, url)
        .title(&title)
        .inner_size(cfg.width.unwrap_or(800.0), cfg.height.unwrap_or(600.0))
        .resizable(cfg.resizable.unwrap_or(true))
        .always_on_top(cfg.always_on_top.unwrap_or(false))
        .decorations(cfg.decorations.unwrap_or(true))
        .skip_taskbar(cfg.skip_taskbar.unwrap_or(false))
        .shadow(cfg.shadow.unwrap_or(true))
        .fullscreen(cfg.fullscreen.unwrap_or(false))
        // 隐藏创建防白屏闪烁：壳页内容就绪后自行 show（PluginWindowShell）
        // Created hidden to avoid a white flash; the shell shows itself once ready
        .visible(false);
    // 透明：macOS 由 macos-private-api feature 门控（已启用），条件调用保持语义清晰
    // Transparent: gated by the macos-private-api feature on macOS (enabled); conditional call
    if cfg.transparent.unwrap_or(false) {
        builder = builder.transparent(true);
    }
    // 居中：保持历史默认（此前无条件居中）
    // Centering: preserves the historical default (previously unconditional)
    if cfg.center.unwrap_or(true) {
        builder = builder.center();
    }
    if let (Some(w), Some(h)) = (cfg.min_width, cfg.min_height) {
        builder = builder.min_inner_size(w, h);
    }
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

// 显示插件独立窗口
// Show the plugin independent window
pub fn show_window(app: &AppHandle, plugin_id: &str) -> Result<(), String> {
    let label = plugin_window_label(plugin_id);
    if let Some(win) = app.get_webview_window(&label) {
        win.show().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err(format!("窗口不存在 / window not found: {plugin_id}"))
    }
}

// 关闭插件独立窗口
// Close the plugin independent window
pub fn close_window(app: &AppHandle, plugin_id: &str) -> Result<(), String> {
    let label = plugin_window_label(plugin_id);
    if let Some(win) = app.get_webview_window(&label) {
        win.close().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

// 统一插件启动入口（Dock / 主面板 / 快捷键共用，docs/ARCHITECTURE.md §2.3）
// Unified plugin launch entry (shared by the Dock, panel and shortcuts)
#[tauri::command]
pub async fn launch_plugin(app: AppHandle, plugin_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;

    let row = sqlx::query(
        "SELECT window_mode, web_path, wasm_path, manifest, is_enabled
         FROM plugins WHERE id = ? AND is_installed = 1",
    )
    .bind(&plugin_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("插件未安装 / plugin not installed: {plugin_id}"))?;

    if row.try_get::<i64, _>("is_enabled").map_err(|e| e.to_string())? == 0 {
        return Err("插件已禁用 / plugin is disabled".into());
    }
    let mode: String = row
        .try_get::<String, _>("window_mode")
        .map_err(|e| e.to_string())?;
    let has_web = row
        .try_get::<Option<String>, _>("web_path")
        .map_err(|e| e.to_string())?
        .is_some();
    let manifest: String = row
        .try_get::<String, _>("manifest")
        .map_err(|e| e.to_string())?;
    let wasm_path: Option<String> = row
        .try_get::<Option<String>, _>("wasm_path")
        .map_err(|e| e.to_string())?;

    // 形态支持以 manifest 为权威（windows[] 优先，legacy window 回退）
    // The manifest is authoritative for shape support (windows[] first, legacy fallback)
    let supported = resolve_supported_windows(&manifest, has_web, wasm_path.is_some())?;

    match mode.as_str() {
        // 内嵌：主窗口唤起并路由到 /plugin/:id（携带该形态的入口）
        // Embedded: show the main window and navigate to /plugin/:id with the shape's entry
        "embedded" => {
            let Some(resolved) = supported.embedded else {
                return Err(format!(
                    "UnsupportedMode: embedded not declared by {plugin_id}"
                ));
            };
            if let Some(main) = app.get_webview_window("main") {
                main.show().map_err(|e| e.to_string())?;
                main.set_focus().map_err(|e| e.to_string())?;
                main.emit("plugin:navigate", json!({ "id": plugin_id, "entry": resolved.entry }))
                    .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("主窗口不存在 / main window not found".into())
            }
        }
        // 独立窗口：存在则聚焦，不存在按该形态的入口与参数创建
        // Independent: focus the existing window or create one from the shape's entry + params
        "independent" => {
            let Some(resolved) = supported.independent else {
                return Err(format!(
                    "UnsupportedMode: independent not declared by {plugin_id}"
                ));
            };
            let label = plugin_window_label(&plugin_id);
            if let Some(win) = app.get_webview_window(&label) {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
                Ok(())
            } else {
                create_plugin_window(&app, &plugin_id, &resolved)
            }
        }
        // headless：立即执行一次默认动作（Phase 6 工作流引擎接管触发编排前）
        // headless: run the default action once (until the Phase 6 workflow engine takes over)
        _ => {
            if wasm_path.is_none() {
                return Err("headless 模式需要 main.wasm / headless mode requires main.wasm".into());
            }
            let action = crate::plugin::pool::pick_headless_action(&manifest);

            let runtime = app.state::<WasmRuntime>();
            let started = std::time::Instant::now();
            let outcome = runtime.invoke_action(&pool, &plugin_id, &action, &json!({})).await;
            let duration_ms = started.elapsed().as_millis() as u64;

            // 成败都写日志页（source = plugin:<id>）；失败原样上抛给前端 toast
            // Both outcomes land in the logs page (source = plugin:<id>); errors propagate to the toast
            match outcome {
                Ok(result) => {
                    write_system_log(
                        &pool,
                        "info",
                        &format!("headless action '{action}' completed"),
                        &format!("plugin:{plugin_id}"),
                        Some(json!({ "action": action, "durationMs": duration_ms, "result": result })),
                    )
                    .await?;
                    let _ = wasm_path; // 语义标记：已确认存在 WASM 入口 / WASM entry confirmed above
                    Ok(())
                }
                Err(err) => {
                    let _ = write_system_log(
                        &pool,
                        "error",
                        &err,
                        &format!("plugin:{plugin_id}"),
                        Some(json!({ "action": action, "durationMs": duration_ms, "error": err })),
                    )
                    .await;
                    Err(err)
                }
            }
        }
    }
}
