// 桥接模块测试
// Bridge module tests
use serde_json::json;
use tauri_app_lib::bridge::dispatch::{
    channel_permission, display_mode, PermissionGuard,
};
use tauri_app_lib::bridge::event_bus::{
    event_bus, event_purge_window, event_subscribe, event_unsubscribe,
};
use tauri_app_lib::bridge::types::StorageGetPayload;
use tauri_app_lib::bridge::validate::{
    parse_payload, validate_event_type, validate_key, validate_message, validate_title,
};

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

// ---- 事件推送总线 / event push bus ----
// 全局单例：本组测试独占使用（cargo 并行下其它测试不触碰 EVENT_BUS）
// Global singleton: owned by this group (no other test touches EVENT_BUS in parallel)

#[test]
fn event_bus_tracks_subscriptions_and_purges() {
    event_subscribe("main", "com.a", "tick");
    event_subscribe("main", "com.a", "tock");
    event_subscribe("plugin-com_b", "com.b", "tick");

    // 快照断言辅助：读 (label, plugin) 当前订阅集合
    // Snapshot helper: the current subscription set of a (label, plugin)
    let types_of = |label: &str, plugin: &str| -> Option<Vec<String>> {
        let subs = event_bus().subs.lock().unwrap();
        subs.get(&(label.to_string(), plugin.to_string()))
            .map(|s| s.iter().cloned().collect())
    };
    let mut a = types_of("main", "com.a").unwrap();
    a.sort();
    assert_eq!(a, vec!["tick".to_string(), "tock".to_string()]);

    // 退订单个类型；集合清空后条目移除
    // Unsubscribe one type; an emptied set removes the entry
    event_unsubscribe("main", "com.a", Some("tick"));
    assert_eq!(types_of("main", "com.a"), Some(vec!["tock".to_string()]));
    event_unsubscribe("main", "com.a", Some("tock"));
    assert_eq!(types_of("main", "com.a"), None);

    // 窗口销毁清理：整 label 移除（未登记的 label 无害）
    // Window purge: the whole label goes; unknown labels are harmless
    event_purge_window("plugin-com_b");
    assert_eq!(types_of("plugin-com_b", "com.b"), None);
    event_purge_window("no-such-window");

    // 退订全部（type 缺省）/ unsubscribe all (missing type)
    event_subscribe("main", "com.a", "tick");
    event_unsubscribe("main", "com.a", None);
    assert_eq!(types_of("main", "com.a"), None);
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

// ---- display_mode：纯函数 / pure function ----

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
    let err = parse_payload::<StorageGetPayload>(serde_json::Value::Null).err().unwrap();
    assert!(err.starts_with("InvalidPayload:"));
    let err = parse_payload::<StorageGetPayload>(json!({ "nope": 1 })).err().unwrap();
    assert!(err.starts_with("InvalidPayload:"));
}
