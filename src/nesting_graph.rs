//! nesting_graph.rs — nestingGraph run/cleanup
//! Faithful port of dagre-js/lib/nesting-graph.ts

use crate::graph::{EdgeLabel, Graph};
use crate::util::{add_border_node, add_dummy_node, apply_max_i32, GRAPH_NODE};
use std::collections::HashMap;

/// Adds a virtual root and nesting edges so that compound-graph ranks are consistent.
pub fn run(graph: &mut Graph) {
    let root = add_dummy_node(graph, "root", Default::default(), "_root");
    let depths = tree_depths(graph);
    let depths_vals: Vec<i32> = depths.values().cloned().collect();
    let height = if depths_vals.is_empty() {
        0
    } else {
        apply_max_i32(&depths_vals) - 1
    };
    let node_sep = 2 * height + 1;

    graph.graph_mut().nesting_root = Some(root.clone());

    // Multiply minlen by nodeSep
    let edges: Vec<_> = graph.edges();
    for e in edges {
        if let Some(label) = graph.edge_mut(&e) {
            if let Some(ml) = label.minlen.as_mut() {
                *ml *= node_sep;
            }
        }
    }

    let weight = sum_weights(graph) + 1.0;

    let root_children: Vec<String> = graph.children(GRAPH_NODE);
    for child in root_children {
        dfs(
            graph,
            &root.clone(),
            node_sep,
            weight,
            height,
            &depths,
            &child.clone(),
        );
    }

    graph.graph_mut().node_rank_factor = Some(node_sep);
}

fn dfs(
    graph: &mut Graph,
    root: &str,
    node_sep: i32,
    weight: f64,
    height: i32,
    depths: &HashMap<String, i32>,
    v: &str,
) {
    let children: Vec<String> = graph.children(v);
    if children.is_empty() {
        if v != root {
            graph.set_edge(
                root,
                v,
                EdgeLabel {
                    weight: Some(0.0),
                    minlen: Some(node_sep),
                    ..Default::default()
                },
                None,
            );
        }
        return;
    }

    let top = add_border_node(graph, "_bt");
    let bottom = add_border_node(graph, "_bb");

    graph.set_parent(&top, Some(v));
    graph.node_mut(v).border_top = Some(top.clone());
    graph.set_parent(&bottom, Some(v));
    graph.node_mut(v).border_bottom = Some(bottom.clone());

    let children_clone: Vec<String> = graph.children(v);
    for child in &children_clone {
        dfs(
            graph,
            root,
            node_sep,
            weight,
            height,
            depths,
            &child.clone(),
        );

        let child_top = graph
            .node(child)
            .border_top
            .clone()
            .unwrap_or_else(|| child.clone());
        let child_bottom = graph
            .node(child)
            .border_bottom
            .clone()
            .unwrap_or_else(|| child.clone());
        let has_border = graph.node(child).border_top.is_some();
        let this_weight = if has_border { weight } else { 2.0 * weight };
        let v_depth = depths.get(v).cloned().unwrap_or(0);
        let minlen = if child_top != child_bottom {
            1
        } else {
            height - v_depth + 1
        };

        graph.set_edge(
            &top,
            &child_top,
            EdgeLabel {
                weight: Some(this_weight),
                minlen: Some(minlen),
                nesting_edge: Some(true),
                ..Default::default()
            },
            None,
        );

        graph.set_edge(
            &child_bottom,
            &bottom,
            EdgeLabel {
                weight: Some(this_weight),
                minlen: Some(minlen),
                nesting_edge: Some(true),
                ..Default::default()
            },
            None,
        );
    }

    if graph.parent(v).is_none() {
        let v_depth = depths.get(v).cloned().unwrap_or(0);
        graph.set_edge(
            root,
            &top,
            EdgeLabel {
                weight: Some(0.0),
                minlen: Some(height + v_depth),
                ..Default::default()
            },
            None,
        );
    }
}

fn tree_depths(graph: &Graph) -> HashMap<String, i32> {
    let mut depths: HashMap<String, i32> = HashMap::new();

    fn dfs_depth(graph: &Graph, v: &str, depth: i32, depths: &mut HashMap<String, i32>) {
        let children = graph.children(v);
        for child in children {
            dfs_depth(graph, &child.clone(), depth + 1, depths);
        }
        depths.insert(v.to_string(), depth);
    }

    let root_children: Vec<String> = graph.children(GRAPH_NODE);
    for v in root_children {
        dfs_depth(graph, &v.clone(), 1, &mut depths);
    }
    depths
}

fn sum_weights(graph: &Graph) -> f64 {
    graph.edges().iter().fold(0.0, |acc, e| {
        acc + graph.edge(e).and_then(|l| l.weight).unwrap_or(0.0)
    })
}

/// Removes the virtual root and nesting edges added by [`run`].
pub fn cleanup(graph: &mut Graph) {
    let nesting_root = graph.graph().nesting_root.clone();
    if let Some(root) = nesting_root {
        graph.remove_node(&root);
    }
    graph.graph_mut().nesting_root = None;

    let nesting_edges: Vec<_> = graph
        .edges()
        .into_iter()
        .filter(|e| graph.edge(e).and_then(|l| l.nesting_edge).unwrap_or(false))
        .collect();
    for e in nesting_edges {
        graph.remove_edge_obj(&e);
    }
}
