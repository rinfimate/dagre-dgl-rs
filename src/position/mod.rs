//! position/mod.rs — position()
//! Faithful port of dagre-js/lib/position/index.ts

pub mod bk;

use self::bk::position_x;
use crate::graph::Graph;
use crate::util::{as_non_compound_graph, build_layer_matrix};

/// Assign x/y coordinates to all nodes using the Brandes-Köpf algorithm.
pub fn position(graph: &mut Graph) {
    let mut g = as_non_compound_graph(graph);

    position_y(&mut g);
    let xs = position_x(&g);

    // Copy y back to original graph (non-compound nodes)
    for v in g.nodes() {
        if let Some(y) = g.node(&v).y {
            if let Some(node) = graph.node_opt_mut(&v) {
                node.y = Some(y);
            }
        }
    }

    // Copy x
    for (v, x) in xs {
        if let Some(node) = graph.node_opt_mut(&v) {
            node.x = Some(x);
        }
    }
}

fn position_y(graph: &mut Graph) {
    let layering = build_layer_matrix(graph);
    let ranksep = graph.graph().ranksep.unwrap_or(50.0);
    let rankalign = graph.graph().rankalign.clone();
    let mut prev_y = 0.0f64;

    let layer_data: Vec<(f64, Vec<String>)> = layering
        .iter()
        .map(|layer| {
            let max_height = layer.iter().fold(0.0f64, |acc, v| {
                // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
                if v.is_empty() {
                    return acc;
                }
                let h = graph.node(v).height;
                if acc > h {
                    acc
                } else {
                    h
                }
            });
            (max_height, layer.clone())
        })
        .collect();

    for (max_height, layer) in layer_data {
        for v in &layer {
            // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
            if v.is_empty() {
                continue;
            }
            let node = graph.node_mut(v);
            let h = node.height;
            node.y = Some(match rankalign.as_deref() {
                Some("top") => prev_y + h / 2.0,
                Some("bottom") => prev_y + max_height - h / 2.0,
                _ => prev_y + max_height / 2.0,
            });
        }
        prev_y += max_height + ranksep;
    }
}
