// D1 完结：Rust 端镜像 TS 端 contract fixture
// D1: Rust side mirrors the TS contract fixture
// 用 include_str! 引入同一份 JSON，避免双端漂移
// Uses include_str! to load the same JSON, preventing drift between sides
use tauri_app_lib::workflow::model::{Workflow, WorkflowTrigger};

const WORKFLOW_FIXTURE: &str = include_str!("../../tests/contract/workflow.json");
const TRIGGER_FIXTURE: &str = include_str!("../../tests/contract/trigger.json");

#[test]
fn workflow_fixture_deserializes() {
    let wf: Workflow = serde_json::from_str(WORKFLOW_FIXTURE)
        .expect("contract/workflow.json must be a valid Workflow");
    assert_eq!(wf.id, "wf-contract-1");
    assert_eq!(wf.schema_version, 1);
    assert!(wf.is_enabled);
    assert_eq!(wf.graph.nodes.len(), 2);
    assert_eq!(wf.graph.edges.len(), 1);
    // 节点字段：type / plugin_id / action / position / display_name
    let n1 = &wf.graph.nodes[0];
    assert_eq!(n1.id, "n-1");
    assert_eq!(n1.node_type, "plugin");
    assert_eq!(n1.plugin_id.as_deref(), Some("com.example.plugin"));
    assert_eq!(n1.action.as_deref(), Some("save"));
    assert_eq!(n1.display_name.as_deref(), Some("Save"));
    // 边字段：map
    let e = &wf.graph.edges[0];
    assert_eq!(e.from, "n-1");
    assert_eq!(e.to, "n-2");
    assert_eq!(e.map.get("text").map(String::as_str), Some("in"));
}

#[test]
fn trigger_fixture_deserializes() {
    let tr: WorkflowTrigger = serde_json::from_str(TRIGGER_FIXTURE)
        .expect("contract/trigger.json must be a valid WorkflowTrigger");
    assert_eq!(tr.id, "tr-contract-1");
    assert_eq!(tr.trigger_type, "schedule");
    assert_eq!(tr.is_enabled, true);
    // P3-1: schedule 用 cron 字符串
    // P3-1: schedule uses cron string
    let cron = tr.config.get("cron").and_then(|v| v.as_str());
    assert_eq!(cron, Some("0 9 * * *"));
}
