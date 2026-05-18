#![deny(missing_docs)]
//! # dagre-rs
//!
//! A faithful Rust port of [dagre-js](https://github.com/dagrejs/dagre) — a directed-graph
//! layout engine that assigns **x/y coordinates** to nodes and **bend-point sequences** to
//! edges, while keeping edge crossings to a minimum.
//!
//! ## Quick start
//!
//! ```rust
//! use dagre_rs::{Graph, GraphLabel, NodeLabel, EdgeLabel, layout};
//!
//! let mut g = Graph::default();
//! g.set_graph(GraphLabel {
//!     rankdir: Some("LR".to_string()),
//!     nodesep: Some(50.0),
//!     ranksep: Some(50.0),
//!     ..Default::default()
//! });
//!
//! g.set_node("a", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
//! g.set_node("b", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
//! g.set_edge("a", "b", EdgeLabel::default(), None);
//!
//! layout(&mut g);
//!
//! let a = g.node("a");
//! println!("node a: ({:?}, {:?})", a.x, a.y);
//! ```
//!
//! ## Modules
//!
//! Most users only need the items re-exported at the crate root.  The sub-modules
//! (`acyclic`, `rank`, `order`, `position`, …) contain the individual pipeline stages
//! and are public for advanced use or testing.

pub mod acyclic;
pub mod add_border_segments;
pub mod coordinate_system;
/// Graph data structures used internally by the layout pipeline (e.g. doubly-linked list).
pub mod data;
pub mod graph;
/// Entry point for the full dagre layout pipeline ([`layout::layout`]).
pub mod layout;
pub mod nesting_graph;
pub mod normalize;
pub mod order;
pub mod parent_dummy_chains;
pub mod position;
pub mod rank;
mod tests;
/// Shared utility functions used across the layout pipeline.
pub mod util;

/// Core graph types: [`Graph`], [`NodeLabel`], [`EdgeLabel`], [`GraphLabel`], [`Edge`],
/// [`Point`], and [`SelfEdge`].
pub use graph::{Edge, EdgeLabel, Graph, GraphLabel, NodeLabel, Point, SelfEdge};

/// Run the full dagre layout pipeline on a graph.  See [`layout::layout`] for details.
pub use layout::layout;
