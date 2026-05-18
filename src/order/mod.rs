//! order/mod.rs — order()
//! Faithful port of dagre-js/lib/order/index.ts

pub mod add_subgraph_constraints;
pub mod barycenter;
pub mod build_layer_graph;
pub mod cross_count;
pub mod init_order;
pub mod resolve_conflicts;
pub mod sort;
pub mod sort_subgraph;

use self::add_subgraph_constraints::add_subgraph_constraints;
use self::build_layer_graph::build_layer_graph;
use self::cross_count::cross_count;
use self::init_order::init_order;
use self::sort_subgraph::sort_subgraph;
use crate::graph::{EdgeLabel, Graph};
use crate::util::{build_layer_matrix, max_rank, range};
use std::collections::HashMap;

/// A constraint that forces `left` to appear before `right` in the same layer.
#[derive(Debug, Clone)]
pub struct OrderConstraint {
    /// The node that must appear to the left.
    pub left: String,
    /// The node that must appear to the right.
    pub right: String,
}

/// Applies heuristics to minimize edge crossings in the graph and sets the best
/// order solution as an order attribute on each node.
pub fn order(graph: &mut Graph, constraints: &[OrderConstraint], disable_optimal: bool) {
    let mr = max_rank(graph);
    let down_ranks = range(1, Some(mr + 1), None);
    let up_ranks = range(mr - 1, Some(-1), Some(-1));

    let mut layering = init_order(graph);
    assign_order(graph, &layering);

    if disable_optimal {
        return;
    }

    let mut best_cc = f64::INFINITY;
    let mut best: Vec<Vec<String>> = layering.clone();

    let mut last_best = 0usize;
    let mut i = 0usize;
    while last_best < 4 {
        // In the JS reference, layer graph nodes share the same object reference as
        // the main graph, so order values are always up-to-date as ranks are swept.
        // In Rust we clone node labels, so we must rebuild each layer graph right
        // before using it (inside the sweep) to pick up the latest order assignments.
        let (ranks, relationship): (&[i32], &str) = if i % 2 == 1 {
            (&down_ranks, "inEdges")
        } else {
            (&up_ranks, "outEdges")
        };
        let bias_right = i % 4 >= 2;
        sweep_layer_graphs_fresh(graph, ranks, relationship, bias_right, constraints);

        layering = build_layer_matrix(graph);
        let cc = cross_count(graph, &layering);
        if cc < best_cc {
            last_best = 0;
            best = layering.clone();
            best_cc = cc;
        }
        // Note: the JS dagre reference does NOT update best when cc == best_cc.
        // Only strict improvements update best.  This preserves the initial
        // insertion-order result from sweep 0 when all cross-counts are equal.

        i += 1;
        last_best += 1;
    }

    assign_order(graph, &best);
}

/// Builds rank -> nodes map for layer graph construction.
fn build_nodes_by_rank(graph: &Graph) -> HashMap<i32, Vec<String>> {
    let mut nodes_by_rank: HashMap<i32, Vec<String>> = HashMap::new();
    for v in graph.nodes() {
        let node = graph.node(&v);
        if let Some(rank) = node.rank {
            nodes_by_rank.entry(rank).or_default().push(v.clone());
        }
        if let (Some(min_r), Some(max_r)) = (node.min_rank, node.max_rank) {
            for r in min_r..=max_r {
                if node.rank != Some(r) {
                    nodes_by_rank.entry(r).or_default().push(v.clone());
                }
            }
        }
    }
    nodes_by_rank
}

/// Sweeps layer graphs, rebuilding each layer's graph from the current main graph
/// state right before processing it. This ensures barycenter computations see
/// up-to-date node order values — matching the JS reference where layer graph
/// nodes share object references with the main graph.
fn sweep_layer_graphs_fresh(
    main_graph: &mut Graph,
    ranks: &[i32],
    relationship: &str,
    bias_right: bool,
    constraints: &[OrderConstraint],
) {
    let mut cg = Graph::with_options(true, false, false);
    let nodes_by_rank = build_nodes_by_rank(main_graph);

    for &rank in ranks {
        for con in constraints {
            cg.set_edge(&con.left, &con.right, EdgeLabel::default(), None);
        }

        // Rebuild this rank's layer graph using current main_graph node orders
        let nodes = nodes_by_rank.get(&rank).cloned().unwrap_or_default();
        let lg = build_layer_graph(main_graph, rank, relationship, &nodes);

        let root = lg.graph().root.clone().expect("layer graph must have root");
        let sorted = sort_subgraph(&lg, &root, &cg, bias_right);

        // Assign order on the layer graph nodes in the main graph
        for (i, v) in sorted.vs.iter().enumerate() {
            if lg.has_node(v) {
                if let Some(node) = main_graph.node_opt_mut(v) {
                    node.order = Some(i as i32);
                }
            }
        }

        add_subgraph_constraints(&lg, &mut cg, &sorted.vs);
    }
}

fn assign_order(graph: &mut Graph, layering: &[Vec<String>]) {
    for layer in layering {
        for (i, v) in layer.iter().enumerate() {
            if let Some(node) = graph.node_opt_mut(v) {
                node.order = Some(i as i32);
            }
        }
    }
}
