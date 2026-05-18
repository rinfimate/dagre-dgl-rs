//! order/sort.rs — sort
//! Faithful port of dagre-js/lib/order/sort.ts

use crate::util::partition;

/// An entry to be sorted by barycenter within a rank layer.
#[derive(Debug, Clone)]
pub struct SortEntry {
    /// Node identifiers in this entry group.
    pub vs: Vec<String>,
    /// Original position index.
    pub i: usize,
    /// Barycenter (average position of neighbours).
    pub barycenter: Option<f64>,
    /// Weight of the entry.
    pub weight: Option<f64>,
}

/// Result of sorting a set of entries within a rank layer.
#[derive(Debug, Clone)]
pub struct SortResult {
    /// Sorted node identifiers.
    pub vs: Vec<String>,
    /// Combined barycenter.
    pub barycenter: Option<f64>,
    /// Combined weight.
    pub weight: Option<f64>,
}

/// Sort rank-layer entries by barycenter, inserting unsortable entries at stable positions.
pub fn sort(entries: Vec<SortEntry>, bias_right: bool) -> SortResult {
    let parts = partition(entries, |e| e.barycenter.is_some());
    let mut sortable = parts.lhs;
    let mut unsortable: Vec<SortEntry> = parts.rhs;
    // unsortable sorted descending by i
    unsortable.sort_by_key(|b| std::cmp::Reverse(b.i));

    let mut vs: Vec<Vec<String>> = Vec::new();
    let mut sum = 0.0f64;
    let mut weight = 0.0f64;
    let mut vs_index = 0usize;

    sortable.sort_by(compare_with_bias(bias_right));

    vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);

    for entry in &sortable {
        vs_index += entry.vs.len();
        vs.push(entry.vs.clone());
        sum += entry.barycenter.unwrap_or(0.0) * entry.weight.unwrap_or(0.0);
        weight += entry.weight.unwrap_or(0.0);
        vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);
    }

    let flat_vs: Vec<String> = vs.into_iter().flatten().collect();
    let mut result = SortResult {
        vs: flat_vs,
        barycenter: None,
        weight: None,
    };
    if weight != 0.0 {
        result.barycenter = Some(sum / weight);
        result.weight = Some(weight);
    }
    result
}

fn consume_unsortable(
    vs: &mut Vec<Vec<String>>,
    unsortable: &mut Vec<SortEntry>,
    mut index: usize,
) -> usize {
    while let Some(last) = unsortable.last() {
        if last.i <= index {
            let last = unsortable.pop().unwrap();
            vs.push(last.vs);
            index += 1;
        } else {
            break;
        }
    }
    index
}

fn compare_with_bias(bias: bool) -> impl Fn(&SortEntry, &SortEntry) -> std::cmp::Ordering {
    // Mirrors the dagre-js tie-breaking exactly:
    //   if (!bias) return a.i > b.i ? 1 : -1;  → descending by i  (right-biased: higher i → right)
    //   else       return a.i < b.i ? 1 : -1;  → ascending by i   (left-biased when bias=true)
    // Note: the JS version never returns 0, making the sort strictly anti-stable
    // in one direction.  We replicate this with explicit Less/Greater.
    move |a: &SortEntry, b: &SortEntry| {
        let ba = a.barycenter.unwrap_or(0.0);
        let bb = b.barycenter.unwrap_or(0.0);
        if ba < bb {
            std::cmp::Ordering::Less
        } else if ba > bb {
            std::cmp::Ordering::Greater
        } else if !bias {
            // bias=false → descending by i (a.i > b.i → a after b → Greater)
            if a.i > b.i {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        } else {
            // bias=true → ascending by i (a.i < b.i → a after b → Greater)
            if a.i < b.i {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }
    }
}
