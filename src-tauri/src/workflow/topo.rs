// 拓扑排序：返回 DAG 执行波次（波次内并发，波次间有序）
// Topological sort: returns DAG execution waves (concurrent within wave, ordered between waves)
// 抛出 CycleDetected 错误 / throws CycleDetected on cycles
use crate::workflow::model::WorkflowGraph;

/// 执行波次集合：Vec<Vec<node_id>>
/// Execution wave set: Vec<Vec<node_id>>
pub fn topological_waves(graph: &WorkflowGraph) -> Result<Vec<Vec<String>>, String> {
    use std::collections::{HashMap, HashSet};

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
