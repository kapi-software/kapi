// 通道分发
// Channel dispatch
use std::collections::HashSet;
use serde_json::{json, Value};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::bridge::event_bus::{event_publish, event_subscribe, event_unsubscribe};
use crate::bridge::log::write_system_log;
use crate::bridge::types::{
    ClipboardWritePayload, EventsEmitPayload, EventsOnPayload, HttpFetchPayload,
    LogPayload, PluginInvokePayload, StorageGetPayload, StorageRemovePayload,
    StorageSetPayload, WindowSetTitlePayload,
};
use crate::bridge::validate::{
    parse_payload, validate_action, validate_event_type, validate_key, validate_message, validate_title,
};
use crate::plugin::install::plugin_window_label;
use crate::plugin::pool::sqlite_pool;
use crate::plugin_protocol::is_valid_plugin_id;

// 桥接通道载荷边界
// Bridge payload limits
const MAX_VALUE_BYTES: usize = 1024 * 1024;

// 权限守卫
// Permission guard
#[derive(Debug, Default, Clone)]
pub struct PermissionGuard {
    permissions: HashSet<String>,
}

impl PermissionGuard {
    // 从 plugins.manifest JSON 解析 permissions 数组
    // Parse the permissions array from plugins.manifest JSON
    pub fn from_manifest_json(manifest_json: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(manifest_json)
            .map_err(|e| format!("PluginNotFound: invalid manifest ({e})"))?;
        let mut permissions = HashSet::new();
        if let Some(arr) = v.get("permissions").and_then(Value::as_array) {
            for p in arr {
                if let Some(s) = p.as_str() {
                    permissions.insert(s.to_string());
                }
            }
        }
        Ok(Self { permissions })
    }

    // 单项权限检查
    // Single permission check
    pub fn require(&self, perm: &str) -> Result<(), String> {
        if self.permissions.contains(perm) {
            Ok(())
        } else {
            Err(format!("PermissionDenied: {perm}"))
        }
    }

    // 域名白名单
    // Host whitelist
    pub fn allows_host(&self, host: &str) -> bool {
        self.permissions.contains(&format!("network:host:{host}"))
            || self.permissions.contains("network:host:*")
    }
}

// 通道 → 所需权限映射
// Channel → required permission
pub fn channel_permission(channel: &str) -> Option<&'static str> {
    match channel {
        "kapi:storage.get" => Some("storage:read"),
        "kapi:storage.set" | "kapi:storage.remove" => Some("storage:write"),
        "kapi:clipboard.read" => Some("clipboard:read"),
        "kapi:clipboard.write" => Some("clipboard:write"),
        "kapi:events.emit" => Some("events:emit"),
        _ => None,
    }
}

// 桥接上下文
// Bridge context
struct BridgeContext {
    guard: PermissionGuard,
}

// 加载桥接上下文
// Load the bridge context
async fn load_bridge_context(
    pool: &sqlx::SqlitePool,
    plugin_id: &str,
) -> Result<BridgeContext, String> {
    let row = sqlx::query("SELECT manifest, is_enabled, is_installed FROM plugins WHERE id = ?")
        .bind(plugin_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?
        .ok_or_else(|| format!("PluginNotFound: {plugin_id}"))?;

    if row.try_get::<i64, _>("is_installed").map_err(|e| format!("StorageError: {e}"))? == 0 {
        return Err(format!("PluginNotFound: {plugin_id} (uninstalled)"));
    }
    if row.try_get::<i64, _>("is_enabled").map_err(|e| format!("StorageError: {e}"))? == 0 {
        return Err(format!("PluginDisabled: {plugin_id}"));
    }
    let manifest: String = row
        .try_get::<String, _>("manifest")
        .map_err(|e| format!("StorageError: {e}"))?;

    Ok(BridgeContext {
        guard: PermissionGuard::from_manifest_json(&manifest)?,
    })
}

// HTTP 相关常量
// HTTP constants
const HTTP_TIMEOUT_SECS: u64 = 10;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const HTTP_METHODS: [&str; 6] = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"];
const STRIPPED_HEADERS: [&str; 2] = ["host", "content-length"];

// 从 URL 提取小写主机名
// Extract the lowercase host
fn extract_host(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw).map_err(|_| "HttpError: invalid url".to_string())?;
    match url.scheme() {
        "http" | "https" => {}
        _ => return Err("HttpError: only http/https urls are allowed".into()),
    }
    url.host_str()
        .map(|h| h.to_ascii_lowercase())
        .ok_or_else(|| "HttpError: url has no host".to_string())
}

// 方法归一化
// Method normalization
fn normalize_method(method: Option<&str>) -> Result<String, String> {
    let m = method.unwrap_or("GET").to_ascii_uppercase();
    if HTTP_METHODS.contains(&m.as_str()) {
        Ok(m)
    } else {
        Err(format!("InvalidPayload: unsupported method {m}"))
    }
}

// 共享 HTTP 客户端
// Shared HTTP client
fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build the plugin http client")
    })
}

// http.fetch handler
// http.fetch handler
pub async fn http_fetch(guard: &PermissionGuard, payload: Value) -> Result<Value, String> {
    let p: HttpFetchPayload = parse_payload(payload)?;
    let host = extract_host(&p.url)?;
    if !guard.allows_host(&host) {
        return Err(format!("PermissionDenied: network:host:{host}"));
    }
    let method = normalize_method(p.method.as_deref())?;
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| format!("InvalidPayload: {e}"))?;

    let mut req = http_client().request(method, &p.url);
    if let Some(headers) = &p.headers {
        for (k, v) in headers {
            if STRIPPED_HEADERS.contains(&k.to_ascii_lowercase().as_str()) {
                continue;
            }
            req = req.header(k, v);
        }
    }
    if let Some(body) = &p.body {
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(format!("InvalidPayload: body exceeds {MAX_RESPONSE_BYTES} bytes"));
        }
        req = req.body(body.clone());
    }

    let mut resp = req.send().await.map_err(|e| format!("HttpError: {e}"))?;
    let status = resp.status().as_u16();
    let mut headers_map = serde_json::Map::new();
    for (name, value) in resp.headers() {
        let val = value.to_str().unwrap_or_default().to_string();
        match headers_map.get_mut(name.as_str()) {
            Some(Value::String(prev)) => {
                prev.push_str(", ");
                prev.push_str(&val);
            }
            _ => {
                headers_map.insert(name.as_str().to_string(), Value::String(val));
            }
        }
    }

    let mut body_bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("HttpError: {e}"))? {
        if body_bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(format!("HttpError: response exceeds {MAX_RESPONSE_BYTES} bytes"));
        }
        body_bytes.extend_from_slice(&chunk);
    }

    Ok(json!({
        "status": status,
        "headers": Value::Object(headers_map),
        "body": String::from_utf8_lossy(&body_bytes),
    }))
}

// 剪贴板读
// Clipboard read
pub async fn clipboard_read() -> Result<Value, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("ClipboardError: {e}"))?;
    let text = cb.get_text().map_err(|e| format!("ClipboardError: {e}"))?;
    Ok(json!({ "text": text }))
}

// 剪贴板写
// Clipboard write
pub async fn clipboard_write(payload: Value) -> Result<Value, String> {
    let p: ClipboardWritePayload = parse_payload(payload)?;
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("ClipboardError: {e}"))?;
    cb.set_text(&p.text).map_err(|e| format!("ClipboardError: {e}"))?;
    Ok(Value::Null)
}

// storage.get
// storage.get
pub async fn storage_get(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
    let p: StorageGetPayload = parse_payload(payload)?;
    validate_key(&p.key)?;
    let row = sqlx::query("SELECT value FROM plugin_data WHERE plugin_id = ? AND key = ?")
        .bind(plugin_id)
        .bind(&p.key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    let value: Option<Value> = row
        .map(|r| -> Result<Value, String> {
            let s: String = r.try_get("value").map_err(|e| format!("StorageError: {e}"))?;
            Ok(serde_json::from_str(&s).unwrap_or(Value::String(s)))
        })
        .transpose()?;
    Ok(json!({ "value": value }))
}

// storage.set
// storage.set
pub async fn storage_set(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
    let p: StorageSetPayload = parse_payload(payload)?;
    validate_key(&p.key)?;
    let value_json = serde_json::to_string(&p.value).map_err(|e| format!("StorageError: {e}"))?;
    if value_json.len() > MAX_VALUE_BYTES {
        return Err(format!("InvalidPayload: value exceeds {MAX_VALUE_BYTES} bytes"));
    }
    sqlx::query(
        "INSERT INTO plugin_data (plugin_id, key, value, updated_at)
         VALUES (?, ?, ?, datetime('now'))
         ON CONFLICT(plugin_id, key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
    )
    .bind(plugin_id)
    .bind(&p.key)
    .bind(&value_json)
    .execute(pool)
    .await
    .map_err(|e| format!("StorageError: {e}"))?;
    Ok(Value::Null)
}

// storage.remove
// storage.remove
pub async fn storage_remove(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
    let p: StorageRemovePayload = parse_payload(payload)?;
    validate_key(&p.key)?;
    sqlx::query("DELETE FROM plugin_data WHERE plugin_id = ? AND key = ?")
        .bind(plugin_id)
        .bind(&p.key)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(Value::Null)
}

// events.emit
// events.emit
pub async fn events_emit(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
    let p: EventsEmitPayload = parse_payload(payload)?;
    validate_event_type(&p.event_type)?;
    let data = p
        .data
        .map(|d| serde_json::to_string(&d).map_err(|e| format!("EventError: {e}")))
        .transpose()?;
    sqlx::query("INSERT INTO plugin_events (event_type, source_plugin_id, data) VALUES (?, ?, ?)")
        .bind(&p.event_type)
        .bind(plugin_id)
        .bind(&data)
        .execute(pool)
        .await
        .map_err(|e| format!("EventError: {e}"))?;

    let data_value = data
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or(Value::Null);
    // 落库（审计）之后立即广播：窗口扇出 + 触发器订阅，均不依赖表轮询
    // Right after the audit insert: window fan-out + trigger subscription, no table polling
    event_publish(&p.event_type, plugin_id, &data_value);
    Ok(Value::Null)
}

// log.* handler
// log.* handler
pub async fn plugin_log(
    pool: &sqlx::SqlitePool,
    plugin_id: &str,
    level: &str,
    payload: Value,
) -> Result<Value, String> {
    let p: LogPayload = parse_payload(payload)?;
    validate_message(&p.message)?;
    write_system_log(pool, level, &p.message, &format!("plugin:{plugin_id}"), p.data).await?;
    Ok(Value::Null)
}

// 窗口控制：仅作用于插件自己的独立窗口
// Window control: targets only the plugin's own independent window
fn ensure_own_window(window: &WebviewWindow, plugin_id: &str) -> Result<(), String> {
    if window.label() == plugin_window_label(plugin_id) {
        Ok(())
    } else {
        Err("WindowNotAllowed: window control requires the plugin's independent window".into())
    }
}

// 展示环境
// Display context
pub fn display_mode(caller_label: &str, plugin_id: &str) -> &'static str {
    if caller_label == plugin_window_label(plugin_id) {
        "independent"
    } else {
        "embedded"
    }
}

// 通道分发
// Channel dispatch
pub async fn dispatch_channel(
    pool: &sqlx::SqlitePool,
    guard: &PermissionGuard,
    plugin_id: &str,
    channel: &str,
    payload: Value,
) -> Result<Value, String> {
    if let Some(perm) = channel_permission(channel) {
        guard.require(perm)?;
    }

    match channel {
        "kapi:storage.get" => storage_get(pool, plugin_id, payload).await,
        "kapi:storage.set" => storage_set(pool, plugin_id, payload).await,
        "kapi:storage.remove" => storage_remove(pool, plugin_id, payload).await,
        "kapi:clipboard.read" => clipboard_read().await,
        "kapi:clipboard.write" => clipboard_write(payload).await,
        "kapi:http.fetch" => http_fetch(guard, payload).await,
        "kapi:events.emit" => events_emit(pool, plugin_id, payload).await,
        "kapi:log.debug" | "kapi:log.info" | "kapi:log.warn" | "kapi:log.error" => {
            let level = channel.trim_start_matches("kapi:log.");
            plugin_log(pool, plugin_id, level, payload).await
        }
        "kapi:window.setTitle" | "kapi:window.close" | "kapi:window.minimize"
        | "kapi:window.startDragging" => Err("WindowNotAllowed: window control is UI-only".into()),
        "kapi:plugin.invoke" => Err("WasmError: kapi:plugin.invoke is not callable from WASM".into()),
        "kapi:events.on" | "kapi:events.off" => {
            Err("EventError: event subscription is UI-only (no push target from WASM)".into())
        }
        other => Err(format!("UnknownChannel: {other}")),
    }
}

// 桥接统一入口
// Bridge entry point
#[tauri::command]
pub async fn plugin_bridge(
    app: AppHandle,
    window: WebviewWindow,
    plugin_id: String,
    channel: String,
    payload: Value,
) -> Result<Value, String> {
    if !is_valid_plugin_id(&plugin_id) {
        return Err(format!("PluginNotFound: {plugin_id}"));
    }
    let pool = sqlite_pool(&app).await?;
    let ctx = load_bridge_context(&pool, &plugin_id).await?;

    match channel.as_str() {
        "kapi:window.getInfo" => Ok(json!({ "mode": display_mode(window.label(), &plugin_id) })),
        "kapi:window.setTitle" => {
            ensure_own_window(&window, &plugin_id)?;
            let p: WindowSetTitlePayload = parse_payload(payload)?;
            validate_title(&p.title)?;
            window.set_title(&p.title).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "kapi:window.close" => {
            if window.label() == plugin_window_label(&plugin_id) {
                window.close().map_err(|e| e.to_string())?;
            } else {
                window
                    .emit_to(window.label(), "plugin:close", ())
                    .map_err(|e| e.to_string())?;
            }
            Ok(Value::Null)
        }
        "kapi:window.minimize" => {
            ensure_own_window(&window, &plugin_id)?;
            window.minimize().map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "kapi:window.startDragging" => {
            ensure_own_window(&window, &plugin_id)?;
            window.start_dragging().map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "kapi:events.on" => {
            let p: EventsOnPayload = parse_payload(payload)?;
            let event_type = p.event_type.ok_or_else(|| {
                "InvalidPayload: event subscription requires a type".to_string()
            })?;
            validate_event_type(&event_type)?;
            ctx.guard.require("events:subscribe")?;
            event_subscribe(window.label(), &plugin_id, &event_type);
            Ok(Value::Null)
        }
        "kapi:events.off" => {
            let p: EventsOnPayload = parse_payload(payload)?;
            if let Some(event_type) = &p.event_type {
                validate_event_type(event_type)?;
            }
            ctx.guard.require("events:subscribe")?;
            event_unsubscribe(window.label(), &plugin_id, p.event_type.as_deref());
            Ok(Value::Null)
        }
        "kapi:plugin.invoke" => {
            let p: PluginInvokePayload = parse_payload(payload)?;
            validate_action(&p.action)?;
            let runtime = app.state::<crate::wasm::engine::WasmRuntime>();
            runtime
                .invoke_action(&pool, &plugin_id, &p.action, &p.payload.unwrap_or(Value::Null))
                .await
        }
        _ => dispatch_channel(&pool, &ctx.guard, &plugin_id, &channel, payload).await,
    }
}
