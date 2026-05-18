//! order/sort_subgraph.rs — sortSubgraph
//! Faithful port of dagre-js/lib/order/sort-subgraph.ts

use crate::graph::Graph;
use crate::order::barycenter::{barycenter, BarycenterEntry};
use crate::order::resolve_conflicts::{resolve_conflicts, BarycenterInput, ResolvedEntry};
use crate::order::sort::{sort, SortEntry, SortResult};
use std::collections::HashMap;

/// Sort nodes within a subgraph by barycenter, respecting subgraph constraints.
pub fn sort_subgraph(
    graph: &Graph,
    v: &str,
    constraint_graph: &Graph,
    bias_right: bool,
) -> SortResult {
    let mut movable = graph.children(v);
    let node = graph.node_opt(v);

    // borderLeft / borderRight at the "flat" level — stored as first element of the vec
    let bl: Option<String> = node.and_then(|n| {
        n.border_left
            .as_ref()
            .and_then(|v| v.first().and_then(|x| x.clone()))
    });
    let br: Option<String> = node.and_then(|n| {
        n.border_right
            .as_ref()
            .and_then(|v| v.first().and_then(|x| x.clone()))
    });

    if bl.is_some() {
        let bl_str = bl.as_deref().unwrap_or("");
        let br_str = br.as_deref().unwrap_or("");
        movable.retain(|w| w != bl_str && w != br_str);
    }

    let barycenters = barycenter(graph, &movable);

    let mut subgraphs: HashMap<String, SortResult> = HashMap::new();
    let mut bc_entries: Vec<BarycenterEntry> = barycenters.clone();

    for entry in bc_entries.iter_mut() {
        if !graph.children(&entry.v).is_empty() {
            let subgraph_result =
                sort_subgraph(graph, &entry.v.clone(), constraint_graph, bias_right);
            if subgraph_result.barycenter.is_some() {
                merge_barycenters(entry, &subgraph_result);
            }
            subgraphs.insert(entry.v.clone(), subgraph_result);
        }
    }

    let bc_inputs: Vec<BarycenterInput> = bc_entries
        .iter()
        .map(|e| BarycenterInput {
            v: e.v.clone(),
            barycenter: e.barycenter,
            weight: e.weight,
        })
        .collect();

    let mut resolved = resolve_conflicts(&bc_inputs, constraint_graph);
    expand_subgraphs(&mut resolved, &subgraphs);

    let sort_entries: Vec<SortEntry> = resolved
        .iter()
        .map(|e| SortEntry {
            vs: e.vs.clone(),
            i: e.i,
            barycenter: e.barycenter,
            weight: e.weight,
        })
        .collect();

    let mut result = sort(sort_entries, bias_right);

    if let (Some(bl_str), Some(br_str)) = (bl, br) {
        let mut new_vs = vec![bl_str.clone()];
        new_vs.extend(result.vs.clone());
        new_vs.push(br_str.clone());
        result.vs = new_vs;

        let bl_preds = graph.predecessors(&bl_str).unwrap_or_default();
        if !bl_preds.is_empty() {
            let bl_pred = graph.node(&bl_preds[0]);
            let br_preds = graph.predecessors(&br_str).unwrap_or_default();
            let br_pred = graph.node(&br_preds[0]);

            if result.barycenter.is_none() {
                result.barycenter = Some(0.0);
                result.weight = Some(0.0);
            }

            let w = result.weight.unwrap_or(0.0);
            let bc = result.barycenter.unwrap_or(0.0);
            let bl_order = bl_pred.order.unwrap_or(0) as f64;
            let br_order = br_pred.order.unwrap_or(0) as f64;
            result.barycenter = Some((bc * w + bl_order + br_order) / (w + 2.0));
            result.weight = Some(w + 2.0);
        }
    }

    result
}

fn expand_subgraphs(entries: &mut [ResolvedEntry], subgraphs: &HashMap<String, SortResult>) {
    for entry in entries.iter_mut() {
        entry.vs = entry
            .vs
            .iter()
            .flat_map(|v| {
                if let Some(sg) = subgraphs.get(v) {
                    sg.vs.clone()
                } else {
                    vec![v.clone()]
                }
            })
            .collect();
    }
}

fn merge_barycenters(target: &mut BarycenterEntry, other: &SortResult) {
    if let Some(target_bc) = target.barycenter {
        let tw = target.weight.unwrap_or(0.0);
        let ow = other.weight.unwrap_or(0.0);
        let obc = other.barycenter.unwrap_or(0.0);
        target.barycenter = Some((target_bc * tw + obc * ow) / (tw + ow));
        target.weight = Some(tw + ow);
    } else {
        target.barycenter = other.barycenter;
        target.weight = other.weight;
    }
}
