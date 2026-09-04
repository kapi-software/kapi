// 拓扑排序：返回 DAG 执行波次（波次内并发，波次间有序）
// Topological sort: returns DAG execution waves (concurrent within wave, ordered between waves)
// 同时提供图校验入口（环/悬空边/重复 id/孤儿节点）
// Also exposes graph validation entry (cycle / dangling edge / duplicate id / orphan node)
use std::collections::{HashMap, HashSet};

use crate::workflow::model::{GraphError, ValidationReport, WorkflowGraph};

/// 校验 graph 并返回错误列表（fatal + warning）
/// Validate the graph and return all errors (fatal + warning).
/// 执行前先调一次可一次性展示给用户
/// Call before execution to surface everything in one shot.
pub fn validate_graph(graph: &WorkflowGraph) -> ValidationReport {
    let mut report = ValidationReport::new();

    // 1) 节点 id 重复
    // 1) Duplicate node ids
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut duplicates: Vec<String> = Vec::new();
    for n in &graph.nodes {
        *seen.entry(n.id.as_str()).or_insert(0) += 1;
    }
    for (id, count) in &seen {
        if *count > 1 {
            duplicates.push((*id).to_string());
        }
    }
    let has_duplicates = !duplicates.is_empty();
    if has_duplicates {
        let mut d = duplicates.clone();
        d.sort();
        report.push(
            GraphError::fatal(
                "duplicate_node_id",
                format!("节点 id 重复：{}", d.join(", ")),
            )
            .with_nodes(d),
        );
    }

    // 后续检查要求节点 id 唯一——否则边引用解析会出错
    // Subsequent checks assume unique node ids; bail out if duplicated
    let node_ids: HashSet<&str> = if !has_duplicates {
        graph.nodes.iter().map(|n| n.id.as_str()).collect()
    } else {
        // 仍收集 id 以便悬空检测能发现"指向重复 id 的边"，但 children/parent 关系会失真
        // Still collect ids for dangling-edge detection, but skip cycle/orphan analysis
        report.push(GraphError::fatal(
            "structural_invalid",
            "由于节点 id 重复，跳过环/孤儿分析。请先修复重复 id。",
        ));
        graph.nodes.iter().map(|n| n.id.as_str()).collect()
    };

    // 2) 悬空边（边的 source/target 不在节点集合里）
    // 2) Dangling edges (edge endpoint not in node set)
    let mut dangling_indices: Vec<usize> = Vec::new();
    for (i, e) in graph.edges.iter().enumerate() {
        if !node_ids.contains(e.from.as_str()) || !node_ids.contains(e.to.as_str()) {
            dangling_indices.push(i);
        }
    }
    if !dangling_indices.is_empty() {
        let sample: Vec<String> = dangling_indices
            .iter()
            .take(3)
            .map(|i| {
                let e = &graph.edges[*i];
                format!("{}-{}", e.from, e.to)
            })
            .collect();
        let more = if dangling_indices.len() > 3 {
            format!(" 等 {} 条", dangling_indices.len())
        } else {
            String::new()
        };
        report.push(
            GraphError::fatal(
                "dangling_edge",
                format!(
                    "边引用了不存在的节点：{}{}",
                    sample.join(", "),
                    more
                ),
            )
            .with_edge_indices(dangling_indices),
        );
    }

    // 3) 重复边（同 from+to）
    // 3) Duplicate edges (same from+to)
    let mut seen_edges: HashMap<(&str, &str), ()> = HashMap::new();
    let mut dup_edge_indices: Vec<usize> = Vec::new();
    for (i, e) in graph.edges.iter().enumerate() {
        let key = (e.from.as_str(), e.to.as_str());
        if seen_edges.contains_key(&key) {
            dup_edge_indices.push(i);
        } else {
            seen_edges.insert(key, ());
        }
    }
    if !dup_edge_indices.is_empty() {
        report.push(
            GraphError::warning(
                "duplicate_edge",
                format!("存在 {} 条重复边（同名 from→to）", dup_edge_indices.len()),
            )
            .with_edge_indices(dup_edge_indices),
        );
    }

    // 4) 自环（from == to）
    // 4) Self-loop (from == to)
    let mut self_loop_indices: Vec<usize> = Vec::new();
    for (i, e) in graph.edges.iter().enumerate() {
        if e.from == e.to {
            self_loop_indices.push(i);
        }
    }
    if !self_loop_indices.is_empty() {
        report.push(
            GraphError::fatal(
                "self_loop",
                format!("存在 {} 条自环（from == to）", self_loop_indices.len()),
            )
            .with_edge_indices(self_loop_indices),
        );
    }

    // 后续结构分析只在节点 id 唯一时跑
    // Structural analysis only with unique ids
    if has_duplicates {
        return report;
    }

    // 5) 环检测：先去掉悬空/自环边，剩下若有环则报 fatal
    // 5) Cycle detection: drop dangling/self-loop edges, report cycles as fatal
    let cycle = find_cycle(graph);
    if let Some(cycle_nodes) = cycle {
        report.push(
            GraphError::fatal(
                "cycle",
                format!("工作流含有环：{}", cycle_nodes.join(" → ")),
            )
            .with_nodes(cycle_nodes),
        );
    }

    // 6) 孤儿节点（既无入边也无出边）—— warning，不致命
    // 6) Orphan node (no in/out edges) — warning, not fatal
    let mut has_in: HashSet<&str> = HashSet::new();
    let mut has_out: HashSet<&str> = HashSet::new();
    for e in &graph.edges {
        if node_ids.contains(e.from.as_str()) && node_ids.contains(e.to.as_str()) && e.from != e.to {
            has_in.insert(e.to.as_str());
            has_out.insert(e.from.as_str());
        }
    }
    let mut orphans: Vec<String> = Vec::new();
    for n in &graph.nodes {
        if !has_in.contains(n.id.as_str()) && !has_out.contains(n.id.as_str()) {
            orphans.push(n.id.clone());
        }
    }
    if !orphans.is_empty() && graph.nodes.len() > 1 {
        orphans.sort();
        report.push(
            GraphError::warning(
                "orphan_node",
                format!("存在 {} 个孤立节点（无入边也无出边）", orphans.len()),
            )
            .with_nodes(orphans),
        );
    }

    report
}

/// 找出一个环（DFS）；返回环上的节点序列，若无环则 None
/// Find a cycle (DFS); return the cycle path, or None if acyclic
fn find_cycle(graph: &WorkflowGraph) -> Option<Vec<String>> {
    use std::collections::HashMap;

    // 邻接表（去重 + 去自环）
    // Adjacency (deduped, no self-loops)
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for n in &graph.nodes {
        adj.entry(n.id.as_str()).or_default();
    }
    for e in &graph.edges {
        if e.from != e.to && adj.contains_key(e.from.as_str()) && adj.contains_key(e.to.as_str()) {
            adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
        }
    }

    enum Color { White, Gray, Black }
    let mut color: HashMap<&str, Color> = adj.keys().map(|k| (*k, Color::White)).collect();
    let mut stack: Vec<&str> = Vec::new();
    let mut parent: HashMap<&str, &str> = HashMap::new();

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        color: &mut HashMap<&'a str, Color>,
        parent: &mut HashMap<&'a str, &'a str>,
        stack: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        color.insert(node, Color::Gray);
        stack.push(node);
        if let Some(neighbors) = adj.get(node) {
            for &next in neighbors {
                match color.get(next) {
                    Some(Color::White) => {
                        parent.insert(next, node);
                        if let Some(cycle) = dfs(next, adj, color, parent, stack) {
                            return Some(cycle);
                        }
                    }
                    Some(Color::Gray) => {
                        // 找到环：从 next 开始回溯 parent 直到再次遇到 next
                        // Found cycle: walk parent from `next` until we hit `next` again
                        let mut path = vec![next.to_string()];
                        let mut cur = node;
                        while cur != next {
                            path.push(cur.to_string());
                            match parent.get(cur) {
                                Some(p) => cur = *p,
                                None => break,
                            }
                        }
                        path.push(next.to_string());
                        path.reverse();
                        return Some(path);
                    }
                    _ => {} // Black: 已完成，无环经过
                }
            }
        }
        color.insert(node, Color::Black);
        stack.pop();
        None
    }

    // 按 id 排序确保稳定输出
    // Sort to ensure deterministic output
    let mut nodes: Vec<&str> = adj.keys().copied().collect();
    nodes.sort();
    for n in nodes {
        if matches!(color.get(n), Some(Color::White)) {
            if let Some(cycle) = dfs(n, &adj, &mut color, &mut parent, &mut stack) {
                return Some(cycle);
            }
        }
    }
    None
}

/// 执行波次集合：Vec<Vec<node_id>>
/// Execution wave set: Vec<Vec<node_id>>
/// 先调 validate_graph 保证无 fatal，再调用本函数
/// Caller should validate first; this function will still return CycleDetected as a fallback
pub fn topological_waves(graph: &WorkflowGraph) -> Result<Vec<Vec<String>>, String> {
    use std::collections::{HashMap, HashSet};

    let report = validate_graph(graph);
    if let Some(fatal) = report.iter().find(|e| matches!(e.kind, crate::workflow::model::GraphErrorKind::Fatal)) {
        return Err(format!("InvalidGraph: {}", fatal.message));
    }

    let mut indeg: HashMap<&str, usize> = HashMap::new();
    let mut succs: HashMap<&str, Vec<&str>> = HashMap::new();
    let node_ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();

    for n in &graph.nodes {
        indeg.entry(n.id.as_str()).or_insert(0);
        succs.entry(n.id.as_str()).or_default();
    }
    for e in &graph.edges {
        if !node_ids.contains(e.from.as_str()) || !node_ids.contains(e.to.as_str()) {
            continue;
        }
        if e.from == e.to {
            continue;
        }
        *indeg.entry(e.to.as_str()).or_insert(0) += 1;
        succs.entry(e.from.as_str()).or_default().push(e.to.as_str());
    }

    let mut waves: Vec<Vec<String>> = Vec::new();
    let mut frontier: Vec<&str> = indeg
        .iter()
        .filter_map(|(id, d)| if *d == 0 { Some(*id) } else { None })
        .collect();
    frontier.sort();

    let mut visited = 0usize;
    while !frontier.is_empty() {
        let wave: Vec<String> = frontier.iter().map(|s| s.to_string()).collect();
        let mut next: Vec<&str> = Vec::new();
        for id in &frontier {
            visited += 1;
            if let Some(children) = succs.get(id) {
                for c in children {
                    if let Some(d) = indeg.get_mut(c) {
                        *d -= 1;
                        if *d == 0 {
                            next.push(c);
                        }
                    }
                }
            }
        }
        next.sort();
        waves.push(wave);
        frontier = next;
    }

    if visited != node_ids.len() {
        return Err("CycleDetected: workflow graph has a cycle".into());
    }
    Ok(waves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::model::WorkflowNode;
    use serde_json::json;

    fn node(id: &str) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            node_type: "plugin".to_string(),
            plugin_id: Some("p".to_string()),
            action: Some("a".to_string()),
            config: Some(json!({})),
            position: crate::workflow::model::Position { x: 0.0, y: 0.0 },
            display_name: None,
        }
    }

    fn graph(nodes: Vec<WorkflowNode>, edges: Vec<(&str, &str)>) -> WorkflowGraph {
        WorkflowGraph {
            nodes,
            edges: edges
                .into_iter()
                .map(|(from, to)| crate::workflow::model::WorkflowEdge {
                    from: from.to_string(),
                    to: to.to_string(),
                    map: Default::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn empty_graph_is_valid() {
        let r = validate_graph(&graph(vec![], vec![]));
        assert!(r.is_empty(), "empty graph should be valid: {r:?}");
    }

    #[test]
    fn linear_graph_is_valid() {
        let r = validate_graph(&graph(
            vec![node("a"), node("b"), node("c")],
            vec![("a", "b"), ("b", "c")],
        ));
        assert!(r.is_empty(), "linear graph should be valid: {r:?}");
    }

    #[test]
    fn duplicate_node_id_is_fatal() {
        let r = validate_graph(&graph(
            vec![node("a"), node("a")],
            vec![],
        ));
        assert!(r.iter().any(|e| e.code == "duplicate_node_id" && matches!(e.kind, crate::workflow::model::GraphErrorKind::Fatal)));
    }

    #[test]
    fn dangling_edge_is_fatal() {
        let r = validate_graph(&graph(
            vec![node("a")],
            vec![("a", "ghost")],
        ));
        assert!(r.iter().any(|e| e.code == "dangling_edge"));
    }

    #[test]
    fn self_loop_is_fatal() {
        let r = validate_graph(&graph(
            vec![node("a")],
            vec![("a", "a")],
        ));
        assert!(r.iter().any(|e| e.code == "self_loop"));
    }

    #[test]
    fn cycle_is_fatal_and_returns_path() {
        let r = validate_graph(&graph(
            vec![node("a"), node("b"), node("c")],
            vec![("a", "b"), ("b", "c"), ("c", "a")],
        ));
        let cycle = r.iter().find(|e| e.code == "cycle").expect("cycle not reported");
        assert_eq!(cycle.node_ids.as_ref().unwrap().len(), 4); // a→b→c→a (含首尾)
    }

    #[test]
    fn duplicate_edge_is_warning() {
        let r = validate_graph(&graph(
            vec![node("a"), node("b")],
            vec![("a", "b"), ("a", "b")],
        ));
        let w = r.iter().find(|e| e.code == "duplicate_edge").expect("dup edge not reported");
        assert!(matches!(w.kind, crate::workflow::model::GraphErrorKind::Warning));
    }

    #[test]
    fn orphan_node_is_warning() {
        let r = validate_graph(&graph(
            vec![node("a"), node("b"), node("c")],
            vec![("a", "b")],
        ));
        let w = r.iter().find(|e| e.code == "orphan_node").expect("orphan not reported");
        assert!(matches!(w.kind, crate::workflow::model::GraphErrorKind::Warning));
        assert!(w.node_ids.as_ref().unwrap().contains(&"c".to_string()));
    }

    #[test]
    fn single_node_no_orphan_warning() {
        // 单节点不报孤儿（没有对比意义）
        // Single node should not report orphan (nothing to compare with)
        let r = validate_graph(&graph(vec![node("solo")], vec![]));
        assert!(!r.iter().any(|e| e.code == "orphan_node"));
    }

    #[test]
    fn topo_waves_still_work_on_valid_graph() {
        let g = graph(
            vec![node("a"), node("b"), node("c")],
            vec![("a", "b"), ("b", "c")],
        );
        let waves = topological_waves(&g).unwrap();
        assert_eq!(waves.len(), 3);
    }

    #[test]
    fn topo_waves_rejects_fatal_graph() {
        let g = graph(
            vec![node("a")],
            vec![("a", "ghost")],
        );
        let err = topological_waves(&g).unwrap_err();
        assert!(err.starts_with("InvalidGraph:"));
    }
}
