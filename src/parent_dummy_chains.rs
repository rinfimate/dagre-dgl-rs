//! parent_dummy_chains.rs — parentDummyChains
//! Faithful port of dagre-js/lib/parent-dummy-chains.ts

use crate::graph::Graph;
use crate::util::GRAPH_NODE;
use std::collections::HashMap;

struct PostorderNum {
    low: i32,
    lim: i32,
}

/// Assign dummy-node chains to their nearest common ancestor compound node.
pub fn parent_dummy_chains(graph: &mut Graph) {
    let postorder_nums = postorder(graph);

    let dummy_chains = graph.graph().dummy_chains.clone().unwrap_or_default();
    for start_v in &dummy_chains {
        let mut v = start_v.clone();
        let node = graph.node(&v).clone();
        if let Some(edge_obj) = node.edge_obj.as_ref().cloned() {
            let path_data = find_path(graph, &postorder_nums, &edge_obj.v, &edge_obj.w);
            let path = path_data.0;
            let lca = path_data.1;

            let mut path_idx = 0usize;
            let mut ascending = true;

            loop {
                if v == edge_obj.w {
                    break;
                }

                let cur_node = graph.node(&v).clone();

                if ascending {
                    loop {
                        let pv = path.get(path_idx).cloned().flatten();
                        match &pv {
                            Some(pv_str) if pv_str == &lca.clone().unwrap_or_default() => {
                                ascending = false;
                                break;
                            }
                            Some(pv_str) => {
                                let max_rank = graph.node(pv_str).max_rank.unwrap_or(i32::MAX);
                                if max_rank < cur_node.rank.unwrap_or(0) {
                                    path_idx += 1;
                                } else {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }

                    if path.get(path_idx).cloned().flatten().as_deref() == lca.as_deref() {
                        ascending = false;
                    }
                }

                if !ascending {
                    loop {
                        if path_idx >= path.len() - 1 {
                            break;
                        }
                        let next = path.get(path_idx + 1).cloned().flatten();
                        match next {
                            Some(next_str) => {
                                let min_rank = graph.node(&next_str).min_rank.unwrap_or(i32::MAX);
                                if min_rank <= cur_node.rank.unwrap_or(0) {
                                    path_idx += 1;
                                } else {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }

                let path_v = path.get(path_idx).cloned().flatten();
                if let Some(pv) = path_v {
                    graph.set_parent(&v, Some(&pv));
                }

                // Move to next in chain
                let succs = graph.successors(&v).unwrap_or_default();
                if succs.is_empty() {
                    break;
                }
                v = succs[0].clone();
            }
        }
    }
}

// Find path from v to w through their LCA
// Returns (path, lca)
fn find_path(
    graph: &Graph,
    postorder_nums: &HashMap<String, PostorderNum>,
    v: &str,
    w: &str,
) -> (Vec<Option<String>>, Option<String>) {
    let v_num = postorder_nums.get(v);
    let w_num = postorder_nums.get(w);

    let low = match (v_num, w_num) {
        (Some(vn), Some(wn)) => vn.low.min(wn.low),
        _ => 0,
    };
    let lim = match (v_num, w_num) {
        (Some(vn), Some(wn)) => vn.lim.max(wn.lim),
        _ => 0,
    };

    let mut v_path: Vec<Option<String>> = Vec::new();
    let mut parent = Some(v.to_string());

    loop {
        parent = graph
            .parent(parent.as_deref().unwrap_or(""))
            .map(|p| p.to_string());
        v_path.push(parent.clone());
        if parent.is_none() {
            break;
        }
        let pstr = parent.as_ref().unwrap();
        let pnum = postorder_nums.get(pstr);
        match pnum {
            Some(pn) => {
                if !(pn.low > low || lim > pn.lim) {
                    break;
                }
            }
            None => break,
        }
    }
    let lca = parent.clone();

    let mut w_path: Vec<Option<String>> = Vec::new();
    let mut w_parent = w.to_string();
    loop {
        let par = graph.parent(&w_parent).map(|p| p.to_string());
        match &par {
            Some(p) if Some(p.as_str()) == lca.as_deref() => break,
            None => break,
            Some(p) => {
                w_path.push(Some(p.clone()));
                w_parent = p.clone();
            }
        }
    }

    w_path.reverse();
    let mut path = v_path;
    path.extend(w_path);
    (path, lca)
}

fn postorder(graph: &Graph) -> HashMap<String, PostorderNum> {
    let mut result: HashMap<String, PostorderNum> = HashMap::new();
    let mut lim = 0i32;

    fn dfs(graph: &Graph, v: &str, lim: &mut i32, result: &mut HashMap<String, PostorderNum>) {
        let low = *lim;
        for child in graph.children(v) {
            dfs(graph, &child.clone(), lim, result);
        }
        result.insert(
            v.to_string(),
            PostorderNum {
                low,
                lim: {
                    let l = *lim;
                    *lim += 1;
                    l
                },
            },
        );
    }

    for v in graph.children(GRAPH_NODE) {
        dfs(graph, &v.clone(), &mut lim, &mut result);
    }
    result
}
