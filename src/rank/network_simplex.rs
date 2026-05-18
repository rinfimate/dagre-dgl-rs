//! rank/network_simplex.rs — networkSimplex
//! Faithful port of dagre-js/lib/rank/network-simplex.ts

use crate::graph::{postorder, preorder, Edge, EdgeLabel, Graph, NodeLabel};
use crate::rank::feasible_tree::feasible_tree;
use crate::rank::util::{longest_path as init_rank, slack};
use crate::util::simplify;
use std::collections::HashMap;

/// The network simplex algorithm assigns ranks to each node in the input graph
/// and iteratively improves the ranking to reduce the length of edges.
pub fn network_simplex(graph: &mut Graph) {
    // Work on simplified copy (no multi-edges)
    let mut g = simplify(graph);
    init_rank(&mut g);

    let mut t = feasible_tree(&mut g);
    init_low_lim_values(&mut t, None);
    init_cut_values(&mut t, &g);

    loop {
        match leave_edge(&t) {
            None => break,
            Some(e) => {
                match try_enter_edge(&t, &g, &e) {
                    None => break, // disconnected component, no improvement possible
                    Some(f) => exchange_edges(&mut t, &mut g, &e, &f),
                }
            }
        }
    }

    // Copy ranks back to original graph
    for v in graph.nodes() {
        if let Some(rank) = g.node_opt(&v).and_then(|n| n.rank) {
            graph.node_mut(&v).rank = Some(rank);
        }
    }
}

/// Initializes cut values for all edges in the tree.
pub fn init_cut_values(tree: &mut Graph, graph: &Graph) {
    let mut visited_nodes = postorder(tree, tree.nodes());
    // Remove last element (root)
    if !visited_nodes.is_empty() {
        visited_nodes.pop();
    }
    for v in visited_nodes {
        assign_cut_value(tree, graph, &v.clone());
    }
}

fn assign_cut_value(tree: &mut Graph, graph: &Graph, child: &str) {
    let parent = {
        let child_lab = tree.node(child);
        child_lab
            .parent_node
            .clone()
            .expect("child must have parent")
    };
    let cv = calc_cut_value(tree, graph, child);
    // Try both orientations (undirected tree)
    if tree.edge_vw(child, &parent).is_some() {
        tree.edge_vw_mut(child, &parent).unwrap().cutvalue = Some(cv);
    } else if tree.edge_vw(&parent, child).is_some() {
        tree.edge_vw_mut(&parent, child).unwrap().cutvalue = Some(cv);
    }
}

/// Given the tight tree, its graph, and a child in the graph calculate and
/// return the cut value for the edge between the child and its parent.
pub fn calc_cut_value(tree: &Graph, graph: &Graph, child: &str) -> f64 {
    let parent = tree
        .node(child)
        .parent_node
        .clone()
        .expect("child must have parent in tree");
    let mut child_is_tail = true;
    let graph_edge = graph.edge_vw(child, &parent);
    let graph_edge_weight = if let Some(e) = graph_edge {
        e.weight.unwrap_or(0.0)
    } else {
        child_is_tail = false;
        graph
            .edge_vw(&parent, child)
            .map_or(0.0, |e| e.weight.unwrap_or(0.0))
    };

    let mut cut_value = graph_edge_weight;

    let node_edges: Vec<Edge> = graph.node_edges(child).unwrap_or_default();
    for edge in node_edges {
        let is_out_edge = edge.v == child;
        let other = if is_out_edge {
            edge.w.clone()
        } else {
            edge.v.clone()
        };

        if other != parent {
            let points_to_head = is_out_edge == child_is_tail;
            let other_weight = graph.edge(&edge).map_or(0.0, |e| e.weight.unwrap_or(0.0));

            cut_value += if points_to_head {
                other_weight
            } else {
                -other_weight
            };
            if is_tree_edge(tree, child, &other) {
                let tree_edge_cv = tree
                    .edge_vw(child, &other)
                    .or_else(|| tree.edge_vw(&other, child))
                    .and_then(|e| e.cutvalue)
                    .unwrap_or(0.0);
                cut_value += if points_to_head {
                    -tree_edge_cv
                } else {
                    tree_edge_cv
                };
            }
        }
    }

    cut_value
}

/// Initialise `low` and `lim` values on spanning-tree nodes for cut-value computation.
pub fn init_low_lim_values(tree: &mut Graph, root: Option<String>) {
    let root = root.unwrap_or_else(|| tree.nodes().into_iter().next().unwrap_or_default());
    dfs_assign_low_lim(tree, &mut HashMap::new(), 1, &root, None);
}

fn dfs_assign_low_lim(
    tree: &mut Graph,
    visited: &mut HashMap<String, bool>,
    mut next_lim: i32,
    v: &str,
    parent: Option<&str>,
) -> i32 {
    let low = next_lim;
    visited.insert(v.to_string(), true);

    let neighbors: Vec<String> = tree.neighbors(v).unwrap_or_default();
    for w in neighbors {
        if !visited.contains_key(&w) {
            next_lim = dfs_assign_low_lim(tree, visited, next_lim, &w.clone(), Some(v));
        }
    }

    {
        let label = tree.node_mut(v);
        label.low = Some(low);
        label.lim = Some(next_lim);
        label.parent_node = parent.map(|p| p.to_string());
    }
    next_lim + 1
}

/// Find a spanning-tree edge with a negative cut value (candidate to leave the tree).
pub fn leave_edge(tree: &Graph) -> Option<Edge> {
    tree.edges().into_iter().find(|e| {
        tree.edge(e)
            .and_then(|l| l.cutvalue)
            .is_some_and(|cv| cv < 0.0)
    })
}

fn try_enter_edge(tree: &Graph, graph: &Graph, edge: &Edge) -> Option<Edge> {
    enter_edge(tree, graph, edge)
}

/// Find the non-tree edge with the least slack to enter the spanning tree in place of `edge`.
pub fn enter_edge(tree: &Graph, graph: &Graph, edge: &Edge) -> Option<Edge> {
    let (mut v, mut w) = (edge.v.clone(), edge.w.clone());

    // Ensure v is tail and w is head
    if !graph.has_edge(&v, &w) {
        std::mem::swap(&mut v, &mut w);
    }

    let v_label = match tree.node_opt(&v) {
        Some(l) => l.clone(),
        None => return None,
    };
    let w_label = match tree.node_opt(&w) {
        Some(l) => l.clone(),
        None => return None,
    };
    let (tail_label, flip) = if v_label.lim.unwrap_or(0) > w_label.lim.unwrap_or(0) {
        (w_label, true)
    } else {
        (v_label, false)
    };

    // JS tree.node(v) returns undefined for nodes not in the tree;
    // is_descendant(undefined, ...) is false. Mirror that with node_opt.
    let candidates: Vec<Edge> = graph
        .edges()
        .into_iter()
        .filter(|e| match (tree.node_opt(&e.v), tree.node_opt(&e.w)) {
            (Some(ev_label), Some(ew_label)) => {
                flip == is_descendant(ev_label, &tail_label)
                    && flip != is_descendant(ew_label, &tail_label)
            }
            _ => false,
        })
        .collect();

    candidates.into_iter().reduce(|acc, e| {
        if slack(graph, &e) < slack(graph, &acc) {
            e
        } else {
            acc
        }
    })
}

/// Swap edge `e` out of the spanning tree and insert edge `f`, then recompute ranks and cut values.
pub fn exchange_edges(tree: &mut Graph, graph: &mut Graph, e: &Edge, f: &Edge) {
    tree.remove_edge_obj(e);
    tree.set_edge(&f.v, &f.w, EdgeLabel::default(), None);
    init_low_lim_values(tree, None);
    init_cut_values(tree, graph);
    update_ranks(tree, graph);
}

fn update_ranks(tree: &Graph, graph: &mut Graph) {
    let root = tree
        .nodes()
        .into_iter()
        .find(|v| tree.node(v).parent_node.is_none());
    let root = match root {
        Some(r) => r,
        None => return,
    };

    let mut vs = preorder(tree, vec![root]);
    if !vs.is_empty() {
        vs.remove(0);
    }

    for v in vs {
        let parent = tree.node(&v).parent_node.clone().expect("must have parent");
        let (edge, flipped) = if graph.edge_vw(&v, &parent).is_some() {
            (graph.edge_vw(&v, &parent).cloned(), false)
        } else {
            (graph.edge_vw(&parent, &v).cloned(), true)
        };
        let minlen = edge.as_ref().and_then(|e| e.minlen).unwrap_or(1);
        let parent_rank = graph.node(&parent).rank.unwrap_or(0);
        graph.node_mut(&v).rank = Some(parent_rank + if flipped { minlen } else { -minlen });
    }
}

fn is_tree_edge(tree: &Graph, u: &str, v: &str) -> bool {
    tree.has_edge(u, v)
}

fn is_descendant(v_label: &NodeLabel, root_label: &NodeLabel) -> bool {
    let root_low = root_label.low.unwrap_or(0);
    let root_lim = root_label.lim.unwrap_or(0);
    let v_lim = v_label.lim.unwrap_or(0);
    root_low <= v_lim && v_lim <= root_lim
}
