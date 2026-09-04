// 插件行变更命令：启停 / 模式切换 / 排序（前端唯一写入口，见 docs/DATABASE.md 写者矩阵）
// Plugin-row mutation commands: enable, mode switch, reorder (the sole frontend write path)
use sqlx::Row;
use tauri::AppHandle;

use crate::plugin::pool::sqlite_pool;
use crate::plugin::resolve::resolve_supported_windows;

// 启用 / 禁用：禁用后 Dock 与侧边栏隐藏，launch 侧仍按行内 is_enabled 裁决
// Enable / disable: disabled plugins hide from the Dock and sidebar; launch still
// decides by the in-row is_enabled flag
#[tauri::command]
pub async fn plugin_set_enabled(
    app: AppHandle,
    plugin_id: String,
    enabled: bool,
) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let res = sqlx::query(
        "UPDATE plugins SET is_enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(if enabled { 1 } else { 0 })
    .bind(&plugin_id)
    .execute(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    if res.rows_affected() == 0 {
        return Err(format!("插件不存在 / plugin not found: {plugin_id}"));
    }
    Ok(())
}

// 切换运行模式：先按 manifest 推导支持形态再落库，不支持的形态直接拒绝
// Switch window mode: derive supported shapes from the manifest first, reject unsupported ones
// 模式合法性由此收口到写路径，前端下拉与 Rust 写库不再各判各的
// Mode legality now lives in the write path; the frontend dropdown and Rust no longer diverge
#[tauri::command]
pub async fn plugin_set_window_mode(
    app: AppHandle,
    plugin_id: String,
    mode: String,
) -> Result<(), String> {
    if !matches!(mode.as_str(), "embedded" | "independent" | "headless") {
        return Err(format!("未知窗口模式 / unknown window mode: {mode}"));
    }

    let pool = sqlite_pool(&app).await?;
    let row = sqlx::query("SELECT manifest, web_path, wasm_path FROM plugins WHERE id = ?")
        .bind(&plugin_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    // 行不存在即拒绝；无法推导支持形态（manifest 损坏）同样拒绝而不是回退
    // Reject when the row is missing; an underivable shape (broken manifest) rejects too
    let row = row.ok_or_else(|| format!("插件不存在 / plugin not found: {plugin_id}"))?;

    let has_web: Option<String> = row
        .try_get("web_path")
        .map_err(|e| format!("StorageError: {e}"))?;
    let has_wasm: Option<String> = row
        .try_get("wasm_path")
        .map_err(|e| format!("StorageError: {e}"))?;
    let manifest: String = row
        .try_get("manifest")
        .map_err(|e| format!("StorageError: {e}"))?;
    let sup = resolve_supported_windows(&manifest, has_web.is_some(), has_wasm.is_some())?;

    let supported = match mode.as_str() {
        "embedded" => sup.embedded.is_some(),
        "independent" => sup.independent.is_some(),
        "headless" => sup.headless,
        _ => false,
    };
    if !supported {
        return Err(format!(
            "插件不支持该窗口模式 / plugin does not support window mode: {mode}"
        ));
    }

    let res = sqlx::query(
        "UPDATE plugins SET window_mode = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&mode)
    .bind(&plugin_id)
    .execute(&pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    if res.rows_affected() == 0 {
        return Err(format!("插件不存在 / plugin not found: {plugin_id}"));
    }
    Ok(())
}

// 批量重排排序：入参即全量顺序，单事务重写（Dock 与插件列表共用）
// Batch reorder: the input is the full ordering, rewritten in one transaction
#[tauri::command]
pub async fn plugin_reorder(app: AppHandle, ordered_ids: Vec<String>) -> Result<(), String> {
    let pool = sqlite_pool(&app).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    for (i, id) in ordered_ids.iter().enumerate() {
        sqlx::query("UPDATE plugins SET sort_order = ? WHERE id = ?")
            .bind(i as i64)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("StorageError: {e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}
