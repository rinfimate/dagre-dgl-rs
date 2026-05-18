//! order/build_layer_graph.rs — buildLayerGraph
//! Faithful port of dagre-js/lib/order/build-layer-graph.ts

use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
use crate::util::unique_id;

/// Constructs a graph that can be used to sort a layer of nodes.
pub fn build_layer_graph(
    graph: &Graph,
    rank: i32,
    relationship: &str, // "inEdges" or "outEdges"
    nodes_with_rank: &[String],
) -> Graph {
    let root = create_root_node(graph);
    let mut result = Graph::with_options(true, false, true);
    let gl = GraphLabel {
        root: Some(root.clone()),
        ..Default::default()
    };
    result.set_graph(gl);

    for v in nodes_with_rank {
        let node = graph.node(v);
        let parent = graph.parent(v);

        let node_rank = node.rank;
        let min_rank = node.min_rank;
        let max_rank = node.max_rank;

        let in_range = node_rank == Some(rank)
            || (min_rank.is_some()
                && max_rank.is_some()
                && min_rank.unwrap() <= rank
                && rank <= max_rank.unwrap());

        if in_range {
            // Set the node in result with the same label from the source graph
            result.set_node(v, graph.node(v).clone());
            result.set_parent(v, Some(parent.unwrap_or(&root)));

            // Add edges from the relationship direction
            let edges = if relationship == "inEdges" {
                graph.in_edges(v).unwrap_or_default()
            } else {
                graph.out_edges(v).unwrap_or_default()
            };

            for e in &edges {
                let u = if e.v == *v { &e.w } else { &e.v };
                // Ensure the neighbor node exists in the result graph with the
                // correct label from the main graph (especially the order attribute
                // which barycenter uses). In JS graphlib, node objects are shared
                // references so they always reflect current state; in Rust we must
                // copy the label explicitly.
                if !result.has_node(u) {
                    if let Some(u_label) = graph.node_opt(u) {
                        result.set_node(u, u_label.clone());
                    }
                } else {
                    // Update the existing node's label in case the order changed
                    // since the node was first added to the layer graph.
                    if let Some(u_label) = graph.node_opt(u) {
                        let order = u_label.order;
                        if let Some(existing) = result.node_opt_mut(u) {
                            existing.order = order;
                        }
                    }
                }
                let edge_weight = graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                let existing_weight = result.edge_vw(u, v).and_then(|l| l.weight).unwrap_or(0.0);
                result.set_edge(
                    u,
                    v,
                    EdgeLabel {
                        weight: Some(edge_weight + existing_weight),
                        ..Default::default()
                    },
                    None,
                );
            }

            // For subgraph nodes (with minRank), override with border info
            if node.min_rank.is_some() {
                let rank_idx = rank as usize;
                let bl = node
                    .border_left
                    .as_ref()
                    .and_then(|v| v.get(rank_idx))
                    .cloned()
                    .flatten();
                let br = node
                    .border_right
                    .as_ref()
                    .and_then(|v| v.get(rank_idx))
                    .cloned()
                    .flatten();
                let override_label = NodeLabel {
                    border_left: Some(vec![bl]),
                    border_right: Some(vec![br]),
                    ..Default::default()
                };
                result.set_node(v, override_label);
            }
        }
    }

    result
}

fn create_root_node(graph: &Graph) -> String {
    let mut v = unique_id("_root");
    while graph.has_node(&v) {
        v = unique_id("_root");
    }
    v
}
