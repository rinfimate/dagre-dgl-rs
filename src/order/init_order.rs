//! order/init_order.rs — initOrder
//! Faithful port of dagre-js/lib/order/init-order.ts

use crate::graph::Graph;
use crate::util::{apply_max_i32, range};
use std::collections::HashMap;

/// Assigns an initial order value for each node by performing a DFS search
/// starting from nodes in the first rank.
///
/// Returns a layering matrix with an array per layer and each layer sorted by
/// the order of its nodes.
pub fn init_order(graph: &Graph) -> Vec<Vec<String>> {
    let mut visited: HashMap<String, bool> = HashMap::new();

    // simple nodes = nodes with no children
    let simple_nodes: Vec<String> = graph
        .nodes()
        .into_iter()
        .filter(|v| graph.children(v).is_empty())
        .collect();

    let simple_node_ranks: Vec<i32> = simple_nodes
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .collect();

    let max_rank_val = if simple_node_ranks.is_empty() {
        0
    } else {
        apply_max_i32(&simple_node_ranks)
    };

    let mut layers: Vec<Vec<String>> = range(max_rank_val + 1, None, None)
        .iter()
        .map(|_| Vec::new())
        .collect();

    // Sort simple_nodes by rank
    let mut ordered_vs: Vec<String> = simple_nodes.clone();
    ordered_vs.sort_by_key(|v| graph.node(v).rank.unwrap_or(0));

    fn dfs(
        graph: &Graph,
        v: &str,
        visited: &mut HashMap<String, bool>,
        layers: &mut Vec<Vec<String>>,
    ) {
        if *visited.get(v).unwrap_or(&false) {
            return;
        }
        visited.insert(v.to_string(), true);
        let rank = graph.node(v).rank.unwrap_or(0) as usize;
        if rank < layers.len() {
            layers[rank].push(v.to_string());
        }
        if let Some(successors) = graph.successors(v) {
            for w in successors {
                dfs(graph, &w.clone(), visited, layers);
            }
        }
    }

    for v in ordered_vs {
        dfs(graph, &v.clone(), &mut visited, &mut layers);
    }

    layers
}
