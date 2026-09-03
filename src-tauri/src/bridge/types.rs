// 类型定义
// Type definitions
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::Value;

// 存储操作载荷
// Storage operation payloads
#[derive(Deserialize)]
pub struct StorageGetPayload {
    pub key: String,
}

#[derive(Deserialize)]
pub struct StorageSetPayload {
    pub key: String,
    pub value: Value,
}

#[derive(Deserialize)]
pub struct StorageRemovePayload {
    pub key: String,
}

// 事件操作载荷
// Event operation payloads
#[derive(Deserialize)]
pub struct EventsEmitPayload {
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: Option<Value>,
}

// events.on / events.off 载荷
// events.on / events.off payload
#[derive(Deserialize)]
pub struct EventsOnPayload {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
}

// 日志载荷
// Log payload
#[derive(Deserialize)]
pub struct LogPayload {
    pub message: String,
    pub data: Option<Value>,
}

// 窗口设置标题载荷
// Window set title payload
#[derive(Deserialize)]
pub struct WindowSetTitlePayload {
    pub title: String,
}

// 插件调用载荷
// Plugin invoke payload
#[derive(Deserialize)]
pub struct PluginInvokePayload {
    pub action: String,
    pub payload: Option<Value>,
}

// HTTP 请求载荷
// HTTP request payload
#[derive(Deserialize)]
pub struct HttpFetchPayload {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
}

// 剪贴板写入载荷
// Clipboard write payload
#[derive(Deserialize)]
pub struct ClipboardWritePayload {
    pub text: String,
}
