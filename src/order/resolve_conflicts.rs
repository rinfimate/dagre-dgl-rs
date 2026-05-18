//! order/resolve_conflicts.rs — resolveConflicts
//! Faithful port of dagre-js/lib/order/resolve-conflicts.ts

use crate::graph::Graph;
use std::collections::HashMap;

/// Input entry for the conflict-resolution algorithm.
#[derive(Debug, Clone)]
pub struct BarycenterInput {
    /// Node identifier.
    pub v: String,
    /// Barycenter value (weighted average position).
    pub barycenter: Option<f64>,
    /// Weight associated with this entry.
    pub weight: Option<f64>,
}

/// A resolved group of nodes after conflict resolution.
#[derive(Debug, Clone)]
pub struct ResolvedEntry {
    /// Nodes in this group, in order.
    pub vs: Vec<String>,
    /// Original index of the group.
    pub i: usize,
    /// Merged barycenter value.
    pub barycenter: Option<f64>,
    /// Merged weight.
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
struct MappedEntry {
    indegree: usize,
    ins: Vec<usize>, // indices into mapped_entries_vec
    outs: Vec<usize>,
    vs: Vec<String>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

/// Resolve ordering conflicts among barycenter entries using a constraint graph.
pub fn resolve_conflicts(
    entries: &[BarycenterInput],
    constraint_graph: &Graph,
) -> Vec<ResolvedEntry> {
    let _n = entries.len();
    let mut mapped: Vec<MappedEntry> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| MappedEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            vs: vec![entry.v.clone()],
            i,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        })
        .collect();

    // Build a name -> index map
    let name_to_idx: HashMap<String, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.v.clone(), i))
        .collect();

    for e in constraint_graph.edges() {
        if let (Some(&vi), Some(&wi)) = (name_to_idx.get(&e.v), name_to_idx.get(&e.w)) {
            mapped[wi].indegree += 1;
            mapped[vi].outs.push(wi);
        }
    }

    let source_set: Vec<usize> = mapped
        .iter()
        .enumerate()
        .filter(|(_, m)| m.indegree == 0)
        .map(|(i, _)| i)
        .collect();

    do_resolve_conflicts(mapped, source_set)
}

fn do_resolve_conflicts(
    mut mapped: Vec<MappedEntry>,
    mut source_set: Vec<usize>,
) -> Vec<ResolvedEntry> {
    let mut result_indices: Vec<usize> = Vec::new();

    // dagre-js uses sourceSet.shift() (FIFO queue), not pop() (LIFO stack).
    // Using pop() reverses tie-breaking order when barycenters are equal,
    // causing nodes at the same rank to appear in wrong order.
    while !source_set.is_empty() {
        let entry_idx = source_set.remove(0);
        result_indices.push(entry_idx);

        // handle_in: process ins in reverse order
        let ins: Vec<usize> = mapped[entry_idx].ins.clone();
        for u_idx in ins.into_iter().rev() {
            if mapped[u_idx].merged {
                continue;
            }
            let u_bc = mapped[u_idx].barycenter;
            let v_bc = mapped[entry_idx].barycenter;
            if u_bc.is_none() || v_bc.is_none() || u_bc.unwrap() >= v_bc.unwrap() {
                merge_entries(&mut mapped, entry_idx, u_idx);
            }
        }

        // handle_out: process outs
        let outs: Vec<usize> = mapped[entry_idx].outs.clone();
        for w_idx in outs {
            mapped[w_idx].ins.push(entry_idx);
            mapped[w_idx].indegree -= 1;
            if mapped[w_idx].indegree == 0 {
                source_set.push(w_idx);
            }
        }
    }

    result_indices
        .into_iter()
        .filter(|&i| !mapped[i].merged)
        .map(|i| {
            let m = &mapped[i];
            ResolvedEntry {
                vs: m.vs.clone(),
                i: m.i,
                barycenter: m.barycenter,
                weight: m.weight,
            }
        })
        .collect()
}

fn merge_entries(mapped: &mut [MappedEntry], target_idx: usize, source_idx: usize) {
    let mut sum = 0.0f64;
    let mut weight = 0.0f64;

    if let Some(w) = mapped[target_idx].weight {
        sum += mapped[target_idx].barycenter.unwrap_or(0.0) * w;
        weight += w;
    }
    if let Some(w) = mapped[source_idx].weight {
        sum += mapped[source_idx].barycenter.unwrap_or(0.0) * w;
        weight += w;
    }

    let mut source_vs = mapped[source_idx].vs.clone();
    source_vs.extend(mapped[target_idx].vs.clone());
    let source_i = mapped[source_idx].i;
    let target_i = mapped[target_idx].i;

    mapped[target_idx].vs = source_vs;
    mapped[target_idx].barycenter = Some(sum / weight);
    mapped[target_idx].weight = Some(weight);
    mapped[target_idx].i = source_i.min(target_i);
    mapped[source_idx].merged = true;
}
