// 节点输入拼装 + TriggerType 单元测试（独立 tests/ 目录）
// Node input assembly + TriggerType unit tests (standalone tests/ directory)
// 运行：cargo test -p kapi --test node_input
use std::collections::HashMap;
use tauri_app_lib::workflow::model::TriggerType;
use tauri_app_lib::workflow::node::assemble_input;

#[test]
fn assemble_input_basic() {
    let mut prior = HashMap::new();
    prior.insert("src".into(), serde_json::json!({ "formatted": "hello", "length": 5 }));
    // 边映射形如 { "<source_node_id>:<field>": "<downstream_input>" }
    // Edge map shape: { "<source_node_id>:<field>": "<downstream_input>" }
    let edge_maps = vec![HashMap::from([("src:formatted".to_string(), "content".to_string())])];
    let cfg = Some(serde_json::json!({ "extra": "config-value" }));
    let input = assemble_input(&edge_maps, &prior, &serde_json::json!({}), &cfg);
    assert_eq!(input["content"], "hello");
    assert_eq!(input["extra"], "config-value");
}

#[test]
fn assemble_input_trigger_source() {
    let prior = HashMap::new();
    // __trigger__ 伪源：从触发器数据取值
    // __trigger__ pseudo-source: reads from trigger data
    let edge_maps = vec![HashMap::from([("__trigger__".to_string(), "text".to_string())])];
    let input = assemble_input(
        &edge_maps,
        &prior,
        &serde_json::json!({ "text": "clipboard data" }),
        &None,
    );
    assert_eq!(input["text"], "clipboard data");
}

#[test]
fn trigger_type_roundtrip() {
    for s in ["clipboard", "hotkey", "schedule", "manual", "plugin_event"] {
        let t = TriggerType::from_str(s).unwrap();
        assert_eq!(t.as_str(), s);
    }
    assert!(TriggerType::from_str("bogus").is_none());
}
