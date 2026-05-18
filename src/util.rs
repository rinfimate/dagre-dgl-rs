use crate::graph::{EdgeLabel, Graph, NodeLabel, Point};
/// util.rs — Faithful port of dagre-js/lib/util.ts
use std::collections::HashMap;

/// The root sentinel node ID used by compound graphs.
pub const GRAPH_NODE: &str = "\x00";

// ─── uniqueId ────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicU64, Ordering};
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique string ID with the given prefix.
pub fn unique_id(prefix: &str) -> String {
    let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{}{}", prefix, id)
}

/// Reset the global ID counter to zero (use between diagrams for determinism).
pub fn reset_id_counter() {
    ID_COUNTER.store(0, Ordering::Relaxed);
}

// ─── addDummyNode ─────────────────────────────────────────────────────────────

/// Add a dummy node of the given type to the graph, returning its ID.
pub fn add_dummy_node(
    graph: &mut Graph,
    node_type: &str,
    mut attrs: NodeLabel,
    name: &str,
) -> String {
    let mut v = name.to_string();
    while graph.has_node(&v) {
        v = unique_id(name);
    }
    attrs.dummy = Some(node_type.to_string());
    graph.set_node(&v, attrs);
    v
}

// ─── addBorderNode (nesting-graph variant) ────────────────────────────────────

/// Used in nesting-graph.ts: addBorderNode(graph, prefix)
pub fn add_border_node(graph: &mut Graph, prefix: &str) -> String {
    let node = NodeLabel {
        width: 0.0,
        height: 0.0,
        ..Default::default()
    };
    add_dummy_node(graph, "border", node, prefix)
}

// ─── simplify ─────────────────────────────────────────────────────────────────

/// Returns a new graph with only simple edges.
pub fn simplify(graph: &Graph) -> Graph {
    let mut simplified = Graph::with_options(graph.is_directed, false, false);
    simplified.set_graph(graph.graph().clone());
    for v in graph.nodes() {
        simplified.set_node(&v, graph.node(&v).clone());
    }
    for e in graph.edges() {
        let label = graph.edge(&e).unwrap();
        let new_weight;
        let new_minlen;
        if let Some(existing) = simplified.edge_vw(&e.v, &e.w) {
            new_weight = existing.weight.unwrap_or(0.0) + label.weight.unwrap_or(0.0);
            new_minlen = existing.minlen.unwrap_or(1).max(label.minlen.unwrap_or(1));
        } else {
            new_weight = label.weight.unwrap_or(0.0);
            new_minlen = label.minlen.unwrap_or(1);
        }
        simplified.set_edge(
            &e.v,
            &e.w,
            EdgeLabel {
                weight: Some(new_weight),
                minlen: Some(new_minlen),
                ..Default::default()
            },
            None,
        );
    }
    simplified
}

// ─── asNonCompoundGraph ───────────────────────────────────────────────────────

/// Create a copy of `graph` with all compound (parent) nodes removed.
pub fn as_non_compound_graph(graph: &Graph) -> Graph {
    let mut simplified = Graph::with_options(graph.is_directed, graph.is_multigraph, false);
    simplified.set_graph(graph.graph().clone());
    for v in graph.nodes() {
        if graph.children(&v).is_empty() {
            simplified.set_node(&v, graph.node(&v).clone());
        }
    }
    for e in graph.edges() {
        let label = graph.edge(&e).unwrap().clone();
        simplified.set_edge_obj(&e, label);
    }
    simplified
}

// ─── buildLayerMatrix ─────────────────────────────────────────────────────────

/// Build a 2-D matrix of node IDs indexed by `[rank][order]`.
pub fn build_layer_matrix(graph: &Graph) -> Vec<Vec<String>> {
    let mr = max_rank(graph);
    if mr < 0 {
        return Vec::new();
    }
    let size = (mr + 1) as usize;
    let mut layering: Vec<Vec<String>> = vec![Vec::new(); size];
    for v in graph.nodes() {
        let node = graph.node(&v);
        if let Some(rank) = node.rank {
            let rank_idx = rank as usize;
            if rank_idx >= layering.len() {
                layering.resize(rank_idx + 1, Vec::new());
            }
            let order = node.order.unwrap_or(0) as usize;
            let layer = &mut layering[rank_idx];
            if order >= layer.len() {
                layer.resize(order + 1, String::new());
            }
            layer[order] = v;
        }
    }
    // DO NOT remove empty strings — downstream code relies on index positions
    // (empty string = undefined slot, matching JS sparse array behaviour)
    layering
}

// ─── normalizeRanks ───────────────────────────────────────────────────────────

/// Shift all node ranks so the minimum rank is 0.
pub fn normalize_ranks(graph: &mut Graph) {
    let min_rank = graph
        .nodes()
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .min()
        .unwrap_or(0);
    let node_ids: Vec<String> = graph.nodes();
    for v in node_ids {
        let node = graph.node_mut(&v);
        if let Some(r) = node.rank.as_mut() {
            *r -= min_rank;
        }
    }
}

// ─── removeEmptyRanks ────────────────────────────────────────────────────────

/// Remove empty rank layers, compacting node ranks toward zero.
pub fn remove_empty_ranks(graph: &mut Graph) {
    let node_ranks: Vec<i32> = graph
        .nodes()
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .collect();
    if node_ranks.is_empty() {
        return;
    }
    let offset = *node_ranks.iter().min().unwrap();
    let node_rank_factor = graph.graph().node_rank_factor.unwrap_or(1);

    let max_adj = node_ranks.iter().map(|r| r - offset).max().unwrap_or(0) as usize;
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_adj + 1];
    for v in graph.nodes() {
        // Skip nodes with no rank (e.g. border dummy nodes added by nesting_graph)
        if let Some(rank) = graph.node(&v).rank {
            let adj = (rank - offset) as usize;
            layers[adj].push(v);
        }
    }

    let mut delta: i32 = 0;
    for (i, vs) in layers.iter().enumerate() {
        if vs.is_empty() && (i as i32) % node_rank_factor != 0 {
            delta -= 1;
        } else if !vs.is_empty() && delta != 0 {
            for v in vs {
                if let Some(r) = graph.node_mut(v).rank.as_mut() {
                    *r += delta;
                }
            }
        }
    }
}

// ─── successorWeights / predecessorWeights ────────────────────────────────────

/// Build a map from each node to the summed edge weights toward each successor.
pub fn successor_weights(graph: &Graph) -> HashMap<String, HashMap<String, f64>> {
    let mut result = HashMap::new();
    for v in graph.nodes() {
        let mut sucs: HashMap<String, f64> = HashMap::new();
        if let Some(out_edges) = graph.out_edges(&v) {
            for e in out_edges {
                let w = graph.edge(&e).map_or(0.0, |l| l.weight.unwrap_or(0.0));
                *sucs.entry(e.w.clone()).or_insert(0.0) += w;
            }
        }
        result.insert(v, sucs);
    }
    result
}

/// Build a map from each node to the summed edge weights from each predecessor.
pub fn predecessor_weights(graph: &Graph) -> HashMap<String, HashMap<String, f64>> {
    let mut result = HashMap::new();
    for v in graph.nodes() {
        let mut preds: HashMap<String, f64> = HashMap::new();
        if let Some(in_edges) = graph.in_edges(&v) {
            for e in in_edges {
                let w = graph.edge(&e).map_or(0.0, |l| l.weight.unwrap_or(0.0));
                *preds.entry(e.v.clone()).or_insert(0.0) += w;
            }
        }
        result.insert(v, preds);
    }
    result
}

// ─── intersectNode ───────────────────────────────────────────────────────────

/// Dispatch to the correct boundary-intersection function based on node shape.
pub fn intersect_node(node: &NodeLabel, point: &Point) -> Point {
    match node.intersect_type {
        Some("diamond") => intersect_diamond(node, point),
        Some("circle") => intersect_ellipse(node, point),
        _ => intersect_rect(node, point),
    }
}

/// Diamond intersection: |dx/hw| + |dy/hh| = 1  →  t = 1 / (|dx|/hw + |dy|/hh)
fn intersect_diamond(node: &NodeLabel, point: &Point) -> Point {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let dx = point.x - x;
    let dy = point.y - y;
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;
    if dx == 0.0 && dy == 0.0 {
        return Point { x, y };
    }
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh);
    Point {
        x: x + dx * t,
        y: y + dy * t,
    }
}

/// Ellipse/circle intersection: (dx/hw)² + (dy/hh)² = 1
fn intersect_ellipse(node: &NodeLabel, point: &Point) -> Point {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let dx = point.x - x;
    let dy = point.y - y;
    let rx = node.width / 2.0;
    let ry = node.height / 2.0;
    if dx == 0.0 && dy == 0.0 {
        return Point { x, y };
    }
    let t = 1.0 / ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
    Point {
        x: x + dx * t,
        y: y + dy * t,
    }
}

// ─── intersectRect ───────────────────────────────────────────────────────────

/// Compute the point where the line from `rect`'s centre to `point` crosses the rect boundary.
pub fn intersect_rect(rect: &NodeLabel, point: &Point) -> Point {
    let x = rect.x.unwrap_or(0.0);
    let y = rect.y.unwrap_or(0.0);
    let dx = point.x - x;
    let dy = point.y - y;
    let mut w = rect.width / 2.0;
    let mut h = rect.height / 2.0;

    if dx == 0.0 && dy == 0.0 {
        // Edge endpoint coincides with node center; return center as fallback.
        return Point { x, y };
    }

    let (sx, sy);
    if dy.abs() * w > dx.abs() * h {
        if dy < 0.0 {
            h = -h;
        }
        sx = if dy != 0.0 { h * dx / dy } else { 0.0 };
        sy = h;
    } else {
        if dx < 0.0 {
            w = -w;
        }
        sx = w;
        sy = if dx != 0.0 { w * dy / dx } else { 0.0 };
    }

    Point {
        x: x + sx,
        y: y + sy,
    }
}

// ─── maxRank ─────────────────────────────────────────────────────────────────

/// Return the maximum rank assigned to any node in the graph.
pub fn max_rank(graph: &Graph) -> i32 {
    graph
        .nodes()
        .iter()
        .filter_map(|v| graph.node(v).rank)
        .max()
        .unwrap_or(i32::MIN)
}

// ─── range ───────────────────────────────────────────────────────────────────

/// range(limit) -> 0..limit
/// range(start, limit) -> start..limit
/// range(start, limit, step) -> start..limit step
pub fn range(start: i32, limit: Option<i32>, step: Option<i32>) -> Vec<i32> {
    let step = step.unwrap_or(1);
    let (actual_start, actual_limit) = match limit {
        Some(lim) => (start, lim),
        None => (0, start),
    };
    let mut v = Vec::new();
    if step > 0 {
        let mut i = actual_start;
        while i < actual_limit {
            v.push(i);
            i += step;
        }
    } else if step < 0 {
        let mut i = actual_start;
        while i > actual_limit {
            v.push(i);
            i += step;
        }
    }
    v
}

// ─── partition ───────────────────────────────────────────────────────────────

/// Result of splitting a collection into two groups by a predicate.
pub struct PartitionResult<T> {
    /// Items for which the predicate returned `true`.
    pub lhs: Vec<T>,
    /// Items for which the predicate returned `false`.
    pub rhs: Vec<T>,
}

/// Split `collection` into two groups based on predicate `f`.
pub fn partition<T, F: Fn(&T) -> bool>(collection: Vec<T>, f: F) -> PartitionResult<T> {
    let mut lhs = Vec::new();
    let mut rhs = Vec::new();
    for item in collection {
        if f(&item) {
            lhs.push(item);
        } else {
            rhs.push(item);
        }
    }
    PartitionResult { lhs, rhs }
}

// ─── mapValues ───────────────────────────────────────────────────────────────

/// Transform each value in a `HashMap` using `f(value, key)`, consuming the map.
pub fn map_values<V, R, F: Fn(V, &str) -> R>(obj: HashMap<String, V>, f: F) -> HashMap<String, R> {
    obj.into_iter()
        .map(|(k, v)| {
            let r = f(v, &k);
            (k, r)
        })
        .collect()
}

/// Transform each value in a `HashMap` by reference using `f(value, key)`.
pub fn map_values_ref<V: Clone, R, F: Fn(&V, &str) -> R>(
    obj: &HashMap<String, V>,
    f: F,
) -> HashMap<String, R> {
    obj.iter()
        .map(|(k, v)| {
            let r = f(v, k);
            (k.clone(), r)
        })
        .collect()
}

// ─── zipObject ───────────────────────────────────────────────────────────────

/// Zip parallel `keys` and `values` slices into a `HashMap`.
pub fn zip_object<V: Clone>(keys: &[String], values: &[V]) -> HashMap<String, V> {
    keys.iter()
        .enumerate()
        .filter_map(|(i, k)| values.get(i).map(|v| (k.clone(), v.clone())))
        .collect()
}

// ─── applyWithChunking ───────────────────────────────────────────────────────

const CHUNKING_THRESHOLD: usize = 65535;

/// Return the minimum value in `arr`, using chunking for large slices.
pub fn apply_min(arr: &[f64]) -> f64 {
    if arr.is_empty() {
        return f64::INFINITY;
    }
    if arr.len() > CHUNKING_THRESHOLD {
        let chunks: Vec<f64> = arr.chunks(CHUNKING_THRESHOLD).map(apply_min).collect();
        apply_min(&chunks)
    } else {
        arr.iter().cloned().fold(f64::INFINITY, f64::min)
    }
}

/// Return the maximum value in `arr`, using chunking for large slices.
pub fn apply_max(arr: &[f64]) -> f64 {
    if arr.is_empty() {
        return f64::NEG_INFINITY;
    }
    if arr.len() > CHUNKING_THRESHOLD {
        let chunks: Vec<f64> = arr.chunks(CHUNKING_THRESHOLD).map(apply_max).collect();
        apply_max(&chunks)
    } else {
        arr.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }
}

/// Return the minimum `i32` in `arr`, or `i32::MAX` if empty.
pub fn apply_min_i32(arr: &[i32]) -> i32 {
    arr.iter().cloned().min().unwrap_or(i32::MAX)
}

/// Return the maximum `i32` in `arr`, or `i32::MIN` if empty.
pub fn apply_max_i32(arr: &[i32]) -> i32 {
    arr.iter().cloned().max().unwrap_or(i32::MIN)
}

/// Build a cubic bezier path string through a list of points.
/// Used for edge routing output.
pub fn points_to_path(points: &[(f64, f64)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        return format!("M {:.1} {:.1}", points[0].0, points[0].1);
    }

    let mut path = format!("M {:.1} {:.1}", points[0].0, points[0].1);
    if points.len() == 2 {
        path.push_str(&format!(" L {:.1} {:.1}", points[1].0, points[1].1));
        return path;
    }

    for i in 1..points.len() - 1 {
        let p0 = points[i - 1];
        let p1 = points[i];
        let p2 = points[i + 1];
        let cp1x = p1.0 - (p2.0 - p0.0) / 6.0;
        let cp1y = p1.1 - (p2.1 - p0.1) / 6.0;
        let cp2x = p1.0 + (p2.0 - p0.0) / 6.0;
        let cp2y = p1.1 + (p2.1 - p0.1) / 6.0;
        path.push_str(&format!(
            " C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            cp1x, cp1y, cp2x, cp2y, p1.0, p1.1
        ));
    }

    let last = points.last().unwrap();
    path.push_str(&format!(" L {:.1} {:.1}", last.0, last.1));
    path
}

/// Clamp `v` to the range `[min, max]`.
pub fn clamp(v: f64, min: f64, max: f64) -> f64 {
    v.max(min).min(max)
}
