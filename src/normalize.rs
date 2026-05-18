//! normalize.rs — normalize run/undo
//! Faithful port of dagre-js/lib/normalize.ts

use crate::graph::{Edge, EdgeLabel, Graph, NodeLabel, Point};
use crate::util::add_dummy_node;

/// Breaks any long edges in the graph into short segments that span 1 layer each.
pub fn run(graph: &mut Graph) {
    graph.graph_mut().dummy_chains = Some(Vec::new());
    let edges: Vec<Edge> = graph.edges();
    for edge in edges {
        normalize_edge(graph, &edge);
    }
}

fn normalize_edge(graph: &mut Graph, e: &Edge) {
    let v = e.v.clone();
    let w = e.w.clone();
    let name = e.name.clone();

    let v_rank = graph.node(&v).rank.unwrap_or(0);
    let w_rank = graph.node(&w).rank.unwrap_or(0);

    if w_rank == v_rank + 1 {
        return;
    }

    let edge_label = graph.edge(e).cloned().unwrap_or_default();
    let label_rank = edge_label.label_rank;

    graph.remove_edge_obj(e);

    let mut current_v = v.clone();
    let mut current_rank = v_rank + 1;
    let mut i = 0i32;

    while current_rank < w_rank {
        let mut attrs = NodeLabel {
            width: 0.0,
            height: 0.0,
            edge_label: Some(Box::new(edge_label.clone())),
            edge_obj: Some(e.clone()),
            rank: Some(current_rank),
            ..Default::default()
        };

        // Points will be collected during undo
        let mut el = edge_label.clone();
        el.points = Some(Vec::new());
        attrs.edge_label = Some(Box::new(el));

        let node_type = if Some(current_rank) == label_rank {
            attrs.width = edge_label.width.unwrap_or(0.0);
            attrs.height = edge_label.height.unwrap_or(0.0);
            attrs.labelpos = edge_label.labelpos.clone();
            "edge-label"
        } else {
            "edge"
        };

        let dummy = add_dummy_node(graph, node_type, attrs, "_d");

        if i == 0 {
            graph
                .graph_mut()
                .dummy_chains
                .as_mut()
                .unwrap()
                .push(dummy.clone());
        }

        graph.set_edge(
            &current_v,
            &dummy,
            EdgeLabel {
                weight: edge_label.weight,
                ..Default::default()
            },
            name.as_deref(),
        );

        current_v = dummy;
        current_rank += 1;
        i += 1;
    }

    graph.set_edge(
        &current_v,
        &w,
        EdgeLabel {
            weight: edge_label.weight,
            ..Default::default()
        },
        name.as_deref(),
    );
}

/// Undoes normalize by reconnecting dummy chains back into the original edges.
pub fn undo(graph: &mut Graph) {
    let dummy_chains = graph.graph().dummy_chains.clone().unwrap_or_default();
    for start_v in dummy_chains {
        let v = start_v.clone();
        let node = graph.node(&v).clone();
        if let (Some(orig_label), Some(orig_edge_obj)) = (
            node.edge_label.as_ref().map(|l| *l.clone()),
            node.edge_obj.as_ref().cloned(),
        ) {
            // Restore original edge
            let final_label = orig_label.clone();
            // Edge was already set earlier in the chain? We'll set it here.
            // The JS sets it unconditionally.
            graph.set_edge_obj(&orig_edge_obj, final_label.clone());

            // Walk the chain collecting points
            let mut cur_v = v.clone();
            loop {
                let cur_node = graph.node(&cur_v).clone();
                if cur_node.dummy.is_none() {
                    break;
                }
                // next node in chain
                let successors = graph.successors(&cur_v).unwrap_or_default();
                let next_v = match successors.into_iter().next() {
                    Some(s) => s,
                    None => break,
                };
                // Collect point
                let x = cur_node.x.unwrap_or(0.0);
                let y = cur_node.y.unwrap_or(0.0);
                // Get the edge label to append to
                if let Some(el) = graph.edge_mut(&orig_edge_obj) {
                    el.points.get_or_insert_with(Vec::new).push(Point { x, y });
                    if cur_node.dummy.as_deref() == Some("edge-label") {
                        el.x = Some(x);
                        el.y = Some(y);
                        el.width = Some(cur_node.width);
                        el.height = Some(cur_node.height);
                    }
                }
                graph.remove_node(&cur_v);
                cur_v = next_v;
            }
        }
    }
}
