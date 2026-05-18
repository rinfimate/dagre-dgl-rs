//! order/barycenter.rs — barycenter
//! Faithful port of dagre-js/lib/order/barycenter.ts

use crate::graph::Graph;

/// Barycenter result for a single node.
#[derive(Debug, Clone)]
pub struct BarycenterEntry {
    /// The node identifier.
    pub v: String,
    /// Weighted average order of predecessor nodes, or `None` if the node has no predecessors.
    pub barycenter: Option<f64>,
    /// Total weight of incoming edges used to compute the barycenter.
    pub weight: Option<f64>,
}

/// Computes barycenter values for each node in `movable` based on the positions of their predecessors.
pub fn barycenter(graph: &Graph, movable: &[String]) -> Vec<BarycenterEntry> {
    movable
        .iter()
        .map(|v| {
            let in_v = graph.in_edges(v).unwrap_or_default();
            if in_v.is_empty() {
                BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                }
            } else {
                let (sum, weight) = in_v.iter().fold((0.0f64, 0.0f64), |(sum, weight), e| {
                    let edge_weight = graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0);
                    let node_order = graph.node(&e.v).order.unwrap_or(0) as f64;
                    (sum + edge_weight * node_order, weight + edge_weight)
                });
                BarycenterEntry {
                    v: v.clone(),
                    barycenter: Some(sum / weight),
                    weight: Some(weight),
                }
            }
        })
        .collect()
}
