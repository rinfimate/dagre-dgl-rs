//! rank/feasible_tree.rs — feasibleTree
//! Faithful port of dagre-js/lib/rank/feasible-tree.ts

use crate::graph::{Edge, EdgeLabel, Graph};
use crate::rank::util::slack;

/// Constructs a spanning tree with tight edges and adjusts the input node's
/// ranks to achieve this.
///
/// Returns a tree (undirected graph) that is constructed using only tight edges.
pub fn feasible_tree(graph: &mut Graph) -> Graph {
    let mut tree = Graph::with_options(false, false, false);

    let nodes = graph.nodes();
    if nodes.is_empty() {
        panic!("Graph must have at least one node");
    }
    let start = nodes[0].clone();
    let size = graph.node_count();
    tree.set_node_default(&start);

    loop {
        let tree_size = tight_tree(&mut tree, graph);
        if tree_size >= size {
            break;
        }
        let edge = find_min_slack_edge(&tree, graph);
        match edge {
            None => break,
            Some(e) => {
                let delta = if tree.has_node(&e.v) {
                    slack(graph, &e)
                } else {
                    -slack(graph, &e)
                };
                shift_ranks(&tree, graph, delta);
            }
        }
    }

    tree
}

/// Finds a maximal tree of tight edges and returns the number of nodes in the tree.
fn tight_tree(tree: &mut Graph, graph: &Graph) -> usize {
    let tree_nodes: Vec<String> = tree.nodes();
    for start in tree_nodes {
        tight_tree_dfs(tree, graph, &start.clone());
    }
    tree.node_count()
}

fn tight_tree_dfs(tree: &mut Graph, graph: &Graph, v: &str) {
    let node_edges: Vec<Edge> = graph.node_edges(v).unwrap_or_default();
    for e in node_edges {
        let edge_v = e.v.clone();
        let w = if v == edge_v { e.w.clone() } else { edge_v };
        if !tree.has_node(&w) && slack(graph, &e) == 0 {
            tree.set_node_default(&w);
            tree.set_edge(v, &w, EdgeLabel::default(), None);
            tight_tree_dfs(tree, graph, &w.clone());
        }
    }
}

/// Finds the edge with the smallest slack that is incident on tree.
fn find_min_slack_edge(tree: &Graph, graph: &Graph) -> Option<Edge> {
    let mut min_slack = i32::MAX;
    let mut result: Option<Edge> = None;

    for e in graph.edges() {
        let v_in_tree = tree.has_node(&e.v);
        let w_in_tree = tree.has_node(&e.w);
        if v_in_tree != w_in_tree {
            let s = slack(graph, &e);
            if s < min_slack {
                min_slack = s;
                result = Some(e);
            }
        }
    }
    result
}

fn shift_ranks(tree: &Graph, graph: &mut Graph, delta: i32) {
    for v in tree.nodes() {
        if let Some(r) = graph.node_mut(&v).rank.as_mut() {
            *r += delta;
        }
    }
}
