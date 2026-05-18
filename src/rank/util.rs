//! rank/util.rs — longestPath, slack
//! Faithful port of dagre-js/lib/rank/util.ts

use crate::graph::{Edge, Graph};

/// Initializes ranks for the input graph using the longest path algorithm.
/// Nodes are pushed to the lowest layer possible.
pub fn longest_path(graph: &mut Graph) {
    let mut visited: std::collections::HashMap<String, bool> = std::collections::HashMap::new();

    // We need to do DFS but graph is borrowed. Collect data first.
    // Use a recursive closure via a helper struct.
    fn dfs(
        graph: &mut Graph,
        v: &str,
        visited: &mut std::collections::HashMap<String, bool>,
    ) -> i32 {
        if visited.contains_key(v) {
            return graph.node(v).rank.unwrap_or(0);
        }
        visited.insert(v.to_string(), true);

        let out_edges: Vec<Edge> = graph.out_edges(v).unwrap_or_default();
        let mut min_rank = i32::MAX;

        for e in out_edges {
            let w_rank = dfs(graph, &e.w.clone(), visited);
            let minlen = graph.edge(&e).map_or(1, |l| l.minlen.unwrap_or(1));
            let candidate = w_rank - minlen;
            if candidate < min_rank {
                min_rank = candidate;
            }
        }

        let rank = if min_rank == i32::MAX { 0 } else { min_rank };
        graph.node_mut(v).rank = Some(rank);
        rank
    }

    let sources: Vec<String> = graph.sources();
    for v in sources {
        dfs(graph, &v, &mut visited);
    }
}

/// Returns the slack for the given edge.
/// slack = rank(w) - rank(v) - minlen
pub fn slack(graph: &Graph, edge: &Edge) -> i32 {
    let rank_w = graph.node(&edge.w).rank.unwrap_or(0);
    let rank_v = graph.node(&edge.v).rank.unwrap_or(0);
    let minlen = graph.edge(edge).map_or(1, |l| l.minlen.unwrap_or(1));
    rank_w - rank_v - minlen
}
