//! add_border_segments.rs — addBorderSegments
//! Faithful port of dagre-js/lib/add-border-segments.ts

use crate::graph::{EdgeLabel, Graph, NodeLabel};
use crate::util::{add_dummy_node, GRAPH_NODE};

/// Adds left/right border segment dummy nodes to each subgraph with a rank range.
pub fn add_border_segments(graph: &mut Graph) {
    let root_children: Vec<String> = graph.children(GRAPH_NODE);
    for v in root_children {
        dfs(graph, &v.clone());
    }
}

fn dfs(graph: &mut Graph, v: &str) {
    let children: Vec<String> = graph.children(v);
    for child in &children {
        dfs(graph, &child.clone());
    }

    let node = graph.node(v).clone();
    if let Some(min_r) = node.min_rank {
        let max_r = node.max_rank.unwrap_or(min_r);

        // Initialize border arrays
        graph.node_mut(v).border_left = Some(Vec::new());
        graph.node_mut(v).border_right = Some(Vec::new());

        for rank in min_r..=max_r {
            add_border_node_for_segment(graph, "borderLeft", "_bl", v, rank);
            add_border_node_for_segment(graph, "borderRight", "_br", v, rank);
        }
    }
}

fn add_border_node_for_segment(graph: &mut Graph, prop: &str, prefix: &str, sg: &str, rank: i32) {
    let label = NodeLabel {
        width: 0.0,
        height: 0.0,
        rank: Some(rank),
        border_type: Some(prop.to_string()),
        ..Default::default()
    };

    // Find prev node
    let prev: Option<String> = {
        let sg_node = graph.node(sg);
        if prop == "borderLeft" {
            sg_node.border_left.as_ref().and_then(|v| {
                if rank > 0 {
                    v.get((rank - 1) as usize).cloned().flatten()
                } else {
                    None
                }
            })
        } else {
            sg_node.border_right.as_ref().and_then(|v| {
                if rank > 0 {
                    v.get((rank - 1) as usize).cloned().flatten()
                } else {
                    None
                }
            })
        }
    };

    let curr = add_dummy_node(graph, "border", label, prefix);

    // Update sg_node's border array
    {
        let sg_node = graph.node_mut(sg);
        let idx = rank as usize;
        if prop == "borderLeft" {
            let bl = sg_node.border_left.get_or_insert_with(Vec::new);
            while bl.len() <= idx {
                bl.push(None);
            }
            bl[idx] = Some(curr.clone());
        } else {
            let br = sg_node.border_right.get_or_insert_with(Vec::new);
            while br.len() <= idx {
                br.push(None);
            }
            br[idx] = Some(curr.clone());
        }
    }

    graph.set_parent(&curr, Some(sg));

    if let Some(prev_id) = prev {
        graph.set_edge(
            &prev_id,
            &curr,
            EdgeLabel {
                weight: Some(1.0),
                ..Default::default()
            },
            None,
        );
    }
}
