// 插件安装与卸载
// Plugin install and uninstall
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sqlx::Row;
use tauri::{AppHandle, Manager};

use crate::plugin::pool::sqlite_pool;
use crate::plugin::resolve::{ensure_entries_exist, plan_install};
use crate::wasm::engine::WasmRuntime;

// 数据库连接键：与前端 Database.load(...) 保持一致
// DB connection key: must match the frontend Database.load(...)

// 插件独立窗口 label：Tauri label 字符集不含 "."，反向域名 id 需替换为 "_"
// Independent-window label: Tauri labels disallow "."; reverse-domain ids map dots to underscores
// label 仅用于窗口查找 / 聚焦 / 关闭的确定性映射；插件 id 权威来源是窗口 URL 的路由参数
// The label is only a deterministic handle for lookup/focus/close; the authoritative
// plugin id travels in the window URL route
pub fn plugin_window_label(plugin_id: &str) -> String {
    format!("plugin-{}", plugin_id.replace('.', "_"))
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

    // 声明形态随行下发（前端模式下拉只渲染可选项；解析失败回退空集，启动裁决仍在 launch）
    // Declared shapes ride the row (the frontend dropdown renders only these; parse
    // failures fall back to an empty set — launch stays the authority)
    let has_web = opt_s("web_path")?.is_some();
    let has_wasm = opt_s("wasm_path")?.is_some();
    let supported_modes: Vec<&str> = crate::plugin::resolve::resolve_supported_windows(&s("manifest")?, has_web, has_wasm)
        .map(|sup| {
            let mut v = Vec::new();
            if sup.embedded.is_some() {
                v.push("embedded");
            }
            if sup.independent.is_some() {
                v.push("independent");
            }
            if sup.headless {
                v.push("headless");
            }
            v
        })
        .unwrap_or_default();

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
        "supported_modes": supported_modes,
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
    install_from_dir(&app, &PathBuf::from(&source_dir), false).await
}

// 安装核心（本地导入与市场安装共用）：校验 → 复制 → 插入或更新行
// allow_update：已安装同名插件时按"更新"处理（市场路径）；本地导入保持拒绝
// Install core (shared by local import and the store): validate -> copy -> insert-or-update.
// allow_update: a same-id plugin updates in place (the store path); local import still rejects
pub(crate) async fn install_from_dir(
    app: &AppHandle,
    source_dir: &Path,
    allow_update: bool,
) -> Result<Value, String> {
    let manifest_json = std::fs::read_to_string(source_dir.join("manifest.json")).map_err(|_| {
        format!(
            "读取 manifest.json 失败 / cannot read manifest.json under {}",
            source_dir.display()
        )
    })?;

    let has_web = source_dir.join("web/index.html").is_file();
    let has_wasm = source_dir.join("main.wasm").is_file();
    let plan = plan_install(&manifest_json, has_web, has_wasm)?;
    // windows[] 入口文件存在性核验（纯函数校验之外的 IO 检查）
    // windows[] entry existence (the IO check beyond the pure validation)
    ensure_entries_exist(source_dir, &crate::plugin::resolve::resolve_supported_windows(&manifest_json, has_web, has_wasm)?)?;

    let pool = sqlite_pool(app).await?;

    // 已安装同名插件：本地导入拒绝；市场更新走 UPDATE（保留启停/排序）
    // Same-id plugin: local import rejects; the store updates in place (keeping enable/order)
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugins WHERE id = ?")
        .bind(&plan.manifest.id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;
    if count > 0 && !allow_update {
        return Err(format!(
            "插件已安装，请先卸载 / plugin already installed: {}",
            plan.manifest.id
        ));
    }

    // 更新时替换文件，先关掉还开着的独立窗口（避免旧页面挂在被替换的目录上）
    // On update, close a still-open independent window first (its page sits on the
    // directory being replaced)
    if count > 0 {
        if let Some(win) = app.get_webview_window(&plugin_window_label(&plan.manifest.id)) {
            let _ = win.close();
        }
    }

    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("plugins")
        .join(&plan.manifest.id);
    // 残留目录自愈：无库记录的历史目录直接清理后重装；更新路径同样先清再拷
    // Self-healing stale dir: a leftover dir with no DB row is removed before reinstall;
    // the update path likewise clears before copying
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("清理残留目录失败 / failed to clean stale dir: {e}"))?;
    }
    copy_dir_recursive(source_dir, &dest)?;

    if count > 0 {
        // 更新：窗口模式保留（新 manifest 不再支持时回退推导默认），其余元数据以新版为准
        // Update: keep window_mode (falling back to the derived default when the new
        // manifest no longer supports it); the rest of the metadata follows the new version
        let current_mode: String = sqlx::query_scalar("SELECT window_mode FROM plugins WHERE id = ?")
            .bind(&plan.manifest.id)
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?;
        let supported = crate::plugin::resolve::resolve_supported_windows(&manifest_json, has_web, has_wasm)?;
        let keep_mode = match current_mode.as_str() {
            "embedded" => supported.embedded.is_some(),
            "independent" => supported.independent.is_some(),
            "headless" => supported.headless,
            _ => false,
        };
        let window_mode = if keep_mode { current_mode } else { plan.window_mode.clone() };

        let updated = sqlx::query(
            "UPDATE plugins SET name=?, version=?, author=?, description=?, icon=?, category=?,
                manifest=?, install_path=?, wasm_path=?, web_path=?, window_mode=?, window_config=?,
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?",
        )
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
        .bind(&window_mode)
        .bind(&plan.window_config)
        .bind(&plan.manifest.id)
        .execute(&pool)
        .await;

        if let Err(e) = updated {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(format!(
                "更新 plugins 行失败 / failed to update plugins row: {e}"
            ));
        }
    } else {
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
    }

    // 重装自愈路径可能替换旧 wasm：清掉编译缓存，确保下次按新文件编译
    // The self-healing reinstall may replace the wasm; evict the compiled cache
    app.state::<WasmRuntime>().evict(&plan.manifest.id);

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
    app.state::<WasmRuntime>().evict(&plugin_id);

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
