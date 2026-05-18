//! order/add_subgraph_constraints.rs — addSubgraphConstraints
//! Faithful port of dagre-js/lib/order/add-subgraph-constraints.ts

use crate::graph::{EdgeLabel, Graph};
use std::collections::HashMap;

/// Adds ordering constraints to `constraint_graph` to preserve subgraph containment order.
pub fn add_subgraph_constraints(graph: &Graph, constraint_graph: &mut Graph, vs: &[String]) {
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut root_prev: Option<String> = None;

    for v in vs {
        let mut child: Option<String> = graph.parent(v).map(|p| p.to_string());
        let mut parent: Option<String>;
        let mut prev_child: Option<String>;

        while let Some(ref child_str) = child.clone() {
            parent = graph.parent(child_str).map(|p| p.to_string());
            if let Some(ref parent_str) = parent {
                prev_child = prev.get(parent_str).cloned();
                prev.insert(parent_str.clone(), child_str.clone());
            } else {
                prev_child = root_prev.clone();
                root_prev = Some(child_str.clone());
            }
            if let Some(ref pc) = prev_child {
                if pc != child_str {
                    constraint_graph.set_edge(pc, child_str, EdgeLabel::default(), None);
                    break;
                }
            }
            child = parent;
        }
    }
}
