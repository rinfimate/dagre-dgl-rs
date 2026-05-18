//! order/cross_count.rs — crossCount
//! Faithful port of dagre-js/lib/order/cross-count.ts

use crate::graph::Graph;
use std::collections::HashMap;

/// A function that takes a layering and a graph and returns a weighted crossing count.
pub fn cross_count(graph: &Graph, layering: &[Vec<String>]) -> f64 {
    let mut cc = 0.0;
    for i in 1..layering.len() {
        cc += two_layer_cross_count(graph, &layering[i - 1], &layering[i]);
    }
    cc
}

struct SouthEntry {
    pos: usize,
    weight: f64,
}

fn two_layer_cross_count(graph: &Graph, north_layer: &[String], south_layer: &[String]) -> f64 {
    let south_pos: HashMap<String, usize> = south_layer
        .iter()
        .enumerate()
        .map(|(i, v)| (v.clone(), i))
        .collect();

    let mut south_entries: Vec<SouthEntry> = Vec::new();
    for v in north_layer {
        let edges = graph.out_edges(v).unwrap_or_default();
        let mut v_entries: Vec<SouthEntry> = edges
            .iter()
            .filter_map(|e| {
                south_pos.get(&e.w).map(|&pos| SouthEntry {
                    pos,
                    weight: graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0),
                })
            })
            .collect();
        v_entries.sort_by_key(|e| e.pos);
        south_entries.extend(v_entries);
    }

    // Build accumulator tree
    let mut first_index = 1usize;
    while first_index < south_layer.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    let first_index_offset = first_index - 1;
    let mut tree = vec![0.0f64; tree_size];

    let mut cc = 0.0;
    for entry in &south_entries {
        let mut index = entry.pos + first_index_offset;
        tree[index] += entry.weight;
        let mut weight_sum = 0.0;
        while index > 0 {
            if index % 2 == 1 {
                weight_sum += tree[index + 1];
            }
            index = (index - 1) >> 1;
            tree[index] += entry.weight;
        }
        cc += entry.weight * weight_sum;
    }

    cc
}
