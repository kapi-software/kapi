// 事件总线
// Event bus
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

// 订阅表 + 宿主句柄
// Subscription table + host handle
#[derive(Default)]
pub struct EventBus {
    // (宿主窗口 label, 插件 id) → 订阅的事件类型集合
    // (host window label, plugin id) -> subscribed event types
    pub subs: std::sync::Mutex<HashMap<(String, String), HashSet<String>>>,
    pub app: OnceLock<AppHandle>,
}

pub static EVENT_BUS: OnceLock<EventBus> = OnceLock::new();

pub fn event_bus() -> &'static EventBus {
    EVENT_BUS.get_or_init(EventBus::default)
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
