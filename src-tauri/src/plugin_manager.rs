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

// 窗口参数（不含 mode）：legacy window 字段与 windows[] 条目共用，对齐 Tauri 窗口选项
// Window params (mode excluded): shared by the legacy window field and windows[] entries
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindowParams {
    pub title: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub resizable: Option<bool>,
    pub always_on_top: Option<bool>,
    // 透明背景：需窗口与页面双透明；Linux X11 无合成器时退化为黑底
    // Transparent: needs window + page transparency; black on X11 without a compositor
    pub transparent: Option<bool>,
    // 无边框（隐藏标题栏）；默认 true / frameless (hides the title bar); default true
    pub decorations: Option<bool>,
    // 不在任务栏显示；默认 false / hide from the taskbar; default false
    pub skip_taskbar: Option<bool>,
    // 窗口投影（仅 Windows/Linux）；默认 true / shadow (Windows/Linux only); default true
    pub shadow: Option<bool>,
    // 居中创建；默认 true / center on creation; default true
    pub center: Option<bool>,
    pub fullscreen: Option<bool>,
}

// manifest.window：legacy 单形态声明（mode 与参数扁平同层；缺省字段由启动时回退默认值）
// manifest.window: the legacy single-shape declaration (mode flattened with params)
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindow {
    pub mode: Option<String>,
    #[serde(flatten)]
    pub params: ManifestWindowParams,
}

// manifest.windows[]：多形态声明（mode + entry + 参数）；entry 相对 web/，如 "index.html"
// manifest.windows[]: multi-shape declaration (mode + entry + params); entry is web/-relative
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestWindowEntry {
    pub mode: Option<String>,
    pub entry: Option<String>,
    #[serde(flatten)]
    pub params: ManifestWindowParams,
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
    pub windows: Option<Vec<ManifestWindowEntry>>,
}

// 解析后的单个形态：入口文件（相对 web/）+ 窗口参数
// One resolved shape: the entry file (web/-relative) plus window params
#[derive(Debug, Default, Clone)]
pub struct ResolvedWindow {
    pub entry: String,
    pub params: ManifestWindowParams,
}

// 插件声明支持的形态：windows 数组优先，legacy window 回退；headless = 有 wasm 入口
// The plugin's declared shapes: the windows array wins, the legacy window field is the
// fallback; headless support equals having a wasm entry
#[derive(Debug, Default)]
pub struct SupportedWindows {
    pub embedded: Option<ResolvedWindow>,
    pub independent: Option<ResolvedWindow>,
    pub headless: bool,
}

// 形态支持解析（纯函数）：windows[] 逐条入位（每 mode 至多一条）；无数组时按 legacy
// window.mode（缺省 embedded）单形态、入口固定 index.html；无 web 入口则无窗口形态
// Shape resolution (pure): windows[] entries slot in (at most one per mode); without the
// array the legacy window.mode (default embedded) yields a single index.html shape;
// no web entry means no window shapes at all
pub fn resolve_supported_windows(
    manifest_json: &str,
    has_web: bool,
    has_wasm: bool,
) -> Result<SupportedWindows, String> {
    let manifest: Manifest = serde_json::from_str(manifest_json)
        .map_err(|e| format!("manifest.json 解析失败 / invalid manifest.json: {e}"))?;
    let mut out = SupportedWindows { headless: has_wasm, ..Default::default() };

    if let Some(entries) = manifest.windows {
        for entry in entries {
            let mode = entry.mode.clone().unwrap_or_else(|| "embedded".into());
            let resolved = ResolvedWindow {
                entry: entry.entry.clone().unwrap_or_else(|| "index.html".into()),
                params: entry.params,
            };
            match mode.as_str() {
                "embedded" if out.embedded.is_none() => out.embedded = Some(resolved),
                "independent" if out.independent.is_none() => out.independent = Some(resolved),
                // headless 由 main.wasm 决定，不进 windows[] / headless comes from main.wasm only
                "headless" => {
                    return Err("windows[] 不支持 headless（由 main.wasm 决定）/ headless is not a windows[] mode (decided by main.wasm)".into())
                }
                other => {
                    return Err(format!(
                        "windows[] mode 非法或重复 / invalid or duplicate windows[] mode: {other}"
                    ))
                }
            }
        }
    } else {
        // legacy：单一形态（mode 缺省 embedded），入口固定 index.html
        // legacy: a single shape (mode defaults to embedded) with the fixed index.html entry
        let window = manifest.window.unwrap_or_default();
        let resolved =
            ResolvedWindow { entry: "index.html".into(), params: window.params };
        match window.mode.as_deref().unwrap_or("embedded") {
            "embedded" => out.embedded = Some(resolved),
            "independent" => out.independent = Some(resolved),
            // headless 声明不产生窗口形态 / a headless declaration yields no window shape
            _ => {}
        }
    }

    // headless-only（无 web 入口）：两种窗口形态都不存在
    // headless-only (no web entry): neither window shape exists
    if !has_web {
        out.embedded = None;
        out.independent = None;
    }
    Ok(out)
}

// entry 文件存在性核验：windows[] 每个形态的入口必须真实存在（命令侧，plan_install 保持纯函数）
// Entry file existence: every declared shape's entry must exist (command-side; plan_install stays pure)
fn ensure_entries_exist(src: &Path, supported: &SupportedWindows) -> Result<(), String> {
    for resolved in [&supported.embedded, &supported.independent].into_iter().flatten() {
        if !src.join("web").join(&resolved.entry).is_file() {
            return Err(format!(
                "windows[].entry 文件不存在 / missing entry file: web/{}",
                resolved.entry
            ));
        }
    }
    Ok(())
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

    // 形态支持解析 + windows[] 校验（mode 白名单/唯一、entry 路径安全；文件存在性由调用侧核验）
    // Shape resolution plus windows[] validation (whitelisted/unique modes; path-safe entries —
    // file existence is the caller's check since plan_install stays pure)
    let supported = resolve_supported_windows(manifest_json, has_web, has_wasm)?;
    for resolved in [&supported.embedded, &supported.independent].into_iter().flatten() {
        if !is_safe_entry(&resolved.entry) {
            return Err(format!(
                "windows[].entry 非法（每段仅限 [A-Za-z0-9._-]）/ invalid windows entry: {}",
                resolved.entry
            ));
        }
    }

    // 运行模式：legacy window.mode 显式声明优先，否则按支持形态取默认（embedded 优先）
    // Window mode: an explicit legacy window.mode wins; otherwise default from the
    // supported shapes (embedded first)
    let window_mode = match manifest.window.as_ref().and_then(|w| w.mode.clone()) {
        Some(mode) => mode,
        None => if supported.embedded.is_some() {
            "embedded"
        } else if supported.independent.is_some() {
            "independent"
        } else {
            "headless"
        }
        .to_string(),
    };

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

// entry 路径安全（相对 web/）：非空、无前导 /、每段仅 [A-Za-z0-9._-]（URL 安全）
// Entry path safety (web/-relative): non-empty, no leading slash, slug segments (URL-safe)
fn is_safe_entry(entry: &str) -> bool {
    !entry.is_empty()
        && !entry.starts_with('/')
        && entry.split('/').all(|seg| {
            !seg.is_empty()
                && seg != ".."
                && seg != "."
                && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
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
pub(crate) fn plugin_window_label(plugin_id: &str) -> String {
    format!("plugin-{}", plugin_id.replace('.', "_"))
}

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

    let has_web = src.join("web/index.html").is_file();
    let has_wasm = src.join("main.wasm").is_file();
    let plan = plan_install(&manifest_json, has_web, has_wasm)?;
    // windows[] 入口文件存在性核验（纯函数校验之外的 IO 检查）
    // windows[] entry existence (the IO check beyond the pure validation)
    ensure_entries_exist(&src, &resolve_supported_windows(&manifest_json, has_web, has_wasm)?)?;

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

    // 重装自愈路径可能替换旧 wasm：清掉编译缓存，确保下次按新文件编译
    // The self-healing reinstall may replace the wasm; evict the compiled cache
    app.state::<crate::wasm_runtime::WasmRuntime>()
        .evict(&plan.manifest.id);

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

    // 卸载即清编译缓存（同名重装不会命中旧模块）
    // Uninstall evicts the compiled cache (a same-id reinstall never hits the old module)
    app.state::<crate::wasm_runtime::WasmRuntime>().evict(&plugin_id);

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
            let action = pick_headless_action(&manifest);

            let runtime = app.state::<crate::wasm_runtime::WasmRuntime>();
            let started = std::time::Instant::now();
            let outcome = runtime.invoke_action(&pool, &plugin_id, &action, &json!({})).await;
            let duration_ms = started.elapsed().as_millis() as u64;

            // 成败都写日志页（source = plugin:<id>）；失败原样上抛给前端 toast
            // Both outcomes land in the logs page (source = plugin:<id>); errors propagate to the toast
            match outcome {
                Ok(result) => {
                    crate::plugin_bridge::write_system_log(
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
                    let _ = crate::plugin_bridge::write_system_log(
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

// headless 默认动作：workflow.actions 中名为 run 的优先，否则首个 action，缺省字面量 "run"
// The headless default action: workflow.actions named "run" wins, then the first action, else "run"
pub(crate) fn pick_headless_action(manifest_json: &str) -> String {
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

// 创建插件独立窗口：入口与参数取自该形态的声明（对齐 Tauri 窗口选项），缺省 800×600 可缩放居中
// Create the independent window: entry + params from the shape's declaration; defaults 800x600
fn create_plugin_window(
    app: &AppHandle,
    plugin_id: &str,
    resolved: &ResolvedWindow,
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

    #[test]
    fn parses_tauri_aligned_window_options() {
        // Tauri 对齐窗口选项：camelCase 解析 + window_config 快照保留新键
        // Tauri-aligned window options: camelCase parsing + snapshot keeps the new keys
        let json = r#"{
            "id": "com.example.demo", "name": "Demo", "version": "1.0.0",
            "window": {"mode": "independent", "transparent": true, "decorations": false,
                        "skipTaskbar": true, "shadow": false, "center": false, "fullscreen": false}
        }"#;
        let plan = plan_install(json, true, false).unwrap();
        let w = &plan.manifest.window.unwrap().params;
        assert_eq!(w.transparent, Some(true));
        assert_eq!(w.decorations, Some(false));
        assert_eq!(w.skip_taskbar, Some(true));
        assert_eq!(w.shadow, Some(false));
        assert_eq!(w.center, Some(false));
        assert_eq!(w.fullscreen, Some(false));
        // 快照序列化回 camelCase，前端 PluginWindowConfig 直接可用
        // The snapshot serializes back to camelCase, directly usable by the frontend
        let wc = plan.window_config.unwrap();
        assert!(wc.contains("\"skipTaskbar\""));
        assert!(wc.contains("\"transparent\""));
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

    // ---- pick_headless_action：headless 默认动作 / default headless action ----

    // ---- resolve_supported_windows：形态支持解析 / shape-support resolution ----

    #[test]
    fn resolves_windows_array_with_per_mode_entries() {
        let json = r#"{
            "id": "com.example.demo", "name": "Demo", "version": "1.0.0",
            "windows": [
                { "mode": "embedded", "entry": "index.html" },
                { "mode": "independent", "entry": "window.html", "width": 420, "transparent": true }
            ]
        }"#;
        let s = resolve_supported_windows(json, true, true).unwrap();
        assert_eq!(s.embedded.as_ref().unwrap().entry, "index.html");
        let indep = s.independent.as_ref().unwrap();
        assert_eq!(indep.entry, "window.html");
        assert_eq!(indep.params.width, Some(420.0));
        assert_eq!(indep.params.transparent, Some(true));
        assert!(s.headless); // 有 wasm / has wasm
    }

    #[test]
    fn resolves_legacy_window_fallback() {
        // legacy：window.mode 单形态，入口固定 index.html
        // legacy: a single window.mode shape with the fixed index.html entry
        let s = resolve_supported_windows(
            r#"{"id":"x","name":"X","version":"1","window":{"mode":"independent","width":480}}"#,
            true,
            false,
        )
        .unwrap();
        assert!(s.embedded.is_none());
        assert_eq!(s.independent.as_ref().unwrap().entry, "index.html");
        assert_eq!(s.independent.as_ref().unwrap().params.width, Some(480.0));
        assert!(!s.headless);

        // 未声明 window：缺省 embedded / no window declared: embedded by default
        let s = resolve_supported_windows(r#"{"id":"x","name":"X","version":"1"}"#, true, false).unwrap();
        assert!(s.embedded.is_some());
        assert!(s.independent.is_none());

        // headless-only：无 web → 无窗口形态 / headless-only: no web -> no window shapes
        let s = resolve_supported_windows(
            r#"{"id":"x","name":"X","version":"1","window":{"mode":"headless"}}"#,
            false,
            true,
        )
        .unwrap();
        assert!(s.embedded.is_none());
        assert!(s.independent.is_none());
        assert!(s.headless);
    }

    #[test]
    fn rejects_bad_windows_arrays() {
        // 拼装最小合法前缀 / assemble the minimal valid prefix
        let base = r#"{"id":"x","name":"X","version":"1""#;
        let with = |windows: &str| format!("{base}, \"windows\": [{windows}]}}");

        // headless 不属于 windows[] / headless is not a windows[] mode
        assert!(resolve_supported_windows(
            &with(r#"{"mode":"headless","entry":"index.html"}"#),
            true,
            false
        )
        .is_err());
        // 重复 mode / duplicate mode
        assert!(resolve_supported_windows(
            &with(r#"{"mode":"embedded","entry":"a.html"},{"mode":"embedded","entry":"b.html"}"#),
            true,
            false
        )
        .is_err());
        // 非法 mode / invalid mode
        assert!(resolve_supported_windows(
            &with(r#"{"mode":"popup","entry":"a.html"}"#),
            true,
            false
        )
        .is_err());
    }

    #[test]
    fn plan_install_validates_entries_and_default_mode() {
        // entry 文件不存在 / missing entry file（此处纯函数层校验路径安全，存在性见下）
        let missing = r#"{
            "id":"com.example.demo","name":"Demo","version":"1.0.0",
            "windows":[{"mode":"embedded","entry":"../evil.html"}]
        }"#;
        assert!(plan_install(missing, true, false).is_err());
        let slash = r#"{
            "id":"com.example.demo","name":"Demo","version":"1.0.0",
            "windows":[{"mode":"embedded","entry":"/abs.html"}]
        }"#;
        assert!(plan_install(slash, true, false).is_err());

        // 仅声明 independent：默认模式取 independent / independent-only: defaults to independent
        let indep_only = r#"{
            "id":"com.example.demo","name":"Demo","version":"1.0.0",
            "windows":[{"mode":"independent","entry":"window.html","width":300}]
        }"#;
        let plan = plan_install(indep_only, true, false).unwrap();
        assert_eq!(plan.window_mode, "independent");

        // 存在性核验：src 缺文件时 ensure_entries_exist 拒绝 / existence check rejects
        let root = temp_dir_for("entries");
        let src = root.join("src");
        std::fs::create_dir_all(src.join("web")).unwrap();
        std::fs::write(src.join("manifest.json"), "{}").unwrap();
        std::fs::write(src.join("web/index.html"), "<html></html>").unwrap();
        let json = r#"{
            "id":"com.example.demo","name":"Demo","version":"1.0.0",
            "windows":[{"mode":"embedded","entry":"index.html"},{"mode":"independent","entry":"window.html"}]
        }"#;
        let supported = resolve_supported_windows(json, true, false).unwrap();
        // window.html 尚未创建 → 拒绝 / window.html not yet created -> rejected
        assert!(ensure_entries_exist(&src, &supported).is_err());
        std::fs::write(src.join("web/window.html"), "<html></html>").unwrap();
        assert!(ensure_entries_exist(&src, &supported).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn headless_action_prefers_run_then_first() {
        // run 优先 / a declared run wins
        let with_run = r#"{"workflow":{"actions":[{"name":"format"},{"name":"run"}]}}"#;
        assert_eq!(pick_headless_action(with_run), "run");
        // 无 run → 首个 action / no run -> the first action
        let no_run = r#"{"workflow":{"actions":[{"name":"format"},{"name":"save"}]}}"#;
        assert_eq!(pick_headless_action(no_run), "format");
        // 无 workflow / actions → 字面量 "run" / no workflow or actions -> the literal "run"
        assert_eq!(pick_headless_action(r#"{"id":"x"}"#), "run");
        assert_eq!(pick_headless_action("{ bad json"), "run");
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
