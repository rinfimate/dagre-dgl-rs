//! rank/mod.rs — rank() dispatcher
//! Faithful port of dagre-js/lib/rank/index.ts

pub mod feasible_tree;
pub mod network_simplex;
pub mod util;

use self::feasible_tree::feasible_tree;
use self::network_simplex::network_simplex;
use self::util::longest_path;
use crate::graph::Graph;

/// Assigns a rank to each node in the input graph that respects the "minlen"
/// constraint specified on edges between nodes.
pub fn rank(graph: &mut Graph) {
    let ranker = graph.graph().ranker.clone();
    match ranker.as_deref() {
        Some("network-simplex") => network_simplex_ranker(graph),
        Some("tight-tree") => tight_tree_ranker(graph),
        Some("longest-path") => longest_path_ranker(graph),
        Some("none") => {}
        _ => network_simplex_ranker(graph),
    }
}

fn longest_path_ranker(g: &mut Graph) {
    longest_path(g);
}

fn tight_tree_ranker(g: &mut Graph) {
    longest_path(g);
    feasible_tree(g);
}

fn network_simplex_ranker(g: &mut Graph) {
    network_simplex(g);
}
