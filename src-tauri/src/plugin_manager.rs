// 插件管理器：本地导入安装 / 卸载 / 统一启动分发（docs/PLUGINS.md §6、ARCHITECTURE.md §2.3）
// Plugin manager: local-dir install / uninstall / unified launch dispatch
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::plugin_protocol::is_valid_plugin_id;

// 数据库连接键：与前端 Database.load(...) 保持一致
// DB connection key: must match the frontend Database.load(...)
const DB_KEY: &str = "sqlite:kapi.db";

// 合法运行模式（docs/PLUGINS.md §2.1）
// Valid window modes (docs/PLUGINS.md §2.1)
const MODES: [&str; 3] = ["embedded", "independent", "headless"];

// manifest.window：独立窗口自定义参数（缺省字段由启动时回退默认值）
// manifest.window: custom independent-window params (launch falls back for missing fields)
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindow {
    pub mode: Option<String>,
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub resizable: Option<bool>,
    pub always_on_top: Option<bool>,
}

// manifest.json：安装校验所需字段（kapi_version / workflow / permissions 原样入库）
// manifest.json: fields needed for install validation (other keys stored verbatim)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub window: Option<ManifestWindow>,
}

// 安装计划：manifest 校验与入口推导的纯函数产物（无 IO，便于单测）
// Install plan: pure-function result of manifest validation and entry derivation (no IO)
#[derive(Debug)]
pub struct InstallPlan {
    pub manifest: Manifest,
    pub manifest_json: String,
    pub window_mode: String,
    pub window_config: Option<String>,
    pub web_path: Option<String>,
    pub wasm_path: Option<String>,
}

// 校验 manifest 并推导安装计划（has_web/has_wasm = 源目录入口探测结果）
// Validate the manifest and derive the install plan (has_web/has_wasm = probed entries)
pub fn plan_install(
    manifest_json: &str,
    has_web: bool,
    has_wasm: bool,
) -> Result<InstallPlan, String> {
    let manifest: Manifest = serde_json::from_str(manifest_json)
        .map_err(|e| format!("manifest.json 解析失败 / invalid manifest.json: {e}"))?;

    if !is_valid_plugin_id(&manifest.id) {
        return Err(format!(
            "插件 id 非法（仅限 [A-Za-z0-9._-]）/ invalid plugin id: {}",
            manifest.id
        ));
    }
    if manifest.name.trim().is_empty() {
        return Err("manifest 缺少 name / manifest is missing name".into());
    }
    if manifest.version.trim().is_empty() {
        return Err("manifest 缺少 version / manifest is missing version".into());
    }
    if let Some(mode) = manifest.window.as_ref().and_then(|w| w.mode.as_deref()) {
        if !MODES.contains(&mode) {
            return Err(format!("非法 window.mode / invalid window.mode: {mode}"));
        }
    }
    if !has_web && !has_wasm {
        return Err(
            "插件缺少入口（web/index.html 或 main.wasm 至少其一）/ plugin has no entry (web/index.html or main.wasm)"
                .into(),
        );
    }

    // 运行模式：显式声明优先，否则按入口推导（有 UI → embedded，纯逻辑 → headless）
    // Window mode: explicit wins; otherwise derived from entries (UI → embedded, logic-only → headless)
    let window_mode = manifest
        .window
        .as_ref()
        .and_then(|w| w.mode.clone())
        .unwrap_or_else(|| if has_web { "embedded".into() } else { "headless".into() });

    // window_config 快照：仅当 manifest 声明了 window 时入库
    // window_config snapshot: stored only when the manifest declares a window
    let window_config = match &manifest.window {
        Some(w) => Some(serde_json::to_string(w).map_err(|e| e.to_string())?),
        None => None,
    };

    Ok(InstallPlan {
        manifest_json: manifest_json.to_string(),
        window_mode,
        window_config,
        web_path: if has_web {
            Some("web/index.html".into())
        } else {
            None
        },
        wasm_path: if has_wasm { Some("main.wasm".into()) } else { None },
        manifest,
    })
}

// 递归复制目录：安装即整包拷贝（源目录 → plugins/{id}）
// Recursively copy a directory: installing copies the whole package
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!(
            "源目录不存在 / source dir not found: {}",
            src.display()
        ));
    }
    std::fs::create_dir_all(dst).map_err(|e| format!("创建目录失败 / failed to create dir: {e}"))?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let to = dst.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)
                .map_err(|e| format!("复制文件失败 / failed to copy file: {e}"))?;
        }
    }
    Ok(())
}

// 插件独立窗口 label：Tauri label 字符集不含 "."，反向域名 id 需替换为 "_"
// Independent-window label: Tauri labels disallow "."; reverse-domain ids map dots to underscores
// label 仅用于窗口查找 / 聚焦 / 关闭的确定性映射；插件 id 权威来源是窗口 URL 的路由参数
// The label is only a deterministic handle for lookup/focus/close; the authoritative
// plugin id travels in the window URL route
fn plugin_window_label(plugin_id: &str) -> String {
    format!("plugin-{}", plugin_id.replace('.', "_"))
}

// 共享 SQLite 连接池：前端 Database.load 创建（tauri-plugin-sql 状态），此处只取用
// Shared SQLite pool: created by the frontend Database.load (tauri-plugin-sql state)
// 插件未导出 sqlite() 访问器，直接匹配枚举变体；SqlitePool 为 Arc 句柄，克隆廉价
// The plugin exports no sqlite() accessor, so match the variant; SqlitePool is an Arc handle
async fn sqlite_pool(app: &AppHandle) -> Result<sqlx::SqlitePool, String> {
    let instances = app.state::<DbInstances>();
    // tokio RwLock：await 取读锁（无中毒语义）；守卫显式绑定，确保 await 后即可释放
    // tokio RwLock: await the read lock (no poisoning); the named guard drops right after the clone
    let guard = instances.0.read().await;
    match guard.get(DB_KEY) {
        Some(DbPool::Sqlite(pool)) => Ok(pool.clone()),
        _ => Err("数据库尚未初始化 / database not initialized yet".to_string()),
    }
}

// plugins 表行 → 前端 Plugin 形状的 JSON（manifest / window_config 已解析）
// plugins row -> frontend Plugin-shaped JSON (manifest / window_config parsed)
fn row_to_plugin(row: &sqlx::sqlite::SqliteRow) -> Result<Value, String> {
    let s = |col: &str| -> Result<String, String> {
        row.try_get::<String, _>(col)
            .map_err(|e| format!("column {col}: {e}"))
    };
    let opt_s = |col: &str| -> Result<Option<String>, String> {
        row.try_get::<Option<String>, _>(col)
            .map_err(|e| format!("column {col}: {e}"))
    };
    let i = |col: &str| -> Result<i64, String> {
        row.try_get::<i64, _>(col)
            .map_err(|e| format!("column {col}: {e}"))
    };

    let manifest: Value = serde_json::from_str(&s("manifest")?).unwrap_or(Value::Null);
    let window_config: Option<Value> = opt_s("window_config")?.and_then(|j| serde_json::from_str(&j).ok());

    Ok(json!({
        "id": s("id")?,
        "name": s("name")?,
        "version": s("version")?,
        "author": opt_s("author")?,
        "description": opt_s("description")?,
        "icon": opt_s("icon")?,
        "category": opt_s("category")?,
        "manifest": manifest,
        "install_path": s("install_path")?,
        "wasm_path": opt_s("wasm_path")?,
        "web_path": opt_s("web_path")?,
        "window_mode": s("window_mode")?,
        "window_config": window_config,
        "is_enabled": i("is_enabled")? != 0,
        "is_installed": i("is_installed")? != 0,
        "sort_order": i("sort_order")?,
        "installed_at": s("installed_at")?,
        "updated_at": s("updated_at")?,
    }))
}

// 本地导入安装：manifest 校验 → 复制到 plugins/{id} → 写 plugins 表
// Install from a local dir: validate the manifest, copy to plugins/{id}, insert the row
#[tauri::command]
pub async fn plugin_install(app: AppHandle, source_dir: String) -> Result<Value, String> {
    let src = PathBuf::from(&source_dir);
    let manifest_json = std::fs::read_to_string(src.join("manifest.json")).map_err(|_| {
        format!(
            "读取 manifest.json 失败 / cannot read manifest.json under {source_dir}"
        )
    })?;

    let plan = plan_install(
        &manifest_json,
        src.join("web/index.html").is_file(),
        src.join("main.wasm").is_file(),
    )?;

    let pool = sqlite_pool(&app).await?;

    // 已安装同名插件 → 拒绝（更新流程属插件市场，Phase 5）
    // Same-id plugin already installed -> reject (updates belong to the store, Phase 5)
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugins WHERE id = ?")
        .bind(&plan.manifest.id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if count > 0 {
        return Err(format!(
            "插件已安装，请先卸载 / plugin already installed: {}",
            plan.manifest.id
        ));
    }

    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("plugins")
        .join(&plan.manifest.id);
    // 残留目录自愈：无库记录的历史目录直接清理后重装
    // Self-healing stale dir: a leftover dir with no DB row is removed before reinstall
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("清理残留目录失败 / failed to clean stale dir: {e}"))?;
    }
    copy_dir_recursive(&src, &dest)?;

    // 入库；sort_order 追加到队尾（Dock 与插件列表共用排序）
    // Insert; sort_order appended at the end (shared by the Dock and plugin list)
    let inserted = sqlx::query(
        "INSERT INTO plugins (id, name, version, author, description, icon, category, manifest,
            install_path, wasm_path, web_path, window_mode, window_config,
            is_enabled, is_installed, sort_order)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,1,1,
            (SELECT COALESCE(MAX(sort_order),-1)+1 FROM plugins))",
    )
    .bind(&plan.manifest.id)
    .bind(&plan.manifest.name)
    .bind(&plan.manifest.version)
    .bind(&plan.manifest.author)
    .bind(&plan.manifest.description)
    .bind(&plan.manifest.icon)
    .bind(&plan.manifest.category)
    .bind(&plan.manifest_json)
    .bind(dest.to_string_lossy().as_ref())
    .bind(&plan.wasm_path)
    .bind(&plan.web_path)
    .bind(&plan.window_mode)
    .bind(&plan.window_config)
    .execute(&pool)
    .await;

    if let Err(e) = inserted {
        // 入库失败回滚目录，避免残留
        // Roll back the copied dir on insert failure
        let _ = std::fs::remove_dir_all(&dest);
        return Err(format!(
            "写入 plugins 表失败 / failed to insert into plugins: {e}"
        ));
    }

    let row = sqlx::query("SELECT * FROM plugins WHERE id = ?")
        .bind(&plan.manifest.id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    row_to_plugin(&row)
}

// 卸载：关闭独立窗口 → 删库行（级联清 plugin_data）→ 删安装目录
// Uninstall: close the independent window, delete the row (cascades plugin_data), remove the dir
#[tauri::command]
pub async fn plugin_uninstall(app: AppHandle, plugin_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugins WHERE id = ?")
        .bind(&plugin_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if count == 0 {
        return Err(format!("插件不存在 / plugin not found: {plugin_id}"));
    }

    if let Some(win) = app.get_webview_window(&plugin_window_label(&plugin_id)) {
        let _ = win.close();
    }

    sqlx::query("DELETE FROM plugins WHERE id = ?")
        .bind(&plugin_id)
        .execute(&pool)
        .await
        .map_err(|e| format!("删除插件记录失败 / failed to delete plugin row: {e}"))?;

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("plugins")
        .join(&plugin_id);
    std::fs::remove_dir_all(&dir)
        .map_err(|e| format!("安装目录删除失败 / failed to remove install dir: {e}"))?;
    Ok(())
}

// 统一插件启动入口（Dock / 主面板 / 快捷键共用，docs/ARCHITECTURE.md §2.3）
// Unified plugin launch entry (shared by the Dock, panel and shortcuts)
#[tauri::command]
pub async fn launch_plugin(app: AppHandle, plugin_id: String) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;

    let row = sqlx::query(
        "SELECT window_mode, window_config, is_enabled FROM plugins WHERE id = ? AND is_installed = 1",
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
    let config_json: Option<String> = row
        .try_get::<Option<String>, _>("window_config")
        .map_err(|e| e.to_string())?;

    match mode.as_str() {
        // 内嵌：主窗口唤起并通知路由到 /plugin/:id
        // Embedded: show the main window and navigate it to /plugin/:id
        "embedded" => {
            if let Some(main) = app.get_webview_window("main") {
                main.show().map_err(|e| e.to_string())?;
                main.set_focus().map_err(|e| e.to_string())?;
                main.emit("plugin:navigate", &plugin_id)
                    .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                Err("主窗口不存在 / main window not found".into())
            }
        }
        // 独立窗口：存在则聚焦，不存在按 manifest.window 创建
        // Independent: focus the existing window or create one per manifest.window
        "independent" => {
            let label = plugin_window_label(&plugin_id);
            if let Some(win) = app.get_webview_window(&label) {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
                Ok(())
            } else {
                create_plugin_window(&app, &plugin_id, config_json.as_deref())
            }
        }
        // headless：WASM 运行时属 Phase 4 后续步骤（wasm_runtime.rs）
        // headless: the WASM runtime is a later Phase 4 step (wasm_runtime.rs)
        _ => Err(
            "headless 模式依赖 WASM 运行时（Phase 4 后续步骤）/ headless mode requires the WASM runtime (later in Phase 4)"
                .into(),
        ),
    }
}

// 创建插件独立窗口：参数取自 manifest.window，缺省 800×600 可缩放居中
// Create the independent window: params from manifest.window; defaults 800x600, resizable, centered
fn create_plugin_window(
    app: &AppHandle,
    plugin_id: &str,
    config_json: Option<&str>,
) -> Result<(), String> {
    let cfg: ManifestWindow = config_json
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();

    let label = plugin_window_label(plugin_id);
    let title = cfg.title.clone().unwrap_or_else(|| plugin_id.to_string());
    let url = WebviewUrl::App(format!("/plugin-window/{plugin_id}").into());

    let mut builder = WebviewWindowBuilder::new(app, &label, url)
        .title(&title)
        .inner_size(cfg.width.unwrap_or(800.0), cfg.height.unwrap_or(600.0))
        .resizable(cfg.resizable.unwrap_or(true))
        .always_on_top(cfg.always_on_top.unwrap_or(false))
        .center();
    if let (Some(w), Some(h)) = (cfg.min_width, cfg.min_height) {
        builder = builder.min_inner_size(w, h);
    }
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- plan_install：manifest 校验 / manifest validation ----

    // 生成最小合法 manifest（按需覆盖字段；JSON 重复键后者生效；_tail 防尾逗号）
    // Minimal valid manifest (override as needed; last duplicate key wins; _tail avoids trailing commas)
    fn manifest_json(overrides: &str) -> String {
        format!(
            r#"{{
                "id": "com.example.demo",
                "name": "Demo",
                "version": "1.0.0",
                "author": "Kapi",
                "description": "demo plugin",
                "icon": "icon.png",
                "category": "tool",
                "window": {{"mode": "embedded", "title": "Demo", "width": 420, "height": 640,
                            "minWidth": 320, "minHeight": 400, "resizable": true, "alwaysOnTop": false}},
                {overrides}
                "_tail": true
            }}"#
        )
    }

    #[test]
    fn plans_full_manifest() {
        let plan = plan_install(&manifest_json(""), true, false).unwrap();
        assert_eq!(plan.manifest.id, "com.example.demo");
        assert_eq!(plan.window_mode, "embedded");
        assert_eq!(plan.web_path.as_deref(), Some("web/index.html"));
        assert_eq!(plan.wasm_path, None);
        // window_config 快照保留 camelCase 键
        // The window_config snapshot keeps camelCase keys
        let wc = plan.window_config.unwrap();
        assert!(wc.contains("\"minWidth\""));
        assert!(wc.contains("\"alwaysOnTop\""));
    }

    #[test]
    fn rejects_missing_required_fields() {
        assert!(plan_install(&manifest_json("\"name\": \"\""), true, false).is_err());
        assert!(plan_install(&manifest_json("\"version\": \"\""), true, false).is_err());
        // id 非法字符 / invalid id charset
        assert!(plan_install(&manifest_json("\"id\": \"com foo\""), true, false).is_err());
        assert!(plan_install(&manifest_json("\"id\": \"../x\""), true, false).is_err());
    }

    #[test]
    fn rejects_invalid_json_and_mode() {
        assert!(plan_install("{ not json }", true, false).is_err());
        assert!(plan_install(&manifest_json("\"window\": {\"mode\": \"popup\"}"), true, false).is_err());
    }

    #[test]
    fn rejects_plugin_without_any_entry() {
        assert!(plan_install(&manifest_json(""), false, false).is_err());
    }

    #[test]
    fn derives_mode_from_entries_when_unset() {
        // 未声明 window：有 web → embedded，仅 wasm → headless
        // No window declared: web → embedded, wasm-only → headless
        let no_window = r#"{"id":"com.example.demo","name":"Demo","version":"1.0.0"}"#;
        assert_eq!(plan_install(no_window, true, false).unwrap().window_mode, "embedded");
        assert_eq!(plan_install(no_window, false, true).unwrap().window_mode, "headless");
        let plan = plan_install(no_window, true, false).unwrap();
        assert_eq!(plan.window_config, None);
    }

    #[test]
    fn explicit_mode_wins_over_entries() {
        let headless = r#"{"id":"com.example.demo","name":"Demo","version":"1.0.0","window":{"mode":"headless"}}"#;
        assert_eq!(plan_install(headless, true, false).unwrap().window_mode, "headless");
    }

    #[test]
    fn wasm_entry_recorded() {
        let plan = plan_install(&manifest_json(""), true, true).unwrap();
        assert_eq!(plan.wasm_path.as_deref(), Some("main.wasm"));
    }

    // ---- copy_dir_recursive：目录复制 / dir copying ----

    fn temp_dir_for(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kapi-mgr-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_nested_tree() {
        let root = temp_dir_for("copy");
        let src = root.join("src");
        std::fs::create_dir_all(src.join("web/assets")).unwrap();
        std::fs::write(src.join("manifest.json"), "{}").unwrap();
        std::fs::write(src.join("web/index.html"), "<html>hi</html>").unwrap();
        std::fs::write(src.join("web/assets/app.js"), "console.log(1)").unwrap();

        let dst = root.join("dst");
        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("web/index.html")).unwrap(),
            "<html>hi</html>"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("web/assets/app.js")).unwrap(),
            "console.log(1)"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_fails_for_missing_source() {
        let root = temp_dir_for("copy-missing");
        assert!(copy_dir_recursive(&root.join("nope"), &root.join("dst")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn window_label_dots_are_sanitized() {
        // Tauri label 禁止 "."：反向域名 id 必须映射为合法字符
        // Tauri labels forbid ".": reverse-domain ids must map to legal characters
        assert_eq!(
            plugin_window_label("com.kapi.sample.plugin-a"),
            "plugin-com_kapi_sample_plugin-a"
        );
        // 同一 id 重复计算结果一致（聚焦已有窗口依赖确定性）
        // Repeated computation stays stable (focusing relies on determinism)
        assert_eq!(
            plugin_window_label("com.kapi.sample.plugin-a"),
            plugin_window_label("com.kapi.sample.plugin-a")
        );
        // 无点 id 原样保留
        // Dot-free ids pass through unchanged
        assert_eq!(plugin_window_label("simple_id-1"), "plugin-simple_id-1");
    }
}
