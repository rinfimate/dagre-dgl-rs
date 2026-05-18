//! coordinate_system.rs — adjust/undo
//! Faithful port of dagre-js/lib/coordinate-system.ts

use crate::graph::Graph;

/// Adjusts node dimensions for non-LR layouts by swapping width/height where needed.
pub fn adjust(graph: &mut Graph) {
    let rankdir = graph
        .graph()
        .rankdir
        .clone()
        .unwrap_or_default()
        .to_lowercase();
    if rankdir == "lr" || rankdir == "rl" {
        swap_width_height(graph);
    }
}

/// Reverses the coordinate-system transformation applied by [`adjust`].
pub fn undo(graph: &mut Graph) {
    let rankdir = graph
        .graph()
        .rankdir
        .clone()
        .unwrap_or_default()
        .to_lowercase();
    if rankdir == "bt" || rankdir == "rl" {
        reverse_y(graph);
    }
    if rankdir == "lr" || rankdir == "rl" {
        swap_xy(graph);
        swap_width_height(graph);
    }
}

fn swap_width_height(graph: &mut Graph) {
    for v in graph.nodes() {
        let node = graph.node_mut(&v);
        std::mem::swap(&mut node.width, &mut node.height);
    }
    for e in graph.edges() {
        if let Some(label) = graph.edge_mut(&e) {
            let w = label.width;
            label.width = Some(label.height.unwrap_or(0.0));
            label.height = w;
        }
    }
}

fn reverse_y(graph: &mut Graph) {
    for v in graph.nodes() {
        let node = graph.node_mut(&v);
        if let Some(y) = node.y.as_mut() {
            *y = -*y;
        }
    }
    for e in graph.edges() {
        if let Some(label) = graph.edge_mut(&e) {
            if let Some(points) = label.points.as_mut() {
                for p in points.iter_mut() {
                    p.y = -p.y;
                }
            }
            if label.y.is_some() {
                if let Some(y) = label.y.as_mut() {
                    *y = -*y;
                }
            }
        }
    }
}

fn swap_xy(graph: &mut Graph) {
    for v in graph.nodes() {
        let node = graph.node_mut(&v);
        std::mem::swap(&mut node.x, &mut node.y);
    }
    for e in graph.edges() {
        if let Some(label) = graph.edge_mut(&e) {
            if let Some(points) = label.points.as_mut() {
                for p in points.iter_mut() {
                    std::mem::swap(&mut p.x, &mut p.y);
                }
            }
            if label.x.is_some() {
                std::mem::swap(&mut label.x, &mut label.y);
            }
        }
    }
}
