//! graph.rs — A faithful port of @dagrejs/graphlib Graph to Rust.
//!
//! The JS graphlib Graph supports:
//!   - String node IDs
//!   - Optional per-node and per-edge "labels" (arbitrary JS objects)
//!   - Compound (parent/child) hierarchy
//!   - Multigraph (multiple edges between same pair)
//!   - Directed or undirected
//!
//! Here we store node labels as NodeLabel structs and edge labels as EdgeLabel
//! structs (defined in this file). Graph-level metadata is GraphLabel.
//! All optional numeric fields use `Option<f64>` or `Option<i32>`.

use indexmap::IndexMap;
use std::collections::HashMap;

// ─── Point ───────────────────────────────────────────────────────────────────

/// A 2-D coordinate in the layout plane.
///
/// After [`layout`](crate::layout()) runs, node centers and edge bend-points are
/// expressed as `Point` values.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    /// Horizontal position, in the layout coordinate system.
    pub x: f64,
    /// Vertical position, in the layout coordinate system.
    pub y: f64,
}

// ─── NodeLabel ───────────────────────────────────────────────────────────────

/// Per-node metadata used both as input to the layout engine and as output.
///
/// Before calling [`layout`](crate::layout()), populate at minimum [`width`](Self::width)
/// and [`height`](Self::height).  After the call, [`x`](Self::x) and [`y`](Self::y)
/// hold the computed center position of the node.
///
/// All other fields are used internally by the layout pipeline and need not be
/// set by callers.
#[derive(Debug, Clone, Default)]
pub struct NodeLabel {
    /// Bounding-box width of the node, in pixels (or any consistent unit).
    /// **Must be set before calling `layout`.**
    pub width: f64,
    /// Bounding-box height of the node.
    /// **Must be set before calling `layout`.**
    pub height: f64,
    /// Computed horizontal center of the node after layout.  `None` before layout runs.
    pub x: Option<f64>,
    /// Computed vertical center of the node after layout.  `None` before layout runs.
    pub y: Option<f64>,
    /// Rank (layer) assigned to the node by the ranking phase.
    pub rank: Option<i32>,
    /// Position of the node within its rank layer, assigned by the ordering phase.
    pub order: Option<i32>,
    /// Marks a dummy node inserted by the layout pipeline.
    /// Values include `"edge"`, `"border"`, `"edge-label"`, `"edge-proxy"`,
    /// `"selfedge"`, and `"root"`.  `None` for real (user-supplied) nodes.
    pub dummy: Option<String>,
    /// For border dummy nodes: `"borderLeft"` or `"borderRight"`.
    pub border_type: Option<String>,
    /// Top border dummy node ID for a compound node.
    pub border_top: Option<String>,
    /// Bottom border dummy node ID for a compound node.
    pub border_bottom: Option<String>,
    /// Left border dummy node IDs, indexed by rank.
    pub border_left: Option<Vec<Option<String>>>,
    /// Right border dummy node IDs, indexed by rank.
    pub border_right: Option<Vec<Option<String>>>,
    /// Minimum rank spanned by this compound node.
    pub min_rank: Option<i32>,
    /// Maximum rank spanned by this compound node.
    pub max_rank: Option<i32>,
    /// Low value used by the network-simplex spanning-tree algorithm.
    pub low: Option<i32>,
    /// Limit value used by the network-simplex spanning-tree algorithm.
    pub lim: Option<i32>,
    /// Parent pointer used inside the network-simplex tree graph (renamed from
    /// `parent` to avoid confusion with the compound-graph parent).
    pub parent_node: Option<String>,
    /// Self-edges originating at this node, collected before layout and
    /// reattached afterwards.
    pub self_edges: Option<Vec<SelfEdge>>,
    /// For edge-proxy dummy nodes: the original [`Edge`] that the proxy represents.
    pub edge_obj: Option<Edge>,
    /// For edge-label / self-edge dummy nodes: the associated [`EdgeLabel`].
    pub edge_label: Option<Box<EdgeLabel>>,
    /// Optional text label stored on the node.
    pub label: Option<String>,
    /// Label position preference (e.g. `"c"`, `"l"`, `"r"`).
    pub labelpos: Option<String>,
    /// Edge reference used by the `build_layer_graph` helper.
    pub e: Option<Edge>,
    /// Node shape used for boundary-intersection calculations during edge routing.
    /// `None` = rectangle (default), `Some("diamond")`, `Some("circle")`, etc.
    pub intersect_type: Option<&'static str>,
}

// ─── SelfEdge ────────────────────────────────────────────────────────────────

/// A self-loop edge (source == target) together with its label.
///
/// Self-edges are removed from the graph before layout and stored in
/// [`NodeLabel::self_edges`].  After layout they are reinserted with computed
/// bend-points.
#[derive(Debug, Clone)]
pub struct SelfEdge {
    /// The edge descriptor (both `v` and `w` point to the same node).
    pub e: Edge,
    /// The edge label, including output `points` after layout.
    pub label: EdgeLabel,
}

// ─── EdgeLabel ───────────────────────────────────────────────────────────────

/// Per-edge metadata used both as input to the layout engine and as output.
///
/// Before calling [`layout`](crate::layout()) you may set the input fields
/// (`minlen`, `weight`, `width`, `height`, `labelpos`, `labeloffset`).
/// After the call, [`points`](Self::points) contains the computed bend-point
/// sequence for the edge, and [`x`](Self::x) / [`y`](Self::y) hold the
/// computed label position if an edge label was specified.
#[derive(Debug, Clone, Default)]
pub struct EdgeLabel {
    /// Sequence of bend-points defining the edge path, populated after layout.
    /// The first and last points touch the source and target node boundaries.
    pub points: Option<Vec<Point>>,
    /// Width of an edge label, used to reserve space during layout.
    pub width: Option<f64>,
    /// Height of an edge label, used to reserve space during layout.
    pub height: Option<f64>,
    /// Minimum number of rank layers the edge must span (default: `1`).
    pub minlen: Option<i32>,
    /// Relative importance of keeping this edge short (default: `1.0`).
    /// Higher values pull the endpoints closer together.
    pub weight: Option<f64>,
    /// Preferred position of the edge label relative to the edge midpoint.
    /// `"l"` = left, `"r"` = right, `"c"` = center (default).
    pub labelpos: Option<String>,
    /// Pixel offset applied to the label when `labelpos` is `"l"` or `"r"` (default: `10`).
    pub labeloffset: Option<f64>,
    /// Rank layer assigned to the edge-label dummy node.
    pub label_rank: Option<i32>,
    /// Computed horizontal center of the edge label after layout.
    pub x: Option<f64>,
    /// Computed vertical center of the edge label after layout.
    pub y: Option<f64>,
    /// `true` when the edge direction was reversed by the acyclic phase.
    pub reversed: Option<bool>,
    /// Original edge name, preserved when an edge is reversed.
    pub forward_name: Option<String>,
    /// `true` for self-loop edges.
    pub self_edge: Option<bool>,
    /// `true` for virtual edges inserted by the nesting-graph phase.
    pub nesting_edge: Option<bool>,
    /// Cut value used by the network-simplex ranking algorithm.
    pub cutvalue: Option<f64>,
    /// Limit value used by the network-simplex spanning-tree algorithm.
    pub lim: Option<i32>,
    /// Low value used by the network-simplex spanning-tree algorithm.
    pub low: Option<i32>,
    /// Parent node ID stored on spanning-tree edges.
    pub parent: Option<String>,
    /// Nested edge label (used by edge-label dummy nodes).
    pub edge_label: Option<Box<EdgeLabel>>,
    /// Original edge object stored on edge-proxy dummy nodes.
    pub edge_obj: Option<Edge>,
}

// ─── GraphLabel ──────────────────────────────────────────────────────────────

/// Graph-level configuration and layout options.
///
/// Set this on the graph via [`Graph::set_graph`] before calling
/// [`layout`](crate::layout()).  After layout the [`width`](Self::width) and
/// [`height`](Self::height) fields are populated with the overall bounding-box
/// of the laid-out graph.
///
/// All fields are optional; the layout engine applies sensible defaults when a
/// field is `None`.
#[derive(Debug, Clone, Default)]
pub struct GraphLabel {
    /// Total width of the laid-out graph, populated after layout.
    pub width: Option<f64>,
    /// Total height of the laid-out graph, populated after layout.
    pub height: Option<f64>,
    /// Whether the graph uses the compound (parent/child) extension.
    /// Set automatically; callers do not need to set this.
    pub compound: Option<bool>,
    /// Direction of rank progression.
    /// `"TB"` (top-to-bottom, default), `"BT"`, `"LR"`, or `"RL"`.
    pub rankdir: Option<String>,
    /// Alignment of nodes within a rank.
    /// `"UL"`, `"UR"`, `"DL"`, `"DR"`, or `None` (default center alignment).
    pub align: Option<String>,
    /// Minimum horizontal separation between adjacent nodes in the same rank
    /// (default: `50`).
    pub nodesep: Option<f64>,
    /// Minimum horizontal separation between adjacent edge segments in the same
    /// rank (default: `20`).
    pub edgesep: Option<f64>,
    /// Minimum vertical separation between adjacent ranks (default: `50`).
    pub ranksep: Option<f64>,
    /// Horizontal margin added around the entire graph (default: `0`).
    pub marginx: Option<f64>,
    /// Vertical margin added around the entire graph (default: `0`).
    pub marginy: Option<f64>,
    /// Algorithm used to break cycles.  `"greedy"` or `None` (default DFS-based).
    pub acyclicer: Option<String>,
    /// Ranking algorithm.  `"network-simplex"` (default), `"tight-tree"`, or
    /// `"longest-path"`.
    pub ranker: Option<String>,
    /// Rank alignment strategy used during positioning.
    pub rankalign: Option<String>,
    /// Node ID of the virtual nesting-graph root, used internally.
    pub nesting_root: Option<String>,
    /// Scaling factor applied to ranks during nesting-graph construction.
    pub node_rank_factor: Option<i32>,
    /// IDs of dummy-chain nodes inserted during normalization.
    pub dummy_chains: Option<Vec<String>>,
    /// Maximum rank in the graph, computed during layout.
    pub max_rank: Option<i32>,
    /// Root node ID used by `build_layer_graph`.
    pub root: Option<String>,
}

// ─── Edge ────────────────────────────────────────────────────────────────────

/// A descriptor that uniquely identifies an edge in a [`Graph`].
///
/// In a simple (non-multigraph) directed graph, the pair `(v, w)` is sufficient
/// to identify an edge.  In a multigraph, the optional `name` field
/// disambiguates parallel edges between the same pair of nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    /// Source node ID.
    pub v: String,
    /// Target node ID.
    pub w: String,
    /// Optional edge name used to distinguish parallel edges in a multigraph.
    pub name: Option<String>,
}

impl Edge {
    /// Creates an unnamed edge from `v` to `w`.
    pub fn new(v: &str, w: &str) -> Self {
        Edge {
            v: v.to_string(),
            w: w.to_string(),
            name: None,
        }
    }

    /// Creates a named edge from `v` to `w` with the given `name`.
    ///
    /// Named edges allow multiple parallel edges between the same pair of nodes
    /// in a multigraph.
    pub fn named(v: &str, w: &str, name: &str) -> Self {
        Edge {
            v: v.to_string(),
            w: w.to_string(),
            name: Some(name.to_string()),
        }
    }
}

// ─── Internal edge storage ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct EdgeEntry {
    v: String,
    w: String,
    name: Option<String>,
    label: EdgeLabel,
}

fn edge_key(v: &str, w: &str, name: Option<&str>) -> String {
    match name {
        Some(n) => format!("{}\x00{}\x00{}", v, w, n),
        None => format!("{}\x00{}\x00\x01", v, w),
    }
}

// For undirected graphs, normalize so smaller node is always v (matches graphlib behaviour).
fn edge_key_undirected(v: &str, w: &str, name: Option<&str>) -> String {
    if v <= w {
        edge_key(v, w, name)
    } else {
        edge_key(w, v, name)
    }
}

// ─── Graph ───────────────────────────────────────────────────────────────────

/// A general-purpose labeled graph modelled after
/// [@dagrejs/graphlib](https://github.com/dagrejs/graphlib).
///
/// `Graph` supports directed or undirected edges, optional multigraph semantics
/// (parallel edges), and optional compound (parent/child) hierarchies.  Nodes
/// are identified by `String` IDs; per-node data is stored in [`NodeLabel`],
/// per-edge data in [`EdgeLabel`], and graph-level options in [`GraphLabel`].
///
/// The most common configuration is a **directed, simple, non-compound** graph,
/// created with [`Graph::default()`].
///
/// # Example
///
/// ```rust
/// use dagre_dgl_rs::{Graph, GraphLabel, NodeLabel, EdgeLabel};
///
/// let mut g = Graph::default();
/// g.set_graph(GraphLabel { nodesep: Some(50.0), ..Default::default() });
/// g.set_node("a", NodeLabel { width: 80.0, height: 30.0, ..Default::default() });
/// g.set_node("b", NodeLabel { width: 80.0, height: 30.0, ..Default::default() });
/// g.set_edge("a", "b", EdgeLabel::default(), None);
///
/// assert_eq!(g.node_count(), 2);
/// assert_eq!(g.edge_count(), 1);
/// assert!(g.has_node("a"));
/// ```
pub struct Graph {
    // Configuration
    /// `true` for directed graphs (default), `false` for undirected.
    pub is_directed: bool,
    /// `true` when the graph allows multiple edges between the same node pair.
    pub is_multigraph: bool,
    /// `true` when the graph supports parent/child compound relationships.
    pub is_compound: bool,

    // Graph-level label
    label: GraphLabel,

    // Nodes: id -> NodeLabel (IndexMap preserves insertion order)
    nodes: IndexMap<String, NodeLabel>,

    // Default node label factory — stored as Option for simplicity
    default_node_label: Option<String>, // Not actually used dynamically in Rust

    // Edges: edge_key -> EdgeEntry
    edges: IndexMap<String, EdgeEntry>,

    // Adjacency: node -> {neighbor -> {edge_key -> ()}}
    // For directed: in and out separately
    // IndexMap preserves insertion order so predecessors()/successors() iterate
    // in insertion order — matching JS graphlib behaviour.
    in_edges: HashMap<String, IndexMap<String, Vec<String>>>, // v -> {w -> [edge_keys]}
    out_edges: HashMap<String, IndexMap<String, Vec<String>>>, // v -> {w -> [edge_keys]}

    // Compound parent/children
    parent: HashMap<String, Option<String>>, // node -> parent (None = root)
    children: HashMap<String, Vec<String>>,  // parent -> children list
}

impl Graph {
    /// Creates an undirected, simple, non-compound graph.
    ///
    /// For the typical directed layout graph use [`Graph::default()`], which
    /// creates a directed graph.
    pub fn new() -> Self {
        Graph::with_options(false, false, false)
    }

    /// Creates a directed, simple, non-compound graph.
    pub fn directed() -> Self {
        Graph::with_options(true, false, false)
    }

    /// Creates a directed, multigraph, compound graph — the most feature-rich
    /// configuration, used internally by the layout pipeline.
    pub fn multigraph_compound() -> Self {
        Graph::with_options(true, true, true)
    }

    /// Creates an undirected, simple, non-compound graph.
    ///
    /// Equivalent to [`Graph::new`].
    pub fn undirected() -> Self {
        Graph::with_options(false, false, false)
    }

    /// Creates a graph with fully explicit options.
    ///
    /// # Parameters
    /// - `directed`   — `true` for a directed graph.
    /// - `multigraph` — `true` to allow multiple edges between the same pair of nodes.
    /// - `compound`   — `true` to enable the parent/child hierarchy.
    pub fn with_options(directed: bool, multigraph: bool, compound: bool) -> Self {
        let mut g = Graph {
            is_directed: directed,
            is_multigraph: multigraph,
            is_compound: compound,
            label: GraphLabel::default(),
            nodes: IndexMap::new(),
            default_node_label: None,
            edges: IndexMap::new(),
            in_edges: HashMap::new(),
            out_edges: HashMap::new(),
            parent: HashMap::new(),
            children: HashMap::new(),
        };
        // The root sentinel "\x00" is the implicit parent of all root-level nodes
        g.children.insert("\x00".to_string(), Vec::new());
        g
    }

    // ── Graph-level label ─────────────────────────────────────────────────

    /// Returns a shared reference to the graph-level label (layout options).
    pub fn graph(&self) -> &GraphLabel {
        &self.label
    }

    /// Returns a mutable reference to the graph-level label.
    pub fn graph_mut(&mut self) -> &mut GraphLabel {
        &mut self.label
    }

    /// Replaces the graph-level label with `label` and returns `&mut self` for chaining.
    pub fn set_graph(&mut self, label: GraphLabel) -> &mut Self {
        self.label = label;
        self
    }

    // ── Node API ──────────────────────────────────────────────────────────

    /// Inserts node `v` with `label`, or replaces its label if it already exists.
    ///
    /// If the graph is compound, `v` is placed at the root level until
    /// [`set_parent`](Self::set_parent) is called.
    pub fn set_node(&mut self, v: &str, label: NodeLabel) {
        if !self.nodes.contains_key(v) {
            self.nodes.insert(v.to_string(), label);
            self.in_edges.insert(v.to_string(), IndexMap::new());
            self.out_edges.insert(v.to_string(), IndexMap::new());
            if self.is_compound {
                self.parent.insert(v.to_string(), None);
                // add to root children
                let root_children = self.children.entry("\x00".to_string()).or_default();
                root_children.push(v.to_string());
                self.children.insert(v.to_string(), Vec::new());
            }
        } else {
            *self.nodes.get_mut(v).unwrap() = label;
        }
    }

    /// Inserts node `v` with a default [`NodeLabel`], or does nothing if it already exists.
    pub fn set_node_default(&mut self, v: &str) {
        self.set_node(v, NodeLabel::default());
    }

    /// Returns a shared reference to the label of node `v`.
    ///
    /// # Panics
    ///
    /// Panics if `v` is not in the graph.  Use [`node_opt`](Self::node_opt) for
    /// a non-panicking alternative.
    pub fn node(&self, v: &str) -> &NodeLabel {
        self.nodes
            .get(v)
            .unwrap_or_else(|| panic!("Node '{}' not found", v))
    }

    /// Returns a mutable reference to the label of node `v`.
    ///
    /// # Panics
    ///
    /// Panics if `v` is not in the graph.  Use [`node_opt_mut`](Self::node_opt_mut)
    /// for a non-panicking alternative.
    pub fn node_mut(&mut self, v: &str) -> &mut NodeLabel {
        self.nodes
            .get_mut(v)
            .unwrap_or_else(|| panic!("Node '{}' not found", v))
    }

    /// Returns a shared reference to the label of node `v`, or `None` if `v` is
    /// not in the graph.
    pub fn node_opt(&self, v: &str) -> Option<&NodeLabel> {
        self.nodes.get(v)
    }

    /// Returns a mutable reference to the label of node `v`, or `None` if `v` is
    /// not in the graph.
    pub fn node_opt_mut(&mut self, v: &str) -> Option<&mut NodeLabel> {
        self.nodes.get_mut(v)
    }

    /// Returns `true` if node `v` is present in the graph.
    pub fn has_node(&self, v: &str) -> bool {
        self.nodes.contains_key(v)
    }

    /// Removes node `v` and all edges incident on it.
    ///
    /// In a compound graph, children of `v` are re-parented to `v`'s parent.
    /// Does nothing if `v` is not in the graph.
    pub fn remove_node(&mut self, v: &str) {
        if !self.nodes.contains_key(v) {
            return;
        }

        // Remove all edges incident on v
        let edges_to_remove: Vec<Edge> =
            self.node_edges(v).unwrap_or_default().into_iter().collect();
        for e in edges_to_remove {
            self.remove_edge_obj(&e);
        }

        // Remove from compound structures
        if self.is_compound {
            // Remove v from its parent's children
            let par = self.parent.remove(v).flatten();
            let par_key = par.unwrap_or_else(|| "\x00".to_string());
            if let Some(ch) = self.children.get_mut(&par_key) {
                ch.retain(|c| c != v);
            }

            // Re-parent v's children to v's parent
            if let Some(ch) = self.children.remove(v) {
                for child in ch {
                    let child_par = self.parent.get_mut(&child);
                    if let Some(cp) = child_par {
                        *cp = if par_key == "\x00" {
                            None
                        } else {
                            Some(par_key.clone())
                        };
                    }
                    let new_par_children = self.children.entry(par_key.clone()).or_default();
                    new_par_children.push(child);
                }
            }
        }

        self.nodes.swap_remove(v);
        self.in_edges.remove(v);
        self.out_edges.remove(v);
    }

    /// Returns all node IDs in insertion order.
    pub fn nodes(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Returns the number of nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns nodes that have no incoming edges (in-degree zero).
    pub fn sources(&self) -> Vec<String> {
        self.nodes
            .keys()
            .filter(|v| self.in_edges.get(*v).is_none_or(|m| m.is_empty()))
            .cloned()
            .collect()
    }

    /// Returns nodes that have no outgoing edges (out-degree zero).
    pub fn sinks(&self) -> Vec<String> {
        self.nodes
            .keys()
            .filter(|v| self.out_edges.get(*v).is_none_or(|m| m.is_empty()))
            .cloned()
            .collect()
    }

    // ── Edge API ──────────────────────────────────────────────────────────

    /// Inserts an edge from `v` to `w` with `label`, or replaces its label if
    /// the edge already exists.
    ///
    /// `name` distinguishes parallel edges in a multigraph; pass `None` for a
    /// simple (unnamed) edge.  If `v` or `w` are not yet in the graph they are
    /// created with a default [`NodeLabel`].
    pub fn set_edge(&mut self, v: &str, w: &str, label: EdgeLabel, name: Option<&str>) {
        let e = Edge {
            v: v.to_string(),
            w: w.to_string(),
            name: name.map(|s| s.to_string()),
        };
        self.set_edge_obj(&e, label);
    }

    fn ekey(&self, v: &str, w: &str, name: Option<&str>) -> String {
        if self.is_directed {
            edge_key(v, w, name)
        } else {
            edge_key_undirected(v, w, name)
        }
    }

    /// Inserts or updates the edge described by `e` with `label`.
    ///
    /// Equivalent to [`set_edge`](Self::set_edge) but takes an [`Edge`] struct
    /// directly.
    pub fn set_edge_obj(&mut self, e: &Edge, label: EdgeLabel) {
        let key = self.ekey(&e.v, &e.w, e.name.as_deref());

        // Ensure nodes exist
        if !self.has_node(&e.v) {
            self.set_node_default(&e.v);
        }
        if !self.has_node(&e.w) {
            self.set_node_default(&e.w);
        }

        if self.edges.contains_key(&key) {
            self.edges.get_mut(&key).unwrap().label = label;
            return;
        }

        if !self.is_multigraph && self.has_edge(&e.v, &e.w) {
            let existing_key = self.ekey(&e.v, &e.w, None);
            if let Some(entry) = self.edges.get_mut(&existing_key) {
                entry.label = label;
            }
            return;
        }

        let entry = EdgeEntry {
            v: e.v.clone(),
            w: e.w.clone(),
            name: e.name.clone(),
            label,
        };

        self.edges.insert(key.clone(), entry);

        self.out_edges
            .entry(e.v.clone())
            .or_default()
            .entry(e.w.clone())
            .or_default()
            .push(key.clone());
        if !self.is_directed {
            self.out_edges
                .entry(e.w.clone())
                .or_default()
                .entry(e.v.clone())
                .or_default()
                .push(key.clone());
        }

        self.in_edges
            .entry(e.w.clone())
            .or_default()
            .entry(e.v.clone())
            .or_default()
            .push(key.clone());
        if !self.is_directed {
            self.in_edges
                .entry(e.v.clone())
                .or_default()
                .entry(e.w.clone())
                .or_default()
                .push(key.clone());
        }
    }

    /// Returns the label of the unnamed edge from `v` to `w`, or `None` if no
    /// such edge exists.
    pub fn edge_label(&self, v: &str, w: &str) -> Option<&EdgeLabel> {
        let key = self.ekey(v, w, None);
        self.edges.get(&key).map(|e| &e.label)
    }

    /// Returns the label of the named edge from `v` to `w`, or `None` if no
    /// such edge exists.
    pub fn edge_label_named(&self, v: &str, w: &str, name: &str) -> Option<&EdgeLabel> {
        let key = self.ekey(v, w, Some(name));
        self.edges.get(&key).map(|e| &e.label)
    }

    /// Returns the label of the edge described by `e`, or `None` if the edge
    /// does not exist.
    pub fn edge(&self, e: &Edge) -> Option<&EdgeLabel> {
        let key = self.ekey(&e.v, &e.w, e.name.as_deref());
        self.edges.get(&key).map(|en| &en.label)
    }

    /// Returns a mutable reference to the label of the edge described by `e`,
    /// or `None` if the edge does not exist.
    pub fn edge_mut(&mut self, e: &Edge) -> Option<&mut EdgeLabel> {
        let key = self.ekey(&e.v, &e.w, e.name.as_deref());
        self.edges.get_mut(&key).map(|en| &mut en.label)
    }

    /// Returns the label of the unnamed edge from `v` to `w`, or `None`.
    ///
    /// Equivalent to [`edge_label`](Self::edge_label).
    pub fn edge_vw(&self, v: &str, w: &str) -> Option<&EdgeLabel> {
        let key = self.ekey(v, w, None);
        self.edges.get(&key).map(|e| &e.label)
    }

    /// Returns a mutable reference to the label of the unnamed edge from `v` to
    /// `w`, or `None` if the edge does not exist.
    pub fn edge_vw_mut(&mut self, v: &str, w: &str) -> Option<&mut EdgeLabel> {
        let key = self.ekey(v, w, None);
        self.edges.get_mut(&key).map(|e| &mut e.label)
    }

    /// Returns `true` if at least one edge from `v` to `w` exists.
    pub fn has_edge(&self, v: &str, w: &str) -> bool {
        self.out_edges.get(v).is_some_and(|m| m.contains_key(w))
    }

    /// Returns `true` if the edge described by `e` (including name) exists.
    pub fn has_edge_obj(&self, e: &Edge) -> bool {
        let key = self.ekey(&e.v, &e.w, e.name.as_deref());
        self.edges.contains_key(&key)
    }

    /// Removes the unnamed edge from `v` to `w`.  Does nothing if it does not exist.
    pub fn remove_edge(&mut self, v: &str, w: &str) {
        let e = Edge::new(v, w);
        self.remove_edge_obj(&e);
    }

    /// Removes the named edge from `v` to `w`.  Does nothing if it does not exist.
    pub fn remove_edge_named(&mut self, v: &str, w: &str, name: &str) {
        let e = Edge::named(v, w, name);
        self.remove_edge_obj(&e);
    }

    /// Removes the edge described by `e`.  Does nothing if the edge does not exist.
    pub fn remove_edge_obj(&mut self, e: &Edge) {
        let key = self.ekey(&e.v, &e.w, e.name.as_deref());
        if !self.edges.contains_key(&key) {
            return;
        }
        self.edges.swap_remove(&key);

        // Remove from out_edges — use shift_remove (not swap_remove) to preserve
        // the insertion order of remaining successors. swap_remove would move the
        // last successor into the removed slot, scrambling the order used by init_order.
        if let Some(m) = self.out_edges.get_mut(&e.v) {
            if let Some(keys) = m.get_mut(&e.w) {
                keys.retain(|k| k != &key);
                if keys.is_empty() {
                    m.shift_remove(&e.w);
                }
            }
        }
        if !self.is_directed {
            if let Some(m) = self.out_edges.get_mut(&e.w) {
                if let Some(keys) = m.get_mut(&e.v) {
                    keys.retain(|k| k != &key);
                    if keys.is_empty() {
                        m.shift_remove(&e.v);
                    }
                }
            }
        }

        // Remove from in_edges — same reason: preserve predecessor insertion order.
        if let Some(m) = self.in_edges.get_mut(&e.w) {
            if let Some(keys) = m.get_mut(&e.v) {
                keys.retain(|k| k != &key);
                if keys.is_empty() {
                    m.shift_remove(&e.v);
                }
            }
        }
        if !self.is_directed {
            if let Some(m) = self.in_edges.get_mut(&e.v) {
                if let Some(keys) = m.get_mut(&e.w) {
                    keys.retain(|k| k != &key);
                    if keys.is_empty() {
                        m.shift_remove(&e.w);
                    }
                }
            }
        }
    }

    /// Returns all edges in the graph in insertion order.
    pub fn edges(&self) -> Vec<Edge> {
        self.edges
            .values()
            .map(|e| Edge {
                v: e.v.clone(),
                w: e.w.clone(),
                name: e.name.clone(),
            })
            .collect()
    }

    /// Returns the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ── Adjacency queries ─────────────────────────────────────────────────

    fn collect_edges_from_keys(&self, keys: Vec<String>) -> Vec<Edge> {
        keys.into_iter()
            .filter_map(|k| {
                self.edges.get(&k).map(|e| Edge {
                    v: e.v.clone(),
                    w: e.w.clone(),
                    name: e.name.clone(),
                })
            })
            .collect()
    }

    /// Returns all edges whose target is `v`, or `None` if `v` is not in the graph.
    pub fn in_edges(&self, v: &str) -> Option<Vec<Edge>> {
        let m = self.in_edges.get(v)?;
        let mut result = Vec::new();
        for keys in m.values() {
            for k in keys {
                if let Some(e) = self.edges.get(k) {
                    result.push(Edge {
                        v: e.v.clone(),
                        w: e.w.clone(),
                        name: e.name.clone(),
                    });
                }
            }
        }
        Some(result)
    }

    /// Returns all edges whose source is `v`, or `None` if `v` is not in the graph.
    pub fn out_edges(&self, v: &str) -> Option<Vec<Edge>> {
        let m = self.out_edges.get(v)?;
        let mut result = Vec::new();
        for keys in m.values() {
            for k in keys {
                if let Some(e) = self.edges.get(k) {
                    result.push(Edge {
                        v: e.v.clone(),
                        w: e.w.clone(),
                        name: e.name.clone(),
                    });
                }
            }
        }
        Some(result)
    }

    /// Returns all edges from `v` to `w`, or `None` if either node is absent.
    pub fn out_edges_to(&self, v: &str, w: &str) -> Option<Vec<Edge>> {
        let keys = self.out_edges.get(v)?.get(w)?;
        Some(self.collect_edges_from_keys(keys.clone()))
    }

    /// Returns all edges incident on `v` (both incoming and outgoing), or `None`
    /// if `v` is not in the graph.
    pub fn node_edges(&self, v: &str) -> Option<Vec<Edge>> {
        if !self.has_node(v) {
            return None;
        }
        let mut result = self.in_edges(v).unwrap_or_default();
        let out = self.out_edges(v).unwrap_or_default();
        // For directed graphs, avoid duplicates where v==w
        for e in out {
            if !(e.v == v
                && e.w == v
                && result
                    .iter()
                    .any(|r| r.v == e.v && r.w == e.w && r.name == e.name))
            {
                result.push(e);
            }
        }
        Some(result)
    }

    /// Returns the IDs of all nodes with an edge pointing **to** `v`, in
    /// insertion order.  Returns `None` if `v` is not in the graph.
    pub fn predecessors(&self, v: &str) -> Option<Vec<String>> {
        let m = self.in_edges.get(v)?;
        Some(m.keys().cloned().collect())
    }

    /// Returns the IDs of all nodes reachable by an edge **from** `v`, in
    /// insertion order.  Returns `None` if `v` is not in the graph.
    pub fn successors(&self, v: &str) -> Option<Vec<String>> {
        let m = self.out_edges.get(v)?;
        Some(m.keys().cloned().collect())
    }

    /// Returns all distinct neighbors of `v` (predecessors ∪ successors), or
    /// `None` if `v` is not in the graph.
    pub fn neighbors(&self, v: &str) -> Option<Vec<String>> {
        let mut n: Vec<String> = self.predecessors(v).unwrap_or_default();
        let succ = self.successors(v).unwrap_or_default();
        for s in succ {
            if !n.contains(&s) {
                n.push(s);
            }
        }
        Some(n)
    }

    // ── Compound graph ────────────────────────────────────────────────────

    /// Returns the parent of v, or None if v is at root level.
    pub fn parent(&self, v: &str) -> Option<&str> {
        self.parent.get(v).and_then(|p| p.as_deref())
    }

    /// Sets parent of v to `parent`. Pass None to set v to root.
    pub fn set_parent(&mut self, v: &str, parent: Option<&str>) {
        if !self.is_compound {
            panic!("Not a compound graph");
        }

        // Ensure v exists
        if !self.has_node(v) {
            self.set_node_default(v);
        }

        // Remove from old parent
        let old_par = self.parent.get(v).cloned().flatten();
        let old_par_key = old_par.unwrap_or_else(|| "\x00".to_string());
        if let Some(ch) = self.children.get_mut(&old_par_key) {
            ch.retain(|c| c != v);
        }

        // Validate new parent is not a descendant
        if let Some(new_par) = parent {
            if !self.has_node(new_par) {
                self.set_node_default(new_par);
            }
            // Set new parent
            self.parent.insert(v.to_string(), Some(new_par.to_string()));
            self.children
                .entry(new_par.to_string())
                .or_default()
                .push(v.to_string());
        } else {
            self.parent.insert(v.to_string(), None);
            self.children
                .entry("\x00".to_string())
                .or_default()
                .push(v.to_string());
        }
    }

    /// Returns the children of v. Pass "\x00" (GRAPH_NODE) for root-level children.
    pub fn children(&self, v: &str) -> Vec<String> {
        if self.is_compound {
            self.children.get(v).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    // ── Graph properties ──────────────────────────────────────────────────

    /// Returns `true` if the graph allows multiple parallel edges between the
    /// same pair of nodes.
    pub fn is_multigraph(&self) -> bool {
        self.is_multigraph
    }

    /// Returns `true` if the graph supports parent/child compound relationships.
    pub fn is_compound(&self) -> bool {
        self.is_compound
    }

    /// Returns `true` for a directed graph, `false` for an undirected graph.
    pub fn is_directed(&self) -> bool {
        self.is_directed
    }

    // ── Filter ────────────────────────────────────────────────────────────

    /// Returns a new graph containing only nodes for which `pred` returns `true`,
    /// together with any edges whose both endpoints are retained.
    ///
    /// The new graph has the same directed/multigraph/compound configuration and
    /// the same graph-level label as the original.
    pub fn filter_nodes<F>(&self, pred: F) -> Graph
    where
        F: Fn(&str) -> bool,
    {
        let mut g = Graph::with_options(self.is_directed, self.is_multigraph, self.is_compound);
        g.label = self.label.clone();

        for v in self.nodes.keys() {
            if pred(v) {
                g.set_node(v, self.node(v).clone());
            }
        }

        for e in self.edges.values() {
            if g.has_node(&e.v) && g.has_node(&e.w) {
                g.set_edge(&e.v, &e.w, e.label.clone(), e.name.as_deref());
            }
        }

        if self.is_compound {
            for v in g.nodes.keys().cloned().collect::<Vec<_>>() {
                if let Some(p) = self.parent(&v) {
                    if g.has_node(p) {
                        g.set_parent(&v, Some(p));
                    }
                }
            }
        }

        g
    }
}

impl Default for Graph {
    fn default() -> Self {
        Graph::with_options(true, false, false)
    }
}

impl Clone for Graph {
    fn clone(&self) -> Self {
        Graph {
            is_directed: self.is_directed,
            is_multigraph: self.is_multigraph,
            is_compound: self.is_compound,
            label: self.label.clone(),
            nodes: self.nodes.clone(),
            default_node_label: self.default_node_label.clone(),
            edges: self.edges.clone(),
            in_edges: self.in_edges.clone(),
            out_edges: self.out_edges.clone(),
            parent: self.parent.clone(),
            children: self.children.clone(),
        }
    }
}

// ─── Graph traversal algorithms (alg module equivalent) ──────────────────────

/// Returns nodes reachable from `roots` in postorder (children before parent).
///
/// Each node is visited at most once.  Traversal follows [`Graph::neighbors`],
/// so it is undirected regardless of whether the graph is directed.
pub fn postorder(graph: &Graph, roots: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();

    fn dfs(
        graph: &Graph,
        v: &str,
        visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<String>,
    ) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        if let Some(neighbors) = graph.neighbors(v) {
            for w in neighbors {
                dfs(graph, &w, visited, result);
            }
        }
        result.push(v.to_string());
    }

    for r in roots {
        dfs(graph, &r, &mut visited, &mut result);
    }
    result
}

/// Returns nodes reachable from `roots` in preorder (parent before children).
///
/// Each node is visited at most once.  Traversal follows [`Graph::neighbors`],
/// so it is undirected regardless of whether the graph is directed.
pub fn preorder(graph: &Graph, roots: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = std::collections::HashSet::new();

    fn dfs(
        graph: &Graph,
        v: &str,
        visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<String>,
    ) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        result.push(v.to_string());
        if let Some(neighbors) = graph.neighbors(v) {
            for w in neighbors {
                dfs(graph, &w, visited, result);
            }
        }
    }

    for r in roots {
        dfs(graph, &r, &mut visited, &mut result);
    }
    result
}
