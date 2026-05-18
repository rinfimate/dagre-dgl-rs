/// acyclic.rs — acyclic run/undo + dfsFAS + greedyFAS
/// Faithful port of dagre-js/lib/acyclic.ts and greedy-fas.ts

use std::collections::HashMap;
use crate::graph::{Edge, EdgeLabel, Graph};
use crate::util::unique_id;

// ─── acyclic.run ──────────────────────────────────────────────────────────────

pub fn run(graph: &mut Graph) {
    let fas = if graph.graph().acyclicer.as_deref() == Some("greedy") {
        greedy_fas(graph)
    } else {
        dfs_fas(graph)
    };

    for e in fas {
        let mut label = graph.edge(&e).cloned().unwrap_or_default();
        graph.remove_edge_obj(&e);
        label.forward_name = e.name.clone();
        label.reversed = Some(true);
        let rev_name = unique_id("rev");
        graph.set_edge(&e.w, &e.v, label, Some(&rev_name));
    }
}

fn dfs_fas(graph: &Graph) -> Vec<Edge> {
    let mut fas: Vec<Edge> = Vec::new();
    let mut stack: HashMap<String, bool> = HashMap::new();
    let mut visited: HashMap<String, bool> = HashMap::new();

    fn dfs_inner(
        graph: &Graph,
        v: &str,
        stack: &mut HashMap<String, bool>,
        visited: &mut HashMap<String, bool>,
        fas: &mut Vec<Edge>,
    ) {
        if visited.contains_key(v) {
            return;
        }
        visited.insert(v.to_string(), true);
        stack.insert(v.to_string(), true);
        let out_edges = graph.out_edges(v).unwrap_or_default();
        for e in out_edges {
            let w = e.w.clone();
            if stack.contains_key(&w) {
                fas.push(e);
            } else {
                dfs_inner(graph, &w, stack, visited, fas);
            }
        }
        stack.remove(v);
    }

    for v in graph.nodes() {
        dfs_inner(graph, &v.clone(), &mut stack, &mut visited, &mut fas);
    }
    fas
}

// ─── acyclic.undo ─────────────────────────────────────────────────────────────

pub fn undo(graph: &mut Graph) {
    let reversed_edges: Vec<Edge> = graph.edges().into_iter()
        .filter(|e| graph.edge(e).and_then(|l| l.reversed).unwrap_or(false))
        .collect();

    for e in reversed_edges {
        let mut label = graph.edge(&e).cloned().unwrap_or_default();
        graph.remove_edge_obj(&e);
        let forward_name = label.forward_name.take();
        label.reversed = None;
        graph.set_edge(&e.w, &e.v, label, forward_name.as_deref());
    }
}

// ─── greedyFAS ────────────────────────────────────────────────────────────────
//
// Port of dagre-js/lib/greedy-fas.ts
// Uses a bucket-based approach. Node in/out weights are stored in the
// fas_graph node labels (x=out_weight, y=in_weight).

pub fn greedy_fas(graph: &Graph) -> Vec<Edge> {
    if graph.node_count() <= 1 {
        return Vec::new();
    }

    let (mut fas_graph, mut buckets, zero_idx) = build_fas_state(graph);
    let results = do_greedy_fas(&mut fas_graph, &mut buckets, zero_idx);

    // Expand multi-edges
    results.into_iter().flat_map(|(v, w)| {
        graph.out_edges_to(&v, &w).unwrap_or_default()
    }).collect()
}

fn do_greedy_fas(
    g: &mut Graph,
    buckets: &mut Vec<Vec<String>>,
    zero_idx: usize,
) -> Vec<(String, String)> {
    let mut results: Vec<(String, String)> = Vec::new();

    while g.node_count() > 0 {
        // Drain sinks (bucket 0)
        loop {
            match pop_live_node(g, &mut buckets[0]) {
                None => break,
                Some(v) => remove_node_fas(g, buckets, zero_idx, &v, false, &mut results),
            }
        }
        // Drain sources (last bucket)
        let last = buckets.len() - 1;
        loop {
            match pop_live_node(g, &mut buckets[last]) {
                None => break,
                Some(v) => remove_node_fas(g, buckets, zero_idx, &v, false, &mut results),
            }
        }
        // Pick highest bucket node
        if g.node_count() > 0 {
            for i in (1..buckets.len() - 1).rev() {
                if let Some(v) = pop_live_node(g, &mut buckets[i]) {
                    remove_node_fas(g, buckets, zero_idx, &v, true, &mut results);
                    break;
                }
            }
        }
    }

    results
}

fn pop_live_node(g: &Graph, bucket: &mut Vec<String>) -> Option<String> {
    loop {
        match bucket.pop() {
            None => return None,
            Some(v) => {
                if g.has_node(&v) {
                    return Some(v);
                }
            }
        }
    }
}

fn remove_node_fas(
    g: &mut Graph,
    buckets: &mut Vec<Vec<String>>,
    zero_idx: usize,
    v: &str,
    collect_predecessors: bool,
    results: &mut Vec<(String, String)>,
) {
    let in_edges: Vec<Edge> = g.in_edges(v).unwrap_or_default();
    for e in &in_edges {
        let weight = g.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
        let u = e.v.clone();
        if collect_predecessors {
            results.push((u.clone(), v.to_string()));
        }
        if let Some(u_node) = g.node_opt_mut(&u) {
            let cur_out = u_node.x.unwrap_or(0.0) - weight;
            u_node.x = Some(cur_out);
        }
        if g.has_node(&u) {
            let out_w = g.node(&u).x.unwrap_or(0.0);
            let in_w = g.node(&u).y.unwrap_or(0.0);
            let idx = fas_bucket_idx(buckets.len(), zero_idx, out_w, in_w);
            buckets[idx].push(u.clone());
        }
    }

    let out_edges: Vec<Edge> = g.out_edges(v).unwrap_or_default();
    for e in &out_edges {
        let weight = g.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
        let w = e.w.clone();
        if let Some(w_node) = g.node_opt_mut(&w) {
            let cur_in = w_node.y.unwrap_or(0.0) - weight;
            w_node.y = Some(cur_in);
        }
        if g.has_node(&w) {
            let out_w = g.node(&w).x.unwrap_or(0.0);
            let in_w = g.node(&w).y.unwrap_or(0.0);
            let idx = fas_bucket_idx(buckets.len(), zero_idx, out_w, in_w);
            buckets[idx].push(w.clone());
        }
    }

    g.remove_node(v);
}

fn fas_bucket_idx(bucket_count: usize, zero_idx: usize, out_w: f64, in_w: f64) -> usize {
    if out_w == 0.0 {
        0
    } else if in_w == 0.0 {
        bucket_count - 1
    } else {
        let idx = (out_w - in_w) as i64 + zero_idx as i64;
        idx.max(0).min(bucket_count as i64 - 1) as usize
    }
}

fn build_fas_state(graph: &Graph) -> (Graph, Vec<Vec<String>>, usize) {
    let mut fas_graph = Graph::with_options(true, false, false);
    let mut max_in = 0.0f64;
    let mut max_out = 0.0f64;

    for v in graph.nodes() {
        let mut n = crate::graph::NodeLabel::default();
        n.x = Some(0.0); // out_weight
        n.y = Some(0.0); // in_weight
        fas_graph.set_node(&v, n);
    }

    for edge in graph.edges() {
        // Default weight function: use edge weight
        let weight = graph.edge(&edge).and_then(|l| l.weight).unwrap_or(1.0);
        let existing = fas_graph.edge_vw(&edge.v, &edge.w).and_then(|l| l.weight).unwrap_or(0.0);
        fas_graph.set_edge(&edge.v, &edge.w, EdgeLabel {
            weight: Some(existing + weight),
            ..Default::default()
        }, None);

        if let Some(v_node) = fas_graph.node_opt_mut(&edge.v) {
            let cur = v_node.x.unwrap_or(0.0) + weight;
            v_node.x = Some(cur);
            if cur > max_out { max_out = cur; }
        }
        if let Some(w_node) = fas_graph.node_opt_mut(&edge.w) {
            let cur = w_node.y.unwrap_or(0.0) + weight;
            w_node.y = Some(cur);
            if cur > max_in { max_in = cur; }
        }
    }

    let bucket_count = (max_out + max_in + 3.0) as usize;
    let zero_idx = (max_in + 1.0) as usize;
    let mut buckets: Vec<Vec<String>> = vec![Vec::new(); bucket_count.max(3)];

    for v in fas_graph.nodes() {
        let out_w = fas_graph.node(&v).x.unwrap_or(0.0);
        let in_w = fas_graph.node(&v).y.unwrap_or(0.0);
        let idx = fas_bucket_idx(buckets.len(), zero_idx, out_w, in_w);
        buckets[idx].push(v);
    }

    (fas_graph, buckets, zero_idx)
}
