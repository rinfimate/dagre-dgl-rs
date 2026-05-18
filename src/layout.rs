use crate::acyclic;
use crate::add_border_segments::add_border_segments;
use crate::coordinate_system;
use crate::graph::{Edge, EdgeLabel, Graph, GraphLabel, NodeLabel, Point, SelfEdge};
use crate::nesting_graph;
use crate::normalize;
use crate::order::order;
use crate::parent_dummy_chains::parent_dummy_chains;
use crate::position::position;
use crate::rank::rank;
use crate::util::{
    add_dummy_node, as_non_compound_graph, build_layer_matrix, intersect_node, normalize_ranks,
    remove_empty_ranks,
};

// ─── Layout option defaults ──────────────────────────────────────────────────
// These document the attribute whitelists from the JS source.
#[allow(dead_code)]
const GRAPH_NUM_ATTRS: &[&str] = &["nodesep", "edgesep", "ranksep", "marginx", "marginy"];
#[allow(dead_code)]
const GRAPH_ATTRS: &[&str] = &["acyclicer", "ranker", "rankdir", "align", "rankalign"];
#[allow(dead_code)]
const NODE_NUM_ATTRS: &[&str] = &["width", "height", "rank"];
#[allow(dead_code)]
const EDGE_NUM_ATTRS: &[&str] = &["minlen", "weight", "width", "height", "labeloffset"];
#[allow(dead_code)]
const EDGE_ATTRS: &[&str] = &["labelpos"];

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the full dagre layout pipeline on graph `g`.
///
/// This is the main entry point for the crate.  It mutates `g` in place,
/// populating:
///
/// - [`NodeLabel::x`] and [`NodeLabel::y`] — computed center coordinates for
///   every node.
/// - [`EdgeLabel::points`] — the sequence of bend-points defining each edge's
///   route, in the layout coordinate system.
/// - [`GraphLabel::width`] and [`GraphLabel::height`] — overall bounding box of
///   the laid-out graph.
///
/// The layout direction and spacing are controlled through the graph-level label
/// set via [`Graph::set_graph`].
///
/// # Example
///
/// ```rust
/// use dagre_dgl_rs::{Graph, GraphLabel, NodeLabel, EdgeLabel, layout};
///
/// let mut g = Graph::default();
/// g.set_graph(GraphLabel {
///     rankdir: Some("LR".to_string()),
///     nodesep: Some(50.0),
///     ranksep: Some(60.0),
///     ..Default::default()
/// });
///
/// g.set_node("a", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
/// g.set_node("b", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
/// g.set_node("c", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
/// g.set_edge("a", "b", EdgeLabel::default(), None);
/// g.set_edge("b", "c", EdgeLabel::default(), None);
///
/// layout(&mut g);
///
/// // After layout, every node has computed coordinates.
/// assert!(g.node("a").x.is_some());
/// assert!(g.node("a").y.is_some());
///
/// // Every edge has a bend-point sequence.
/// let ab = dagre_dgl_rs::Edge::new("a", "b");
/// assert!(g.edge(&ab).unwrap().points.is_some());
/// ```
pub fn layout(g: &mut Graph) {
    let layout_graph = build_layout_graph(g);
    let mut layout_graph = layout_graph;
    run_layout(&mut layout_graph);
    update_input_graph(g, &layout_graph);
}

fn run_layout(g: &mut Graph) {
    make_space_for_edge_labels(g);
    remove_self_edges(g);
    acyclic::run(g);
    nesting_graph::run(g);
    {
        let mut nc = as_non_compound_graph(g);
        rank(&mut nc);
        // Copy ranks back
        for v in nc.nodes() {
            if let Some(r) = nc.node(&v).rank {
                if let Some(n) = g.node_opt_mut(&v) {
                    n.rank = Some(r);
                }
            }
        }
    }
    inject_edge_label_proxies(g);
    remove_empty_ranks(g);
    nesting_graph::cleanup(g);
    normalize_ranks(g);
    assign_rank_min_max(g);
    remove_edge_label_proxies(g);
    normalize::run(g);
    parent_dummy_chains(g);
    add_border_segments(g);
    order(g, &[], false);
    insert_self_edges(g);
    coordinate_system::adjust(g);
    position(g);
    position_self_edges(g);
    remove_border_nodes(g);
    normalize::undo(g);
    fixup_edge_label_coords(g);
    coordinate_system::undo(g);
    translate_graph(g);
    assign_node_intersects(g);
    reverse_points_for_reversed_edges(g);
    acyclic::undo(g);
}

// ─── buildLayoutGraph ────────────────────────────────────────────────────────

fn build_layout_graph(input_graph: &Graph) -> Graph {
    let mut g = Graph::with_options(true, true, true);

    let graph = canonicalize_obj(input_graph.graph());
    let new_label = GraphLabel {
        ranksep: graph.ranksep.or(Some(50.0)),
        edgesep: graph.edgesep.or(Some(20.0)),
        nodesep: graph.nodesep.or(Some(50.0)),
        rankdir: graph.rankdir.clone().or_else(|| Some("TB".to_string())),
        rankalign: graph
            .rankalign
            .clone()
            .or_else(|| Some("center".to_string())),
        marginx: graph.marginx,
        marginy: graph.marginy,
        acyclicer: graph.acyclicer.clone(),
        ranker: graph.ranker.clone(),
        align: graph.align.clone(),
        ..Default::default()
    };

    g.set_graph(new_label);

    for v in input_graph.nodes() {
        let node = canonicalize_node(input_graph.node(&v));
        let new_node = NodeLabel {
            width: node.width,
            height: node.height,
            intersect_type: node.intersect_type,
            rank: node.rank,
            ..Default::default()
        };

        g.set_node(&v, new_node);
        if let Some(parent) = input_graph.parent(&v) {
            g.set_parent(&v, Some(parent));
        }
    }

    for e in input_graph.edges() {
        let edge = input_graph.edge(&e).cloned().unwrap_or_default();
        let edge = canonicalize_edge(&edge);
        let new_edge = EdgeLabel {
            minlen: Some(edge.minlen.unwrap_or(1)),
            weight: Some(edge.weight.unwrap_or(1.0)),
            width: Some(edge.width.unwrap_or(0.0)),
            height: Some(edge.height.unwrap_or(0.0)),
            labeloffset: Some(edge.labeloffset.unwrap_or(10.0)),
            labelpos: edge.labelpos.clone().or_else(|| Some("c".to_string())),
            ..Default::default()
        };
        g.set_edge_obj(&e, new_edge);
    }

    g
}

// Canonicalize: lowercase all string keys.
// Since we have typed structs, we just apply the lowercase transformation
// to specific string fields.
fn canonicalize_obj(label: &GraphLabel) -> GraphLabel {
    GraphLabel {
        rankdir: label.rankdir.as_ref().map(|s| s.to_lowercase()),
        align: label.align.as_ref().map(|s| s.to_lowercase()),
        acyclicer: label.acyclicer.clone(),
        ranker: label.ranker.clone(),
        rankalign: label.rankalign.clone(),
        nodesep: label.nodesep,
        edgesep: label.edgesep,
        ranksep: label.ranksep,
        marginx: label.marginx,
        marginy: label.marginy,
        width: label.width,
        height: label.height,
        compound: label.compound,
        nesting_root: label.nesting_root.clone(),
        node_rank_factor: label.node_rank_factor,
        dummy_chains: label.dummy_chains.clone(),
        max_rank: label.max_rank,
        root: label.root.clone(),
    }
}

fn canonicalize_node(node: &NodeLabel) -> NodeLabel {
    node.clone()
}

fn canonicalize_edge(edge: &EdgeLabel) -> EdgeLabel {
    EdgeLabel {
        labelpos: edge.labelpos.as_ref().map(|s| s.to_lowercase()),
        ..edge.clone()
    }
}

// ─── makeSpaceForEdgeLabels ───────────────────────────────────────────────────

fn make_space_for_edge_labels(g: &mut Graph) {
    {
        let rs = g.graph_mut().ranksep.get_or_insert(50.0);
        *rs /= 2.0;
    }
    let rankdir = g
        .graph()
        .rankdir
        .clone()
        .unwrap_or_else(|| "TB".to_string());
    let edges: Vec<Edge> = g.edges();
    for e in edges {
        if let Some(label) = g.edge_mut(&e) {
            if let Some(ml) = label.minlen.as_mut() {
                *ml *= 2;
            }
            let lp = label.labelpos.clone().unwrap_or_else(|| "r".to_string());
            if lp.to_lowercase() != "c" {
                let lo = label.labeloffset.unwrap_or(10.0);
                if rankdir.to_uppercase() == "TB" || rankdir.to_uppercase() == "BT" {
                    if let Some(w) = label.width.as_mut() {
                        *w += lo;
                    }
                } else {
                    if let Some(h) = label.height.as_mut() {
                        *h += lo;
                    }
                }
            }
        }
    }
}

// ─── removeSelfEdges ─────────────────────────────────────────────────────────

fn remove_self_edges(g: &mut Graph) {
    let self_edges: Vec<Edge> = g.edges().into_iter().filter(|e| e.v == e.w).collect();
    for e in self_edges {
        let label = g.edge(&e).cloned().unwrap_or_default();
        let node = g.node_mut(&e.v);
        if node.self_edges.is_none() {
            node.self_edges = Some(Vec::new());
        }
        node.self_edges.as_mut().unwrap().push(SelfEdge {
            e: e.clone(),
            label,
        });
        g.remove_edge_obj(&e);
    }
}

// ─── injectEdgeLabelProxies ───────────────────────────────────────────────────

fn inject_edge_label_proxies(g: &mut Graph) {
    let edges: Vec<Edge> = g.edges();
    for e in edges {
        let (label_w, label_h, v_rank, w_rank) = {
            let label = g.edge(&e).cloned().unwrap_or_default();
            let w = label.width.unwrap_or(0.0);
            let h = label.height.unwrap_or(0.0);
            if w == 0.0 || h == 0.0 {
                continue;
            }
            let v_rank = g.node(&e.v).rank.unwrap_or(0) as f64;
            let w_rank = g.node(&e.w).rank.unwrap_or(0) as f64;
            (w, h, v_rank, w_rank)
        };
        let label_rank = (w_rank - v_rank) / 2.0 + v_rank;
        let attrs = NodeLabel {
            rank: Some(label_rank as i32),
            e: Some(e.clone()),
            width: label_w,
            height: label_h,
            ..Default::default()
        };
        add_dummy_node(g, "edge-proxy", attrs, "_ep");
    }
}

// ─── assignRankMinMax ────────────────────────────────────────────────────────

fn assign_rank_min_max(g: &mut Graph) {
    let mut max_rank = 0i32;
    let nodes: Vec<String> = g.nodes();
    for v in &nodes {
        let node = g.node(v).clone();
        if node.border_top.is_some() {
            let min_rank = node
                .border_top
                .as_ref()
                .and_then(|bt| g.node(bt).rank)
                .unwrap_or(0);
            let max_rank_v = node
                .border_bottom
                .as_ref()
                .and_then(|bb| g.node(bb).rank)
                .unwrap_or(0);
            g.node_mut(v).min_rank = Some(min_rank);
            g.node_mut(v).max_rank = Some(max_rank_v);
            if max_rank_v > max_rank {
                max_rank = max_rank_v;
            }
        }
    }
    g.graph_mut().max_rank = Some(max_rank);
}

// ─── removeEdgeLabelProxies ───────────────────────────────────────────────────

fn remove_edge_label_proxies(g: &mut Graph) {
    let proxy_nodes: Vec<String> = g
        .nodes()
        .into_iter()
        .filter(|v| g.node(v).dummy.as_deref() == Some("edge-proxy"))
        .collect();
    for v in proxy_nodes {
        let node = g.node(&v).clone();
        let rank = node.rank.unwrap_or(0);
        if let Some(e_ref) = node.e.as_ref() {
            let e = e_ref.clone();
            if let Some(label) = g.edge_mut(&e) {
                label.label_rank = Some(rank);
            }
        }
        g.remove_node(&v);
    }
}

// ─── translateGraph ───────────────────────────────────────────────────────────

fn translate_graph(g: &mut Graph) {
    let mut min_x = f64::INFINITY;
    let mut max_x = 0.0f64;
    let mut min_y = f64::INFINITY;
    let mut max_y = 0.0f64;
    let margin_x = g.graph().marginx.unwrap_or(0.0);
    let margin_y = g.graph().marginy.unwrap_or(0.0);

    // Collect extremes
    for v in g.nodes() {
        let node = g.node(&v);
        let x = node.x.unwrap_or(0.0);
        let y = node.y.unwrap_or(0.0);
        let w = node.width / 2.0;
        let h = node.height / 2.0;
        min_x = min_x.min(x - w);
        max_x = max_x.max(x + w);
        min_y = min_y.min(y - h);
        max_y = max_y.max(y + h);
    }
    for e in g.edges() {
        if let Some(label) = g.edge(&e) {
            if label.x.is_some() {
                let x = label.x.unwrap_or(0.0);
                let y = label.y.unwrap_or(0.0);
                let w = label.width.unwrap_or(0.0) / 2.0;
                let h = label.height.unwrap_or(0.0) / 2.0;
                min_x = min_x.min(x - w);
                max_x = max_x.max(x + w);
                min_y = min_y.min(y - h);
                max_y = max_y.max(y + h);
            }
        }
    }

    min_x -= margin_x;
    min_y -= margin_y;

    for v in g.nodes() {
        let node = g.node_mut(&v);
        if let Some(x) = node.x.as_mut() {
            *x -= min_x;
        }
        if let Some(y) = node.y.as_mut() {
            *y -= min_y;
        }
    }

    let edges: Vec<Edge> = g.edges();
    for e in edges {
        if let Some(label) = g.edge_mut(&e) {
            if let Some(points) = label.points.as_mut() {
                for p in points.iter_mut() {
                    p.x -= min_x;
                    p.y -= min_y;
                }
            }
            if label.x.is_some() {
                if let Some(x) = label.x.as_mut() {
                    *x -= min_x;
                }
            }
            if label.y.is_some() {
                if let Some(y) = label.y.as_mut() {
                    *y -= min_y;
                }
            }
        }
    }

    {
        let gl = g.graph_mut();
        gl.width = Some(max_x - min_x + margin_x);
        gl.height = Some(max_y - min_y + margin_y);
    }
}

// ─── assignNodeIntersects ────────────────────────────────────────────────────

fn assign_node_intersects(g: &mut Graph) {
    let edges: Vec<Edge> = g.edges();
    for e in edges {
        let (node_v, node_w) = {
            let nv = g.node(&e.v).clone();
            let nw = g.node(&e.w).clone();
            (nv, nw)
        };

        let label = g.edge(&e).cloned().unwrap_or_default();
        let (p1, p2) = if label.points.as_ref().is_none_or(|p| p.is_empty()) {
            if let Some(label_mut) = g.edge_mut(&e) {
                label_mut.points = Some(Vec::new());
            }
            (
                Point {
                    x: node_w.x.unwrap_or(0.0),
                    y: node_w.y.unwrap_or(0.0),
                },
                Point {
                    x: node_v.x.unwrap_or(0.0),
                    y: node_v.y.unwrap_or(0.0),
                },
            )
        } else {
            let pts = label.points.as_ref().unwrap();
            (pts[0].clone(), pts[pts.len() - 1].clone())
        };

        let p_start = intersect_node(&node_v, &p1);
        let p_end = intersect_node(&node_w, &p2);

        if let Some(label) = g.edge_mut(&e) {
            let pts = label.points.get_or_insert_with(Vec::new);
            pts.insert(0, p_start);
            pts.push(p_end);
        }
    }
}

// ─── fixupEdgeLabelCoords ────────────────────────────────────────────────────

fn fixup_edge_label_coords(g: &mut Graph) {
    let edges: Vec<Edge> = g.edges();
    for e in edges {
        let has_x = g.edge(&e).and_then(|l| l.x).is_some();
        if !has_x {
            continue;
        }
        let labelpos = g
            .edge(&e)
            .and_then(|l| l.labelpos.clone())
            .unwrap_or_default();
        let lo = g.edge(&e).and_then(|l| l.labeloffset).unwrap_or(10.0);

        if labelpos == "l" || labelpos == "r" {
            if let Some(label) = g.edge_mut(&e) {
                if let Some(w) = label.width.as_mut() {
                    *w -= lo;
                }
            }
        }

        match labelpos.as_str() {
            "l" => {
                if let Some(label) = g.edge_mut(&e) {
                    let w = label.width.unwrap_or(0.0);
                    if let Some(x) = label.x.as_mut() {
                        *x -= w / 2.0 + lo;
                    }
                }
            }
            "r" => {
                if let Some(label) = g.edge_mut(&e) {
                    let w = label.width.unwrap_or(0.0);
                    if let Some(x) = label.x.as_mut() {
                        *x += w / 2.0 + lo;
                    }
                }
            }
            _ => {}
        }
    }
}

// ─── reversePointsForReversedEdges ───────────────────────────────────────────

fn reverse_points_for_reversed_edges(g: &mut Graph) {
    let edges: Vec<Edge> = g.edges();
    for e in edges {
        let reversed = g.edge(&e).and_then(|l| l.reversed).unwrap_or(false);
        if reversed {
            if let Some(label) = g.edge_mut(&e) {
                if let Some(pts) = label.points.as_mut() {
                    pts.reverse();
                }
            }
        }
    }
}

// ─── removeBorderNodes ───────────────────────────────────────────────────────

fn remove_border_nodes(g: &mut Graph) {
    // First pass: compute compound node sizes from borders
    let nodes_with_children: Vec<String> = g
        .nodes()
        .into_iter()
        .filter(|v| !g.children(v).is_empty())
        .collect();

    for v in &nodes_with_children {
        let node = g.node(v).clone();
        if let (Some(bt), Some(bb)) = (node.border_top.as_ref(), node.border_bottom.as_ref()) {
            let t_y = g.node(bt).y.unwrap_or(0.0);
            let b_y = g.node(bb).y.unwrap_or(0.0);

            let bl_last = node
                .border_left
                .as_ref()
                .and_then(|v| v.iter().rev().filter_map(|x| x.clone()).next());
            let br_last = node
                .border_right
                .as_ref()
                .and_then(|v| v.iter().rev().filter_map(|x| x.clone()).next());

            if let (Some(bl), Some(br)) = (bl_last, br_last) {
                let l_x = g.node(&bl).x.unwrap_or(0.0);
                let r_x = g.node(&br).x.unwrap_or(0.0);
                let width = (r_x - l_x).abs();
                let height = (b_y - t_y).abs();
                let n = g.node_mut(v);
                n.width = width;
                n.height = height;
                n.x = Some(l_x + width / 2.0);
                n.y = Some(t_y + height / 2.0);
            }
        }
    }

    // Second pass: remove border dummy nodes
    let border_nodes: Vec<String> = g
        .nodes()
        .into_iter()
        .filter(|v| g.node(v).dummy.as_deref() == Some("border"))
        .collect();
    for v in border_nodes {
        g.remove_node(&v);
    }
}

// ─── insertSelfEdges ─────────────────────────────────────────────────────────

fn insert_self_edges(g: &mut Graph) {
    let layers = build_layer_matrix(g);
    for layer in &layers {
        let mut order_shift = 0i32;
        for (i, v) in layer.iter().enumerate() {
            // Skip empty-string slots (sparse layer gaps, equivalent to JS undefined)
            if v.is_empty() {
                continue;
            }
            let node = g.node(v).clone();
            g.node_mut(v).order = Some(i as i32 + order_shift);
            let self_edges = node.self_edges.unwrap_or_default();
            for se in self_edges {
                let _dummy = add_dummy_node(
                    g,
                    "selfedge",
                    NodeLabel {
                        width: se.label.width.unwrap_or(0.0),
                        height: se.label.height.unwrap_or(0.0),
                        rank: node.rank,
                        order: Some(i as i32 + order_shift + 1),
                        edge_obj: Some(se.e.clone()),
                        edge_label: Some(Box::new(se.label.clone())),
                        ..Default::default()
                    },
                    "_se",
                );
                order_shift += 1;
            }
            g.node_mut(v).self_edges = None;
        }
    }
}

// ─── positionSelfEdges ───────────────────────────────────────────────────────

fn position_self_edges(g: &mut Graph) {
    let self_edge_nodes: Vec<String> = g
        .nodes()
        .into_iter()
        .filter(|v| g.node(v).dummy.as_deref() == Some("selfedge"))
        .collect();

    for v in self_edge_nodes {
        let node = g.node(&v).clone();
        let edge_obj = match node.edge_obj.as_ref() {
            Some(e) => e.clone(),
            None => continue,
        };
        let edge_label = match node.edge_label.as_ref() {
            Some(l) => *l.clone(),
            None => continue,
        };

        let self_node = g.node(&edge_obj.v).clone();
        let self_x = self_node.x.unwrap_or(0.0) + self_node.width / 2.0;
        let self_y = self_node.y.unwrap_or(0.0);
        let self_dy = self_node.height / 2.0;

        let node_x = node.x.unwrap_or(0.0);
        let dx = node_x - self_x;

        let mut new_label = edge_label.clone();
        new_label.points = Some(vec![
            Point {
                x: self_x + 2.0 * dx / 3.0,
                y: self_y - self_dy,
            },
            Point {
                x: self_x + 5.0 * dx / 6.0,
                y: self_y - self_dy,
            },
            Point {
                x: self_x + dx,
                y: self_y,
            },
            Point {
                x: self_x + 5.0 * dx / 6.0,
                y: self_y + self_dy,
            },
            Point {
                x: self_x + 2.0 * dx / 3.0,
                y: self_y + self_dy,
            },
        ]);
        new_label.x = Some(node_x);
        new_label.y = Some(node.y.unwrap_or(0.0));

        g.set_edge_obj(&edge_obj, new_label);
        g.remove_node(&v);
    }
}

// ─── updateInputGraph ────────────────────────────────────────────────────────

fn update_input_graph(input_graph: &mut Graph, layout_graph: &Graph) {
    for v in input_graph.nodes() {
        let layout_node = layout_graph.node_opt(&v).cloned();
        if let Some(ln) = layout_node {
            let node = input_graph.node_mut(&v);
            node.x = ln.x;
            node.y = ln.y;
            node.order = ln.order;
            node.rank = ln.rank;
            if !layout_graph.children(&v).is_empty() {
                node.width = ln.width;
                node.height = ln.height;
            }
        }
    }

    for e in input_graph.edges() {
        let layout_label = layout_graph.edge(&e).cloned();
        if let Some(ll) = layout_label {
            let label = input_graph.edge_mut(&e);
            if let Some(l) = label {
                l.points = ll.points;
                if ll.x.is_some() {
                    l.x = ll.x;
                    l.y = ll.y;
                }
            }
        }
    }

    input_graph.graph_mut().width = layout_graph.graph().width;
    input_graph.graph_mut().height = layout_graph.graph().height;
}
