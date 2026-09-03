// 节点输入拼装 + TriggerType 单元测试（独立 tests/ 目录）
// Node input assembly + TriggerType unit tests (standalone tests/ directory)
// 运行：cargo test -p kapi --test node_input
use std::collections::HashMap;
use tauri_app_lib::workflow::model::{DataBinding, TriggerType};
use tauri_app_lib::workflow::node::assemble_input;

#[test]
fn assemble_input_basic() {
    let mut prior = HashMap::new();
    prior.insert("src".into(), serde_json::json!({ "formatted": "hello", "length": 5 }));
    let bindings = vec![DataBinding {
        from: "src".into(),
        output: "formatted".into(),
        to: "dst".into(),
        input: "content".into(),
    }];
    let cfg = Some(serde_json::json!({ "extra": "config-value" }));
    let input = assemble_input(&bindings, &prior, &serde_json::json!({}), &cfg);
    assert_eq!(input["content"], "hello");
    assert_eq!(input["extra"], "config-value");
}

#[test]
fn assemble_input_trigger_source() {
    let prior = HashMap::new();
    let bindings = vec![DataBinding {
        from: "__trigger__".into(),
        output: "text".into(),
        to: "node".into(),
        input: "content".into(),
    }];
    let input = assemble_input(
        &bindings,
        &prior,
        &serde_json::json!({ "text": "clipboard data" }),
        &None,
    );
    assert_eq!(input["content"], "clipboard data");
}

#[test]
fn trigger_type_roundtrip() {
    for s in ["clipboard", "hotkey", "schedule", "manual", "plugin_event"] {
        let t = TriggerType::from_str(s).unwrap();
        assert_eq!(t.as_str(), s);
    }
    assert!(TriggerType::from_str("bogus").is_none());
}
