// 事件总线
// Event bus
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

// 进程内广播通道容量：超出后慢订阅者丢最旧消息（Lagged），触发器宁可丢也不阻塞发射方
// In-process broadcast capacity: a slow subscriber loses the oldest messages (Lagged)
// rather than ever blocking the emitter
const EVENT_CHANNEL_CAPACITY: usize = 1024;

// 广播给进程内订阅者（触发器）的消息
// Message broadcast to in-process subscribers (triggers)
#[derive(Clone)]
pub struct EventMessage {
    pub event_type: String,
    pub source: String,
    pub data: Value,
}

// 订阅表 + 宿主句柄 + 进程内广播通道
// Subscription table + host handle + in-process broadcast channel
pub struct EventBus {
    // (宿主窗口 label, 插件 id) → 订阅的事件类型集合
    // (host window label, plugin id) -> subscribed event types
    pub subs: std::sync::Mutex<HashMap<(String, String), HashSet<String>>>,
    pub app: OnceLock<AppHandle>,
    pub event_tx: tokio::sync::broadcast::Sender<EventMessage>,
}

pub static EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

pub fn event_bus() -> &'static EventBus {
    EVENT_BUS.get_or_init(|| EventBus {
        subs: std::sync::Mutex::new(HashMap::new()),
        app: OnceLock::new(),
        event_tx: tokio::sync::broadcast::channel(EVENT_CHANNEL_CAPACITY).0,
    })
}

// setup 注入宿主句柄
// Inject the host handle at setup
pub fn init_event_bus(app: AppHandle) {
    let _ = event_bus().app.set(app);
}

// 订阅
// Subscribe
pub fn event_subscribe(label: &str, plugin_id: &str, event_type: &str) {
    event_bus()
        .subs
        .lock()
        .expect("event bus poisoned")
        .entry((label.to_string(), plugin_id.to_string()))
        .or_default()
        .insert(event_type.to_string());
}

// 退订
// Unsubscribe
pub fn event_unsubscribe(label: &str, plugin_id: &str, event_type: Option<&str>) {
    let mut subs = event_bus().subs.lock().expect("event bus poisoned");
    match event_type {
        Some(t) => {
            if let Some(types) = subs.get_mut(&(label.to_string(), plugin_id.to_string())) {
                types.remove(t);
                if types.is_empty() {
                    subs.remove(&(label.to_string(), plugin_id.to_string()));
                }
            }
        }
        None => {
            subs.remove(&(label.to_string(), plugin_id.to_string()));
        }
    }
}

// 窗口销毁清理
// Window-destroy cleanup
pub fn event_purge_window(label: &str) {
    event_bus()
        .subs
        .lock()
        .expect("event bus poisoned")
        .retain(|(l, _), _| l != label);
}

// 扇出
// Fan-out
pub fn event_fanout(event_type: &str, source: &str, data: &Value) {
    let Some(app) = event_bus().app.get() else {
        return;
    };
    let targets: Vec<(String, String)> = {
        let subs = event_bus().subs.lock().expect("event bus poisoned");
        subs.iter()
            .filter(|(_, types)| types.contains(event_type))
            .map(|((label, plugin_id), _)| (label.clone(), plugin_id.clone()))
            .collect()
    };
    for (label, plugin_id) in targets {
        let payload = json!({
            "pluginId": plugin_id,
            "type": event_type,
            "data": data,
            "source": source,
        });
        if let Err(e) = app.emit_to(label.as_str(), "plugin:event", payload) {
            eprintln!("kapi: event push to '{label}' failed: {e}");
        }
    }
}

// 发布：窗口扇出 + 进程内广播（触发器等消费者经 subscribe_events 消费）
// Publish: window fan-out + in-process broadcast (triggers consume via subscribe_events)
// DB 落库仅作审计历史，不再作为队列被轮询
// The DB row stays audit-only history; nothing polls it as a queue anymore
pub fn event_publish(event_type: &str, source: &str, data: &Value) {
    event_fanout(event_type, source, data);
    let _ = event_bus().event_tx.send(EventMessage {
        event_type: event_type.to_string(),
        source: source.to_string(),
        data: data.clone(),
    });
}

// 订阅进程内事件流：返回广播 Receiver（多订阅者各自独立消费）
// Subscribe to the in-process event stream: returns a broadcast Receiver
// (each subscriber consumes independently)
pub fn subscribe_events() -> tokio::sync::broadcast::Receiver<EventMessage> {
    event_bus().event_tx.subscribe()
}
