//! list.rs — Doubly linked list used by greedy-fas.
//!
//! Faithful port of dagre-js/lib/data/list.ts
//! The JS version stores arbitrary objects as list nodes and mutates _prev/_next
//! pointers on the objects themselves.
//!
//! In Rust we store indices into a Vec as the "pointers".

/// A handle to a node in the List.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListHandle(pub usize);

struct ListNode {
    prev: usize,
    next: usize,
}

/// Doubly linked list.
/// Nodes are identified by ListHandle values.
/// The sentinel node is always at index 0.
pub struct List {
    nodes: Vec<ListNode>,
    linked: Vec<bool>, // whether node i is currently in the list
}

impl List {
    /// Creates a new empty doubly linked list with a sentinel node.
    pub fn new() -> Self {
        // Sentinel at index 0: next=0, prev=0
        List {
            nodes: vec![ListNode { prev: 0, next: 0 }],
            linked: vec![false], // sentinel is never "linked" in user sense
        }
    }

    /// Allocate a new handle (not yet in the list).
    pub fn alloc(&mut self) -> ListHandle {
        let idx = self.nodes.len();
        self.nodes.push(ListNode { prev: 0, next: 0 });
        self.linked.push(false);
        ListHandle(idx)
    }

    /// Enqueue: insert at the front (after sentinel).
    pub fn enqueue(&mut self, h: ListHandle) {
        let idx = h.0;
        // If already linked, unlink first
        if self.linked[idx] {
            self.unlink(idx);
        }
        let sentinel = 0usize;
        let old_next = self.nodes[sentinel].next;
        self.nodes[idx].next = old_next;
        self.nodes[idx].prev = sentinel;
        self.nodes[old_next].prev = idx;
        self.nodes[sentinel].next = idx;
        self.linked[idx] = true;
    }

    /// Dequeue: remove from the back (before sentinel).
    pub fn dequeue(&mut self) -> Option<ListHandle> {
        let sentinel = 0usize;
        let entry = self.nodes[sentinel].prev;
        if entry == sentinel {
            return None;
        }
        self.unlink(entry);
        Some(ListHandle(entry))
    }

    fn unlink(&mut self, idx: usize) {
        let prev = self.nodes[idx].prev;
        let next = self.nodes[idx].next;
        self.nodes[prev].next = next;
        self.nodes[next].prev = prev;
        self.linked[idx] = false;
    }

    /// Returns `true` if the node identified by `h` is currently in the list.
    pub fn is_linked(&self, h: ListHandle) -> bool {
        self.linked[h.0]
    }
}

impl Default for List {
    fn default() -> Self {
        List::new()
    }
}
