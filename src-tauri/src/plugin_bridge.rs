// 插件桥接：postMessage → plugin_bridge 命令的统一入口（docs/PLUGINS.md §4、PANEL.md §3）
// Plugin bridge: the unified postMessage → plugin_bridge command pipeline
use std::collections::{HashMap, HashSet};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::plugin_manager::{plugin_window_label, sqlite_pool};
use crate::plugin_protocol::is_valid_plugin_id;

// 桥接通道载荷边界（防滥用：超长 key/消息、超大存储值）
// Bridge payload limits (guard against oversized keys/messages and storage values)
const MAX_KEY_LEN: usize = 256;
const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_TITLE_LEN: usize = 256;
const MAX_MESSAGE_LEN: usize = 2000;
const MAX_EVENT_TYPE_LEN: usize = 128;

// 权限守卫：manifest.permissions 的内存快照，默认全部拒绝（docs/PLUGINS.md §3）
// Permission guard: in-memory snapshot of manifest.permissions, deny-by-default
#[derive(Debug, Default, Clone)]
pub struct PermissionGuard {
    permissions: HashSet<String>,
}

impl PermissionGuard {
    // 从 plugins.manifest JSON 解析 permissions 数组（缺失 → 空集 = 全拒）
    // Parse the permissions array from plugins.manifest JSON (missing → empty set = deny all)
    pub fn from_manifest_json(manifest_json: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(manifest_json)
            .map_err(|e| format!("PluginNotFound: invalid manifest ({e})"))?;
        let mut permissions = HashSet::new();
        // 仅收字符串项；非字符串项忽略（宽松解析，避免畸形 manifest 拖垮桥接）
        // Collect string items only; non-string entries are ignored
        if let Some(arr) = v.get("permissions").and_then(Value::as_array) {
            for p in arr {
                if let Some(s) = p.as_str() {
                    permissions.insert(s.to_string());
                }
            }
        }
        Ok(Self { permissions })
    }

    // 单项权限检查：未声明即拒绝
    // Single permission check: undeclared means denied
    pub fn require(&self, perm: &str) -> Result<(), String> {
        if self.permissions.contains(perm) {
            Ok(())
        } else {
            Err(format!("PermissionDenied: {perm}"))
        }
    }

    // 域名白名单：network:host:<domain> 精确匹配或 network:host:* 通配；子域不隐式放行
    // Host whitelist: exact network:host:<domain> or the network:host:* wildcard; subdomains never implied
    pub fn allows_host(&self, host: &str) -> bool {
        self.permissions.contains(&format!("network:host:{host}"))
            || self.permissions.contains("network:host:*")
    }
}

// 通道 → 所需权限映射（None = 无需声明或需特判；未知通道由 dispatch 报 UnknownChannel）
// Channel → required permission (None = none needed or special-cased; unknown channels fail in dispatch)
pub(crate) fn channel_permission(channel: &str) -> Option<&'static str> {
    match channel {
        "kapi:storage.get" => Some("storage:read"),
        "kapi:storage.set" | "kapi:storage.remove" => Some("storage:write"),
        "kapi:clipboard.read" => Some("clipboard:read"),
        "kapi:clipboard.write" => Some("clipboard:write"),
        "kapi:events.emit" => Some("events:emit"),
        // window/log 只触碰调用方自身资源，无需权限声明
        // window/log touch only the caller's own resources: no permission needed
        _ => None,
    }
}

// ---- payload 结构与校验 / payload shapes and validation ----

#[derive(Deserialize)]
struct StorageGetPayload {
    key: String,
}

#[derive(Deserialize)]
struct StorageSetPayload {
    key: String,
    value: Value,
}

#[derive(Deserialize)]
struct StorageRemovePayload {
    key: String,
}

#[derive(Deserialize)]
struct EventsEmitPayload {
    #[serde(rename = "type")]
    event_type: String,
    data: Option<Value>,
}

#[derive(Deserialize)]
struct LogPayload {
    message: String,
    data: Option<Value>,
}

#[derive(Deserialize)]
struct WindowSetTitlePayload {
    title: String,
}

#[derive(Deserialize)]
struct PluginInvokePayload {
    action: String,
    payload: Option<Value>,
}

// payload 反序列化：统一报 InvalidPayload
// Payload deserialization with a uniform InvalidPayload error
fn parse_payload<T: DeserializeOwned>(payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|e| format!("InvalidPayload: {e}"))
}

// 存储键：非空且不超长
// Storage key: non-empty and within the length cap
fn validate_key(key: &str) -> Result<(), String> {
    let n = key.chars().count();
    if n == 0 || n > MAX_KEY_LEN {
        Err(format!("InvalidPayload: key must be 1..={MAX_KEY_LEN} chars"))
    } else {
        Ok(())
    }
}

// 事件类型：非空、不超长、仅 [A-Za-z0-9._-]（与插件 id 同字符集）
// Event type: non-empty, within the cap, [A-Za-z0-9._-] only (same charset as plugin ids)
fn validate_event_type(event_type: &str) -> Result<(), String> {
    let n = event_type.chars().count();
    let valid = n > 0
        && n <= MAX_EVENT_TYPE_LEN
        && event_type
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "InvalidPayload: event type must be 1..={MAX_EVENT_TYPE_LEN} chars of [A-Za-z0-9._-]"
        ))
    }
}

// 日志消息：非空且不超长
// Log message: non-empty and within the length cap
fn validate_message(message: &str) -> Result<(), String> {
    let n = message.chars().count();
    if n == 0 || n > MAX_MESSAGE_LEN {
        Err(format!("InvalidPayload: message must be 1..={MAX_MESSAGE_LEN} chars"))
    } else {
        Ok(())
    }
}

// 窗口标题：非空且不超长
// Window title: non-empty and within the length cap
fn validate_title(title: &str) -> Result<(), String> {
    let n = title.chars().count();
    if n == 0 || n > MAX_TITLE_LEN {
        Err(format!("InvalidPayload: title must be 1..={MAX_TITLE_LEN} chars"))
    } else {
        Ok(())
    }
}

// ---- 桥接上下文 / bridge context ----

// 桥接上下文：本次调用的权限快照（manifest 按次加载，禁用/卸载即时生效）
// Bridge context: the per-call permission snapshot (manifest loaded per call)
struct BridgeContext {
    guard: PermissionGuard,
}

// 加载桥接上下文：未安装/禁用/manifest 损坏的插件拿不到任何通道
// Load the bridge context: uninstalled/disabled/broken plugins get no channels at all
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

// ---- 各通道 handler / channel handlers ----

// ---- HTTP 代理（kapi:http.fetch）/ HTTP proxy ----

// 超时与响应体上限：防慢速/超大响应拖垮桥接
// Timeout and response cap: slow/huge responses must not stall the bridge
const HTTP_TIMEOUT_SECS: u64 = 10;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

// 方法白名单 / the method whitelist
const HTTP_METHODS: [&str; 6] = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"];
// 逐出的请求头：由客户端按实际请求重算，不接受插件伪造
// Stripped request headers: recomputed by the client; plugins cannot spoof them
const STRIPPED_HEADERS: [&str; 2] = ["host", "content-length"];

#[derive(Deserialize)]
struct HttpFetchPayload {
    url: String,
    method: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
}

#[derive(Deserialize)]
struct ClipboardWritePayload {
    text: String,
}

// 从 URL 提取小写主机名（去端口）；仅允许 http/https
// Extract the lowercase host (port stripped) from a URL; http/https only
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

// 方法归一化：缺省 GET；大写化；仅白名单方法
// Method normalization: default GET; uppercased; whitelist only
fn normalize_method(method: Option<&str>) -> Result<String, String> {
    let m = method.unwrap_or("GET").to_ascii_uppercase();
    if HTTP_METHODS.contains(&m.as_str()) {
        Ok(m)
    } else {
        Err(format!("InvalidPayload: unsupported method {m}"))
    }
}

// 共享 HTTP 客户端：10s 超时；禁跟随重定向（防 3xx 绕过域名白名单，3xx 原样返回）
// Shared HTTP client: 10s timeout; redirects disabled (a 3xx must not bypass the host whitelist)
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

// http.fetch：域名白名单预检 → 宿主代理请求 → 流式累计响应体
// http.fetch: host whitelist first, then the proxied request with a streamed body
async fn http_fetch(guard: &PermissionGuard, payload: Value) -> Result<Value, String> {
    let p: HttpFetchPayload = parse_payload(payload)?;
    let host = extract_host(&p.url)?;
    // 域名检查先于发请求 / the host check runs before any request is sent
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
    // 同名响应头合并为逗号拼接 / repeated response headers join with commas
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

    // 流式累计响应体，超过 1MiB 立即中断 / stream the body; abort past 1 MiB
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

// ---- 剪贴板（kapi:clipboard.*）/ clipboard ----

// 每次调用新建 Clipboard：arboard 句柄非 Sync，不作静态跨 await 持有
// Create the Clipboard per call: the arboard handle is not Sync; never a static across awaits
async fn clipboard_read() -> Result<Value, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("ClipboardError: {e}"))?;
    let text = cb.get_text().map_err(|e| format!("ClipboardError: {e}"))?;
    Ok(json!({ "text": text }))
}

async fn clipboard_write(payload: Value) -> Result<Value, String> {
    let p: ClipboardWritePayload = parse_payload(payload)?;
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("ClipboardError: {e}"))?;
    cb.set_text(&p.text).map_err(|e| format!("ClipboardError: {e}"))?;
    Ok(Value::Null)
}


// storage.get：value 列为 JSON 文本；读到非法 JSON 回退原字符串（兼容手写数据）
// storage.get: the value column holds JSON text; fall back to the raw string on invalid JSON
async fn storage_get(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
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

// storage.set：值以 JSON 文本入库，超限拒绝
// storage.set: store the value as JSON text; reject oversized values
async fn storage_set(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
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

// storage.remove：键不存在视为成功（幂等）
// storage.remove: a missing key counts as success (idempotent)
async fn storage_remove(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
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

// events.emit：写入事件总线历史（工作流触发器与审计的数据源）
// events.emit: append to the event-bus history (source for workflow triggers and auditing)
async fn events_emit(pool: &sqlx::SqlitePool, plugin_id: &str, payload: Value) -> Result<Value, String> {
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
    Ok(Value::Null)
}

// 写 system_logs 的公共入口（plugin_log / headless 启动 / WASM stderr 摘录共用）
// Shared system_logs writer (used by plugin_log, headless launch and WASM stderr excerpts)
pub(crate) async fn write_system_log(
    pool: &sqlx::SqlitePool,
    level: &str,
    message: &str,
    source: &str,
    data: Option<Value>,
) -> Result<(), String> {
    let data = data
        .map(|d| serde_json::to_string(&d).map_err(|e| format!("StorageError: {e}")))
        .transpose()?;
    sqlx::query("INSERT INTO system_logs (level, message, source, data) VALUES (?, ?, ?, ?)")
        .bind(level)
        .bind(message)
        .bind(source)
        .bind(&data)
        .execute(pool)
        .await
        .map_err(|e| format!("StorageError: {e}"))?;
    Ok(())
}

// log.*：写入 system_logs，source 固定为 plugin:<id>（日志页可过滤）
// log.*: append to system_logs with source = plugin:<id> (filterable in the logs page)
async fn plugin_log(
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

// 窗口控制仅作用于插件自己的独立窗口：调用方窗口 label 必须精确匹配
// Window control targets only the plugin's own independent window: the caller's label must match
fn ensure_own_window(window: &WebviewWindow, plugin_id: &str) -> Result<(), String> {
    if window.label() == plugin_window_label(plugin_id) {
        Ok(())
    } else {
        Err("WindowNotAllowed: window control requires the plugin's independent window".into())
    }
}

// 动作名：与事件类型同字符集 / action names share the event-type charset
fn validate_action(action: &str) -> Result<(), String> {
    let n = action.chars().count();
    let valid = n > 0
        && n <= MAX_EVENT_TYPE_LEN
        && action
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "InvalidPayload: action must be 1..={MAX_EVENT_TYPE_LEN} chars of [A-Za-z0-9._-]"
        ))
    }
}

// 展示环境：调用方窗口 label 精确匹配插件独立窗口 → independent，否则 embedded
// Display context: an exact label match means the plugin's own window; anything else is embedded
fn display_mode(caller_label: &str, plugin_id: &str) -> &'static str {
    if caller_label == plugin_window_label(plugin_id) {
        "independent"
    } else {
        "embedded"
    }
}

// ---- 命令与分发 / command and dispatch ----

// 通道分发：UI 桥（plugin_bridge 命令）与 WASM 宿主导入（kapi_host_call）共用的唯一权限闸与路由
// Channel dispatch: the single permission gate and routing shared by the UI bridge
// and the WASM host import (kapi_host_call)
// window.* 与 plugin.invoke 只允许 UI 路径（命令内先行处理），WASM 侧在此拒绝
// window.* and plugin.invoke are UI-only (handled earlier in the command); denied here for WASM
pub(crate) async fn dispatch_channel(
    pool: &sqlx::SqlitePool,
    guard: &PermissionGuard,
    plugin_id: &str,
    channel: &str,
    payload: Value,
) -> Result<Value, String> {
    // 权限闸：先查映射表再过守卫（kapi:http.fetch 的域名白名单在 handler 内特判）
    // Permission gate: map the channel, then consult the guard (http.fetch host check is in-handler)
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
            // level 由通道名决定，无需（也不接受）payload 传入
            // The channel name decides the level; payloads cannot override it
            let level = channel.trim_start_matches("kapi:log.");
            plugin_log(pool, plugin_id, level, payload).await
        }
        // 窗口控制属 UI 专属（需要 WebviewWindow 与自有窗口校验）
        // Window control is UI-only (needs the WebviewWindow and own-window check)
        "kapi:window.setTitle" | "kapi:window.close" | "kapi:window.minimize"
        | "kapi:window.startDragging" => Err("WindowNotAllowed: window control is UI-only".into()),
        // 禁止 WASM 内嵌套调用自己的 wasm 入口
        // Nested self-invocation from WASM is forbidden
        "kapi:plugin.invoke" => Err("WasmError: kapi:plugin.invoke is not callable from WASM".into()),
        // 事件订阅需要 SDK 侧的推送协议，与 @kapi/plugin-sdk 同轮落地
        // Subscription needs the SDK-side push protocol; lands with @kapi/plugin-sdk
        "kapi:events.on" => Err("NotImplemented: event subscription lands with the plugin SDK".into()),
        other => Err(format!("UnknownChannel: {other}")),
    }
}

// 桥接统一入口：PluginHost（postMessage）→ invoke → 此处（docs/ARCHITECTURE.md §3.3）
// Bridge entry point: PluginHost (postMessage) → invoke → here
// 权限检查只在 Rust 执行，前端不做任何权限判断
// Permission checks live in Rust only; the frontend performs no permission logic
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
        // 只读环境查询：两种模式均可调用（插件据此隐藏/展示窗口控制按钮）
        // Read-only context query: callable in both modes (plugins toggle window controls with it)
        "kapi:window.getInfo" => Ok(json!({ "mode": display_mode(window.label(), &plugin_id) })),
        // 窗口控制仅 UI 路径可用（需调用方 WebviewWindow）
        // Window control is available only on the UI path (needs the caller's WebviewWindow)
        "kapi:window.setTitle" => {
            ensure_own_window(&window, &plugin_id)?;
            let p: WindowSetTitlePayload = parse_payload(payload)?;
            validate_title(&p.title)?;
            window.set_title(&p.title).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "kapi:window.close" => {
            if window.label() == plugin_window_label(&plugin_id) {
                // 独立窗口：真正关窗 / own window: actually close it
                window.close().map_err(|e| e.to_string())?;
            } else {
                // 内嵌宿主：等效"关闭插件页面"——通知本窗口离开内嵌视图（App.tsx 监听 plugin:close）
                // Embedded host: equivalent to closing the plugin page — tell this window
                // to leave the embed view (App.tsx listens for plugin:close)
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
        // 调用自身 WASM 入口（无需权限声明）：payload {action, payload?}
        // Invoke the plugin's own WASM entry (no permission needed): payload {action, payload?}
        "kapi:plugin.invoke" => {
            let p: PluginInvokePayload = parse_payload(payload)?;
            validate_action(&p.action)?;
            let runtime = app.state::<crate::wasm_runtime::WasmRuntime>();
            runtime
                .invoke_action(&pool, &plugin_id, &p.action, &p.payload.unwrap_or(Value::Null))
                .await
        }
        // 其余通道全部委托共享分发（storage/clipboard/http/events/log）
        // Everything else delegates to the shared dispatch (storage/clipboard/http/events/log)
        _ => dispatch_channel(&pool, &ctx.guard, &plugin_id, &channel, payload).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- PermissionGuard：解析 / parsing ----

    #[test]
    fn guard_parses_permissions_from_manifest() {
        let g = PermissionGuard::from_manifest_json(
            r#"{"id":"com.example.demo","permissions":["storage:read","storage:write","network:host:api.github.com"]}"#,
        )
        .unwrap();
        assert!(g.require("storage:read").is_ok());
        assert!(g.require("storage:write").is_ok());
        // 未声明权限默认拒绝
        // Undeclared permissions are denied by default
        assert!(g.require("clipboard:read").is_err());
    }

    #[test]
    fn guard_defaults_to_deny_without_permissions_field() {
        let g = PermissionGuard::from_manifest_json(r#"{"id":"com.example.demo"}"#).unwrap();
        assert!(g.require("storage:read").is_err());
    }

    #[test]
    fn guard_rejects_invalid_manifest_json() {
        assert!(PermissionGuard::from_manifest_json("{ not json }").is_err());
    }

    #[test]
    fn guard_require_error_carries_stable_prefix() {
        let g = PermissionGuard::default();
        // 错误码可被 SDK 机器解析 / the error code stays machine-parseable
        assert_eq!(
            g.require("storage:read").unwrap_err(),
            "PermissionDenied: storage:read"
        );
    }

    #[test]
    fn guard_allows_exact_and_wildcard_hosts() {
        let g = PermissionGuard::from_manifest_json(
            r#"{"permissions":["network:host:api.github.com","network:host:*"]}"#,
        )
        .unwrap();
        assert!(g.allows_host("api.github.com"));
        assert!(g.allows_host("example.org"));
    }

    #[test]
    fn guard_denies_unlisted_and_subdomain_hosts() {
        let g = PermissionGuard::from_manifest_json(
            r#"{"permissions":["network:host:api.github.com"]}"#,
        )
        .unwrap();
        assert!(!g.allows_host("evil.com"));
        // 子域不隐式放行 / subdomains are never implied
        assert!(!g.allows_host("sub.api.github.com"));
    }

    // ---- channel_permission：映射表 / mapping table ----

    #[test]
    fn channel_permission_maps_every_declared_channel() {
        assert_eq!(channel_permission("kapi:storage.get"), Some("storage:read"));
        assert_eq!(channel_permission("kapi:storage.set"), Some("storage:write"));
        assert_eq!(channel_permission("kapi:storage.remove"), Some("storage:write"));
        assert_eq!(channel_permission("kapi:clipboard.read"), Some("clipboard:read"));
        assert_eq!(channel_permission("kapi:clipboard.write"), Some("clipboard:write"));
        assert_eq!(channel_permission("kapi:events.emit"), Some("events:emit"));
    }

    #[test]
    fn channel_permission_returns_none_for_privileged_free_channels() {
        // window/log 无需权限；http 域名特判；未知通道同样 None（由 dispatch 收口）
        // window/log need none; http is host-special-cased; unknown → None (dispatch decides)
        for ch in [
            "kapi:window.close",
            "kapi:log.info",
            "kapi:http.fetch",
            "kapi:whatever",
        ] {
            assert_eq!(channel_permission(ch), None);
        }
    }

    // ---- payload 校验 / payload validation ----

    #[test]
    fn validate_key_bounds() {
        assert!(validate_key("counter").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key(&"k".repeat(257)).is_err());
        assert!(validate_key(&"k".repeat(256)).is_ok());
    }

    #[test]
    fn validate_event_type_charset_and_length() {
        // [A-Za-z0-9._-] 全部合法 / every char in [A-Za-z0-9._-] is accepted
        assert!(validate_event_type("clipboard_changed.1").is_ok());
        assert!(validate_event_type("clipboard-changed.v2").is_ok());
        assert!(validate_event_type("").is_err());
        assert!(validate_event_type("bad type!").is_err());
        assert!(validate_event_type(&"e".repeat(129)).is_err());
        assert!(validate_event_type(&"e".repeat(128)).is_ok());
    }

    #[test]
    fn validate_message_and_title_bounds() {
        assert!(validate_message("hello").is_ok());
        assert!(validate_message("").is_err());
        assert!(validate_message(&"m".repeat(2001)).is_err());
        assert!(validate_title("Demo").is_ok());
        assert!(validate_title("").is_err());
        assert!(validate_title(&"t".repeat(257)).is_err());
    }

    // ---- extract_host / normalize_method：HTTP 纯函数 / HTTP pure helpers ----

    #[test]
    fn extract_host_normalizes_case_and_strips_port() {
        assert_eq!(extract_host("https://API.GitHub.com/v1").unwrap(), "api.github.com");
        assert_eq!(extract_host("http://example.org:8080/path").unwrap(), "example.org");
    }

    #[test]
    fn extract_host_rejects_non_http_schemes_and_garbage() {
        assert!(extract_host("ftp://example.org").is_err());
        assert!(extract_host("file:///C:/x").is_err());
        assert!(extract_host("not a url").is_err());
        // 无 scheme / no scheme
        assert!(extract_host("example.org/x").is_err());
    }

    #[test]
    fn normalize_method_defaults_uppercases_and_whitelelists() {
        assert_eq!(normalize_method(None).unwrap(), "GET");
        assert_eq!(normalize_method(Some("post")).unwrap(), "POST");
        assert!(normalize_method(Some("TRACE")).is_err());
        assert!(normalize_method(Some("connect")).is_err());
    }

    #[test]
    fn display_mode_matches_own_window_label_only() {
        // 精确匹配自身窗口 label → independent
        // An exact own-label match -> independent
        assert_eq!(
            display_mode("plugin-com_kapi_sample_plugin-c", "com.kapi.sample.plugin-c"),
            "independent"
        );
        // 主面板（embedded 宿主）/ the main panel (the embedded host)
        assert_eq!(display_mode("main", "com.kapi.sample.plugin-c"), "embedded");
        // 其它插件的窗口同样不算 / another plugin's window doesn't count either
        assert_eq!(
            display_mode("plugin-com_kapi_sample_plugin-b", "com.kapi.sample.plugin-c"),
            "embedded"
        );
    }

    #[test]
    fn parse_payload_rejects_null_and_wrong_shape() {
        // 前端缺省 payload 恒传 null：必须报 InvalidPayload 而非 panic
        // The frontend always passes null for a missing payload: InvalidPayload, never a panic
        // （.err() 绕开 T: Debug 约束，仅断言错误串）
        // (.err() sidesteps the T: Debug bound; only the error string is asserted)
        let err = parse_payload::<StorageGetPayload>(Value::Null).err().unwrap();
        assert!(err.starts_with("InvalidPayload:"));
        let err = parse_payload::<StorageGetPayload>(json!({ "nope": 1 })).err().unwrap();
        assert!(err.starts_with("InvalidPayload:"));
    }
}
