//! position/bk.rs — positionX (Brandes-Köpf)
//! Faithful port of dagre-js/lib/position/bk.ts

use crate::graph::{EdgeLabel, Graph};
use crate::util::{apply_max, apply_min, build_layer_matrix};
use std::collections::HashMap;

type Conflicts = HashMap<String, HashMap<String, bool>>;
type PositionMap = HashMap<String, f64>;
type AlignmentResult = (HashMap<String, String>, HashMap<String, String>); // (root, align)
type NeighborFn<'a> = Box<dyn Fn(&str) -> Vec<String> + 'a>;

/// Marks all edges in the graph with a type-1 conflict.
pub fn find_type1_conflicts(graph: &Graph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts: Conflicts = HashMap::new();

    fn visit_layer(
        graph: &Graph,
        conflicts: &mut Conflicts,
        prev_layer: &[String],
        layer: &[String],
    ) -> Vec<String> {
        let mut k0 = 0usize;
        let mut scan_pos = 0usize;
        let prev_layer_length = prev_layer.len();
        let last_node = layer.last().cloned().unwrap_or_default();

        for (i, v) in layer.iter().enumerate() {
            // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
            if v.is_empty() {
                continue;
            }
            let w = find_other_inner_segment_node(graph, v);
            let k1 = w.as_ref().map_or(prev_layer_length, |w_str| {
                graph.node(w_str).order.unwrap_or(0) as usize
            });

            if w.is_some() || *v == last_node {
                for scan_node in &layer[scan_pos..=i] {
                    if scan_node.is_empty() {
                        continue;
                    }
                    let preds = graph.predecessors(scan_node).unwrap_or_default();
                    for u in preds {
                        let u_label = graph.node(&u);
                        let u_pos = u_label.order.unwrap_or(0) as usize;
                        let scan_label = graph.node(scan_node);
                        if (u_pos < k0 || k1 < u_pos)
                            && !(u_label.dummy.is_some() && scan_label.dummy.is_some())
                        {
                            add_conflict(conflicts, &u, scan_node);
                        }
                    }
                }
                scan_pos = i + 1;
                k0 = k1;
            }
        }

        layer.to_vec()
    }

    if layering.len() >= 2 {
        let mut prev = &layering[0];
        for curr in &layering[1..] {
            visit_layer(graph, &mut conflicts, prev, curr);
            prev = curr;
        }
    }

    conflicts
}

/// Marks type-2 conflicts.
pub fn find_type2_conflicts(graph: &Graph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts: Conflicts = HashMap::new();

    fn scan(
        graph: &Graph,
        conflicts: &mut Conflicts,
        south: &[String],
        south_pos: usize,
        south_end: usize,
        prev_north_border: i32,
        next_north_border: i32,
    ) {
        for i in south_pos..south_end {
            if let Some(v) = south.get(i) {
                // Skip empty-string slots (sparse layer gaps)
                if v.is_empty() {
                    continue;
                }
                let node = graph.node(v);
                if node.dummy.is_some() {
                    let preds = graph.predecessors(v).unwrap_or_default();
                    for u in preds {
                        let u_node = graph.node(&u);
                        if u_node.dummy.is_some() {
                            let u_order = u_node.order.unwrap_or(0);
                            if u_order < prev_north_border || u_order > next_north_border {
                                add_conflict(conflicts, &u, v);
                            }
                        }
                    }
                }
            }
        }
    }

    if layering.len() >= 2 {
        let mut north = &layering[0];
        for south_layer in &layering[1..] {
            let mut prev_north_pos: i32 = -1;
            let mut next_north_pos: i32 = -1;
            let mut south_pos = 0usize;

            for south_lookahead in 0..south_layer.len() {
                let v = &south_layer[south_lookahead];
                // Skip empty-string slots (sparse layer gaps)
                if v.is_empty() {
                    continue;
                }
                let node = graph.node(v);
                if node.dummy.as_deref() == Some("border") {
                    let preds = graph.predecessors(v).unwrap_or_default();
                    if !preds.is_empty() {
                        let first_pred = &preds[0];
                        next_north_pos = graph.node(first_pred).order.unwrap_or(0);
                        scan(
                            graph,
                            &mut conflicts,
                            south_layer,
                            south_pos,
                            south_lookahead,
                            prev_north_pos,
                            next_north_pos,
                        );
                        south_pos = south_lookahead;
                        prev_north_pos = next_north_pos;
                    }
                }
                scan(
                    graph,
                    &mut conflicts,
                    south_layer,
                    south_pos,
                    south_layer.len(),
                    next_north_pos,
                    north.len() as i32,
                );
            }
            north = south_layer;
        }
    }

    conflicts
}

fn find_other_inner_segment_node(graph: &Graph, v: &str) -> Option<String> {
    if graph.node(v).dummy.is_some() {
        graph
            .predecessors(v)
            .unwrap_or_default()
            .into_iter()
            .find(|u| graph.node(u).dummy.is_some())
    } else {
        None
    }
}

/// Record a type-1 conflict between nodes `v` and `w`.
pub fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts
        .entry(v.to_string())
        .or_default()
        .insert(w.to_string(), true);
}

/// Return `true` if a type-1 conflict exists between nodes `v` and `w`.
pub fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts.get(v).is_some_and(|m| m.contains_key(w))
}

/// Vertical alignment pass.
pub fn vertical_alignment(
    _graph: &Graph,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    neighbor_fn: &dyn Fn(&str) -> Vec<String>,
) -> AlignmentResult {
    let mut root: HashMap<String, String> = HashMap::new();
    let mut align: HashMap<String, String> = HashMap::new();
    let mut pos: HashMap<String, usize> = HashMap::new();

    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
            if v.is_empty() {
                continue;
            }
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: i32 = -1;
        for v in layer {
            // Skip empty-string slots (sparse layer gaps)
            if v.is_empty() {
                continue;
            }
            let ws_raw = neighbor_fn(v);
            if !ws_raw.is_empty() {
                let mut ws: Vec<String> = ws_raw;
                ws.sort_by_key(|a| pos.get(a).cloned().unwrap_or(0));
                let mp = (ws.len() as f64 - 1.0) / 2.0;
                let lo = mp.floor() as usize;
                let hi = mp.ceil() as usize;
                for i in lo..=hi {
                    if let Some(w) = ws.get(i) {
                        if let Some(&pos_w) = pos.get(w) {
                            let align_v = align.get(v).cloned().unwrap_or_default();
                            if align_v == *v
                                && prev_idx < pos_w as i32
                                && !has_conflict(conflicts, v, w)
                            {
                                let root_w = root.get(w).cloned().unwrap_or_default();
                                align.insert(w.clone(), v.clone());
                                let root_v = root_w.clone();
                                align.insert(v.clone(), root_v.clone());
                                root.insert(v.clone(), root_v);
                                prev_idx = pos_w as i32;
                            }
                        }
                    }
                }
            }
        }
    }

    (root, align)
}

/// Horizontal compaction pass.
pub fn horizontal_compaction(
    graph: &Graph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> PositionMap {
    let mut xs: PositionMap = HashMap::new();
    let block_g = build_block_graph(graph, layering, root, reverse_sep);
    let border_type = if reverse_sep {
        "borderLeft"
    } else {
        "borderRight"
    };

    // First pass: assign smallest coordinates using DFS traversal
    {
        let mut stack: Vec<String> = block_g.nodes();
        let mut visited: HashMap<String, bool> = HashMap::new();
        while let Some(elem) = stack.pop() {
            if *visited.get(&elem).unwrap_or(&false) {
                // Process: compute xs[elem] from predecessors
                let in_edges = block_g.in_edges(&elem).unwrap_or_default();
                let val = in_edges.iter().fold(0.0f64, |acc, e| {
                    let xs_v = xs.get(&e.v).cloned().unwrap_or(0.0);
                    let edge_weight = block_g.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                    acc.max(xs_v + edge_weight)
                });
                xs.insert(elem.to_string(), val);
            } else {
                visited.insert(elem.clone(), true);
                stack.push(elem.clone());
                let preds = block_g.predecessors(&elem).unwrap_or_default();
                for p in preds {
                    stack.push(p);
                }
            }
        }
    }

    // Second pass: assign greatest coordinates
    {
        let mut stack: Vec<String> = block_g.nodes();
        let mut visited: HashMap<String, bool> = HashMap::new();
        while let Some(elem) = stack.pop() {
            if *visited.get(&elem).unwrap_or(&false) {
                let out_edges = block_g.out_edges(&elem).unwrap_or_default();
                let min_val = out_edges.iter().fold(f64::INFINITY, |acc, e| {
                    let xs_w = xs.get(&e.w).cloned().unwrap_or(0.0);
                    let edge_weight = block_g.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                    acc.min(xs_w - edge_weight)
                });
                let node_border_type = graph
                    .node_opt(&elem)
                    .and_then(|n| n.border_type.as_deref().map(|s| s.to_string()));
                if min_val != f64::INFINITY && node_border_type.as_deref() != Some(border_type) {
                    let cur = xs.get(&elem).cloned().unwrap_or(0.0);
                    xs.insert(elem.to_string(), cur.max(min_val));
                }
            } else {
                visited.insert(elem.clone(), true);
                stack.push(elem.clone());
                let succs = block_g.successors(&elem).unwrap_or_default();
                for s in succs {
                    stack.push(s);
                }
            }
        }
    }

    // Assign x coordinates to all nodes
    for v in align.keys() {
        let root_v_real = root.get(v).cloned().unwrap_or_default();
        let val = xs.get(&root_v_real).cloned().unwrap_or(0.0);
        xs.insert(v.clone(), val);
    }

    xs
}

fn build_block_graph(
    graph: &Graph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    reverse_sep: bool,
) -> Graph {
    let mut block_graph = Graph::with_options(true, false, false);
    let graph_label = graph.graph();
    let nodesep = graph_label.nodesep.unwrap_or(50.0);
    let edgesep = graph_label.edgesep.unwrap_or(20.0);

    for layer in layering {
        let mut u: Option<String> = None;
        for v in layer {
            // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
            if v.is_empty() {
                continue;
            }
            let v_root = match root.get(v) {
                Some(r) => r.clone(),
                None => continue,
            };
            block_graph.set_node_default(&v_root);
            if let Some(ref u_str) = u {
                let u_root = match root.get(u_str) {
                    Some(r) => r.clone(),
                    None => {
                        u = Some(v.clone());
                        continue;
                    }
                };
                let sep_val = sep(graph, v, u_str, nodesep, edgesep, reverse_sep);
                let prev_max = block_graph
                    .edge_vw(&u_root, &v_root)
                    .and_then(|e| e.weight)
                    .unwrap_or(0.0);
                block_graph.set_edge(
                    &u_root,
                    &v_root,
                    EdgeLabel {
                        weight: Some(sep_val.max(prev_max)),
                        ..Default::default()
                    },
                    None,
                );
            }
            u = Some(v.clone());
        }
    }

    block_graph
}

/// Returns the alignment that has the smallest width.
pub fn find_smallest_width_alignment(
    graph: &Graph,
    xss: &HashMap<String, PositionMap>,
) -> PositionMap {
    let mut best: (f64, Option<&PositionMap>) = (f64::INFINITY, None);

    for xs in xss.values() {
        let mut max_val = f64::NEG_INFINITY;
        let mut min_val = f64::INFINITY;
        for (v, &x) in xs {
            let half_width = graph.node_opt(v).map_or(0.0, |n| n.width / 2.0);
            max_val = max_val.max(x + half_width);
            min_val = min_val.min(x - half_width);
        }
        let span = max_val - min_val;
        if span < best.0 {
            best = (span, Some(xs));
        }
    }

    best.1.cloned().unwrap_or_default()
}

/// Aligns coordinates.
pub fn align_coordinates(xss: &mut HashMap<String, PositionMap>, align_to: &PositionMap) {
    let align_to_vals: Vec<f64> = align_to.values().cloned().collect();
    let align_to_min = apply_min(&align_to_vals);
    let align_to_max = apply_max(&align_to_vals);

    for vert in &["u", "d"] {
        for horiz in &["l", "r"] {
            let alignment = format!("{}{}", vert, horiz);
            let xs_ptr = xss.get(&alignment).cloned();
            if let Some(xs) = xs_ptr {
                // Skip if this is the align_to
                if xs.iter().all(|(k, v)| align_to.get(k) == Some(v)) {
                    continue;
                }
                let xs_vals: Vec<f64> = xs.values().cloned().collect();
                let delta = if *horiz == "l" {
                    align_to_min - apply_min(&xs_vals)
                } else {
                    align_to_max - apply_max(&xs_vals)
                };
                if delta != 0.0 {
                    let new_xs: PositionMap = xs.into_iter().map(|(k, v)| (k, v + delta)).collect();
                    xss.insert(alignment, new_xs);
                }
            }
        }
    }
}

/// Balance the four alignments.
pub fn balance(xss: &HashMap<String, PositionMap>, align: Option<&str>) -> PositionMap {
    let ul_map = match xss.get("ul") {
        Some(m) => m,
        None => return HashMap::new(),
    };

    ul_map
        .keys()
        .map(|v| {
            let x = if let Some(a) = align {
                let key = a.to_lowercase();
                xss.get(&key).and_then(|m| m.get(v)).cloned().unwrap_or(0.0)
            } else {
                let mut xs: Vec<f64> = xss.values().filter_map(|m| m.get(v)).cloned().collect();
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                // median of 4 values: average of indices 1 and 2
                let v1 = xs.get(1).cloned().unwrap_or(0.0);
                let v2 = xs.get(2).cloned().unwrap_or(0.0);
                (v1 + v2) / 2.0
            };
            (v.clone(), x)
        })
        .collect()
}

/// Top-level positionX function.
pub fn position_x(graph: &Graph) -> PositionMap {
    let layering = build_layer_matrix(graph);

    let mut conflicts = find_type1_conflicts(graph, &layering);
    let type2 = find_type2_conflicts(graph, &layering);
    // Merge type2 into conflicts
    for (v, ws) in type2 {
        let entry = conflicts.entry(v).or_default();
        for (w, val) in ws {
            entry.insert(w, val);
        }
    }

    let mut xss: HashMap<String, PositionMap> = HashMap::new();
    let mut adjusted_layering: Vec<Vec<String>>;

    for vert in &["u", "d"] {
        if *vert == "u" {
            adjusted_layering = layering.clone();
        } else {
            adjusted_layering = layering.iter().rev().cloned().collect();
        }

        for horiz in &["l", "r"] {
            if *horiz == "r" {
                adjusted_layering = adjusted_layering
                    .iter()
                    .map(|inner| inner.iter().rev().cloned().collect())
                    .collect();
            }

            let neighbor_fn: NeighborFn<'_> = if *vert == "u" {
                Box::new(|v: &str| graph.predecessors(v).unwrap_or_default())
            } else {
                Box::new(|v: &str| graph.successors(v).unwrap_or_default())
            };

            let (root, align) =
                vertical_alignment(graph, &adjusted_layering, &conflicts, &*neighbor_fn);

            let mut xs =
                horizontal_compaction(graph, &adjusted_layering, &root, &align, *horiz == "r");

            if *horiz == "r" {
                let new_xs: PositionMap = xs.into_iter().map(|(k, v)| (k, -v)).collect();
                xs = new_xs;
            }

            xss.insert(format!("{}{}", vert, horiz), xs);
        }
    }

    let smallest_width = find_smallest_width_alignment(graph, &xss);
    align_coordinates(&mut xss, &smallest_width);
    let graph_align = graph.graph().align.as_deref().map(|s| s.to_string());
    balance(&xss, graph_align.as_deref())
}

fn sep(graph: &Graph, v: &str, w: &str, nodesep: f64, edgesep: f64, reverse_sep: bool) -> f64 {
    let v_label = graph.node(v);
    let w_label = graph.node(w);
    let mut sum = 0.0;

    sum += v_label.width / 2.0;
    if v_label.labelpos.is_some() {
        let mut delta: Option<f64> = None;
        match v_label
            .labelpos
            .as_deref()
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("l") => delta = Some(-v_label.width / 2.0),
            Some("r") => delta = Some(v_label.width / 2.0),
            _ => {}
        }
        if let Some(d) = delta {
            sum += if reverse_sep { d } else { -d };
        }
    }

    sum += (if v_label.dummy.is_some() {
        edgesep
    } else {
        nodesep
    }) / 2.0;
    sum += (if w_label.dummy.is_some() {
        edgesep
    } else {
        nodesep
    }) / 2.0;

    sum += w_label.width / 2.0;
    if w_label.labelpos.is_some() {
        let mut delta: Option<f64> = None;
        match w_label
            .labelpos
            .as_deref()
            .map(|s| s.to_lowercase())
            .as_deref()
        {
            Some("l") => delta = Some(w_label.width / 2.0),
            Some("r") => delta = Some(-w_label.width / 2.0),
            _ => {}
        }
        if let Some(d) = delta {
            sum += if reverse_sep { d } else { -d };
        }
    }

    sum
}
