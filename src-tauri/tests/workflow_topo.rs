// 拓扑排序单元测试（独立 tests/ 目录）
// Topological sort unit tests (standalone tests/ directory)
// 运行：cargo test -p kapi --test topo
use tauri_app_lib::workflow::topo::topological_waves;
use tauri_app_lib::workflow::model::{WorkflowEdge, WorkflowGraph, WorkflowNode};

fn make_graph(nodes: Vec<(&str, &str)>, edges: Vec<(&str, &str)>) -> WorkflowGraph {
    WorkflowGraph {
        nodes: nodes
            .into_iter()
            .map(|(id, ty)| WorkflowNode {
                id: id.into(),
                node_type: ty.into(),
                plugin_id: None,
                action: None,
                config: None,
                position: tauri_app_lib::workflow::model::Position { x: 0.0, y: 0.0 },
                display_name: None,
            })
            .collect(),
        edges: edges
            .into_iter()
            .map(|(f, t)| WorkflowEdge { from: f.into(), to: t.into(), map: Default::default() })
            .collect(),
    }
}

#[test]
fn topo_single_node() {
    let g = make_graph(vec![("a", "plugin")], vec![]);
    let waves = topological_waves(&g).unwrap();
    assert_eq!(waves.len(), 1);
    assert_eq!(waves[0], vec!["a"]);
}

#[test]
fn topo_chain() {
    let g = make_graph(
        vec![("a", "plugin"), ("b", "plugin"), ("c", "plugin")],
        vec![("a", "b"), ("b", "c")],
    );
    let waves = topological_waves(&g).unwrap();
    assert_eq!(waves.len(), 3);
    assert_eq!(waves[0], vec!["a"]);
    assert_eq!(waves[1], vec!["b"]);
    assert_eq!(waves[2], vec!["c"]);
}

#[test]
fn topo_diamond_waves() {
    let g = make_graph(
        vec![("a", "plugin"), ("b", "plugin"), ("c", "plugin"), ("d", "plugin")],
        vec![("a", "d"), ("b", "d"), ("c", "d")],
    );
    let waves = topological_waves(&g).unwrap();
    assert_eq!(waves.len(), 2);
    assert_eq!(waves[0].len(), 3);
    assert!(waves[0].contains(&"a".to_string()));
    assert!(waves[0].contains(&"b".to_string()));
    assert!(waves[0].contains(&"c".to_string()));
    assert_eq!(waves[1], vec!["d"]);
}

#[test]
fn topo_cycle_detected() {
    let g = make_graph(
        vec![("a", "plugin"), ("b", "plugin")],
        vec![("a", "b"), ("b", "a")],
    );
    let err = topological_waves(&g).unwrap_err();
    assert!(err.starts_with("CycleDetected"), "got {err}");
}

#[test]
fn topo_empty() {
    let g = make_graph(vec![], vec![]);
    let waves = topological_waves(&g).unwrap();
    assert!(waves.is_empty());
}
