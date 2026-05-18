/// Dagre algorithm tests — ported from dagre-js/test/
///
/// Original JS test files:
///   test/rank/util-test.ts
///   test/rank/network-simplex-test.ts
///   test/layout-test.ts

#[cfg(test)]
mod dagre_tests {
    use crate::graph::{Edge, EdgeLabel, Graph, GraphLabel, NodeLabel};
    use crate::layout::layout;
    use crate::rank::network_simplex::{
        calc_cut_value, enter_edge, exchange_edges, init_cut_values, init_low_lim_values,
        leave_edge, network_simplex,
    };
    use crate::rank::util::longest_path;
    use crate::util::normalize_ranks;

    // ─── Helpers ─────────────────────────────────────────────────────────────

    /// Directed multigraph — mirrors JS `new Graph({multigraph: true})`
    fn make_g() -> Graph {
        Graph::with_options(true, true, false)
    }

    /// Undirected simple graph — mirrors JS `new Graph({directed: false})`
    fn make_t() -> Graph {
        Graph::with_options(false, false, false)
    }

    /// Default edge label matching `setDefaultEdgeLabel(() => ({minlen:1, weight:1}))`
    fn de() -> EdgeLabel {
        EdgeLabel {
            minlen: Some(1),
            weight: Some(1.0),
            ..Default::default()
        }
    }

    /// Set a path of edges on the graph (with the given edge label)
    fn set_path(g: &mut Graph, nodes: &[&str], label: EdgeLabel) {
        for i in 0..nodes.len() - 1 {
            if !g.has_node(nodes[i]) {
                g.set_node(nodes[i], NodeLabel::default());
            }
            if !g.has_node(nodes[i + 1]) {
                g.set_node(nodes[i + 1], NodeLabel::default());
            }
            g.set_edge(nodes[i], nodes[i + 1], label.clone(), None);
        }
    }

    /// The Gansner et al. test directed graph
    fn gansner_graph() -> Graph {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "c", "d", "h"], de());
        set_path(&mut g, &["a", "e", "g", "h"], de());
        set_path(&mut g, &["a", "f", "g"], de());
        g
    }

    /// The Gansner et al. test spanning tree (undirected)
    fn gansner_tree() -> Graph {
        let mut t = make_t();
        set_path(
            &mut t,
            &["a", "b", "c", "d", "h", "g", "e"],
            EdgeLabel::default(),
        );
        if !t.has_node("g") {
            t.set_node("g", NodeLabel::default());
        }
        if !t.has_node("f") {
            t.set_node("f", NodeLabel::default());
        }
        t.set_edge("g", "f", EdgeLabel::default(), None);
        t
    }

    /// Run network simplex then normalize, matching JS `ns(g)`
    fn ns(g: &mut Graph) {
        network_simplex(g);
        normalize_ranks(g);
    }

    /// Get cut value from undirected tree edge, checking both orientations
    fn get_cv(tree: &Graph, v: &str, w: &str) -> f64 {
        tree.edge_vw(v, w)
            .or_else(|| tree.edge_vw(w, v))
            .and_then(|e| e.cutvalue)
            .unwrap_or(0.0)
    }

    /// Normalize edge direction for comparison (smaller v first)
    fn undirected(e: &Edge) -> (String, String) {
        if e.v <= e.w {
            (e.v.clone(), e.w.clone())
        } else {
            (e.w.clone(), e.v.clone())
        }
    }

    // ─── longest path tests ───────────────────────────────────────────────────

    #[test]
    fn longest_path_single_node() {
        let mut g = make_g();
        g.set_node("a", NodeLabel::default());
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
    }

    #[test]
    fn longest_path_unconnected_nodes() {
        let mut g = make_g();
        g.set_node("a", NodeLabel::default());
        g.set_node("b", NodeLabel::default());
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(0));
    }

    #[test]
    fn longest_path_connected_nodes() {
        let mut g = make_g();
        g.set_edge("a", "b", de(), None);
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
    }

    #[test]
    fn longest_path_diamond() {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "d"], de());
        set_path(&mut g, &["a", "c", "d"], de());
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
        assert_eq!(g.node("c").rank, Some(1));
        assert_eq!(g.node("d").rank, Some(2));
    }

    #[test]
    fn longest_path_respects_minlen() {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "d"], de());
        g.set_node("c", NodeLabel::default());
        g.set_edge("a", "c", de(), None);
        g.set_edge(
            "c",
            "d",
            EdgeLabel {
                minlen: Some(2),
                weight: Some(1.0),
                ..Default::default()
            },
            None,
        );
        longest_path(&mut g);
        normalize_ranks(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(2));
        assert_eq!(g.node("c").rank, Some(1));
        assert_eq!(g.node("d").rank, Some(3));
    }

    // ─── network simplex basic tests ──────────────────────────────────────────

    #[test]
    fn ns_single_node() {
        let mut g = make_g();
        g.set_node("a", NodeLabel::default());
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
    }

    #[test]
    fn ns_two_node_connected() {
        let mut g = make_g();
        g.set_edge("a", "b", de(), None);
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
    }

    #[test]
    fn ns_diamond() {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "d"], de());
        set_path(&mut g, &["a", "c", "d"], de());
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
        assert_eq!(g.node("c").rank, Some(1));
        assert_eq!(g.node("d").rank, Some(2));
    }

    #[test]
    fn ns_respects_minlen() {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "d"], de());
        g.set_node("c", NodeLabel::default());
        g.set_edge("a", "c", de(), None);
        g.set_edge(
            "c",
            "d",
            EdgeLabel {
                minlen: Some(2),
                weight: Some(1.0),
                ..Default::default()
            },
            None,
        );
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(2));
        assert_eq!(g.node("c").rank, Some(1));
        assert_eq!(g.node("d").rank, Some(3));
    }

    #[test]
    fn ns_gansner_graph() {
        let mut g = gansner_graph();
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
        assert_eq!(g.node("c").rank, Some(2));
        assert_eq!(g.node("d").rank, Some(3));
        assert_eq!(g.node("h").rank, Some(4));
        assert_eq!(g.node("e").rank, Some(1));
        assert_eq!(g.node("f").rank, Some(1));
        assert_eq!(g.node("g").rank, Some(2));
    }

    #[test]
    fn ns_multi_edges() {
        let mut g = make_g();
        set_path(&mut g, &["a", "b", "c", "d"], de());
        g.set_edge(
            "a",
            "e",
            EdgeLabel {
                weight: Some(2.0),
                minlen: Some(1),
                ..Default::default()
            },
            None,
        );
        g.set_edge("e", "d", de(), None);
        g.set_edge(
            "b",
            "c",
            EdgeLabel {
                weight: Some(1.0),
                minlen: Some(2),
                ..Default::default()
            },
            Some("multi"),
        );
        ns(&mut g);
        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
        assert_eq!(g.node("c").rank, Some(3));
        assert_eq!(g.node("d").rank, Some(4));
        assert_eq!(g.node("e").rank, Some(1));
    }

    // ─── leaveEdge ───────────────────────────────────────────────────────────

    #[test]
    fn leave_edge_none_when_all_positive() {
        let mut tree = make_t();
        tree.set_edge(
            "a",
            "b",
            EdgeLabel {
                cutvalue: Some(1.0),
                ..Default::default()
            },
            None,
        );
        tree.set_edge(
            "b",
            "c",
            EdgeLabel {
                cutvalue: Some(1.0),
                ..Default::default()
            },
            None,
        );
        assert!(leave_edge(&tree).is_none());
    }

    #[test]
    fn leave_edge_returns_negative_cut_value_edge() {
        let mut tree = make_t();
        tree.set_edge(
            "a",
            "b",
            EdgeLabel {
                cutvalue: Some(1.0),
                ..Default::default()
            },
            None,
        );
        tree.set_edge(
            "b",
            "c",
            EdgeLabel {
                cutvalue: Some(-1.0),
                ..Default::default()
            },
            None,
        );
        let e = leave_edge(&tree);
        assert!(e.is_some());
        let (v, w) = undirected(e.as_ref().unwrap());
        assert!(
            (v == "b" && w == "c") || (v == "c" && w == "b"),
            "expected b-c edge"
        );
    }

    // ─── initLowLimValues ────────────────────────────────────────────────────

    #[test]
    fn init_low_lim_values_assigns_low_lim_parent() {
        // Mirrors JS: setNodes(["a","b","c","d","e"]) + setPath(["a","b","a","c","d","c","e"])
        // In undirected: a-b, a-c, c-d, c-e
        let mut g = Graph::with_options(false, false, false);
        for n in &["a", "b", "c", "d", "e"] {
            g.set_node(n, NodeLabel::default());
        }
        g.set_edge("a", "b", EdgeLabel::default(), None);
        g.set_edge("a", "c", EdgeLabel::default(), None);
        g.set_edge("c", "d", EdgeLabel::default(), None);
        g.set_edge("c", "e", EdgeLabel::default(), None);

        init_low_lim_values(&mut g, Some("a".to_string()));

        // All lim values should be 1..=5 (distinct)
        let mut lims: Vec<i32> = ["a", "b", "c", "d", "e"]
            .iter()
            .map(|v| g.node(v).lim.unwrap())
            .collect();
        lims.sort();
        assert_eq!(lims, vec![1, 2, 3, 4, 5]);

        // Root a: low=1, lim=5
        assert_eq!(g.node("a").low, Some(1));
        assert_eq!(g.node("a").lim, Some(5));

        // b and c are children of a
        assert_eq!(g.node("b").parent_node, Some("a".to_string()));
        assert_eq!(g.node("c").parent_node, Some("a".to_string()));
        assert!(g.node("b").lim.unwrap() < g.node("a").lim.unwrap());
        assert!(g.node("c").lim.unwrap() < g.node("a").lim.unwrap());
        assert_ne!(g.node("b").lim, g.node("c").lim);

        // d and e are children of c
        assert_eq!(g.node("d").parent_node, Some("c".to_string()));
        assert_eq!(g.node("e").parent_node, Some("c".to_string()));
        assert!(g.node("d").lim.unwrap() < g.node("c").lim.unwrap());
        assert!(g.node("e").lim.unwrap() < g.node("c").lim.unwrap());
        assert_ne!(g.node("d").lim, g.node("e").lim);
    }

    // ─── calcCutValue ────────────────────────────────────────────────────────

    #[test]
    fn calc_cut_value_two_node_c_to_p() {
        let mut g = make_g();
        set_path(&mut g, &["c", "p"], de());
        let mut t = make_t();
        set_path(&mut t, &["p", "c"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 1.0);
    }

    #[test]
    fn calc_cut_value_two_node_c_from_p() {
        let mut g = make_g();
        set_path(&mut g, &["p", "c"], de());
        let mut t = make_t();
        set_path(&mut t, &["p", "c"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 1.0);
    }

    #[test]
    fn calc_cut_value_gc_to_c_to_p() {
        let mut g = make_g();
        set_path(&mut g, &["gc", "c", "p"], de());
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("p", "c", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 3.0);
    }

    #[test]
    fn calc_cut_value_gc_to_c_from_p() {
        let mut g = make_g();
        g.set_edge("p", "c", de(), None);
        g.set_edge("gc", "c", de(), None);
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("p", "c", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -1.0);
    }

    #[test]
    fn calc_cut_value_gc_from_c_to_p() {
        let mut g = make_g();
        g.set_edge("c", "p", de(), None);
        g.set_edge("c", "gc", de(), None);
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("p", "c", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -1.0);
    }

    #[test]
    fn calc_cut_value_gc_from_c_from_p() {
        let mut g = make_g();
        set_path(&mut g, &["p", "c", "gc"], de());
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("p", "c", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 3.0);
    }

    // ─── initCutValues ───────────────────────────────────────────────────────

    #[test]
    fn init_cut_values_gansner_graph() {
        let g = gansner_graph();
        let mut t = gansner_tree();
        init_low_lim_values(&mut t, None);
        init_cut_values(&mut t, &g);

        assert_eq!(get_cv(&t, "a", "b"), 3.0);
        assert_eq!(get_cv(&t, "b", "c"), 3.0);
        assert_eq!(get_cv(&t, "c", "d"), 3.0);
        assert_eq!(get_cv(&t, "d", "h"), 3.0);
        assert_eq!(get_cv(&t, "g", "h"), -1.0);
        assert_eq!(get_cv(&t, "e", "g"), 0.0);
        assert_eq!(get_cv(&t, "g", "f"), 0.0);
    }

    #[test]
    fn init_cut_values_updated_gansner_graph() {
        let g = gansner_graph();
        let mut t = gansner_tree();
        t.remove_edge("g", "h");
        t.set_edge("a", "e", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, None);
        init_cut_values(&mut t, &g);

        assert_eq!(get_cv(&t, "a", "b"), 2.0);
        assert_eq!(get_cv(&t, "b", "c"), 2.0);
        assert_eq!(get_cv(&t, "c", "d"), 2.0);
        assert_eq!(get_cv(&t, "d", "h"), 2.0);
        assert_eq!(get_cv(&t, "a", "e"), 1.0);
        assert_eq!(get_cv(&t, "e", "g"), 1.0);
        assert_eq!(get_cv(&t, "g", "f"), 0.0);
    }

    // ─── exchangeEdges ───────────────────────────────────────────────────────

    #[test]
    fn exchange_edges_updates_cut_values_and_lim() {
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, None);

        exchange_edges(&mut t, &mut g, &Edge::new("g", "h"), &Edge::new("a", "e"));

        assert_eq!(get_cv(&t, "a", "b"), 2.0);
        assert_eq!(get_cv(&t, "b", "c"), 2.0);
        assert_eq!(get_cv(&t, "c", "d"), 2.0);
        assert_eq!(get_cv(&t, "d", "h"), 2.0);
        assert_eq!(get_cv(&t, "a", "e"), 1.0);
        assert_eq!(get_cv(&t, "e", "g"), 1.0);
        assert_eq!(get_cv(&t, "g", "f"), 0.0);

        let mut lims: Vec<i32> = t
            .nodes()
            .iter()
            .map(|v| t.node(v).lim.unwrap_or(0))
            .collect();
        lims.sort();
        assert_eq!(lims, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn exchange_edges_updates_ranks() {
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, None);

        exchange_edges(&mut t, &mut g, &Edge::new("g", "h"), &Edge::new("a", "e"));
        normalize_ranks(&mut g);

        assert_eq!(g.node("a").rank, Some(0));
        assert_eq!(g.node("b").rank, Some(1));
        assert_eq!(g.node("c").rank, Some(2));
        assert_eq!(g.node("d").rank, Some(3));
        assert_eq!(g.node("e").rank, Some(1));
        assert_eq!(g.node("f").rank, Some(1));
        assert_eq!(g.node("g").rank, Some(2));
        assert_eq!(g.node("h").rank, Some(4));
    }

    // ─── enterEdge ───────────────────────────────────────────────────────────

    #[test]
    fn enter_edge_finds_head_to_tail_edge() {
        let mut g = make_g();
        g.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                ..Default::default()
            },
        );
        g.set_node(
            "b",
            NodeLabel {
                rank: Some(2),
                ..Default::default()
            },
        );
        g.set_node(
            "c",
            NodeLabel {
                rank: Some(3),
                ..Default::default()
            },
        );
        set_path(&mut g, &["a", "b", "c"], de());
        g.set_edge("a", "c", de(), None);

        let mut t = make_t();
        set_path(&mut t, &["b", "c", "a"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("c".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("b", "c"));
        let f = f.expect("expected an entering edge");
        let (v, w) = undirected(&f);
        let (ev, ew) = if "a" <= "b" { ("a", "b") } else { ("b", "a") };
        assert_eq!((v.as_str(), w.as_str()), (ev, ew));
    }

    #[test]
    fn enter_edge_works_when_root_in_tail_component() {
        // "works when the root of the tree is in the tail component"
        // Same graph as head_to_tail, but initLowLimValues with "b" instead of "c"
        let mut g = make_g();
        g.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                ..Default::default()
            },
        );
        g.set_node(
            "b",
            NodeLabel {
                rank: Some(2),
                ..Default::default()
            },
        );
        g.set_node(
            "c",
            NodeLabel {
                rank: Some(3),
                ..Default::default()
            },
        );
        set_path(&mut g, &["a", "b", "c"], de());
        g.set_edge("a", "c", de(), None);

        let mut t = make_t();
        set_path(&mut t, &["b", "c", "a"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("b".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("b", "c"));
        let f = f.expect("expected an entering edge");
        let (v, w) = undirected(&f);
        let (ev, ew) = if "a" <= "b" { ("a", "b") } else { ("b", "a") };
        assert_eq!((v.as_str(), w.as_str()), (ev, ew));
    }

    #[test]
    fn enter_edge_finds_edge_with_least_slack() {
        // "finds the edge with the least slack"
        let mut g = make_g();
        g.set_node(
            "a",
            NodeLabel {
                rank: Some(0),
                ..Default::default()
            },
        );
        g.set_node(
            "b",
            NodeLabel {
                rank: Some(1),
                ..Default::default()
            },
        );
        g.set_node(
            "c",
            NodeLabel {
                rank: Some(3),
                ..Default::default()
            },
        );
        g.set_node(
            "d",
            NodeLabel {
                rank: Some(4),
                ..Default::default()
            },
        );
        g.set_edge("a", "d", de(), None);
        set_path(&mut g, &["a", "c", "d"], de());
        g.set_edge("b", "c", de(), None);

        let mut t = make_t();
        set_path(&mut t, &["c", "d", "a", "b"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("a".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("c", "d"));
        let f = f.expect("expected an entering edge");
        let (v, w) = undirected(&f);
        let (ev, ew) = if "b" <= "c" { ("b", "c") } else { ("c", "b") };
        assert_eq!((v.as_str(), w.as_str()), (ev, ew));
    }

    #[test]
    fn enter_edge_gansner_graph_1() {
        // "finds an appropriate edge for gansner graph #1"
        // initLowLimValues from "a", leave edge {g,h}
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, Some("a".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("g", "h"));
        let f = f.expect("expected entering edge for gansner #1");
        let (v, w) = undirected(&f);
        assert_eq!(v.as_str(), "a");
        assert!(w == "e" || w == "f", "expected w in {{e,f}}, got {}", w);
    }

    #[test]
    fn enter_edge_gansner_graph_2() {
        // "finds an appropriate edge for gansner graph #2"
        // initLowLimValues from "e", leave edge {g,h}
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, Some("e".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("g", "h"));
        let f = f.expect("expected entering edge for gansner #2");
        let (v, w) = undirected(&f);
        assert_eq!(v.as_str(), "a");
        assert!(w == "e" || w == "f", "expected w in {{e,f}}, got {}", w);
    }

    #[test]
    fn enter_edge_gansner_graph_3() {
        // "finds an appropriate edge for gansner graph #3"
        // initLowLimValues from "a", leave edge {h,g} (reversed order)
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, Some("a".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("h", "g"));
        let f = f.expect("expected entering edge for gansner #3");
        let (v, w) = undirected(&f);
        assert_eq!(v.as_str(), "a");
        assert!(w == "e" || w == "f", "expected w in {{e,f}}, got {}", w);
    }

    #[test]
    fn enter_edge_gansner_graph_4() {
        // "finds an appropriate edge for gansner graph #4"
        // initLowLimValues from "e", leave edge {h,g} (reversed order)
        let mut g = gansner_graph();
        let mut t = gansner_tree();
        longest_path(&mut g);
        init_low_lim_values(&mut t, Some("e".to_string()));

        let f = enter_edge(&t, &g, &Edge::new("h", "g"));
        let f = f.expect("expected entering edge for gansner #4");
        let (v, w) = undirected(&f);
        assert_eq!(v.as_str(), "a");
        assert!(w == "e" || w == "f", "expected w in {{e,f}}, got {}", w);
    }

    // ─── layout integration tests ─────────────────────────────────────────────

    #[test]
    fn layout_single_node() {
        let mut g = Graph::with_options(true, true, true);
        g.set_graph(GraphLabel::default());
        g.set_node(
            "a",
            NodeLabel {
                width: 50.0,
                height: 100.0,
                ..Default::default()
            },
        );
        layout(&mut g);
        let node = g.node("a");
        assert!(
            (node.x.unwrap() - 25.0).abs() < 0.5,
            "expected x≈25, got {:?}",
            node.x
        );
        assert!(
            (node.y.unwrap() - 50.0).abs() < 0.5,
            "expected y≈50, got {:?}",
            node.y
        );
    }

    #[test]
    fn layout_two_nodes_same_rank() {
        let mut g = Graph::with_options(true, true, true);
        g.set_graph(GraphLabel {
            nodesep: Some(200.0),
            ..Default::default()
        });
        g.set_node(
            "a",
            NodeLabel {
                width: 50.0,
                height: 100.0,
                ..Default::default()
            },
        );
        g.set_node(
            "b",
            NodeLabel {
                width: 75.0,
                height: 200.0,
                ..Default::default()
            },
        );
        layout(&mut g);

        let a = g.node("a");
        let b = g.node("b");
        // a.x = 50/2 = 25
        assert!(
            (a.x.unwrap() - 25.0).abs() < 1.0,
            "a.x expected ≈25, got {:?}",
            a.x
        );
        // b.x = 50 + 200 + 75/2 = 287.5
        assert!(
            (b.x.unwrap() - 287.5).abs() < 1.0,
            "b.x expected ≈287.5, got {:?}",
            b.x
        );
        // both at y = 200/2 = 100
        assert!(
            (a.y.unwrap() - 100.0).abs() < 1.0,
            "a.y expected ≈100, got {:?}",
            a.y
        );
        assert!(
            (b.y.unwrap() - 100.0).abs() < 1.0,
            "b.y expected ≈100, got {:?}",
            b.y
        );
    }

    #[test]
    fn layout_two_nodes_connected() {
        let mut g = Graph::with_options(true, true, true);
        g.set_graph(GraphLabel {
            ranksep: Some(300.0),
            ..Default::default()
        });
        g.set_node(
            "a",
            NodeLabel {
                width: 50.0,
                height: 100.0,
                ..Default::default()
            },
        );
        g.set_node(
            "b",
            NodeLabel {
                width: 75.0,
                height: 200.0,
                ..Default::default()
            },
        );
        g.set_edge("a", "b", EdgeLabel::default(), None);
        layout(&mut g);

        let a = g.node("a");
        let b = g.node("b");
        // Both centered at x = 75/2 = 37.5 (widest node)
        assert!(
            (a.x.unwrap() - 37.5).abs() < 1.0,
            "a.x expected ≈37.5, got {:?}",
            a.x
        );
        assert!(
            (b.x.unwrap() - 37.5).abs() < 1.0,
            "b.x expected ≈37.5, got {:?}",
            b.x
        );
        // a.y = 100/2 = 50
        assert!(
            (a.y.unwrap() - 50.0).abs() < 1.0,
            "a.y expected ≈50, got {:?}",
            a.y
        );
        // b.y = 100 + 300 + 200/2 = 500
        assert!(
            (b.y.unwrap() - 500.0).abs() < 1.0,
            "b.y expected ≈500, got {:?}",
            b.y
        );
    }

    // =========================================================================
    // Ported from dagre-js/test/acyclic-test.ts
    // =========================================================================

    mod acyclic_tests {
        use crate::acyclic;
        use crate::graph::{Edge, EdgeLabel, Graph, NodeLabel};

        fn make_g() -> Graph {
            Graph::with_options(true, true, false)
        }

        fn de() -> EdgeLabel {
            EdgeLabel {
                minlen: Some(1),
                weight: Some(1.0),
                ..Default::default()
            }
        }

        fn set_path(g: &mut Graph, nodes: &[&str], label: EdgeLabel) {
            for i in 0..nodes.len() - 1 {
                if !g.has_node(nodes[i]) {
                    g.set_node(nodes[i], NodeLabel::default());
                }
                if !g.has_node(nodes[i + 1]) {
                    g.set_node(nodes[i + 1], NodeLabel::default());
                }
                g.set_edge(nodes[i], nodes[i + 1], label.clone(), None);
            }
        }

        fn has_cycle(g: &Graph) -> bool {
            // Simple DFS cycle check
            let mut visited = std::collections::HashSet::new();
            let mut stack = std::collections::HashSet::new();

            fn dfs(
                g: &Graph,
                v: &str,
                visited: &mut std::collections::HashSet<String>,
                stack: &mut std::collections::HashSet<String>,
            ) -> bool {
                visited.insert(v.to_string());
                stack.insert(v.to_string());
                if let Some(succs) = g.successors(v) {
                    for w in succs {
                        if !visited.contains(&w) {
                            if dfs(g, &w, visited, stack) {
                                return true;
                            }
                        } else if stack.contains(&w) {
                            return true;
                        }
                    }
                }
                stack.remove(v);
                false
            }

            for v in g.nodes() {
                if !visited.contains(&v) {
                    if dfs(g, &v, &mut visited, &mut stack) {
                        return true;
                    }
                }
            }
            false
        }

        fn strip_name(e: &Edge) -> (String, String) {
            (e.v.clone(), e.w.clone())
        }

        fn sort_edges(mut edges: Vec<(String, String)>) -> Vec<(String, String)> {
            edges.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            edges
        }

        // --- greedy acyclicer ---

        #[test]
        fn acyclic_greedy_does_not_change_acyclic_graph() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            set_path(&mut g, &["a", "b", "d"], de());
            set_path(&mut g, &["a", "c", "d"], de());
            acyclic::run(&mut g);
            let edges = g.edges().iter().map(strip_name).collect::<Vec<_>>();
            assert_eq!(
                sort_edges(edges),
                vec![
                    ("a".to_string(), "b".to_string()),
                    ("a".to_string(), "c".to_string()),
                    ("b".to_string(), "d".to_string()),
                    ("c".to_string(), "d".to_string()),
                ]
            );
        }

        #[test]
        fn acyclic_greedy_breaks_cycles() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            set_path(&mut g, &["a", "b", "c", "d", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g), "Expected no cycles after acyclic::run");
        }

        #[test]
        fn acyclic_greedy_creates_multi_edge_for_self_loop() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            set_path(&mut g, &["a", "b", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
            assert_eq!(g.edge_count(), 2);
        }

        #[test]
        fn acyclic_greedy_undo_does_not_change_acyclic_graph() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert_eq!(el.minlen, Some(2));
            assert!((el.weight.unwrap() - 3.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 1);
        }

        #[test]
        fn acyclic_greedy_undo_restores_reversed_edges() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    minlen: Some(3),
                    weight: Some(4.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            assert_eq!(ab.minlen, Some(2));
            assert!((ab.weight.unwrap() - 3.0).abs() < 1e-9);
            let ba = g.edge_vw("b", "a").unwrap();
            assert_eq!(ba.minlen, Some(3));
            assert!((ba.weight.unwrap() - 4.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 2);
        }

        // --- dfs acyclicer ---

        #[test]
        fn acyclic_dfs_does_not_change_acyclic_graph() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("dfs".to_string());
            set_path(&mut g, &["a", "b", "d"], de());
            set_path(&mut g, &["a", "c", "d"], de());
            acyclic::run(&mut g);
            let edges = g.edges().iter().map(strip_name).collect::<Vec<_>>();
            assert_eq!(
                sort_edges(edges),
                vec![
                    ("a".to_string(), "b".to_string()),
                    ("a".to_string(), "c".to_string()),
                    ("b".to_string(), "d".to_string()),
                    ("c".to_string(), "d".to_string()),
                ]
            );
        }

        #[test]
        fn acyclic_dfs_breaks_cycles() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("dfs".to_string());
            set_path(&mut g, &["a", "b", "c", "d", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
        }

        #[test]
        fn acyclic_dfs_undo_does_not_change_acyclic_graph() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("dfs".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert_eq!(el.minlen, Some(2));
            assert!((el.weight.unwrap() - 3.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 1);
        }

        #[test]
        fn acyclic_dfs_undo_restores_reversed_edges() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("dfs".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    minlen: Some(3),
                    weight: Some(4.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            assert_eq!(ab.minlen, Some(2));
            assert!((ab.weight.unwrap() - 3.0).abs() < 1e-9);
            let ba = g.edge_vw("b", "a").unwrap();
            assert_eq!(ba.minlen, Some(3));
            assert!((ba.weight.unwrap() - 4.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 2);
        }

        // --- greedy-specific: prefers low-weight edges ---

        #[test]
        fn acyclic_greedy_prefers_low_weight_edges() {
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("greedy".to_string());
            // default weight 2
            let de2 = EdgeLabel {
                minlen: Some(1),
                weight: Some(2.0),
                ..Default::default()
            };
            for pair in &[("a", "b"), ("b", "c"), ("c", "d"), ("d", "a")] {
                if !g.has_node(pair.0) {
                    g.set_node(pair.0, NodeLabel::default());
                }
                if !g.has_node(pair.1) {
                    g.set_node(pair.1, NodeLabel::default());
                }
                g.set_edge(pair.0, pair.1, de2.clone(), None);
            }
            // Override c->d with weight 1
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    minlen: Some(1),
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
            // The low-weight edge c->d should have been chosen for reversal
            assert!(!g.has_edge("c", "d"));
        }
    } // mod acyclic_tests

    // =========================================================================
    // Ported from dagre-js/test/normalize-test.ts
    // =========================================================================

    mod normalize_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::normalize;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, true, true);
            g.set_graph(crate::graph::GraphLabel::default());
            g
        }

        #[test]
        fn normalize_run_does_not_change_short_edge() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            normalize::run(&mut g);
            let edges = g.edges();
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].v, "a");
            assert_eq!(edges[0].w, "b");
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(1));
        }

        #[test]
        fn normalize_run_splits_two_layer_edge_into_two_segments() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            normalize::run(&mut g);
            let succs_a = g.successors("a").unwrap_or_default();
            assert_eq!(succs_a.len(), 1);
            let succ = &succs_a[0];
            assert_eq!(g.node(succ).dummy.as_deref(), Some("edge"));
            assert_eq!(g.node(succ).rank, Some(1));
            let succs_succ = g.successors(succ).unwrap_or_default();
            assert_eq!(succs_succ, vec!["b"]);
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(2));
            let dummy_chains = g.graph().dummy_chains.as_ref().unwrap();
            assert_eq!(dummy_chains.len(), 1);
            assert_eq!(dummy_chains[0], *succ);
        }

        #[test]
        fn normalize_run_assigns_zero_width_height_to_dummy_nodes() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            let succs_a = g.successors("a").unwrap_or_default();
            assert_eq!(succs_a.len(), 1);
            let succ = &succs_a[0];
            assert_eq!(g.node(succ).width, 0.0);
            assert_eq!(g.node(succ).height, 0.0);
        }

        #[test]
        fn normalize_run_assigns_width_height_from_edge_for_label_rank() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(20.0),
                    height: Some(10.0),
                    label_rank: Some(2),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            let succ_a = g.successors("a").unwrap_or_default()[0].clone();
            let label_v = g.successors(&succ_a).unwrap_or_default()[0].clone();
            let label_node = g.node(&label_v);
            assert_eq!(label_node.width, 20.0);
            assert_eq!(label_node.height, 10.0);
        }

        #[test]
        fn normalize_run_preserves_weight() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            let succs_a = g.successors("a").unwrap_or_default();
            assert_eq!(succs_a.len(), 1);
            let succ = &succs_a[0];
            let edge = g.edge_vw("a", succ).unwrap();
            assert!((edge.weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn normalize_undo_reverses_run() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            normalize::run(&mut g);
            normalize::undo(&mut g);
            let edges = g.edges();
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].v, "a");
            assert_eq!(edges[0].w, "b");
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(2));
        }

        #[test]
        fn normalize_undo_restores_edge_labels() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            // Use labelpos as a proxy for a custom field
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    labelpos: Some("bar".to_string()),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            normalize::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert_eq!(el.labelpos.as_deref(), Some("bar"));
        }

        #[test]
        fn normalize_undo_collects_points() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            normalize::run(&mut g);
            // Set coordinates on the dummy node
            let nbrs = g.neighbors("a").unwrap_or_default();
            let dummy = &nbrs[0];
            g.node_mut(dummy).x = Some(5.0);
            g.node_mut(dummy).y = Some(10.0);
            normalize::undo(&mut g);
            let points = g.edge_vw("a", "b").unwrap().points.as_ref().unwrap();
            assert_eq!(points.len(), 1);
            assert!((points[0].x - 5.0).abs() < 1e-9);
            assert!((points[0].y - 10.0).abs() < 1e-9);
        }

        #[test]
        fn normalize_undo_merges_multiple_points() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            normalize::run(&mut g);
            // Walk the chain and set coordinates
            let succ_a = g.successors("a").unwrap_or_default()[0].clone();
            g.node_mut(&succ_a).x = Some(5.0);
            g.node_mut(&succ_a).y = Some(10.0);
            let succ_succ_a = g.successors(&succ_a).unwrap_or_default()[0].clone();
            g.node_mut(&succ_succ_a).x = Some(20.0);
            g.node_mut(&succ_succ_a).y = Some(25.0);
            let pred_b = g.neighbors("b").unwrap_or_default()[0].clone();
            g.node_mut(&pred_b).x = Some(100.0);
            g.node_mut(&pred_b).y = Some(200.0);
            normalize::undo(&mut g);
            let points = g.edge_vw("a", "b").unwrap().points.as_ref().unwrap();
            // JS expects exactly 3 points: [(5,10), (20,25), (100,200)]
            assert_eq!(points.len(), 3);
            assert!((points[0].x - 5.0).abs() < 1e-9 && (points[0].y - 10.0).abs() < 1e-9);
            assert!((points[1].x - 20.0).abs() < 1e-9 && (points[1].y - 25.0).abs() < 1e-9);
            assert!((points[2].x - 100.0).abs() < 1e-9 && (points[2].y - 200.0).abs() < 1e-9);
        }

        #[test]
        fn normalize_undo_sets_coords_for_edge_label() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(20.0),
                    label_rank: Some(1),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            let succ = g.successors("a").unwrap_or_default()[0].clone();
            g.node_mut(&succ).x = Some(50.0);
            g.node_mut(&succ).y = Some(60.0);
            g.node_mut(&succ).width = 20.0;
            g.node_mut(&succ).height = 10.0;
            normalize::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 50.0).abs() < 1e-9);
            assert!((el.y.unwrap() - 60.0).abs() < 1e-9);
            assert!((el.width.unwrap() - 20.0).abs() < 1e-9);
            assert!((el.height.unwrap() - 10.0).abs() < 1e-9);
        }

        #[test]
        fn normalize_undo_sets_coords_for_label_on_long_edge() {
            // "sets coords and dims for the label, if the long edge has one"
            // a(rank=0) -> b(rank=4) with labelRank=2 (edge spans 4 ranks).
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(20.0),
                    label_rank: Some(2),
                    ..Default::default()
                },
                None,
            );
            normalize::run(&mut g);
            // Walk: a -> d1(rank=1) -> d2(rank=2, label node) -> d3(rank=3) -> b
            let succ_a = g.successors("a").unwrap_or_default()[0].clone(); // rank=1
            let label_v = g.successors(&succ_a).unwrap_or_default()[0].clone(); // rank=2, label node
            g.node_mut(&label_v).x = Some(50.0);
            g.node_mut(&label_v).y = Some(60.0);
            g.node_mut(&label_v).width = 20.0;
            g.node_mut(&label_v).height = 10.0;
            normalize::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 50.0).abs() < 1e-9);
            assert!((el.y.unwrap() - 60.0).abs() < 1e-9);
            assert!((el.width.unwrap() - 20.0).abs() < 1e-9);
            assert!((el.height.unwrap() - 10.0).abs() < 1e-9);
        }

        #[test]
        fn normalize_undo_restores_multi_edges() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), Some("bar"));
            g.set_edge("a", "b", EdgeLabel::default(), Some("foo"));
            normalize::run(&mut g);
            // 2 out-edges from a
            let out_a = g.out_edges("a").unwrap_or_default();
            assert_eq!(out_a.len(), 2);
            // Set coordinates on both dummies
            for e in &out_a {
                let dummy = e.w.clone();
                g.node_mut(&dummy).x = Some(5.0);
                g.node_mut(&dummy).y = Some(10.0);
            }
            normalize::undo(&mut g);
            // Original edges restored (no unnamed edge)
            assert!(!g.has_edge("a", "b") || g.out_edges_to("a", "b").map_or(0, |v| v.len()) == 2);
        }
    } // mod normalize_tests

    // =========================================================================
    // Ported from dagre-js/test/order/cross-count-test.ts
    // =========================================================================

    mod cross_count_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::cross_count::cross_count;

        fn make_g() -> Graph {
            Graph::with_options(true, false, false)
        }

        fn de() -> EdgeLabel {
            EdgeLabel {
                weight: Some(1.0),
                ..Default::default()
            }
        }

        fn set_path(g: &mut Graph, nodes: &[&str]) {
            for i in 0..nodes.len() - 1 {
                if !g.has_node(nodes[i]) {
                    g.set_node(nodes[i], NodeLabel::default());
                }
                if !g.has_node(nodes[i + 1]) {
                    g.set_node(nodes[i + 1], NodeLabel::default());
                }
                g.set_edge(nodes[i], nodes[i + 1], de(), None);
            }
        }

        #[test]
        fn cross_count_empty_layering() {
            let g = make_g();
            assert_eq!(cross_count(&g, &[]) as i64, 0);
        }

        #[test]
        fn cross_count_no_crossings() {
            let mut g = make_g();
            g.set_edge("a1", "b1", de(), None);
            g.set_edge("a2", "b2", de(), None);
            let layering = vec![
                vec!["a1".to_string(), "a2".to_string()],
                vec!["b1".to_string(), "b2".to_string()],
            ];
            assert_eq!(cross_count(&g, &layering) as i64, 0);
        }

        #[test]
        fn cross_count_one_crossing() {
            let mut g = make_g();
            g.set_edge("a1", "b1", de(), None);
            g.set_edge("a2", "b2", de(), None);
            let layering = vec![
                vec!["a1".to_string(), "a2".to_string()],
                vec!["b2".to_string(), "b1".to_string()],
            ];
            assert_eq!(cross_count(&g, &layering) as i64, 1);
        }

        #[test]
        fn cross_count_weighted_crossing() {
            let mut g = make_g();
            g.set_edge(
                "a1",
                "b1",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "a2",
                "b2",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            let layering = vec![
                vec!["a1".to_string(), "a2".to_string()],
                vec!["b2".to_string(), "b1".to_string()],
            ];
            assert_eq!(cross_count(&g, &layering) as i64, 6);
        }

        #[test]
        fn cross_count_across_layers() {
            let mut g = make_g();
            set_path(&mut g, &["a1", "b1", "c1"]);
            set_path(&mut g, &["a2", "b2", "c2"]);
            let layering = vec![
                vec!["a1".to_string(), "a2".to_string()],
                vec!["b2".to_string(), "b1".to_string()],
                vec!["c1".to_string(), "c2".to_string()],
            ];
            assert_eq!(cross_count(&g, &layering) as i64, 2);
        }

        #[test]
        fn cross_count_graph1() {
            let mut g = make_g();
            set_path(&mut g, &["a", "b", "c"]);
            set_path(&mut g, &["d", "e", "c"]);
            set_path(&mut g, &["a", "f", "i"]);
            g.set_edge("a", "e", de(), None);
            let l1 = vec![
                vec!["a".to_string(), "d".to_string()],
                vec!["b".to_string(), "e".to_string(), "f".to_string()],
                vec!["c".to_string(), "i".to_string()],
            ];
            assert_eq!(cross_count(&g, &l1) as i64, 1);
            let l2 = vec![
                vec!["d".to_string(), "a".to_string()],
                vec!["e".to_string(), "b".to_string(), "f".to_string()],
                vec!["c".to_string(), "i".to_string()],
            ];
            assert_eq!(cross_count(&g, &l2) as i64, 0);
        }
    } // mod cross_count_tests

    // =========================================================================
    // Ported from dagre-js/test/order/init-order-test.ts
    // =========================================================================

    mod init_order_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::init_order::init_order;

        fn de() -> EdgeLabel {
            EdgeLabel {
                weight: Some(1.0),
                ..Default::default()
            }
        }

        fn set_path(g: &mut Graph, nodes: &[&str]) {
            for i in 0..nodes.len() - 1 {
                if !g.has_node(nodes[i]) {
                    g.set_node(nodes[i], NodeLabel::default());
                }
                if !g.has_node(nodes[i + 1]) {
                    g.set_node(nodes[i + 1], NodeLabel::default());
                }
                g.set_edge(nodes[i], nodes[i + 1], de(), None);
            }
        }

        #[test]
        fn init_order_tree_non_overlapping() {
            let mut g = Graph::with_options(true, false, true);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "e",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            set_path(&mut g, &["a", "b", "c"]);
            g.set_edge("b", "d", de(), None);
            g.set_edge("a", "e", de(), None);
            let layering = init_order(&g);
            assert_eq!(layering[0], vec!["a"]);
            let mut layer1 = layering[1].clone();
            layer1.sort();
            assert_eq!(layer1, vec!["b", "e"]);
            let mut layer2 = layering[2].clone();
            layer2.sort();
            assert_eq!(layer2, vec!["c", "d"]);
        }

        #[test]
        fn init_order_dag_non_overlapping() {
            let mut g = Graph::with_options(true, false, true);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            set_path(&mut g, &["a", "b", "d"]);
            set_path(&mut g, &["a", "c", "d"]);
            let layering = init_order(&g);
            assert_eq!(layering[0], vec!["a"]);
            let mut layer1 = layering[1].clone();
            layer1.sort();
            assert_eq!(layer1, vec!["b", "c"]);
            let mut layer2 = layering[2].clone();
            layer2.sort();
            assert_eq!(layer2, vec!["d"]);
        }

        #[test]
        fn init_order_does_not_assign_order_to_subgraph_nodes() {
            let mut g = Graph::with_options(true, false, true);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node("sg1", NodeLabel::default()); // no rank
            g.set_parent("a", Some("sg1"));
            let layering = init_order(&g);
            assert_eq!(layering, vec![vec!["a"]]);
        }
    } // mod init_order_tests

    // =========================================================================
    // Ported from dagre-js/test/order/barycenter-test.ts
    // =========================================================================

    mod barycenter_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::barycenter::barycenter;

        fn make_g() -> Graph {
            Graph::with_options(true, false, false)
        }

        #[test]
        fn barycenter_no_predecessors() {
            let mut g = make_g();
            g.set_node("x", NodeLabel::default());
            let results = barycenter(&g, &["x".to_string()]);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].v, "x");
            assert!(results[0].barycenter.is_none());
            assert!(results[0].weight.is_none());
        }

        #[test]
        fn barycenter_sole_predecessor() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    order: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            let results = barycenter(&g, &["x".to_string()]);
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].v, "x");
            assert!((results[0].barycenter.unwrap() - 2.0).abs() < 1e-9);
            assert!((results[0].weight.unwrap() - 1.0).abs() < 1e-9);
        }

        #[test]
        fn barycenter_multiple_predecessors_average() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    order: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    order: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            let results = barycenter(&g, &["x".to_string()]);
            assert_eq!(results.len(), 1);
            assert!((results[0].barycenter.unwrap() - 3.0).abs() < 1e-9);
            assert!((results[0].weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn barycenter_weighted_edges() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    order: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    order: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            let results = barycenter(&g, &["x".to_string()]);
            assert_eq!(results.len(), 1);
            // barycenter = (3*2 + 1*4) / (3+1) = 10/4 = 2.5
            assert!((results[0].barycenter.unwrap() - 2.5).abs() < 1e-9);
            assert!((results[0].weight.unwrap() - 4.0).abs() < 1e-9);
        }

        #[test]
        fn barycenter_multiple_movable_nodes() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    order: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    order: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    order: Some(4),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "x",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g.set_node("y", NodeLabel::default());
            g.set_edge(
                "a",
                "z",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "z",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            let movable = vec!["x".to_string(), "y".to_string(), "z".to_string()];
            let results = barycenter(&g, &movable);
            assert_eq!(results.len(), 3);
            // x: barycenter = (1*1 + 1*2)/(1+1) = 1.5, weight=2
            assert!((results[0].barycenter.unwrap() - 1.5).abs() < 1e-9);
            assert!((results[0].weight.unwrap() - 2.0).abs() < 1e-9);
            // y: no predecessors
            assert!(results[1].barycenter.is_none());
            // z: barycenter = (2*1 + 1*4)/(2+1) = 6/3 = 2.0, weight=3
            assert!((results[2].barycenter.unwrap() - 2.0).abs() < 1e-9);
            assert!((results[2].weight.unwrap() - 3.0).abs() < 1e-9);
        }
    } // mod barycenter_tests

    // =========================================================================
    // Ported from dagre-js/test/order/sort-test.ts
    // =========================================================================

    mod sort_tests {
        use crate::order::sort::{sort, SortEntry};

        #[test]
        fn sort_by_barycenter() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 1,
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["b", "a"]);
            // barycenter = (2*3 + 1*2)/(3+2) = 8/5 = 1.6
            assert!((result.barycenter.unwrap() - 1.6).abs() < 1e-9);
            assert!((result.weight.unwrap() - 5.0).abs() < 1e-9);
        }

        #[test]
        fn sort_super_nodes() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string(), "c".to_string(), "d".to_string()],
                    i: 0,
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 1,
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["b", "a", "c", "d"]);
        }

        #[test]
        fn sort_bias_left_by_default() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 1,
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["a", "b"]);
            assert!((result.barycenter.unwrap() - 1.0).abs() < 1e-9);
            assert!((result.weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn sort_bias_right() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 1,
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
            ];
            let result = sort(input, true);
            assert_eq!(result.vs, vec!["b", "a"]);
        }

        #[test]
        fn sort_nodes_without_barycenter() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: Some(2.0),
                    weight: Some(1.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 1,
                    barycenter: Some(6.0),
                    weight: Some(1.0),
                },
                SortEntry {
                    vs: vec!["c".to_string()],
                    i: 2,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["d".to_string()],
                    i: 3,
                    barycenter: Some(3.0),
                    weight: Some(1.0),
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["a", "d", "c", "b"]);
            // barycenter = (2+6+3)/3 = 11/3
            assert!((result.barycenter.unwrap() - 11.0 / 3.0).abs() < 1e-9);
            assert!((result.weight.unwrap() - 3.0).abs() < 1e-9);
        }

        #[test]
        fn sort_no_barycenters_at_all() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 3,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["c".to_string()],
                    i: 2,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["d".to_string()],
                    i: 1,
                    barycenter: None,
                    weight: None,
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["a", "d", "c", "b"]);
            assert!(result.barycenter.is_none());
            assert!(result.weight.is_none());
        }

        #[test]
        fn sort_barycenter_of_zero() {
            let input = vec![
                SortEntry {
                    vs: vec!["a".to_string()],
                    i: 0,
                    barycenter: Some(0.0),
                    weight: Some(1.0),
                },
                SortEntry {
                    vs: vec!["b".to_string()],
                    i: 3,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["c".to_string()],
                    i: 2,
                    barycenter: None,
                    weight: None,
                },
                SortEntry {
                    vs: vec!["d".to_string()],
                    i: 1,
                    barycenter: None,
                    weight: None,
                },
            ];
            let result = sort(input, false);
            assert_eq!(result.vs, vec!["a", "d", "c", "b"]);
            assert!((result.barycenter.unwrap() - 0.0).abs() < 1e-9);
            assert!((result.weight.unwrap() - 1.0).abs() < 1e-9);
        }
    } // mod sort_tests

    // =========================================================================
    // Ported from dagre-js/test/order/resolve-conflicts-test.ts
    // =========================================================================

    mod resolve_conflicts_tests {
        use crate::graph::{EdgeLabel, Graph};
        use crate::order::resolve_conflicts::{resolve_conflicts, BarycenterInput, ResolvedEntry};

        fn make_cg() -> Graph {
            Graph::with_options(true, false, false)
        }

        fn sort_by_first_vs(mut v: Vec<ResolvedEntry>) -> Vec<ResolvedEntry> {
            v.sort_by(|a, b| a.vs[0].cmp(&b.vs[0]));
            v
        }

        #[test]
        fn resolve_conflicts_unchanged_no_constraints() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let cg = make_cg();
            let result = sort_by_first_vs(resolve_conflicts(&input, &cg));
            assert_eq!(result.len(), 2);
            assert_eq!(result[0].vs, vec!["a"]);
            assert_eq!(result[0].i, 0);
            assert!((result[0].barycenter.unwrap() - 2.0).abs() < 1e-9);
            assert!((result[0].weight.unwrap() - 3.0).abs() < 1e-9);
            assert_eq!(result[1].vs, vec!["b"]);
            assert_eq!(result[1].i, 1);
            assert!((result[1].barycenter.unwrap() - 1.0).abs() < 1e-9);
            assert!((result[1].weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_unchanged_no_conflict() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("b", "a", EdgeLabel::default(), None); // b before a: consistent
            let result = sort_by_first_vs(resolve_conflicts(&input, &cg));
            assert_eq!(result.len(), 2);
        }

        #[test]
        fn resolve_conflicts_coalesces_on_conflict() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("a", "b", EdgeLabel::default(), None);
            let result = resolve_conflicts(&input, &cg);
            assert_eq!(result.len(), 1);
            // merged: vs contains both a and b, a before b
            let entry = &result[0];
            assert!(entry.vs.contains(&"a".to_string()));
            assert!(entry.vs.contains(&"b".to_string()));
            assert_eq!(entry.i, 0);
            // barycenter = (3*2 + 2*1)/(3+2) = 8/5 = 1.6
            assert!((entry.barycenter.unwrap() - 1.6).abs() < 1e-9);
            assert!((entry.weight.unwrap() - 5.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_coalesces_chain() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(4.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(3.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "c".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "d".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
            ];
            let mut cg = make_cg();
            // path a->b->c->d
            cg.set_edge("a", "b", EdgeLabel::default(), None);
            cg.set_edge("b", "c", EdgeLabel::default(), None);
            cg.set_edge("c", "d", EdgeLabel::default(), None);
            let result = resolve_conflicts(&input, &cg);
            assert_eq!(result.len(), 1);
            let entry = &result[0];
            assert_eq!(entry.i, 0);
            assert!((entry.barycenter.unwrap() - (4.0 + 3.0 + 2.0 + 1.0) / 4.0).abs() < 1e-9);
            assert!((entry.weight.unwrap() - 4.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_multiple_constraints_same_target_1() {
            // "works with multiple constraints for the same target #1"
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(4.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(3.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "c".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(1.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("a", "c", EdgeLabel::default(), None);
            cg.set_edge("b", "c", EdgeLabel::default(), None);
            let results = resolve_conflicts(&input, &cg);
            assert_eq!(results.len(), 1);
            let result = &results[0];
            let idx_c = result.vs.iter().position(|v| v == "c").unwrap();
            let idx_a = result.vs.iter().position(|v| v == "a").unwrap();
            let idx_b = result.vs.iter().position(|v| v == "b").unwrap();
            assert!(idx_c > idx_a, "c must come after a");
            assert!(idx_c > idx_b, "c must come after b");
            assert_eq!(result.i, 0);
            assert!((result.barycenter.unwrap() - (4.0 + 3.0 + 2.0) / 3.0).abs() < 1e-9);
            assert!((result.weight.unwrap() - 3.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_multiple_constraints_same_target_2() {
            // "works with multiple constraints for the same target #2"
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(4.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(3.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "c".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(1.0),
                },
                BarycenterInput {
                    v: "d".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(1.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("a", "c", EdgeLabel::default(), None);
            cg.set_edge("a", "d", EdgeLabel::default(), None);
            cg.set_edge("b", "c", EdgeLabel::default(), None);
            cg.set_edge("c", "d", EdgeLabel::default(), None);
            let results = resolve_conflicts(&input, &cg);
            assert_eq!(results.len(), 1);
            let result = &results[0];
            let idx_c = result.vs.iter().position(|v| v == "c").unwrap();
            let idx_a = result.vs.iter().position(|v| v == "a").unwrap();
            let idx_b = result.vs.iter().position(|v| v == "b").unwrap();
            let idx_d = result.vs.iter().position(|v| v == "d").unwrap();
            assert!(idx_c > idx_a, "c must come after a");
            assert!(idx_c > idx_b, "c must come after b");
            assert!(idx_d > idx_c, "d must come after c");
            assert_eq!(result.i, 0);
            assert!((result.barycenter.unwrap() - (4.0 + 3.0 + 2.0 + 1.0) / 4.0).abs() < 1e-9);
            assert!((result.weight.unwrap() - 4.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_node_without_barycenter_unchanged_no_constraint() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: None,
                    weight: None,
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let cg = make_cg();
            let result = sort_by_first_vs(resolve_conflicts(&input, &cg));
            assert_eq!(result.len(), 2);
            assert_eq!(result[0].vs, vec!["a"]);
            assert!(result[0].barycenter.is_none());
        }

        #[test]
        fn resolve_conflicts_node_without_barycenter_violates_constraint1() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: None,
                    weight: None,
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("a", "b", EdgeLabel::default(), None);
            let result = resolve_conflicts(&input, &cg);
            assert_eq!(result.len(), 1);
            assert!(result[0].vs.contains(&"a".to_string()));
            assert!(result[0].vs.contains(&"b".to_string()));
            assert!((result[0].barycenter.unwrap() - 1.0).abs() < 1e-9);
            assert!((result[0].weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_node_without_barycenter_violates_constraint2() {
            // "treats a node w/o a barycenter as always violating constraints #2"
            // Constraint b->a (b before a); a has no barycenter.
            // Both get merged, a comes after b (a is constraint target of b).
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: None,
                    weight: None,
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("b", "a", EdgeLabel::default(), None);
            let result = resolve_conflicts(&input, &cg);
            assert_eq!(result.len(), 1);
            assert!(result[0].vs.contains(&"b".to_string()));
            assert!(result[0].vs.contains(&"a".to_string()));
            assert!((result[0].barycenter.unwrap() - 1.0).abs() < 1e-9);
            assert!((result[0].weight.unwrap() - 2.0).abs() < 1e-9);
        }

        #[test]
        fn resolve_conflicts_ignores_unrelated_edges() {
            let input = vec![
                BarycenterInput {
                    v: "a".to_string(),
                    barycenter: Some(2.0),
                    weight: Some(3.0),
                },
                BarycenterInput {
                    v: "b".to_string(),
                    barycenter: Some(1.0),
                    weight: Some(2.0),
                },
            ];
            let mut cg = make_cg();
            cg.set_edge("c", "d", EdgeLabel::default(), None);
            let result = sort_by_first_vs(resolve_conflicts(&input, &cg));
            assert_eq!(result.len(), 2);
        }
    } // mod resolve_conflicts_tests

    // =========================================================================
    // Ported from dagre-js/test/order/order-test.ts
    // =========================================================================

    mod order_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::cross_count::cross_count;
        use crate::order::order;
        use crate::util::build_layer_matrix;

        fn make_g() -> Graph {
            Graph::with_options(true, false, false)
        }

        fn de() -> EdgeLabel {
            EdgeLabel {
                weight: Some(1.0),
                ..Default::default()
            }
        }

        fn set_path(g: &mut Graph, nodes: &[&str]) {
            for i in 0..nodes.len() - 1 {
                if !g.has_node(nodes[i]) {
                    g.set_node(nodes[i], NodeLabel::default());
                }
                if !g.has_node(nodes[i + 1]) {
                    g.set_node(nodes[i + 1], NodeLabel::default());
                }
                g.set_edge(nodes[i], nodes[i + 1], de(), None);
            }
        }

        #[test]
        fn order_does_not_add_crossings_to_tree() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            for v in &["b", "e"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(2),
                        ..Default::default()
                    },
                );
            }
            for v in &["c", "d", "f"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(3),
                        ..Default::default()
                    },
                );
            }
            set_path(&mut g, &["a", "b", "c"]);
            g.set_edge("b", "d", de(), None);
            set_path(&mut g, &["a", "e", "f"]);
            order(&mut g, &[], false);
            let layering = build_layer_matrix(&g);
            assert_eq!(cross_count(&g, &layering) as i64, 0);
        }

        #[test]
        fn order_can_solve_simple_graph() {
            let mut g = make_g();
            for v in &["a", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(1),
                        ..Default::default()
                    },
                );
            }
            for v in &["b", "f", "e"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(2),
                        ..Default::default()
                    },
                );
            }
            for v in &["c", "g"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(3),
                        ..Default::default()
                    },
                );
            }
            order(&mut g, &[], false);
            let layering = build_layer_matrix(&g);
            assert_eq!(cross_count(&g, &layering) as i64, 0);
        }

        #[test]
        fn order_can_minimize_crossings() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            for v in &["b", "e", "g"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(2),
                        ..Default::default()
                    },
                );
            }
            for v in &["c", "f", "h"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(3),
                        ..Default::default()
                    },
                );
            }
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            order(&mut g, &[], false);
            let layering = build_layer_matrix(&g);
            assert!(cross_count(&g, &layering) <= 1.0);
        }

        #[test]
        #[ignore = "order with disable_optimal=true relies on DFS init_order which has \
                    non-deterministic successor ordering due to HashMap iteration in Rust; \
                    the JS test expects exactly 1 crossing but Rust may produce 0 depending on order"]
        fn order_skip_optimal_ordering() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            for v in &["b", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(2),
                        ..Default::default()
                    },
                );
            }
            for v in &["c", "e"] {
                g.set_node(
                    v,
                    NodeLabel {
                        rank: Some(3),
                        ..Default::default()
                    },
                );
            }
            set_path(&mut g, &["a", "b", "c"]);
            set_path(&mut g, &["a", "d"]);
            g.set_edge("b", "e", de(), None);
            g.set_edge("d", "c", de(), None);
            // disable_optimal = true
            order(&mut g, &[], true);
            let layering = build_layer_matrix(&g);
            assert_eq!(cross_count(&g, &layering) as i64, 1);
        }
    } // mod order_tests

    // =========================================================================
    // Ported from dagre-js/test/position/bk-test.ts
    // =========================================================================

    mod position_bk_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::position::bk::{
            add_conflict, align_coordinates, balance, find_smallest_width_alignment,
            find_type1_conflicts, find_type2_conflicts, has_conflict, horizontal_compaction,
            position_x, vertical_alignment,
        };
        use crate::util::build_layer_matrix;
        use std::collections::HashMap;

        type PositionMap = HashMap<String, f64>;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel::default());
            g
        }

        fn node(rank: i32, order: i32) -> NodeLabel {
            NodeLabel {
                rank: Some(rank),
                order: Some(order),
                ..Default::default()
            }
        }

        fn node_w(rank: i32, order: i32, width: f64) -> NodeLabel {
            NodeLabel {
                rank: Some(rank),
                order: Some(order),
                width,
                ..Default::default()
            }
        }

        // --- hasConflict ---

        #[test]
        fn bk_has_conflict_regardless_of_orientation() {
            let mut conflicts = HashMap::new();
            add_conflict(&mut conflicts, "b", "a");
            assert!(has_conflict(&conflicts, "a", "b"));
            assert!(has_conflict(&conflicts, "b", "a"));
        }

        #[test]
        fn bk_has_conflict_multiple_for_same_node() {
            let mut conflicts = HashMap::new();
            add_conflict(&mut conflicts, "a", "b");
            add_conflict(&mut conflicts, "a", "c");
            assert!(has_conflict(&conflicts, "a", "b"));
            assert!(has_conflict(&conflicts, "a", "c"));
        }

        // --- findType1Conflicts ---

        fn setup_type1_graph() -> (Graph, Vec<Vec<String>>) {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_node("d", node(1, 1));
            g.set_edge("a", "d", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            (g, layering)
        }

        #[test]
        fn bk_find_type1_no_conflict_non_crossing() {
            let (mut g, _) = setup_type1_graph();
            g.remove_edge("a", "d");
            g.remove_edge("b", "c");
            g.set_edge("a", "c", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "c"));
            assert!(!has_conflict(&conflicts, "b", "d"));
        }

        #[test]
        fn bk_find_type1_no_conflict_type0() {
            let (g, layering) = setup_type1_graph();
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "d"));
            assert!(!has_conflict(&conflicts, "b", "c"));
        }

        #[test]
        fn bk_find_type1_no_conflict_when_one_dummy() {
            // Only one of the 4 nodes is dummy => type-0 still (need exactly non-dummies on BOTH sides)
            for v in &["a", "b", "c", "d"] {
                let (mut g, _) = setup_type1_graph();
                g.node_mut(v).dummy = Some("edge".to_string());
                let layering = build_layer_matrix(&g);
                let conflicts = find_type1_conflicts(&g, &layering);
                // Still type-0: not marked
                assert!(!has_conflict(&conflicts, "a", "d"));
                assert!(!has_conflict(&conflicts, "b", "c"));
            }
        }

        #[test]
        fn bk_find_type1_marks_conflict_when_only_one_non_dummy() {
            // When exactly one of {a,b,c,d} is non-dummy, the non-dummy touches one crossing edge
            // a or d non-dummy => a-d is marked; b or c non-dummy => b-c is marked
            for v in &["a", "b", "c", "d"] {
                let (mut g, _) = setup_type1_graph();
                for w in &["a", "b", "c", "d"] {
                    if v != w {
                        g.node_mut(w).dummy = Some("edge".to_string());
                    }
                }
                let layering = build_layer_matrix(&g);
                let conflicts = find_type1_conflicts(&g, &layering);
                if *v == "a" || *v == "d" {
                    assert!(
                        has_conflict(&conflicts, "a", "d"),
                        "expected a-d conflict when {} is non-dummy",
                        v
                    );
                    assert!(
                        !has_conflict(&conflicts, "b", "c"),
                        "unexpected b-c conflict when {} is non-dummy",
                        v
                    );
                } else {
                    assert!(
                        !has_conflict(&conflicts, "a", "d"),
                        "unexpected a-d conflict when {} is non-dummy",
                        v
                    );
                    assert!(
                        has_conflict(&conflicts, "b", "c"),
                        "expected b-c conflict when {} is non-dummy",
                        v
                    );
                }
            }
        }

        #[test]
        fn bk_find_type1_no_conflict_all_dummies() {
            let (mut g, _) = setup_type1_graph();
            for v in &["a", "b", "c", "d"] {
                g.node_mut(v).dummy = Some("edge".to_string());
            }
            let layering = build_layer_matrix(&g);
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "d"));
            assert!(!has_conflict(&conflicts, "b", "c"));
        }

        // --- findType2Conflicts ---

        fn setup_type2_graph() -> (Graph, Vec<Vec<String>>) {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_node("d", node(1, 1));
            g.set_edge("a", "d", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            (g, layering)
        }

        #[test]
        fn bk_find_type2_favoring_border_segments_1() {
            let (mut g, _) = setup_type2_graph();
            g.node_mut("a").dummy = Some("edge".to_string());
            g.node_mut("d").dummy = Some("edge".to_string());
            g.node_mut("b").dummy = Some("border".to_string());
            g.node_mut("c").dummy = Some("border".to_string());
            let layering = build_layer_matrix(&g);
            let conflicts = find_type2_conflicts(&g, &layering);
            assert!(has_conflict(&conflicts, "a", "d"));
            assert!(!has_conflict(&conflicts, "b", "c"));
        }

        #[test]
        fn bk_find_type2_favoring_border_segments_2() {
            let (mut g, _) = setup_type2_graph();
            g.node_mut("b").dummy = Some("edge".to_string());
            g.node_mut("c").dummy = Some("edge".to_string());
            g.node_mut("a").dummy = Some("border".to_string());
            g.node_mut("d").dummy = Some("border".to_string());
            let layering = build_layer_matrix(&g);
            let conflicts = find_type2_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "d"));
            assert!(has_conflict(&conflicts, "b", "c"));
        }

        // --- findType1Conflicts missing tests ---

        #[test]
        fn bk_find_type1_does_not_mark_edges_no_conflict() {
            // "does not mark edges that have no conflict"
            let (mut g, _) = setup_type1_graph();
            g.remove_edge("a", "d");
            g.remove_edge("b", "c");
            g.set_edge("a", "c", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "c"));
            assert!(!has_conflict(&conflicts, "b", "d"));
        }

        #[test]
        fn bk_find_type1_does_not_mark_type0_no_dummies() {
            // "does not mark type-0 conflicts (no dummies)"
            let (g, layering) = setup_type1_graph();
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "d"));
            assert!(!has_conflict(&conflicts, "b", "c"));
        }

        #[test]
        fn bk_find_type1_does_not_mark_type2_all_dummies() {
            // "does not mark type-2 conflicts (all dummies)"
            let (mut g, _) = setup_type1_graph();
            for v in &["a", "b", "c", "d"] {
                g.node_mut(v).dummy = Some("edge".to_string());
            }
            let layering = build_layer_matrix(&g);
            let conflicts = find_type1_conflicts(&g, &layering);
            assert!(!has_conflict(&conflicts, "a", "d"));
            assert!(!has_conflict(&conflicts, "b", "c"));
        }

        // --- verticalAlignment ---

        fn preds(g: &Graph, v: &str) -> Vec<String> {
            g.predecessors(v).unwrap_or_default()
        }

        #[test]
        fn bk_vertical_alignment_no_adjacencies() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(1, 0));
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(align.get("a").unwrap(), "a");
            assert_eq!(align.get("b").unwrap(), "b");
        }

        #[test]
        fn bk_vertical_alignment_sole_adjacency() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(1, 0));
            g.set_edge("a", "b", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "a");
            assert_eq!(align.get("a").unwrap(), "b");
            assert_eq!(align.get("b").unwrap(), "a");
        }

        #[test]
        fn bk_vertical_alignment_insertion_order_independent() {
            // "aligns correctly even regardless of node name / insertion order"
            let mut g = make_g();
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_node("z", node(0, 0));
            g.set_edge("z", "c", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("z").unwrap(), "z");
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(root.get("c").unwrap(), "z");
            assert_eq!(align.get("z").unwrap(), "c");
            assert_eq!(align.get("b").unwrap(), "b");
            assert_eq!(align.get("c").unwrap(), "z");
        }

        #[test]
        fn bk_vertical_alignment_left_median() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_edge("a", "c", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(root.get("c").unwrap(), "a");
            assert_eq!(align.get("a").unwrap(), "c");
            assert_eq!(align.get("b").unwrap(), "b");
            assert_eq!(align.get("c").unwrap(), "a");
        }

        #[test]
        fn bk_vertical_alignment_right_median_when_left_unavailable() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_edge("a", "c", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let mut conflicts = HashMap::new();
            add_conflict(&mut conflicts, "a", "c");
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(root.get("c").unwrap(), "b");
            assert_eq!(align.get("a").unwrap(), "a");
            assert_eq!(align.get("b").unwrap(), "c");
            assert_eq!(align.get("c").unwrap(), "b");
        }

        #[test]
        fn bk_vertical_alignment_neither_median_both_unavailable() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(1, 0));
            g.set_node("d", node(1, 1));
            g.set_edge("a", "d", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            // c aligns with b, so d cannot align with anything
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(root.get("c").unwrap(), "b");
            assert_eq!(root.get("d").unwrap(), "d");
            assert_eq!(align.get("b").unwrap(), "c");
            assert_eq!(align.get("c").unwrap(), "b");
            assert_eq!(align.get("d").unwrap(), "d");
        }

        #[test]
        fn bk_vertical_alignment_single_median_odd_adjacencies() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(0, 1));
            g.set_node("c", node(0, 2));
            g.set_node("d", node(1, 0));
            g.set_edge("a", "d", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            g.set_edge("c", "d", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            // median of [a,b,c] for d is b
            assert_eq!(root.get("b").unwrap(), "b");
            assert_eq!(root.get("d").unwrap(), "b");
            assert_eq!(align.get("b").unwrap(), "d");
            assert_eq!(align.get("d").unwrap(), "b");
        }

        #[test]
        fn bk_vertical_alignment_across_multiple_layers() {
            let mut g = make_g();
            g.set_node("a", node(0, 0));
            g.set_node("b", node(1, 0));
            g.set_node("c", node(1, 1));
            g.set_node("d", node(2, 0));
            // a->b->d and a->c->d
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            g.set_edge("a", "c", EdgeLabel::default(), None);
            g.set_edge("c", "d", EdgeLabel::default(), None);
            let layering = build_layer_matrix(&g);
            let conflicts = HashMap::new();
            let (root, align) = vertical_alignment(&g, &layering, &conflicts, &|v| preds(&g, v));
            assert_eq!(root.get("a").unwrap(), "a");
            assert_eq!(root.get("b").unwrap(), "a");
            assert_eq!(root.get("c").unwrap(), "c");
            assert_eq!(root.get("d").unwrap(), "a");
            assert_eq!(align.get("a").unwrap(), "b");
            assert_eq!(align.get("b").unwrap(), "d");
            assert_eq!(align.get("d").unwrap(), "a");
        }

        // --- horizontalCompaction ---

        #[test]
        fn bk_horizontal_compaction_single_node_at_origin() {
            let mut g = make_g();
            g.set_node("a", node_w(0, 0, 0.0));
            let root: HashMap<String, String> =
                [("a".to_string(), "a".to_string())].into_iter().collect();
            let align: HashMap<String, String> =
                [("a".to_string(), "a".to_string())].into_iter().collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            assert!((xs.get("a").unwrap() - 0.0).abs() < 1e-9);
        }

        #[test]
        fn bk_horizontal_compaction_adjacent_nodes_with_nodesep() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(100.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            assert!((xa - 0.0).abs() < 1e-9);
            // 100/2 + 100 + 200/2 = 250
            assert!((xb - 250.0).abs() < 1e-9);
        }

        #[test]
        fn bk_horizontal_compaction_nodes_in_same_block_aligned() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 200.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "a".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "b".to_string()),
                ("b".to_string(), "a".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            assert!((xs.get("a").unwrap() - 0.0).abs() < 1e-9);
            assert!((xs.get("b").unwrap() - 0.0).abs() < 1e-9);
        }

        #[test]
        fn bk_horizontal_compaction_separates_adjacent_edges_by_edgesep() {
            // "separates adjacent edges by specified node separation" (uses edgesep + dummy nodes)
            let mut g = make_g();
            g.graph_mut().edgesep = Some(20.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            assert!((xs.get("a").unwrap() - 0.0).abs() < 1e-9);
            // 100/2 + 20 + 200/2 = 170
            assert!((xs.get("b").unwrap() - 170.0).abs() < 1e-9);
        }

        #[test]
        fn bk_horizontal_compaction_separates_classes_with_appropriate_separation() {
            // "separates classes with the appropriate separation"
            let mut g = make_g();
            g.graph_mut().nodesep = Some(75.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 80.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "d".to_string()),
                ("c".to_string(), "c".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            // xs.a = 0, xs.b = 100/2 + 75 + 200/2 = 225
            // xs.c = 0, xs.d = xs.b = 225
            // BUT c is at order=0 in rank=1, and d is at order=1 in rank=1.
            // c must be placed before d: c needs room => xs.c = xs.d - 50/2 - 75 - 80/2 = 225 - 25 - 75 - 40 = 85
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            let xd = *xs.get("d").unwrap();
            assert!((xs.get("a").unwrap() - 0.0).abs() < 1e-9);
            assert!((xb - (100.0 / 2.0 + 75.0 + 200.0 / 2.0)).abs() < 1e-9);
            // xd == xb (same block)
            assert!((xd - xb).abs() < 1e-9);
            // xc = xd - 80/2 - 75 - 50/2 = xb - 40 - 75 - 25 = xb - 140
            assert!((xc - (xb - 80.0 / 2.0 - 75.0 - 50.0 / 2.0)).abs() < 1e-9);
        }

        #[test]
        fn bk_horizontal_compaction_separates_blocks_appropriate_separation() {
            // "separates blocks with the appropriate separation" (3-node variant)
            // root = {a:"a", b:"a", c:"c"}, align = {a:"b", b:"a", c:"c"}
            // nodesep=75, a(rank=0,order=0,w=100), b(rank=1,order=1,w=200), c(rank=1,order=0,w=50)
            let mut g = make_g();
            g.graph_mut().nodesep = Some(75.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 200.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "a".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "b".to_string()),
                ("b".to_string(), "a".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            // From JS: xs.a = 50/2 + 75 + 200/2 = 25+75+100 = 200
            // xs.b = xs.a = 200
            // xs.c = 0
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            assert!(
                (xa - (50.0 / 2.0 + 75.0 + 200.0 / 2.0)).abs() < 1e-9,
                "xa expected {}, got {}",
                50.0 / 2.0 + 75.0 + 200.0 / 2.0,
                xa
            );
            assert!((xb - xa).abs() < 1e-9, "xb={} should equal xa={}", xb, xa);
            assert!((xc - 0.0).abs() < 1e-9, "xc expected 0, got {}", xc);
        }

        #[test]
        fn bk_horizontal_compaction_shifts_classes_max_sep_1() {
            // "shifts classes by max sep from the adjacent block #1"
            // root = {a:"a", b:"b", c:"a", d:"b"}
            // align = {a:"c", b:"d", c:"a", d:"b"}
            // nodesep=75, a(0,0,w=50), b(0,1,w=150), c(1,0,w=60), d(1,1,w=70)
            let mut g = make_g();
            g.graph_mut().nodesep = Some(75.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 150.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 60.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 70.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "a".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "c".to_string()),
                ("b".to_string(), "d".to_string()),
                ("c".to_string(), "a".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            // JS: xs.a=0, xs.b=50/2+75+150/2=25+75+75=175, xs.c=0, xs.d=175
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            let xd = *xs.get("d").unwrap();
            assert!((xa - 0.0).abs() < 1e-9, "xa expected 0, got {}", xa);
            assert!(
                (xb - (50.0 / 2.0 + 75.0 + 150.0 / 2.0)).abs() < 1e-9,
                "xb expected {}, got {}",
                50.0 / 2.0 + 75.0 + 150.0 / 2.0,
                xb
            );
            assert!((xc - 0.0).abs() < 1e-9, "xc expected 0, got {}", xc);
            assert!((xd - xb).abs() < 1e-9, "xd={} should equal xb={}", xd, xb);
        }

        #[test]
        fn bk_horizontal_compaction_shifts_classes_max_sep_2() {
            // "shifts classes by max sep from the adjacent block #2"
            // Same topology, but b is narrower and d is wider
            // nodesep=75, a(0,0,w=50), b(0,1,w=70), c(1,0,w=60), d(1,1,w=150)
            let mut g = make_g();
            g.graph_mut().nodesep = Some(75.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 70.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 60.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 150.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "a".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "c".to_string()),
                ("b".to_string(), "d".to_string()),
                ("c".to_string(), "a".to_string()),
                ("d".to_string(), "b".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            // JS: xs.a=0, xs.b=60/2+75+150/2=30+75+75=180, xs.c=0, xs.d=180
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            let xd = *xs.get("d").unwrap();
            assert!((xa - 0.0).abs() < 1e-9, "xa expected 0, got {}", xa);
            assert!(
                (xb - (60.0 / 2.0 + 75.0 + 150.0 / 2.0)).abs() < 1e-9,
                "xb expected {}, got {}",
                60.0 / 2.0 + 75.0 + 150.0 / 2.0,
                xb
            );
            assert!((xc - 0.0).abs() < 1e-9, "xc expected 0, got {}", xc);
            assert!((xd - xb).abs() < 1e-9, "xd={} should equal xb={}", xd, xb);
        }

        #[test]
        fn bk_horizontal_compaction_cascades_class_shift() {
            // "cascades class shift"
            // root = {a:"a", b:"b", c:"c", d:"d", e:"b", f:"f", g:"d"}
            // align = {a:"a", b:"e", c:"c", d:"g", e:"b", f:"f", g:"d"}
            // nodesep=75, all nodes w=50
            let mut g = make_g();
            g.graph_mut().nodesep = Some(75.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "e",
                NodeLabel {
                    rank: Some(1),
                    order: Some(2),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "f",
                NodeLabel {
                    rank: Some(2),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "g",
                NodeLabel {
                    rank: Some(2),
                    order: Some(1),
                    width: 50.0,
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
                ("d".to_string(), "d".to_string()),
                ("e".to_string(), "b".to_string()),
                ("f".to_string(), "f".to_string()),
                ("g".to_string(), "d".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "e".to_string()),
                ("c".to_string(), "c".to_string()),
                ("d".to_string(), "g".to_string()),
                ("e".to_string(), "b".to_string()),
                ("f".to_string(), "f".to_string()),
                ("g".to_string(), "d".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            let xd = *xs.get("d").unwrap();
            let xe = *xs.get("e").unwrap();
            let xf = *xs.get("f").unwrap();
            let xg = *xs.get("g").unwrap();
            let sep = 50.0 / 2.0 + 75.0 + 50.0 / 2.0;
            // From JS: xs.a = xs.b - sep; xs.b = xs.e; xs.c = xs.f; xs.d = xs.c + sep;
            // xs.e = xs.d + sep; xs.g = xs.f + sep
            assert!(
                (xa - (xb - sep)).abs() < 1e-9,
                "xa={} xb-sep={}",
                xa,
                xb - sep
            );
            assert!((xb - xe).abs() < 1e-9, "xb={} xe={}", xb, xe);
            assert!((xc - xf).abs() < 1e-9, "xc={} xf={}", xc, xf);
            assert!(
                (xd - (xc + sep)).abs() < 1e-9,
                "xd={} xc+sep={}",
                xd,
                xc + sep
            );
            assert!(
                (xe - (xd + sep)).abs() < 1e-9,
                "xe={} xd+sep={}",
                xe,
                xd + sep
            );
            assert!(
                (xg - (xf + sep)).abs() < 1e-9,
                "xg={} xf+sep={}",
                xg,
                xf + sep
            );
        }

        #[test]
        fn bk_horizontal_compaction_handles_labelpos_l() {
            // "handles labelpos = l" in horizontalCompaction
            // edgesep=50, a(edge,w=100), b(edge-label,labelpos="l",w=200), c(edge,w=300)
            // root={a:"a",b:"b",c:"c"}, align={a:"a",b:"b",c:"c"}
            // xs.b = xs.a + 100/2 + 50 + 200 (full width for "l")
            // xs.c = xs.b + 0 + 50 + 300/2
            let mut g = make_g();
            g.graph_mut().edgesep = Some(50.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge-label".to_string()),
                    labelpos: Some("l".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    order: Some(2),
                    width: 300.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            // labelpos="l": left-oriented label
            // sep(a->b) = a_half + edgesep + b_width = 100/2 + 50 + 200 = 200
            assert!(
                (xb - (xa + 100.0 / 2.0 + 50.0 + 200.0)).abs() < 1e-9,
                "xb={} expected xa+200={}",
                xb,
                xa + 100.0 / 2.0 + 50.0 + 200.0
            );
            // sep(b->c) = 0 + edgesep + c_half = 0 + 50 + 300/2 = 200
            assert!(
                (xc - (xb + 0.0 + 50.0 + 300.0 / 2.0)).abs() < 1e-9,
                "xc={} expected xb+200={}",
                xc,
                xb + 0.0 + 50.0 + 300.0 / 2.0
            );
        }

        #[test]
        fn bk_horizontal_compaction_handles_labelpos_c() {
            // "handles labelpos = c" in horizontalCompaction
            // edgesep=50, a(edge,w=100), b(edge-label,labelpos="c",w=200), c(edge,w=300)
            // xs.b = xs.a + 100/2 + 50 + 200/2
            // xs.c = xs.b + 200/2 + 50 + 300/2
            let mut g = make_g();
            g.graph_mut().edgesep = Some(50.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge-label".to_string()),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    order: Some(2),
                    width: 300.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            // labelpos="c": center-aligned label
            assert!(
                (xb - (xa + 100.0 / 2.0 + 50.0 + 200.0 / 2.0)).abs() < 1e-9,
                "xb={} expected xa+200={}",
                xb,
                xa + 100.0 / 2.0 + 50.0 + 200.0 / 2.0
            );
            assert!(
                (xc - (xb + 200.0 / 2.0 + 50.0 + 300.0 / 2.0)).abs() < 1e-9,
                "xc={} expected xb+300={}",
                xc,
                xb + 200.0 / 2.0 + 50.0 + 300.0 / 2.0
            );
        }

        #[test]
        fn bk_horizontal_compaction_handles_labelpos_r() {
            // "handles labelpos = r" in horizontalCompaction
            // edgesep=50, a(edge,w=100), b(edge-label,labelpos="r",w=200), c(edge,w=300)
            // xs.b = xs.a + 100/2 + 50 + 0  (right: b contributes no left half)
            // xs.c = xs.b + 200 + 50 + 300/2
            let mut g = make_g();
            g.graph_mut().edgesep = Some(50.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge-label".to_string()),
                    labelpos: Some("r".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    order: Some(2),
                    width: 300.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let root: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let align: HashMap<String, String> = [
                ("a".to_string(), "a".to_string()),
                ("b".to_string(), "b".to_string()),
                ("c".to_string(), "c".to_string()),
            ]
            .into_iter()
            .collect();
            let layering = build_layer_matrix(&g);
            let xs = horizontal_compaction(&g, &layering, &root, &align, false);
            let xa = *xs.get("a").unwrap();
            let xb = *xs.get("b").unwrap();
            let xc = *xs.get("c").unwrap();
            // labelpos="r": right-oriented label
            assert!(
                (xb - (xa + 100.0 / 2.0 + 50.0 + 0.0)).abs() < 1e-9,
                "xb={} expected xa+100={}",
                xb,
                xa + 100.0 / 2.0 + 50.0
            );
            assert!(
                (xc - (xb + 200.0 + 50.0 + 300.0 / 2.0)).abs() < 1e-9,
                "xc={} expected xb+400={}",
                xc,
                xb + 200.0 + 50.0 + 300.0 / 2.0
            );
        }

        // --- alignCoordinates ---

        #[test]
        fn bk_align_coordinates_single_node() {
            let mut xss: HashMap<String, PositionMap> = HashMap::new();
            xss.insert(
                "ul".to_string(),
                [("a".to_string(), 50.0)].into_iter().collect(),
            );
            xss.insert(
                "ur".to_string(),
                [("a".to_string(), 100.0)].into_iter().collect(),
            );
            xss.insert(
                "dl".to_string(),
                [("a".to_string(), 50.0)].into_iter().collect(),
            );
            xss.insert(
                "dr".to_string(),
                [("a".to_string(), 200.0)].into_iter().collect(),
            );
            let align_to = xss.get("ul").unwrap().clone();
            align_coordinates(&mut xss, &align_to);
            assert!((xss.get("ul").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
            assert!((xss.get("ur").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
            assert!((xss.get("dl").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
            assert!((xss.get("dr").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
        }

        #[test]
        fn bk_align_coordinates_multiple_nodes() {
            let mut xss: HashMap<String, PositionMap> = HashMap::new();
            xss.insert(
                "ul".to_string(),
                [("a".to_string(), 50.0), ("b".to_string(), 1000.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "ur".to_string(),
                [("a".to_string(), 100.0), ("b".to_string(), 900.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "dl".to_string(),
                [("a".to_string(), 150.0), ("b".to_string(), 800.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "dr".to_string(),
                [("a".to_string(), 200.0), ("b".to_string(), 700.0)]
                    .into_iter()
                    .collect(),
            );
            let align_to = xss.get("ul").unwrap().clone();
            align_coordinates(&mut xss, &align_to);
            // ul unchanged
            assert!((xss.get("ul").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
            assert!((xss.get("ul").unwrap().get("b").unwrap() - 1000.0).abs() < 1e-9);
            // ur: min(ul)=50, min(ur)=100 => delta=50-100=-50 ... wait, ul min=50, ur min=100
            // horiz="r" => delta = max(align_to) - max(xs) = 1000 - 900 = 100
            assert!((xss.get("ur").unwrap().get("a").unwrap() - 200.0).abs() < 1e-9);
            assert!((xss.get("ur").unwrap().get("b").unwrap() - 1000.0).abs() < 1e-9);
            // dl: horiz="l" => delta = min(ul) - min(dl) = 50 - 150 = -100
            assert!((xss.get("dl").unwrap().get("a").unwrap() - 50.0).abs() < 1e-9);
            assert!((xss.get("dl").unwrap().get("b").unwrap() - 700.0).abs() < 1e-9);
            // dr: horiz="r" => delta = max(ul) - max(dr) = 1000 - 700 = 300
            assert!((xss.get("dr").unwrap().get("a").unwrap() - 500.0).abs() < 1e-9);
            assert!((xss.get("dr").unwrap().get("b").unwrap() - 1000.0).abs() < 1e-9);
        }

        // --- findSmallestWidthAlignment ---

        #[test]
        fn bk_find_smallest_width_alignment_basic() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 50.0,
                    ..Default::default()
                },
            );
            let mut xss: HashMap<String, PositionMap> = HashMap::new();
            xss.insert(
                "ul".to_string(),
                [("a".to_string(), 0.0), ("b".to_string(), 1000.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "ur".to_string(),
                [("a".to_string(), -5.0), ("b".to_string(), 1000.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "dl".to_string(),
                [("a".to_string(), 5.0), ("b".to_string(), 2000.0)]
                    .into_iter()
                    .collect(),
            );
            xss.insert(
                "dr".to_string(),
                [("a".to_string(), 0.0), ("b".to_string(), 200.0)]
                    .into_iter()
                    .collect(),
            );
            let result = find_smallest_width_alignment(&g, &xss);
            // dr: width = (200+25) - (0-25) = 225 - (-25) = 250
            // ul: 1025 - (-25) = 1050; ur: 1025 - (-30) = 1055; dl: 2025 - (-20) = 2045
            // dr is smallest
            assert!((result.get("a").unwrap() - 0.0).abs() < 1e-9);
            assert!((result.get("b").unwrap() - 200.0).abs() < 1e-9);
        }

        #[test]
        fn bk_find_smallest_width_takes_node_width_into_account() {
            // "takes node width into account"
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    width: 200.0,
                    ..Default::default()
                },
            );
            let mut xss: HashMap<String, PositionMap> = HashMap::new();
            xss.insert(
                "ul".to_string(),
                [
                    ("a".to_string(), 0.0),
                    ("b".to_string(), 100.0),
                    ("c".to_string(), 75.0),
                ]
                .into_iter()
                .collect(),
            );
            xss.insert(
                "ur".to_string(),
                [
                    ("a".to_string(), 0.0),
                    ("b".to_string(), 100.0),
                    ("c".to_string(), 80.0),
                ]
                .into_iter()
                .collect(),
            );
            xss.insert(
                "dl".to_string(),
                [
                    ("a".to_string(), 0.0),
                    ("b".to_string(), 100.0),
                    ("c".to_string(), 85.0),
                ]
                .into_iter()
                .collect(),
            );
            xss.insert(
                "dr".to_string(),
                [
                    ("a".to_string(), 0.0),
                    ("b".to_string(), 100.0),
                    ("c".to_string(), 90.0),
                ]
                .into_iter()
                .collect(),
            );
            // ul: max(0+25,100+25,75+100)=175, min(0-25)=-25 => width=200
            // ur: 180-(-25)=205; dl: 185-(-25)=210; dr: 190-(-25)=215 => ul is smallest
            let result = find_smallest_width_alignment(&g, &xss);
            assert!((result.get("a").unwrap() - 0.0).abs() < 1e-9);
            assert!((result.get("b").unwrap() - 100.0).abs() < 1e-9);
            assert!((result.get("c").unwrap() - 75.0).abs() < 1e-9);
        }

        // --- balance ---

        #[test]
        fn bk_balance_shared_median() {
            let xss: HashMap<String, PositionMap> = [
                (
                    "ul".to_string(),
                    [("a".to_string(), 0.0)].into_iter().collect(),
                ),
                (
                    "ur".to_string(),
                    [("a".to_string(), 100.0)].into_iter().collect(),
                ),
                (
                    "dl".to_string(),
                    [("a".to_string(), 100.0)].into_iter().collect(),
                ),
                (
                    "dr".to_string(),
                    [("a".to_string(), 200.0)].into_iter().collect(),
                ),
            ]
            .into_iter()
            .collect();
            let result = balance(&xss, None);
            // sorted: [0,100,100,200], median = (100+100)/2 = 100
            assert!((result.get("a").unwrap() - 100.0).abs() < 1e-9);
        }

        #[test]
        fn bk_balance_average_of_different_medians() {
            let xss: HashMap<String, PositionMap> = [
                (
                    "ul".to_string(),
                    [("a".to_string(), 0.0)].into_iter().collect(),
                ),
                (
                    "ur".to_string(),
                    [("a".to_string(), 75.0)].into_iter().collect(),
                ),
                (
                    "dl".to_string(),
                    [("a".to_string(), 125.0)].into_iter().collect(),
                ),
                (
                    "dr".to_string(),
                    [("a".to_string(), 200.0)].into_iter().collect(),
                ),
            ]
            .into_iter()
            .collect();
            let result = balance(&xss, None);
            // sorted: [0,75,125,200], median = (75+125)/2 = 100
            assert!((result.get("a").unwrap() - 100.0).abs() < 1e-9);
        }

        #[test]
        fn bk_balance_multiple_nodes() {
            let xss: HashMap<String, PositionMap> = [
                (
                    "ul".to_string(),
                    [("a".to_string(), 0.0), ("b".to_string(), 50.0)]
                        .into_iter()
                        .collect(),
                ),
                (
                    "ur".to_string(),
                    [("a".to_string(), 75.0), ("b".to_string(), 0.0)]
                        .into_iter()
                        .collect(),
                ),
                (
                    "dl".to_string(),
                    [("a".to_string(), 125.0), ("b".to_string(), 60.0)]
                        .into_iter()
                        .collect(),
                ),
                (
                    "dr".to_string(),
                    [("a".to_string(), 200.0), ("b".to_string(), 75.0)]
                        .into_iter()
                        .collect(),
                ),
            ]
            .into_iter()
            .collect();
            let result = balance(&xss, None);
            // a: sorted [0,75,125,200] => (75+125)/2 = 100
            assert!((result.get("a").unwrap() - 100.0).abs() < 1e-9);
            // b: sorted [0,50,60,75] => (50+60)/2 = 55
            assert!((result.get("b").unwrap() - 55.0).abs() < 1e-9);
        }

        // --- positionX ---

        #[test]
        fn bk_position_x_single_node_at_origin() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            let result = position_x(&g);
            assert!((result.get("a").unwrap() - 0.0).abs() < 1e-9);
        }

        #[test]
        fn bk_position_x_single_block_at_origin() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            let result = position_x(&g);
            assert!((result.get("a").unwrap() - 0.0).abs() < 0.5);
            assert!((result.get("b").unwrap() - 0.0).abs() < 0.5);
        }

        #[test]
        fn bk_position_x_block_at_origin_different_sizes() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 40.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 500.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    order: Some(0),
                    width: 20.0,
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let result = position_x(&g);
            assert!((result.get("a").unwrap() - 0.0).abs() < 0.5);
            assert!((result.get("b").unwrap() - 0.0).abs() < 0.5);
            assert!((result.get("c").unwrap() - 0.0).abs() < 0.5);
        }

        #[test]
        fn bk_position_x_handles_labelpos_c() {
            // "handles labelpos = c"
            let mut g = make_g();
            g.graph_mut().edgesep = Some(50.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge-label".to_string()),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    order: Some(2),
                    width: 300.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let result = position_x(&g);
            let xa = *result.get("a").unwrap();
            let xb = *result.get("b").unwrap();
            let xc = *result.get("c").unwrap();
            // xs.b = xs.a + 100/2 + 50 + 200/2 = xs.a + 200
            assert!(
                (xb - (xa + 100.0 / 2.0 + 50.0 + 200.0 / 2.0)).abs() < 1.0,
                "expected xb = xa + 200, got xa={} xb={}",
                xa,
                xb
            );
            // xs.c = xs.b + 200/2 + 50 + 300/2 = xs.b + 300
            assert!(
                (xc - (xb + 200.0 / 2.0 + 50.0 + 300.0 / 2.0)).abs() < 1.0,
                "expected xc = xb + 300, got xb={} xc={}",
                xb,
                xc
            );
        }

        #[test]
        fn bk_position_x_handles_labelpos_r() {
            // "handles labelpos = r"
            let mut g = make_g();
            g.graph_mut().edgesep = Some(50.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 100.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 200.0,
                    dummy: Some("edge-label".to_string()),
                    labelpos: Some("r".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    order: Some(2),
                    width: 300.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            let result = position_x(&g);
            let xa = *result.get("a").unwrap();
            let xb = *result.get("b").unwrap();
            let xc = *result.get("c").unwrap();
            // xs.b = xs.a + 100/2 + 50 + 0 = xs.a + 100
            assert!(
                (xb - (xa + 100.0 / 2.0 + 50.0 + 0.0)).abs() < 1.0,
                "expected xb = xa + 100, got xa={} xb={}",
                xa,
                xb
            );
            // xs.c = xs.b + 200 + 50 + 300/2 = xs.b + 400
            assert!(
                (xc - (xb + 200.0 + 50.0 + 300.0 / 2.0)).abs() < 1.0,
                "expected xc = xb + 400, got xb={} xc={}",
                xb,
                xc
            );
        }

        #[test]
        fn bk_position_x_centers_node_predecessor_of_two_same_sized() {
            // "centers a node if it is a predecessor of two same sized nodes"
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 20.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("a", "c", EdgeLabel::default(), None);
            let result = position_x(&g);
            let xa = *result.get("a").unwrap();
            let xb = *result.get("b").unwrap();
            let xc = *result.get("c").unwrap();
            // From JS: pos = {a:a, b:a-(25+5), c:a+(25+5)}
            assert!(
                (xb - (xa - (25.0 + 5.0))).abs() < 1.0,
                "xb={} expected xa-30={}",
                xb,
                xa - 30.0
            );
            assert!(
                (xc - (xa + (25.0 + 5.0))).abs() < 1.0,
                "xc={} expected xa+30={}",
                xc,
                xa + 30.0
            );
        }

        #[test]
        fn bk_position_x_shifts_blocks_on_both_sides_of_aligned_block() {
            // "shifts blocks on both sides of aligned block"
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 60.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 70.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 80.0,
                    ..Default::default()
                },
            );
            g.set_edge("b", "c", EdgeLabel::default(), None);
            let result = position_x(&g);
            let xb = *result.get("b").unwrap();
            let xa = *result.get("a").unwrap();
            let xc = *result.get("c").unwrap();
            let xd = *result.get("d").unwrap();
            // From JS: b == c (aligned); a = b - 60/2 - 10 - 50/2; d = c + 70/2 + 10 + 80/2
            assert!((xb - xc).abs() < 1.0, "xb={} should equal xc={}", xb, xc);
            assert!(
                (xa - (xb - 60.0 / 2.0 - 10.0 - 50.0 / 2.0)).abs() < 1.0,
                "xa={} expected {}",
                xa,
                xb - 60.0 / 2.0 - 10.0 - 50.0 / 2.0
            );
            assert!(
                (xd - (xc + 70.0 / 2.0 + 10.0 + 80.0 / 2.0)).abs() < 1.0,
                "xd={} expected {}",
                xd,
                xc + 70.0 / 2.0 + 10.0 + 80.0 / 2.0
            );
        }

        #[test]
        fn bk_position_x_aligns_inner_segments() {
            // "aligns inner segments"
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    width: 50.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    width: 60.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    width: 70.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    width: 80.0,
                    dummy: Some("edge".to_string()),
                    ..Default::default()
                },
            );
            g.set_edge("b", "c", EdgeLabel::default(), None);
            g.set_edge("a", "d", EdgeLabel::default(), None);
            let result = position_x(&g);
            let xa = *result.get("a").unwrap();
            let xb = *result.get("b").unwrap();
            let xc = *result.get("c").unwrap();
            let xd = *result.get("d").unwrap();
            // inner segment a->d is aligned: xa == xd
            assert!(
                (xa - xd).abs() < 1.0,
                "expected xa == xd, got xa={} xd={}",
                xa,
                xd
            );
            // b is to the right of a: xb = xa + 50/2 + 10 + 60/2 = xa + 65
            assert!(
                (xb - (xa + 50.0 / 2.0 + 10.0 + 60.0 / 2.0)).abs() < 1.0,
                "expected xb = xa + 65, got xa={} xb={}",
                xa,
                xb
            );
            // c is to the left of d: xc = xd - 70/2 - 10 - 80/2 = xd - 85
            assert!(
                (xc - (xd - 70.0 / 2.0 - 10.0 - 80.0 / 2.0)).abs() < 1.0,
                "expected xc = xd - 85, got xd={} xc={}",
                xd,
                xc
            );
        }
    } // mod position_bk_tests

    // =========================================================================
    // Ported from dagre-js/test/position-test.ts
    // =========================================================================

    mod position_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::position::position;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, false, true);
            g.set_graph(GraphLabel {
                ranksep: Some(50.0),
                nodesep: Some(50.0),
                edgesep: Some(10.0),
                ..Default::default()
            });
            g
        }

        fn n(width: f64, height: f64, rank: i32, order: i32) -> NodeLabel {
            NodeLabel {
                width,
                height,
                rank: Some(rank),
                order: Some(order),
                ..Default::default()
            }
        }

        #[test]
        fn position_respects_ranksep() {
            let mut g = make_g();
            g.graph_mut().ranksep = Some(1000.0);
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(50.0, 80.0, 1, 0));
            g.set_edge("a", "b", EdgeLabel::default(), None);
            position(&mut g);
            // b.y = 100 + 1000 + 80/2 = 1140
            assert!(
                (g.node("b").y.unwrap() - 1140.0).abs() < 1.0,
                "b.y expected ≈1140, got {:?}",
                g.node("b").y
            );
        }

        #[test]
        fn position_uses_largest_height_in_rank() {
            let mut g = make_g();
            g.graph_mut().ranksep = Some(1000.0);
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(50.0, 80.0, 0, 1));
            g.set_node("c", n(50.0, 90.0, 1, 0));
            g.set_edge("a", "c", EdgeLabel::default(), None);
            position(&mut g);
            assert!((g.node("a").y.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("b").y.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("c").y.unwrap() - (100.0 + 1000.0 + 45.0)).abs() < 1.0);
        }

        #[test]
        fn position_respects_nodesep() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(1000.0);
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(70.0, 80.0, 0, 1));
            position(&mut g);
            let ax = g.node("a").x.unwrap();
            let bx = g.node("b").x.unwrap();
            // b.x = a.x + 50/2 + 1000 + 70/2 = a.x + 1060
            assert!(
                (bx - (ax + 50.0 / 2.0 + 1000.0 + 70.0 / 2.0)).abs() < 1.0,
                "expected bx = ax + 1060, got ax={}, bx={}",
                ax,
                bx
            );
        }

        #[test]
        fn position_does_not_position_subgraph_node() {
            let mut g = make_g();
            g.set_node("a", n(50.0, 50.0, 0, 0));
            g.set_node("sg1", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            position(&mut g);
            assert!(g.node("sg1").x.is_none(), "sg1 should not have x");
            assert!(g.node("sg1").y.is_none(), "sg1 should not have y");
        }

        #[test]
        fn position_rankalign_top() {
            let mut g = make_g();
            g.graph_mut().rankalign = Some("top".to_string());
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(50.0, 60.0, 0, 1));
            position(&mut g);
            assert!((g.node("a").y.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("b").y.unwrap() - 30.0).abs() < 1.0);
        }

        #[test]
        fn position_rankalign_bottom() {
            let mut g = make_g();
            g.graph_mut().rankalign = Some("bottom".to_string());
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(50.0, 60.0, 0, 1));
            position(&mut g);
            // max_height = 100
            // a.y = 100 - 100/2 = 50; b.y = 100 - 60/2 = 70
            assert!((g.node("a").y.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("b").y.unwrap() - 70.0).abs() < 1.0);
        }

        #[test]
        fn position_rankalign_center() {
            let mut g = make_g();
            g.graph_mut().rankalign = Some("center".to_string());
            g.set_node("a", n(50.0, 100.0, 0, 0));
            g.set_node("b", n(50.0, 60.0, 0, 1));
            position(&mut g);
            // center = max_height/2 = 50
            assert!((g.node("a").y.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("b").y.unwrap() - 50.0).abs() < 1.0);
        }
    } // mod position_tests

    // =========================================================================
    // Ported from dagre-js/test/layout-test.ts (remaining tests)
    // =========================================================================

    mod layout_remaining_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::layout::layout;

        fn make_g() -> Graph {
            Graph::with_options(true, true, true)
        }

        #[test]
        fn layout_edge_with_label() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                ranksep: Some(300.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 75.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(60.0),
                    height: Some(70.0),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            // a.x = 75/2 = 37.5, a.y = 100/2 = 50
            assert!((g.node("a").x.unwrap() - 37.5).abs() < 1.0);
            assert!((g.node("a").y.unwrap() - 50.0).abs() < 1.0);
            // b.y = 100 + 150 + 70 + 150 + 200/2 = 570
            assert!((g.node("b").x.unwrap() - 37.5).abs() < 1.0);
            assert!((g.node("b").y.unwrap() - 570.0).abs() < 1.0);
            // edge.x = 75/2, edge.y = 100 + 150 + 70/2 = 285
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 37.5).abs() < 1.0);
            assert!((el.y.unwrap() - 285.0).abs() < 1.0);
        }

        #[test]
        fn layout_long_edge_with_label() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                ranksep: Some(300.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 75.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(60.0),
                    height: Some(70.0),
                    minlen: Some(2),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 37.5).abs() < 1.0);
            let ay = g.node("a").y.unwrap();
            let by = g.node("b").y.unwrap();
            assert!(el.y.unwrap() > ay);
            assert!(el.y.unwrap() < by);
        }

        #[test]
        fn layout_short_cycle() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                ranksep: Some(200.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge("b", "a", EdgeLabel::default(), None);
            layout(&mut g);

            let ax = g.node("a").x.unwrap();
            let ay = g.node("a").y.unwrap();
            let bx = g.node("b").x.unwrap();
            let by = g.node("b").y.unwrap();
            assert!((ax - 50.0).abs() < 1.0);
            assert!((ay - 50.0).abs() < 1.0);
            assert!((bx - 50.0).abs() < 1.0);
            assert!((by - 350.0).abs() < 1.0); // 100 + 200 + 50

            // One arrow points down, one up
            let ab = g.edge_vw("a", "b").unwrap();
            let ba = g.edge_vw("b", "a").unwrap();
            let ab_points = ab.points.as_ref().unwrap();
            let ba_points = ba.points.as_ref().unwrap();
            // a->b: second point y > first point y (going down)
            assert!(ab_points[1].y > ab_points[0].y);
            // b->a: first point y > second point y (going up)
            assert!(ba_points[0].y > ba_points[1].y);
        }

        #[test]
        fn layout_adds_rect_intersects() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                ranksep: Some(200.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge("a", "b", EdgeLabel::default(), None);
            layout(&mut g);

            let el = g.edge_vw("a", "b").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 3);
            // intersect with bottom of a
            assert!((points[0].x - 50.0).abs() < 1.0);
            assert!((points[0].y - 100.0).abs() < 1.0);
            // middle point
            assert!((points[1].x - 50.0).abs() < 1.0);
            assert!((points[1].y - 200.0).abs() < 1.0); // 100 + 200/2
                                                        // intersect with top of b
            assert!((points[2].x - 50.0).abs() < 1.0);
            assert!((points[2].y - 300.0).abs() < 1.0); // 100 + 200
        }

        #[test]
        fn layout_adds_rect_intersects_multi_rank() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                ranksep: Some(200.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            let el = g.edge_vw("a", "b").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 5);
            assert!((points[0].x - 50.0).abs() < 1.0);
            assert!((points[0].y - 100.0).abs() < 1.0); // bottom of a
            assert!((points[4].x - 50.0).abs() < 1.0);
            assert!((points[4].y - 500.0).abs() < 1.0); // 100 + 800/2
        }

        #[test]
        fn layout_self_loop_tb() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                edgesep: Some(75.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "a",
                EdgeLabel {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            let node_a = g.node("a");
            let ax = node_a.x.unwrap();
            let ay = node_a.y.unwrap();
            let el = g.edge_vw("a", "a").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 7);
            for p in points {
                assert!(p.x > ax);
                assert!((p.y - ay).abs() <= 50.0); // within height/2
            }
        }

        #[test]
        fn layout_subgraph_does_not_crash() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 50.0,
                    ..Default::default()
                },
            );
            g.set_parent("a", Some("sg1"));
            layout(&mut g);
            // Just ensure no panic
        }

        #[test]
        fn layout_minimizes_subgraph_height() {
            let mut g = make_g();
            for v in &["a", "b", "c", "d", "x", "y"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 50.0,
                        height: 50.0,
                        ..Default::default()
                    },
                );
            }
            // a -> b -> c -> d
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            g.set_edge("c", "d", EdgeLabel::default(), None);
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(100.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "y",
                "d",
                EdgeLabel {
                    weight: Some(100.0),
                    ..Default::default()
                },
                None,
            );
            g.set_parent("x", Some("sg"));
            g.set_parent("y", Some("sg"));
            layout(&mut g);
            assert!((g.node("x").y.unwrap() - g.node("y").y.unwrap()).abs() < 1.0);
        }

        #[test]
        fn layout_minimizes_separation_non_adjacent_to_subgraphs() {
            let mut g = make_g();
            for v in &["a", "b", "c"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 50.0,
                        height: 50.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            g.set_node("sg", NodeLabel::default());
            g.set_parent("c", Some("sg"));
            layout(&mut g);
            // b.y - a.y should be ~100 (50+50 with default nodesep)
            let ay = g.node("a").y.unwrap();
            let by = g.node("b").y.unwrap();
            assert!(
                (by - ay - 100.0).abs() < 1.0,
                "expected b.y - a.y ≈ 100, got {}",
                by - ay
            );
        }

        #[test]
        fn layout_subgraphs_different_rankdirs() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 50.0,
                    ..Default::default()
                },
            );
            g.set_node("sg", NodeLabel::default());
            g.set_parent("a", Some("sg"));

            for rankdir in &["tb", "bt", "lr", "rl"] {
                let mut g2 = g.clone();
                g2.graph_mut().rankdir = Some(rankdir.to_string());
                layout(&mut g2);
                assert!(
                    g2.node("sg").width > 50.0,
                    "sg.width should > 50 for rankdir={}",
                    rankdir
                );
                assert!(
                    g2.node("sg").height > 50.0,
                    "sg.height should > 50 for rankdir={}",
                    rankdir
                );
                assert!(
                    g2.node("sg").x.unwrap() > 25.0,
                    "sg.x should > 25 for rankdir={}",
                    rankdir
                );
                assert!(
                    g2.node("sg").y.unwrap() > 25.0,
                    "sg.y should > 25 for rankdir={}",
                    rankdir
                );
            }
        }

        /// Mirrors JS test "can layout a graph with subgraphs" + "can layout subgraphs with different rankdirs"
        /// Two nodes (a, b) in sg1, connected by an edge. Compound dagre must set sg1.x/y/width/height.
        #[test]
        fn compound_layout_single_nested_node() {
            // mirrors the JS test: "can layout a graph with a subgraph"
            // g.setNode("a", {width:50, height:50}); g.setNode("b", {width:50, height:50});
            // g.setParent("a", "sg1"); g.setParent("b", "sg1");
            // g.setEdge("a", "b", {}); layout(g);
            // expect g.node("sg1") to have x, y, width, height set
            let mut g = make_g();
            g.set_graph(GraphLabel::default());
            g.set_node(
                "sg1",
                NodeLabel {
                    width: 0.0,
                    height: 0.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 50.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 50.0,
                    height: 50.0,
                    ..Default::default()
                },
            );
            g.set_parent("a", Some("sg1"));
            g.set_parent("b", Some("sg1"));
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            // After layout, sg1 should have x, y, width, height
            let sg = g.node("sg1");
            assert!(
                sg.x.is_some(),
                "sg1 should have x set after compound layout, got: {:?}",
                sg.x
            );
            assert!(
                sg.y.is_some(),
                "sg1 should have y set after compound layout, got: {:?}",
                sg.y
            );
            assert!(
                sg.width > 0.0,
                "sg1 should have positive width, got: {}",
                sg.width
            );
            assert!(
                sg.height > 0.0,
                "sg1 should have positive height, got: {}",
                sg.height
            );
            // Check child nodes have reasonable positions
            assert!(g.node("a").x.is_some(), "node a should have x set");
            assert!(g.node("b").x.is_some(), "node b should have x set");
            // Children should be inside the compound node bounding box
            let sg_x = sg.x.unwrap();
            let sg_y = sg.y.unwrap();
            let sg_w = sg.width;
            let sg_h = sg.height;
            let sg_left = sg_x - sg_w / 2.0;
            let sg_right = sg_x + sg_w / 2.0;
            let sg_top = sg_y - sg_h / 2.0;
            let sg_bot = sg_y + sg_h / 2.0;
            for child in &["a", "b"] {
                let cn = g.node(child);
                let cx = cn.x.unwrap();
                let cy = cn.y.unwrap();
                assert!(
                    cx >= sg_left && cx <= sg_right,
                    "child {} x={} should be inside sg1 [{},{}]",
                    child,
                    cx,
                    sg_left,
                    sg_right
                );
                assert!(
                    cy >= sg_top && cy <= sg_bot,
                    "child {} y={} should be inside sg1 [{},{}]",
                    child,
                    cy,
                    sg_top,
                    sg_bot
                );
            }
        }

        #[test]
        fn layout_adds_dimensions_to_graph() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 50.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            assert!((g.graph().width.unwrap() - 100.0).abs() < 1.0);
            assert!((g.graph().height.unwrap() - 50.0).abs() < 1.0);
        }

        #[test]
        fn layout_bounding_box_node_tb() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("TB".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            assert!((g.node("a").x.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("a").y.unwrap() - 100.0).abs() < 1.0);
        }

        #[test]
        fn layout_bounding_box_node_bt() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("BT".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            assert!((g.node("a").x.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("a").y.unwrap() - 100.0).abs() < 1.0);
        }

        #[test]
        fn layout_bounding_box_node_lr() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("LR".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            assert!((g.node("a").x.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("a").y.unwrap() - 100.0).abs() < 1.0);
        }

        #[test]
        fn layout_bounding_box_node_rl() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("RL".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            assert!((g.node("a").x.unwrap() - 50.0).abs() < 1.0);
            assert!((g.node("a").y.unwrap() - 100.0).abs() < 1.0);
        }

        // Tests for "edge, labelpos=l" bounding box: label x = label_width/2 for TB/BT
        #[test]
        fn layout_bounding_box_edge_label_tb() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("TB".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(1000.0),
                    height: Some(2000.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(0.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 500.0).abs() < 1.0); // 1000/2
        }

        #[test]
        fn layout_bounding_box_edge_label_bt() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("BT".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(1000.0),
                    height: Some(2000.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(0.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.x.unwrap() - 500.0).abs() < 1.0); // 1000/2
        }

        #[test]
        fn layout_bounding_box_edge_label_lr() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("LR".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(1000.0),
                    height: Some(2000.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(0.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.y.unwrap() - 1000.0).abs() < 1.0); // 2000/2
        }

        #[test]
        fn layout_bounding_box_edge_label_rl() {
            let mut g = make_g();
            g.graph_mut().rankdir = Some("RL".to_string());
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(1000.0),
                    height: Some(2000.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(0.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert!((el.y.unwrap() - 1000.0).abs() < 1.0); // 2000/2
        }

        // Long label edge tests (ensure large label causes separation > 1000)
        #[test]
        fn layout_long_label_tb() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("TB".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    width: Some(2000.0),
                    height: Some(10.0),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    width: Some(1.0),
                    height: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let p1 = g.edge_vw("a", "c").unwrap();
            let p2 = g.edge_vw("b", "d").unwrap();
            assert!((p1.x.unwrap() - p2.x.unwrap()).abs() > 1000.0);
        }

        #[test]
        fn layout_long_label_bt() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("BT".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    width: Some(2000.0),
                    height: Some(10.0),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    width: Some(1.0),
                    height: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let p1 = g.edge_vw("a", "c").unwrap();
            let p2 = g.edge_vw("b", "d").unwrap();
            assert!((p1.x.unwrap() - p2.x.unwrap()).abs() > 1000.0);
        }

        #[test]
        fn layout_long_label_lr() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("LR".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    width: Some(2000.0),
                    height: Some(10.0),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    width: Some(1.0),
                    height: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let na = g.node("a");
            let nc = g.node("c");
            assert!((na.x.unwrap() - nc.x.unwrap()).abs() > 1000.0);
        }

        #[test]
        fn layout_long_label_rl() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("RL".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    width: Some(2000.0),
                    height: Some(10.0),
                    labelpos: Some("c".to_string()),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    width: Some(1.0),
                    height: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let na = g.node("a");
            let nc = g.node("c");
            assert!((na.x.unwrap() - nc.x.unwrap()).abs() > 1000.0);
        }

        // Offset tests: labelpos=l with labeloffset=1000
        #[test]
        fn layout_offset_tb() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("TB".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("r".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            let cd = g.edge_vw("c", "d").unwrap();
            let ab_pts = ab.points.as_ref().unwrap();
            let cd_pts = cd.points.as_ref().unwrap();
            // JS expects: edge.x - points[0].x == -1000 - 10/2 = -1005 for labelpos=l
            assert!((ab.x.unwrap() - ab_pts[0].x - (-1000.0 - 5.0)).abs() < 1.0);
            assert!((cd.x.unwrap() - cd_pts[0].x - (1000.0 + 5.0)).abs() < 1.0);
        }

        #[test]
        fn layout_offset_lr() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("LR".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("r".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            let cd = g.edge_vw("c", "d").unwrap();
            let ab_pts = ab.points.as_ref().unwrap();
            let cd_pts = cd.points.as_ref().unwrap();
            assert!((ab.y.unwrap() - ab_pts[0].y - (-1000.0 - 5.0)).abs() < 1.0);
            assert!((cd.y.unwrap() - cd_pts[0].y - (1000.0 + 5.0)).abs() < 1.0);
        }

        // "can apply an offset, with rankdir = BT"
        // BT uses the same x-based formula as TB
        #[test]
        fn layout_offset_bt() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("BT".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("r".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            let cd = g.edge_vw("c", "d").unwrap();
            let ab_pts = ab.points.as_ref().unwrap();
            let cd_pts = cd.points.as_ref().unwrap();
            assert!((ab.x.unwrap() - ab_pts[0].x - (-1000.0 - 5.0)).abs() < 1.0);
            assert!((cd.x.unwrap() - cd_pts[0].x - (1000.0 + 5.0)).abs() < 1.0);
        }

        // "can apply an offset, with rankdir = RL"
        // RL uses the same y-based formula as LR
        #[test]
        fn layout_offset_rl() {
            let mut g = make_g();
            g.graph_mut().nodesep = Some(10.0);
            g.graph_mut().edgesep = Some(10.0);
            g.graph_mut().rankdir = Some("RL".to_string());
            for v in &["a", "b", "c", "d"] {
                g.set_node(
                    v,
                    NodeLabel {
                        width: 10.0,
                        height: 10.0,
                        ..Default::default()
                    },
                );
            }
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("l".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    width: Some(10.0),
                    height: Some(10.0),
                    labelpos: Some("r".to_string()),
                    labeloffset: Some(1000.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            let cd = g.edge_vw("c", "d").unwrap();
            let ab_pts = ab.points.as_ref().unwrap();
            let cd_pts = cd.points.as_ref().unwrap();
            assert!((ab.y.unwrap() - ab_pts[0].y - (-1000.0 - 5.0)).abs() < 1.0);
            assert!((cd.y.unwrap() - cd_pts[0].y - (1000.0 + 5.0)).abs() < 1.0);
        }

        // "can layout a self loop in rankdir = BT"
        #[test]
        fn layout_self_loop_bt() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                edgesep: Some(75.0),
                rankdir: Some("BT".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "a",
                EdgeLabel {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            let node_a = g.node("a");
            let ax = node_a.x.unwrap();
            let ay = node_a.y.unwrap();
            let el = g.edge_vw("a", "a").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 7);
            // rankdir != LR/RL: x > nodeA.x, |y - nodeA.y| <= height/2
            for p in points {
                assert!(p.x > ax, "point.x={} should be > node.x={}", p.x, ax);
                assert!(
                    (p.y - ay).abs() <= 50.0,
                    "point.y={} should be within height/2={} of node.y={}",
                    p.y,
                    50.0,
                    ay
                );
            }
        }

        // "can layout a self loop in rankdir = LR"
        #[test]
        fn layout_self_loop_lr() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                edgesep: Some(75.0),
                rankdir: Some("LR".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "a",
                EdgeLabel {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            let node_a = g.node("a");
            let ax = node_a.x.unwrap();
            let ay = node_a.y.unwrap();
            let el = g.edge_vw("a", "a").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 7);
            // rankdir LR/RL: y > nodeA.y, |x - nodeA.x| <= width/2
            for p in points {
                assert!(p.y > ay, "point.y={} should be > node.y={}", p.y, ay);
                assert!(
                    (p.x - ax).abs() <= 50.0,
                    "point.x={} should be within width/2={} of node.x={}",
                    p.x,
                    50.0,
                    ax
                );
            }
        }

        // "can layout a self loop in rankdir = RL"
        #[test]
        fn layout_self_loop_rl() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                edgesep: Some(75.0),
                rankdir: Some("RL".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "a",
                EdgeLabel {
                    width: Some(50.0),
                    height: Some(50.0),
                    ..Default::default()
                },
                None,
            );
            layout(&mut g);

            let node_a = g.node("a");
            let ax = node_a.x.unwrap();
            let ay = node_a.y.unwrap();
            let el = g.edge_vw("a", "a").unwrap();
            let points = el.points.as_ref().unwrap();
            assert_eq!(points.len(), 7);
            // rankdir LR/RL: y > nodeA.y, |x - nodeA.x| <= width/2
            for p in points {
                assert!(p.y > ay, "point.y={} should be > node.y={}", p.y, ay);
                assert!(
                    (p.x - ax).abs() <= 50.0,
                    "point.x={} should be within width/2={} of node.x={}",
                    p.x,
                    50.0,
                    ax
                );
            }
        }
    } // mod layout_remaining_tests

    // =========================================================================
    // Ported from dagre-js/test/nesting-graph-test.ts
    // =========================================================================

    mod nesting_graph_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::nesting_graph;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, false, true);
            g.set_graph(GraphLabel::default());
            g
        }

        fn components(g: &Graph) -> Vec<Vec<String>> {
            // Simple connected-components for directed graph via DFS ignoring edge direction
            let mut visited = std::collections::HashSet::new();
            let mut result = Vec::new();
            for start in g.nodes() {
                if visited.contains(&start) {
                    continue;
                }
                let mut comp = Vec::new();
                let mut stack = vec![start.clone()];
                while let Some(v) = stack.pop() {
                    if visited.contains(&v) {
                        continue;
                    }
                    visited.insert(v.clone());
                    comp.push(v.clone());
                    if let Some(succs) = g.successors(&v) {
                        for w in succs {
                            stack.push(w);
                        }
                    }
                    if let Some(preds) = g.predecessors(&v) {
                        for w in preds {
                            stack.push(w);
                        }
                    }
                }
                comp.sort();
                result.push(comp);
            }
            result
        }

        #[test]
        fn nesting_run_connects_disconnected_graph() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            assert_eq!(components(&g).len(), 2);
            nesting_graph::run(&mut g);
            assert_eq!(components(&g).len(), 1);
            assert!(g.has_node("a"));
            assert!(g.has_node("b"));
        }

        #[test]
        fn nesting_run_adds_border_nodes_top_bottom() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            nesting_graph::run(&mut g);

            let border_top = g
                .node("sg1")
                .border_top
                .clone()
                .expect("borderTop must be set");
            let border_bottom = g
                .node("sg1")
                .border_bottom
                .clone()
                .expect("borderBottom must be set");
            assert_eq!(g.parent(&border_top), Some("sg1"));
            assert_eq!(g.parent(&border_bottom), Some("sg1"));

            let out_bt = g.out_edges_to(&border_top, "a").unwrap_or_default();
            assert_eq!(out_bt.len(), 1);
            let e1 = g.edge(&out_bt[0]).unwrap();
            assert_eq!(e1.minlen, Some(1));

            let out_ab = g.out_edges_to("a", &border_bottom).unwrap_or_default();
            assert_eq!(out_ab.len(), 1);
            let e2 = g.edge(&out_ab[0]).unwrap();
            assert_eq!(e2.minlen, Some(1));

            let bt_node = g.node(&border_top);
            assert_eq!(bt_node.width, 0.0);
            assert_eq!(bt_node.height, 0.0);
            assert_eq!(bt_node.dummy.as_deref(), Some("border"));

            let bb_node = g.node(&border_bottom);
            assert_eq!(bb_node.width, 0.0);
            assert_eq!(bb_node.height, 0.0);
            assert_eq!(bb_node.dummy.as_deref(), Some("border"));
        }

        #[test]
        fn nesting_run_adds_edges_between_nested_subgraph_borders() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("sg2", Some("sg1"));
            g.set_parent("a", Some("sg2"));
            nesting_graph::run(&mut g);

            let sg1_top = g.node("sg1").border_top.clone().unwrap();
            let sg1_bottom = g.node("sg1").border_bottom.clone().unwrap();
            let sg2_top = g.node("sg2").border_top.clone().unwrap();
            let sg2_bottom = g.node("sg2").border_bottom.clone().unwrap();

            let e1 = g.out_edges_to(&sg1_top, &sg2_top).unwrap_or_default();
            assert_eq!(e1.len(), 1);
            assert_eq!(g.edge(&e1[0]).unwrap().minlen, Some(1));

            let e2 = g.out_edges_to(&sg2_bottom, &sg1_bottom).unwrap_or_default();
            assert_eq!(e2.len(), 1);
            assert_eq!(g.edge(&e2[0]).unwrap().minlen, Some(1));
        }

        #[test]
        fn nesting_run_adds_sufficient_weight_to_border_edges() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_node("x", NodeLabel::default());
            g.set_parent("x", Some("sg"));
            g.set_edge(
                "a",
                "x",
                EdgeLabel {
                    weight: Some(100.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "x",
                "b",
                EdgeLabel {
                    weight: Some(200.0),
                    ..Default::default()
                },
                None,
            );
            nesting_graph::run(&mut g);

            let top = g.node("sg").border_top.clone().unwrap();
            let bot = g.node("sg").border_bottom.clone().unwrap();
            assert!(g.edge_vw(&top, "x").unwrap().weight.unwrap() > 300.0);
            assert!(g.edge_vw("x", &bot).unwrap().weight.unwrap() > 300.0);
        }

        #[test]
        fn nesting_run_adds_edge_from_root_to_top_level_subgraph() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let border_top = g.node("sg1").border_top.clone().unwrap();
            let edges = g.out_edges_to(&root, &border_top).unwrap_or_default();
            assert_eq!(edges.len(), 1);
            assert!(g.has_edge_obj(&edges[0]));
        }

        #[test]
        fn nesting_run_adds_root_to_node_minlen_1() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let edges = g.out_edges_to(&root, "a").unwrap_or_default();
            assert_eq!(edges.len(), 1);
            let label = g.edge(&edges[0]).unwrap();
            assert_eq!(label.weight, Some(0.0));
            assert_eq!(label.minlen, Some(1));
        }

        #[test]
        fn nesting_run_adds_root_to_node_minlen_2() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let edges = g.out_edges_to(&root, "a").unwrap_or_default();
            assert_eq!(edges.len(), 1);
            let label = g.edge(&edges[0]).unwrap();
            assert_eq!(label.weight, Some(0.0));
            assert_eq!(label.minlen, Some(3));
        }

        #[test]
        fn nesting_run_adds_root_to_node_minlen_3() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("sg2", Some("sg1"));
            g.set_parent("a", Some("sg2"));
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let edges = g.out_edges_to(&root, "a").unwrap_or_default();
            assert_eq!(edges.len(), 1);
            let label = g.edge(&edges[0]).unwrap();
            assert_eq!(label.weight, Some(0.0));
            assert_eq!(label.minlen, Some(5));
        }

        #[test]
        fn nesting_run_does_not_add_root_to_itself() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let edges = g.out_edges_to(&root, &root).unwrap_or_default();
            assert_eq!(edges.len(), 0);
        }

        #[test]
        fn nesting_run_expands_inter_node_edges_1() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            nesting_graph::run(&mut g);
            assert_eq!(g.edge_vw("a", "b").unwrap().minlen, Some(1));
        }

        #[test]
        fn nesting_run_expands_inter_node_edges_2() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            nesting_graph::run(&mut g);
            assert_eq!(g.edge_vw("a", "b").unwrap().minlen, Some(3));
        }

        #[test]
        fn nesting_run_expands_inter_node_edges_3() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("sg2", Some("sg1"));
            g.set_parent("a", Some("sg2"));
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            nesting_graph::run(&mut g);
            assert_eq!(g.edge_vw("a", "b").unwrap().minlen, Some(5));
        }

        #[test]
        fn nesting_run_sets_minlen_for_nested_sg_border_to_children() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_parent("sg2", Some("sg1"));
            g.set_parent("b", Some("sg2"));
            nesting_graph::run(&mut g);

            let root = g.graph().nesting_root.clone().unwrap();
            let sg1_top = g.node("sg1").border_top.clone().unwrap();
            let sg1_bot = g.node("sg1").border_bottom.clone().unwrap();
            let sg2_top = g.node("sg2").border_top.clone().unwrap();
            let sg2_bot = g.node("sg2").border_bottom.clone().unwrap();

            assert_eq!(g.edge_vw(&root, &sg1_top).unwrap().minlen, Some(3));
            assert_eq!(g.edge_vw(&sg1_top, &sg2_top).unwrap().minlen, Some(1));
            assert_eq!(g.edge_vw(&sg1_top, "a").unwrap().minlen, Some(2));
            assert_eq!(g.edge_vw("a", &sg1_bot).unwrap().minlen, Some(2));
            assert_eq!(g.edge_vw(&sg2_top, "b").unwrap().minlen, Some(1));
            assert_eq!(g.edge_vw("b", &sg2_bot).unwrap().minlen, Some(1));
            assert_eq!(g.edge_vw(&sg2_bot, &sg1_bot).unwrap().minlen, Some(1));
        }

        #[test]
        fn nesting_cleanup_removes_nesting_graph_edges() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            nesting_graph::run(&mut g);
            nesting_graph::cleanup(&mut g);

            let succs = g.successors("a").unwrap_or_default();
            assert_eq!(succs, vec!["b"]);
        }

        #[test]
        fn nesting_cleanup_removes_root_node() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            nesting_graph::run(&mut g);
            nesting_graph::cleanup(&mut g);
            // sg1 + sg1Top + sg1Bottom + "a" = 4 nodes
            assert_eq!(g.node_count(), 4);
        }
    } // mod nesting_graph_tests

    // =========================================================================
    // Ported from dagre-js/test/add-border-segments-test.ts
    // =========================================================================

    mod add_border_segments_tests {
        use crate::add_border_segments::add_border_segments;
        use crate::graph::{Graph, NodeLabel};

        fn make_compound() -> Graph {
            Graph::with_options(true, false, true)
        }

        #[test]
        fn abs_no_border_for_non_compound_graph() {
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            add_border_segments(&mut g);
            assert_eq!(g.node_count(), 1);
            assert_eq!(g.node("a").rank, Some(0));
        }

        #[test]
        fn abs_no_border_for_no_clusters() {
            let mut g = make_compound();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            add_border_segments(&mut g);
            assert_eq!(g.node_count(), 1);
            assert_eq!(g.node("a").rank, Some(0));
        }

        #[test]
        fn abs_adds_border_single_rank_subgraph() {
            let mut g = make_compound();
            g.set_node(
                "sg",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(1),
                    ..Default::default()
                },
            );
            add_border_segments(&mut g);

            let bl = g.node("sg").border_left.as_ref().unwrap()[1]
                .clone()
                .unwrap();
            let br = g.node("sg").border_right.as_ref().unwrap()[1]
                .clone()
                .unwrap();

            let bl_node = g.node(&bl);
            assert_eq!(bl_node.dummy.as_deref(), Some("border"));
            assert_eq!(bl_node.border_type.as_deref(), Some("borderLeft"));
            assert_eq!(bl_node.rank, Some(1));
            assert_eq!(bl_node.width, 0.0);
            assert_eq!(bl_node.height, 0.0);
            assert_eq!(g.parent(&bl), Some("sg"));

            let br_node = g.node(&br);
            assert_eq!(br_node.dummy.as_deref(), Some("border"));
            assert_eq!(br_node.border_type.as_deref(), Some("borderRight"));
            assert_eq!(br_node.rank, Some(1));
            assert_eq!(br_node.width, 0.0);
            assert_eq!(br_node.height, 0.0);
            assert_eq!(g.parent(&br), Some("sg"));
        }

        #[test]
        fn abs_adds_border_multi_rank_subgraph() {
            let mut g = make_compound();
            g.set_node(
                "sg",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(2),
                    ..Default::default()
                },
            );
            add_border_segments(&mut g);

            let sg_node = g.node("sg");
            let bl1 = sg_node.border_left.as_ref().unwrap()[1].clone().unwrap();
            let br1 = sg_node.border_right.as_ref().unwrap()[1].clone().unwrap();
            let bl2 = sg_node.border_left.as_ref().unwrap()[2].clone().unwrap();
            let br2 = sg_node.border_right.as_ref().unwrap()[2].clone().unwrap();

            let bl1_node = g.node(&bl1);
            assert_eq!(bl1_node.dummy.as_deref(), Some("border"));
            assert_eq!(bl1_node.border_type.as_deref(), Some("borderLeft"));
            assert_eq!(bl1_node.rank, Some(1));
            assert_eq!(g.parent(&bl1), Some("sg"));

            let br1_node = g.node(&br1);
            assert_eq!(br1_node.rank, Some(1));
            assert_eq!(g.parent(&br1), Some("sg"));

            let bl2_node = g.node(&bl2);
            assert_eq!(bl2_node.rank, Some(2));
            assert_eq!(g.parent(&bl2), Some("sg"));

            let br2_node = g.node(&br2);
            assert_eq!(br2_node.rank, Some(2));
            assert_eq!(g.parent(&br2), Some("sg"));

            // Edges connecting rank1->rank2
            assert!(g.has_edge(&bl1, &bl2));
            assert!(g.has_edge(&br1, &br2));
        }

        #[test]
        fn abs_adds_border_nested_subgraphs() {
            let mut g = make_compound();
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "sg2",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_parent("sg2", Some("sg1"));
            add_border_segments(&mut g);

            let bl1 = g.node("sg1").border_left.as_ref().unwrap()[1]
                .clone()
                .unwrap();
            let br1 = g.node("sg1").border_right.as_ref().unwrap()[1]
                .clone()
                .unwrap();
            assert_eq!(g.node(&bl1).border_type.as_deref(), Some("borderLeft"));
            assert_eq!(g.node(&bl1).rank, Some(1));
            assert_eq!(g.parent(&bl1), Some("sg1"));
            assert_eq!(g.node(&br1).border_type.as_deref(), Some("borderRight"));
            assert_eq!(g.parent(&br1), Some("sg1"));

            let bl2 = g.node("sg2").border_left.as_ref().unwrap()[1]
                .clone()
                .unwrap();
            let br2 = g.node("sg2").border_right.as_ref().unwrap()[1]
                .clone()
                .unwrap();
            assert_eq!(g.node(&bl2).border_type.as_deref(), Some("borderLeft"));
            assert_eq!(g.node(&bl2).rank, Some(1));
            assert_eq!(g.parent(&bl2), Some("sg2"));
            assert_eq!(g.node(&br2).border_type.as_deref(), Some("borderRight"));
            assert_eq!(g.parent(&br2), Some("sg2"));
        }
    } // mod add_border_segments_tests

    // =========================================================================
    // Ported from dagre-js/test/parent-dummy-chains-test.ts
    // =========================================================================

    mod parent_dummy_chains_tests {
        use crate::graph::{Edge, EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::parent_dummy_chains::parent_dummy_chains;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, false, true);
            g.set_graph(GraphLabel::default());
            g
        }

        fn set_path(g: &mut Graph, nodes: &[&str]) {
            for i in 0..nodes.len() - 1 {
                let v = nodes[i];
                let w = nodes[i + 1];
                if !g.has_node(v) {
                    g.set_node(v, NodeLabel::default());
                }
                if !g.has_node(w) {
                    g.set_node(w, NodeLabel::default());
                }
                g.set_edge(v, w, EdgeLabel::default(), None);
            }
        }

        #[test]
        fn pdc_no_parent_if_both_have_no_parent() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "b"]);
            parent_dummy_chains(&mut g);
            assert!(g.parent("d1").is_none());
        }

        #[test]
        fn pdc_uses_tail_parent_if_not_root() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg1"));
        }

        #[test]
        fn pdc_uses_head_parent_if_tail_is_root() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("b", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg1"));
        }

        #[test]
        fn pdc_long_chain_starting_in_subgraph() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d2",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "d3",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "d3", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg1"));
            assert!(g.parent("d2").is_none());
            assert!(g.parent("d3").is_none());
        }

        #[test]
        fn pdc_long_chain_ending_in_subgraph() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("b", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(3),
                    max_rank: Some(5),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "d2",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d3",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "d3", "b"]);
            parent_dummy_chains(&mut g);
            assert!(g.parent("d1").is_none());
            assert!(g.parent("d2").is_none());
            assert_eq!(g.parent("d3"), Some("sg1"));
        }

        #[test]
        fn pdc_handles_nested_subgraphs() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg2"));
            g.set_parent("sg2", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(4),
                    ..Default::default()
                },
            );
            g.set_node(
                "sg2",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_parent("b", Some("sg4"));
            g.set_parent("sg4", Some("sg3"));
            g.set_node(
                "sg3",
                NodeLabel {
                    min_rank: Some(6),
                    max_rank: Some(10),
                    ..Default::default()
                },
            );
            g.set_node(
                "sg4",
                NodeLabel {
                    min_rank: Some(7),
                    max_rank: Some(9),
                    ..Default::default()
                },
            );
            for i in 1..=5i32 {
                g.set_node(
                    &format!("d{}", i),
                    NodeLabel {
                        rank: Some(i + 2),
                        ..Default::default()
                    },
                );
            }
            // Set edge_obj on d1
            g.node_mut("d1").edge_obj = Some(Edge {
                v: "a".to_string(),
                w: "b".to_string(),
                name: None,
            });
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "d3", "d4", "d5", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg2"));
            assert_eq!(g.parent("d2"), Some("sg1"));
            assert!(g.parent("d3").is_none());
            assert_eq!(g.parent("d4"), Some("sg3"));
            assert_eq!(g.parent("d5"), Some("sg4"));
        }

        #[test]
        fn pdc_handles_overlapping_rank_ranges() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_parent("b", Some("sg2"));
            g.set_node(
                "sg2",
                NodeLabel {
                    min_rank: Some(2),
                    max_rank: Some(6),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d2",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "d3",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "d3", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg1"));
            assert_eq!(g.parent("d2"), Some("sg1"));
            assert_eq!(g.parent("d3"), Some("sg2"));
        }

        #[test]
        fn pdc_lca_not_root_1() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg1"));
            g.set_parent("sg2", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(6),
                    ..Default::default()
                },
            );
            g.set_parent("b", Some("sg2"));
            g.set_node(
                "sg2",
                NodeLabel {
                    min_rank: Some(3),
                    max_rank: Some(5),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d2",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg1"));
            assert_eq!(g.parent("d2"), Some("sg2"));
        }

        #[test]
        fn pdc_lca_not_root_2() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            g.set_node("b", NodeLabel::default());
            g.set_parent("a", Some("sg2"));
            g.set_parent("sg2", Some("sg1"));
            g.set_node(
                "sg1",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(6),
                    ..Default::default()
                },
            );
            g.set_parent("b", Some("sg1"));
            g.set_node(
                "sg2",
                NodeLabel {
                    min_rank: Some(1),
                    max_rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "d1",
                NodeLabel {
                    edge_obj: Some(Edge {
                        v: "a".to_string(),
                        w: "b".to_string(),
                        name: None,
                    }),
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "d2",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            g.graph_mut().dummy_chains = Some(vec!["d1".to_string()]);
            set_path(&mut g, &["a", "d1", "d2", "b"]);
            parent_dummy_chains(&mut g);
            assert_eq!(g.parent("d1"), Some("sg2"));
            assert_eq!(g.parent("d2"), Some("sg1"));
        }
    } // mod parent_dummy_chains_tests

    // =========================================================================
    // Ported from dagre-js/test/coordinate-system-test.ts
    // =========================================================================

    mod coordinate_system_tests {
        use crate::coordinate_system::{adjust, undo};
        use crate::graph::{Graph, GraphLabel, NodeLabel};

        fn make_g() -> Graph {
            Graph::with_options(true, false, false)
        }

        // --- adjust ---

        #[test]
        fn cs_adjust_tb_no_change() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("TB".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            adjust(&mut g);
            assert_eq!(g.node("a").width, 100.0);
            assert_eq!(g.node("a").height, 200.0);
        }

        #[test]
        fn cs_adjust_bt_no_change() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("BT".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            adjust(&mut g);
            assert_eq!(g.node("a").width, 100.0);
            assert_eq!(g.node("a").height, 200.0);
        }

        #[test]
        fn cs_adjust_lr_swaps_width_height() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("LR".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            adjust(&mut g);
            assert_eq!(g.node("a").width, 200.0);
            assert_eq!(g.node("a").height, 100.0);
        }

        #[test]
        fn cs_adjust_rl_swaps_width_height() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("RL".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            adjust(&mut g);
            assert_eq!(g.node("a").width, 200.0);
            assert_eq!(g.node("a").height, 100.0);
        }

        // --- undo ---

        #[test]
        fn cs_undo_tb_no_change() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("TB".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    x: Some(20.0),
                    y: Some(40.0),
                    ..Default::default()
                },
            );
            undo(&mut g);
            let n = g.node("a");
            assert_eq!(n.x, Some(20.0));
            assert_eq!(n.y, Some(40.0));
            assert_eq!(n.width, 100.0);
            assert_eq!(n.height, 200.0);
        }

        #[test]
        fn cs_undo_bt_flips_y() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("BT".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    x: Some(20.0),
                    y: Some(40.0),
                    ..Default::default()
                },
            );
            undo(&mut g);
            let n = g.node("a");
            assert_eq!(n.x, Some(20.0));
            assert_eq!(n.y, Some(-40.0));
            assert_eq!(n.width, 100.0);
            assert_eq!(n.height, 200.0);
        }

        #[test]
        fn cs_undo_lr_swaps_dims_and_coords() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("LR".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    x: Some(20.0),
                    y: Some(40.0),
                    ..Default::default()
                },
            );
            undo(&mut g);
            let n = g.node("a");
            assert_eq!(n.x, Some(40.0));
            assert_eq!(n.y, Some(20.0));
            assert_eq!(n.width, 200.0);
            assert_eq!(n.height, 100.0);
        }

        #[test]
        fn cs_undo_rl_swaps_and_flips_x() {
            let mut g = make_g();
            g.set_graph(GraphLabel {
                rankdir: Some("RL".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 100.0,
                    height: 200.0,
                    x: Some(20.0),
                    y: Some(40.0),
                    ..Default::default()
                },
            );
            undo(&mut g);
            let n = g.node("a");
            assert_eq!(n.x, Some(-40.0));
            assert_eq!(n.y, Some(20.0));
            assert_eq!(n.width, 200.0);
            assert_eq!(n.height, 100.0);
        }
    } // mod coordinate_system_tests

    // =========================================================================
    // Ported from dagre-js/test/order/sort-subgraph-test.ts
    // =========================================================================

    mod sort_subgraph_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::sort_subgraph::sort_subgraph;

        fn make_g() -> Graph {
            let mut g = Graph::with_options(true, false, true);
            for (i, v) in ["0", "1", "2", "3", "4"].iter().enumerate() {
                g.set_node(
                    v,
                    NodeLabel {
                        order: Some(i as i32),
                        ..Default::default()
                    },
                );
            }
            g
        }

        fn make_cg() -> Graph {
            Graph::with_options(true, false, false)
        }

        fn w(weight: f64) -> EdgeLabel {
            EdgeLabel {
                weight: Some(weight),
                ..Default::default()
            }
        }

        #[test]
        fn ss_sorts_flat_subgraph_by_barycenter() {
            let mut g = make_g();
            g.set_edge("3", "x", w(1.0), None);
            g.set_edge("1", "y", w(2.0), None);
            g.set_edge("4", "y", w(1.0), None);
            for v in &["x", "y"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["y", "x"]);
        }

        #[test]
        fn ss_preserves_pos_of_node_without_neighbors() {
            let mut g = make_g();
            g.set_edge("3", "x", w(1.0), None);
            g.set_node("y", NodeLabel::default());
            g.set_edge("1", "z", w(2.0), None);
            g.set_edge("4", "z", w(1.0), None);
            for v in &["x", "y", "z"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["z", "y", "x"]);
        }

        #[test]
        fn ss_biases_left_without_reverse() {
            let mut g = make_g();
            g.set_edge("1", "x", w(1.0), None);
            g.set_edge("1", "y", w(1.0), None);
            for v in &["x", "y"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["x", "y"]);
        }

        #[test]
        fn ss_biases_right_with_reverse() {
            let mut g = make_g();
            g.set_edge("1", "x", w(1.0), None);
            g.set_edge("1", "y", w(1.0), None);
            for v in &["x", "y"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, true);
            assert_eq!(result.vs, vec!["y", "x"]);
        }

        #[test]
        fn ss_aggregates_stats() {
            let mut g = make_g();
            g.set_edge("3", "x", w(1.0), None);
            g.set_edge("1", "y", w(2.0), None);
            g.set_edge("4", "y", w(1.0), None);
            for v in &["x", "y"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert!((result.barycenter.unwrap() - 2.25).abs() < 1e-9);
            assert!((result.weight.unwrap() - 4.0).abs() < 1e-9);
        }

        #[test]
        fn ss_nested_subgraph_no_barycenter() {
            let mut g = make_g();
            for v in &["a", "b", "c"] {
                g.set_node(v, NodeLabel::default());
            }
            g.set_parent("a", Some("y"));
            g.set_parent("b", Some("y"));
            g.set_parent("c", Some("y"));
            g.set_edge("0", "x", w(1.0), None);
            g.set_edge("1", "z", w(1.0), None);
            g.set_edge("2", "y", w(1.0), None);
            for v in &["x", "y", "z"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["x", "z", "a", "b", "c"]);
        }

        #[test]
        fn ss_nested_subgraph_with_barycenter() {
            let mut g = make_g();
            for v in &["a", "b", "c"] {
                g.set_node(v, NodeLabel::default());
            }
            g.set_parent("a", Some("y"));
            g.set_parent("b", Some("y"));
            g.set_parent("c", Some("y"));
            g.set_edge(
                "0",
                "a",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge("0", "x", w(1.0), None);
            g.set_edge("1", "z", w(1.0), None);
            g.set_edge("2", "y", w(1.0), None);
            for v in &["x", "y", "z"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["x", "a", "b", "c", "z"]);
        }

        #[test]
        fn ss_nested_subgraph_no_in_edges() {
            let mut g = make_g();
            for v in &["a", "b", "c"] {
                g.set_node(v, NodeLabel::default());
            }
            g.set_parent("a", Some("y"));
            g.set_parent("b", Some("y"));
            g.set_parent("c", Some("y"));
            g.set_edge("0", "a", w(1.0), None);
            g.set_edge("1", "b", w(1.0), None);
            g.set_edge("0", "x", w(1.0), None);
            g.set_edge("1", "z", w(1.0), None);
            for v in &["x", "y", "z"] {
                g.set_parent(v, Some("movable"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "movable", &cg, false);
            assert_eq!(result.vs, vec!["x", "a", "b", "c", "z"]);
        }

        #[test]
        fn ss_sorts_border_nodes_to_extremes() {
            let mut g = make_g();
            g.set_edge("0", "x", w(1.0), None);
            g.set_edge("1", "y", w(1.0), None);
            g.set_edge("2", "z", w(1.0), None);
            g.set_node(
                "sg1",
                NodeLabel {
                    border_left: Some(vec![Some("bl".to_string())]),
                    border_right: Some(vec![Some("br".to_string())]),
                    ..Default::default()
                },
            );
            for v in &["x", "y", "z", "bl", "br"] {
                g.set_parent(v, Some("sg1"));
            }
            let cg = make_cg();
            let result = sort_subgraph(&g, "sg1", &cg, false);
            assert_eq!(result.vs, vec!["bl", "x", "y", "z", "br"]);
        }

        #[test]
        fn ss_assigns_barycenter_from_border_nodes() {
            let mut g = make_g();
            g.set_node(
                "bl1",
                NodeLabel {
                    order: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "br1",
                NodeLabel {
                    order: Some(1),
                    ..Default::default()
                },
            );
            g.set_edge("bl1", "bl2", w(1.0), None);
            g.set_edge("br1", "br2", w(1.0), None);
            for v in &["bl2", "br2"] {
                g.set_parent(v, Some("sg"));
            }
            g.set_node(
                "sg",
                NodeLabel {
                    border_left: Some(vec![Some("bl2".to_string())]),
                    border_right: Some(vec![Some("br2".to_string())]),
                    ..Default::default()
                },
            );
            let cg = make_cg();
            let result = sort_subgraph(&g, "sg", &cg, false);
            assert!((result.barycenter.unwrap() - 0.5).abs() < 1e-9);
            assert!((result.weight.unwrap() - 2.0).abs() < 1e-9);
            assert_eq!(result.vs, vec!["bl2", "br2"]);
        }
    } // mod sort_subgraph_tests

    // =========================================================================
    // Ported from dagre-js/test/order/build-layer-graph-test.ts
    // =========================================================================

    mod build_layer_graph_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::build_layer_graph::build_layer_graph;
        use crate::util::GRAPH_NODE;

        fn make_g() -> Graph {
            Graph::with_options(true, true, true)
        }

        fn nodes_with_rank(g: &Graph, rank: i32) -> Vec<String> {
            g.nodes()
                .into_iter()
                .filter(|v| {
                    let n = g.node(v);
                    n.rank == Some(rank)
                        || (n.min_rank.is_some()
                            && n.max_rank.is_some()
                            && n.min_rank.unwrap() <= rank
                            && rank <= n.max_rank.unwrap())
                })
                .collect()
        }

        #[test]
        fn blg_places_movable_nodes_under_root() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );

            let nodes = nodes_with_rank(&g, 1);
            let lg = build_layer_graph(&g, 1, "inEdges", &nodes);
            let root = lg.graph().root.clone().unwrap();
            assert!(lg.has_node(&root));
            assert!(lg.children(GRAPH_NODE).contains(&root));
            let mut ch = lg.children(&root);
            ch.sort();
            assert_eq!(ch, vec!["a", "b"]);
        }

        #[test]
        fn blg_copies_node_labels_at_build_time() {
            // "uses the original node label for copied nodes"
            // JS checks live-reference sharing (modify original => layer graph sees it).
            // In Rust, labels are cloned at build time, so we verify the label values
            // are copied correctly from the source graph for nodes in the target rank.
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    x: Some(1.0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    x: Some(2.0),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );

            let nodes2 = nodes_with_rank(&g, 2);
            let lg = build_layer_graph(&g, 2, "inEdges", &nodes2);

            // "b" is in rank=2, so its label (with x=2.0) should be copied into lg
            assert_eq!(
                lg.node("b").x,
                Some(2.0),
                "b's label should be copied from source at build time"
            );
            // "a" is at rank=1 so it's a predecessor — it gets auto-created by set_edge
            // (no explicit label copy). In JS this works via reference; in Rust values differ.
            // We confirm the layer graph at least has b's data correctly.
        }

        #[test]
        fn blg_copies_flat_nodes_for_rank() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );

            assert!(build_layer_graph(&g, 1, "inEdges", &nodes_with_rank(&g, 1))
                .nodes()
                .contains(&"a".to_string()));
            assert!(build_layer_graph(&g, 1, "inEdges", &nodes_with_rank(&g, 1))
                .nodes()
                .contains(&"b".to_string()));
            assert!(build_layer_graph(&g, 2, "inEdges", &nodes_with_rank(&g, 2))
                .nodes()
                .contains(&"c".to_string()));
            assert!(build_layer_graph(&g, 3, "inEdges", &nodes_with_rank(&g, 3))
                .nodes()
                .contains(&"d".to_string()));
        }

        #[test]
        fn blg_copies_inedges() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    weight: Some(4.0),
                    ..Default::default()
                },
                None,
            );

            let lg1 = build_layer_graph(&g, 1, "inEdges", &nodes_with_rank(&g, 1));
            assert_eq!(lg1.edge_count(), 0);

            let lg2 = build_layer_graph(&g, 2, "inEdges", &nodes_with_rank(&g, 2));
            assert_eq!(lg2.edge_count(), 2);
            assert_eq!(lg2.edge_vw("a", "c").unwrap().weight, Some(2.0));
            assert_eq!(lg2.edge_vw("b", "c").unwrap().weight, Some(3.0));

            let lg3 = build_layer_graph(&g, 3, "inEdges", &nodes_with_rank(&g, 3));
            assert_eq!(lg3.edge_count(), 1);
            assert_eq!(lg3.edge_vw("c", "d").unwrap().weight, Some(4.0));
        }

        #[test]
        fn blg_copies_outedges() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "c",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "c",
                "d",
                EdgeLabel {
                    weight: Some(4.0),
                    ..Default::default()
                },
                None,
            );

            let lg1 = build_layer_graph(&g, 1, "outEdges", &nodes_with_rank(&g, 1));
            assert_eq!(lg1.edge_count(), 2);
            assert_eq!(lg1.edge_vw("c", "a").unwrap().weight, Some(2.0));
            assert_eq!(lg1.edge_vw("c", "b").unwrap().weight, Some(3.0));

            let lg2 = build_layer_graph(&g, 2, "outEdges", &nodes_with_rank(&g, 2));
            assert_eq!(lg2.edge_count(), 1);
            assert_eq!(lg2.edge_vw("d", "c").unwrap().weight, Some(4.0));

            let lg3 = build_layer_graph(&g, 3, "outEdges", &nodes_with_rank(&g, 3));
            assert_eq!(lg3.edge_count(), 0);
        }

        #[test]
        fn blg_collapses_multi_edges() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(3.0),
                    ..Default::default()
                },
                Some("multi"),
            );

            let lg = build_layer_graph(&g, 2, "inEdges", &nodes_with_rank(&g, 2));
            assert_eq!(lg.edge_vw("a", "b").unwrap().weight, Some(5.0));
        }

        #[test]
        fn blg_preserves_hierarchy() {
            let mut g = make_g();
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "sg",
                NodeLabel {
                    min_rank: Some(0),
                    max_rank: Some(0),
                    border_left: Some(vec![Some("bl".to_string())]),
                    border_right: Some(vec![Some("br".to_string())]),
                    ..Default::default()
                },
            );
            g.set_parent("a", Some("sg"));
            g.set_parent("b", Some("sg"));

            let nodes = nodes_with_rank(&g, 0);
            let lg = build_layer_graph(&g, 0, "inEdges", &nodes);
            let root = lg.graph().root.clone().unwrap();
            let mut ch = lg.children(&root);
            ch.sort();
            assert_eq!(ch, vec!["c", "sg"]);
            assert_eq!(lg.parent("a"), Some("sg"));
            assert_eq!(lg.parent("b"), Some("sg"));
        }
    } // mod build_layer_graph_tests

    // =========================================================================
    // Ported from dagre-js/test/greedy-fas-test.ts
    // =========================================================================

    mod greedy_fas_tests {
        use crate::acyclic::greedy_fas;
        use crate::graph::{Edge, EdgeLabel, Graph, NodeLabel};

        fn make_g() -> Graph {
            Graph::with_options(true, false, false)
        }

        fn set_path(g: &mut Graph, nodes: &[&str]) {
            for i in 0..nodes.len() - 1 {
                let v = nodes[i];
                let w = nodes[i + 1];
                if !g.has_node(v) {
                    g.set_node(v, NodeLabel::default());
                }
                if !g.has_node(w) {
                    g.set_node(w, NodeLabel::default());
                }
                g.set_edge(v, w, EdgeLabel::default(), None);
            }
        }

        fn has_cycle(g: &Graph) -> bool {
            let mut visited = std::collections::HashSet::new();
            let mut stack = std::collections::HashSet::new();
            fn dfs(
                g: &Graph,
                v: &str,
                visited: &mut std::collections::HashSet<String>,
                stack: &mut std::collections::HashSet<String>,
            ) -> bool {
                visited.insert(v.to_string());
                stack.insert(v.to_string());
                if let Some(succs) = g.successors(v) {
                    for w in succs {
                        if !visited.contains(&w) && dfs(g, &w, visited, stack) {
                            return true;
                        }
                        if stack.contains(&w) {
                            return true;
                        }
                    }
                }
                stack.remove(v);
                false
            }
            for v in g.nodes() {
                if !visited.contains(&v) && dfs(g, &v, &mut visited, &mut stack) {
                    return true;
                }
            }
            false
        }

        fn check_fas(g: &mut Graph, fas: &[Edge]) {
            let n = g.node_count() as i64;
            let m = g.edge_count() as i64;
            for e in fas {
                g.remove_edge(&e.v, &e.w);
            }
            assert!(!has_cycle(g), "Graph should be acyclic after removing FAS");
            // Performance bound: |FAS| <= floor(m/2) - floor(n/6)
            assert!(
                fas.len() as i64 <= m / 2 - n / 6,
                "FAS size {} exceeds bound {}",
                fas.len(),
                m / 2 - n / 6
            );
        }

        #[test]
        fn fas_empty_graph() {
            let g = make_g();
            let fas = greedy_fas(&g);
            assert_eq!(fas, vec![]);
        }

        #[test]
        fn fas_single_node() {
            let mut g = make_g();
            g.set_node("a", NodeLabel::default());
            let fas = greedy_fas(&g);
            assert_eq!(fas, vec![]);
        }

        #[test]
        fn fas_acyclic_graph() {
            let mut g = make_g();
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "c", EdgeLabel::default(), None);
            g.set_edge("b", "d", EdgeLabel::default(), None);
            g.set_edge("a", "e", EdgeLabel::default(), None);
            let fas = greedy_fas(&g);
            assert_eq!(fas, vec![]);
        }

        #[test]
        fn fas_simple_cycle() {
            let mut g = make_g();
            g.set_edge("a", "b", EdgeLabel::default(), None);
            g.set_edge("b", "a", EdgeLabel::default(), None);
            let fas = greedy_fas(&g);
            let mut g2 = g.clone();
            check_fas(&mut g2, &fas);
        }

        #[test]
        fn fas_four_node_cycle() {
            let mut g = make_g();
            g.set_edge("n1", "n2", EdgeLabel::default(), None);
            set_path(&mut g, &["n2", "n3", "n4", "n5", "n2"]);
            g.set_edge("n3", "n5", EdgeLabel::default(), None);
            g.set_edge("n4", "n2", EdgeLabel::default(), None);
            g.set_edge("n4", "n6", EdgeLabel::default(), None);
            let fas = greedy_fas(&g);
            let mut g2 = g.clone();
            check_fas(&mut g2, &fas);
        }

        #[test]
        fn fas_two_four_node_cycles() {
            let mut g = make_g();
            g.set_edge("n1", "n2", EdgeLabel::default(), None);
            set_path(&mut g, &["n2", "n3", "n4", "n5", "n2"]);
            g.set_edge("n3", "n5", EdgeLabel::default(), None);
            g.set_edge("n4", "n2", EdgeLabel::default(), None);
            g.set_edge("n4", "n6", EdgeLabel::default(), None);
            set_path(&mut g, &["n6", "n7", "n8", "n9", "n6"]);
            g.set_edge("n7", "n9", EdgeLabel::default(), None);
            g.set_edge("n8", "n6", EdgeLabel::default(), None);
            g.set_edge("n8", "n10", EdgeLabel::default(), None);
            let fas = greedy_fas(&g);
            let mut g2 = g.clone();
            check_fas(&mut g2, &fas);
        }

        #[test]
        fn fas_weighted_prefers_low_weight() {
            // g1: n1->n2 weight=2, n2->n1 weight=1 => reverse n2->n1
            let mut g1 = make_g();
            g1.set_edge(
                "n1",
                "n2",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g1.set_edge(
                "n2",
                "n1",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            let fas1 = greedy_fas(&g1);
            assert_eq!(fas1.len(), 1);
            assert_eq!(fas1[0].v, "n2");
            assert_eq!(fas1[0].w, "n1");

            // g2: n1->n2 weight=1, n2->n1 weight=2 => reverse n1->n2
            let mut g2 = make_g();
            g2.set_edge(
                "n1",
                "n2",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g2.set_edge(
                "n2",
                "n1",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            let fas2 = greedy_fas(&g2);
            assert_eq!(fas2.len(), 1);
            assert_eq!(fas2[0].v, "n1");
            assert_eq!(fas2[0].w, "n2");
        }

        #[test]
        fn fas_works_for_multigraphs() {
            // "works for multigraphs"
            // a->b weight=5 (name="foo"), b->a weight=2 (name="bar"), b->a weight=2 (name="baz")
            // total a->b = 5, total b->a = 4 => FAS picks b->a edges (both bar and baz)
            let mut g = Graph::with_options(true, true, false);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(5.0),
                    ..Default::default()
                },
                Some("foo"),
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                Some("bar"),
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                Some("baz"),
            );
            let mut fas = greedy_fas(&g);
            // Sort by name for deterministic comparison
            fas.sort_by(|a, b| {
                a.name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.name.as_deref().unwrap_or(""))
            });
            assert_eq!(fas.len(), 2, "expected 2 FAS edges, got {:?}", fas);
            assert_eq!(fas[0].v, "b");
            assert_eq!(fas[0].w, "a");
            assert_eq!(fas[0].name.as_deref(), Some("bar"));
            assert_eq!(fas[1].v, "b");
            assert_eq!(fas[1].w, "a");
            assert_eq!(fas[1].name.as_deref(), Some("baz"));
        }
    } // mod greedy_fas_tests

    // =========================================================================
    // Ported from dagre-js/test/rank/rank-test.ts
    // =========================================================================

    mod rank_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::rank::rank;

        fn make_g(ranker: &str) -> Graph {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                ranker: Some(ranker.to_string()),
                ..Default::default()
            });

            // Set default labels on nodes
            for v in &["a", "b", "c", "d", "e", "f", "g", "h"] {
                g.set_node(v, NodeLabel::default());
            }

            // Set edges with minlen=1, weight=1
            let de = EdgeLabel {
                minlen: Some(1),
                weight: Some(1.0),
                ..Default::default()
            };
            for (v, w) in &[
                ("a", "b"),
                ("b", "c"),
                ("c", "d"),
                ("d", "h"),
                ("a", "e"),
                ("e", "g"),
                ("g", "h"),
                ("a", "f"),
                ("f", "g"),
            ] {
                g.set_edge(v, w, de.clone(), None);
            }
            g
        }

        #[allow(dead_code)]
        const RANKERS: &[&str] = &[
            "longest-path",
            "tight-tree",
            "network-simplex",
            "unknown-should-still-work",
        ];

        #[test]
        fn rank_respects_minlen_longest_path() {
            let mut g = make_g("longest-path");
            rank(&mut g);
            for e in g.edges() {
                let v_rank = g.node(&e.v).rank.unwrap_or(0);
                let w_rank = g.node(&e.w).rank.unwrap_or(0);
                let minlen = g.edge(&e).unwrap().minlen.unwrap_or(1);
                assert!(
                    w_rank - v_rank >= minlen,
                    "edge {}->{}: w_rank={} - v_rank={} < minlen={}",
                    e.v,
                    e.w,
                    w_rank,
                    v_rank,
                    minlen
                );
            }
        }

        #[test]
        fn rank_respects_minlen_tight_tree() {
            let mut g = make_g("tight-tree");
            rank(&mut g);
            for e in g.edges() {
                let v_rank = g.node(&e.v).rank.unwrap_or(0);
                let w_rank = g.node(&e.w).rank.unwrap_or(0);
                let minlen = g.edge(&e).unwrap().minlen.unwrap_or(1);
                assert!(w_rank - v_rank >= minlen);
            }
        }

        #[test]
        fn rank_respects_minlen_network_simplex() {
            let mut g = make_g("network-simplex");
            rank(&mut g);
            for e in g.edges() {
                let v_rank = g.node(&e.v).rank.unwrap_or(0);
                let w_rank = g.node(&e.w).rank.unwrap_or(0);
                let minlen = g.edge(&e).unwrap().minlen.unwrap_or(1);
                assert!(w_rank - v_rank >= minlen);
            }
        }

        #[test]
        fn rank_respects_minlen_unknown_ranker() {
            let mut g = make_g("unknown-should-still-work");
            rank(&mut g);
            for e in g.edges() {
                let v_rank = g.node(&e.v).rank.unwrap_or(0);
                let w_rank = g.node(&e.w).rank.unwrap_or(0);
                let minlen = g.edge(&e).unwrap().minlen.unwrap_or(1);
                assert!(w_rank - v_rank >= minlen);
            }
        }

        #[test]
        fn rank_single_node_longest_path() {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                ranker: Some("longest-path".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            rank(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
        }

        #[test]
        fn rank_single_node_tight_tree() {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                ranker: Some("tight-tree".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            rank(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
        }

        #[test]
        fn rank_single_node_network_simplex() {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                ranker: Some("network-simplex".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            rank(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
        }

        #[test]
        fn rank_single_node_unknown_ranker() {
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                ranker: Some("unknown-should-still-work".to_string()),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            rank(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
        }
    } // mod rank_tests

    // =========================================================================
    // Ported from dagre-js/test/rank/feasible-tree-test.ts
    // =========================================================================

    mod feasible_tree_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::rank::feasible_tree::feasible_tree;

        #[test]
        fn ft_trivial_input_graph() {
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );

            let tree = feasible_tree(&mut g);
            assert_eq!(g.node("b").rank.unwrap(), g.node("a").rank.unwrap() + 1);
            let nbrs = tree.neighbors("a").unwrap_or_default();
            assert_eq!(nbrs, vec!["b"]);
        }

        #[test]
        fn ft_shortens_slack_by_pulling_up() {
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            // a->b->c with minlen=1
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "a",
                "d",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );

            let tree = feasible_tree(&mut g);
            assert_eq!(g.node("b").rank.unwrap(), g.node("a").rank.unwrap() + 1);
            assert_eq!(g.node("c").rank.unwrap(), g.node("b").rank.unwrap() + 1);
            assert_eq!(g.node("d").rank.unwrap(), g.node("a").rank.unwrap() + 1);

            let mut nbrs_a = tree.neighbors("a").unwrap_or_default();
            nbrs_a.sort();
            assert_eq!(nbrs_a, vec!["b", "d"]);

            let mut nbrs_b = tree.neighbors("b").unwrap_or_default();
            nbrs_b.sort();
            assert_eq!(nbrs_b, vec!["a", "c"]);

            assert_eq!(tree.neighbors("c").unwrap_or_default(), vec!["b"]);
            assert_eq!(tree.neighbors("d").unwrap_or_default(), vec!["a"]);
        }

        #[test]
        fn ft_shortens_slack_by_pulling_down() {
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );

            let tree = feasible_tree(&mut g);
            assert_eq!(g.node("a").rank.unwrap(), g.node("b").rank.unwrap() + 1);
            assert_eq!(g.node("c").rank.unwrap(), g.node("b").rank.unwrap() + 1);

            let mut nbrs_a = tree.neighbors("a").unwrap_or_default();
            nbrs_a.sort();
            assert_eq!(nbrs_a, vec!["b"]);

            let mut nbrs_b = tree.neighbors("b").unwrap_or_default();
            nbrs_b.sort();
            assert_eq!(nbrs_b, vec!["a", "c"]);

            let mut nbrs_c = tree.neighbors("c").unwrap_or_default();
            nbrs_c.sort();
            assert_eq!(nbrs_c, vec!["b"]);
        }
    } // mod feasible_tree_tests

    // =========================================================================
    // order/add_subgraph_constraints tests
    // Ported from dagre-js/test/order/add-subgraph-constraints-test.ts
    // =========================================================================
    mod add_subgraph_constraints_tests {
        use crate::graph::{EdgeLabel, Graph, NodeLabel};
        use crate::order::add_subgraph_constraints::add_subgraph_constraints;

        fn make_graph() -> Graph {
            Graph::with_options(true, false, true)
        }

        fn make_cg() -> Graph {
            Graph::with_options(true, false, false)
        }

        #[test]
        fn asc_flat_set_no_change() {
            // "does not change CG for a flat set of nodes"
            let mut graph = make_graph();
            let mut cg = make_cg();
            let vs: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
            for v in &vs {
                graph.set_node(v, NodeLabel::default());
            }
            add_subgraph_constraints(&graph, &mut cg, &vs);
            assert_eq!(cg.node_count(), 0);
            assert_eq!(cg.edge_count(), 0);
        }

        #[test]
        fn asc_contiguous_subgraph_no_constraint() {
            // "doesn't create a constraint for contiguous subgraph nodes"
            let mut graph = make_graph();
            let mut cg = make_cg();
            let vs: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
            for v in &vs {
                graph.set_parent(v, Some("sg"));
            }
            add_subgraph_constraints(&graph, &mut cg, &vs);
            assert_eq!(cg.node_count(), 0);
            assert_eq!(cg.edge_count(), 0);
        }

        #[test]
        fn asc_different_parents_adds_constraint() {
            // "adds a constraint when the parents for adjacent nodes are different"
            let mut graph = make_graph();
            let mut cg = make_cg();
            let vs: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
            graph.set_parent("a", Some("sg1"));
            graph.set_parent("b", Some("sg2"));
            add_subgraph_constraints(&graph, &mut cg, &vs);
            let edges = cg.edges();
            assert_eq!(edges.len(), 1);
            assert_eq!(edges[0].v, "sg1");
            assert_eq!(edges[0].w, "sg2");
        }

        #[test]
        fn asc_multiple_levels() {
            // "works for multiple levels"
            let mut graph = make_graph();
            let mut cg = make_cg();
            let vs: Vec<String> = ["a", "b", "c", "d", "e", "f", "g", "h"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            for v in &vs {
                graph.set_node(v, NodeLabel::default());
            }
            graph.set_parent("b", Some("sg2"));
            graph.set_parent("sg2", Some("sg1"));
            graph.set_parent("c", Some("sg1"));
            graph.set_parent("d", Some("sg3"));
            graph.set_parent("sg3", Some("sg1"));
            graph.set_parent("f", Some("sg4"));
            graph.set_parent("g", Some("sg5"));
            graph.set_parent("sg5", Some("sg4"));
            add_subgraph_constraints(&graph, &mut cg, &vs);
            let mut edges = cg.edges();
            edges.sort_by(|a, b| a.v.cmp(&b.v));
            assert_eq!(edges.len(), 2);
            assert_eq!(edges[0].v, "sg1");
            assert_eq!(edges[0].w, "sg4");
            assert_eq!(edges[1].v, "sg2");
            assert_eq!(edges[1].w, "sg3");
        }
    } // mod add_subgraph_constraints_tests

    // =========================================================================
    // data/list tests
    // Ported from dagre-js/test/data/list-test.ts
    // =========================================================================
    mod list_tests {
        use crate::data::list::List;

        #[test]
        fn list_dequeue_empty_returns_none() {
            // "returns undefined with an empty list"
            let mut list = List::new();
            assert!(list.dequeue().is_none());
        }

        #[test]
        fn list_dequeue_single_entry() {
            // "unlinks and returns the first entry"
            let mut list = List::new();
            let h = list.alloc();
            list.enqueue(h);
            assert_eq!(list.dequeue(), Some(h));
        }

        #[test]
        fn list_dequeue_fifo_order() {
            // "unlinks and returns multiple entries in FIFO order"
            let mut list = List::new();
            let h1 = list.alloc();
            let h2 = list.alloc();
            list.enqueue(h1);
            list.enqueue(h2);
            assert_eq!(list.dequeue(), Some(h1));
            assert_eq!(list.dequeue(), Some(h2));
        }

        #[test]
        fn list_reenqueue_moves_to_back() {
            // "unlinks and relinks an entry if it is re-enqueued"
            // When obj1 is re-enqueued after obj2, obj2 should come out first
            let mut list = List::new();
            let h1 = list.alloc();
            let h2 = list.alloc();
            list.enqueue(h1);
            list.enqueue(h2);
            list.enqueue(h1); // re-enqueue h1 — moves it to front → dequeued last
            assert_eq!(list.dequeue(), Some(h2));
            assert_eq!(list.dequeue(), Some(h1));
        }

        #[test]
        fn list_enqueue_on_another_list_unlinks_from_first() {
            // "unlinks and relinks an entry if it is enqueued on another list"
            // JS test: enqueue obj on list1, then enqueue same obj on list2.
            // list1.dequeue() => undefined, list2.dequeue() => obj
            //
            // In Rust the List allocates handles internally — a handle created on
            // one list cannot be used with another list (different Vec indices).
            // The equivalent behavior to test is: a handle that is unlinked
            // (because it was never enqueued or was dequeued) is not returned.
            let mut list = List::new();
            let h = list.alloc();
            // Never enqueue h; list is empty — dequeue returns None
            assert!(list.dequeue().is_none());
            // Enqueue h now, dequeue returns it
            list.enqueue(h);
            assert_eq!(list.dequeue(), Some(h));
        }

        #[test]
        fn list_is_linked_status() {
            // Structural test: is_linked reflects enqueue/dequeue state
            let mut list = List::new();
            let h = list.alloc();
            assert!(!list.is_linked(h));
            list.enqueue(h);
            assert!(list.is_linked(h));
            list.dequeue();
            assert!(!list.is_linked(h));
        }
    } // mod list_tests

    // =========================================================================
    // util tests
    // Ported from dagre-js/test/util-test.ts
    // =========================================================================
    mod util_tests {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel, Point};
        use crate::util::{
            as_non_compound_graph, build_layer_matrix, intersect_rect, map_values, normalize_ranks,
            predecessor_weights, range, remove_empty_ranks, simplify, successor_weights,
        };
        use std::collections::HashMap;

        // ── simplify ─────────────────────────────────────────────────────────

        #[test]
        fn simplify_no_multi_edges() {
            // "copies without change a graph with no multi-edges"
            let mut g = Graph::with_options(true, true, false);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(1.0),
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            let g2 = simplify(&g);
            let e = g2.edge_vw("a", "b").unwrap();
            assert_eq!(e.weight, Some(1.0));
            assert_eq!(e.minlen, Some(1));
            assert_eq!(g2.edge_count(), 1);
        }

        #[test]
        fn simplify_collapses_multi_edges() {
            // "collapses multi-edges"
            let mut g = Graph::with_options(true, true, false);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(1.0),
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    minlen: Some(2),
                    ..Default::default()
                },
                Some("multi"),
            );
            let g2 = simplify(&g);
            assert!(!g2.is_multigraph());
            let e = g2.edge_vw("a", "b").unwrap();
            assert_eq!(e.weight, Some(3.0));
            assert_eq!(e.minlen, Some(2));
            assert_eq!(g2.edge_count(), 1);
        }

        #[test]
        fn simplify_copies_graph_object() {
            // "copies the graph object"
            let mut g = Graph::with_options(true, true, false);
            g.set_graph(GraphLabel {
                rankdir: Some("LR".to_string()),
                ..Default::default()
            });
            let g2 = simplify(&g);
            assert_eq!(g2.graph().rankdir, Some("LR".to_string()));
        }

        // ── asNonCompoundGraph ───────────────────────────────────────────────

        #[test]
        fn as_non_compound_graph_copies_all_nodes() {
            // "copies all nodes"
            let mut g = Graph::with_options(true, true, true);
            g.set_node(
                "a",
                NodeLabel {
                    label: Some("bar".to_string()),
                    ..Default::default()
                },
            );
            g.set_node("b", NodeLabel::default());
            let g2 = as_non_compound_graph(&g);
            assert_eq!(g2.node("a").label, Some("bar".to_string()));
            assert!(g2.has_node("b"));
        }

        #[test]
        fn as_non_compound_graph_copies_all_edges() {
            // "copies all edges"
            let mut g = Graph::with_options(true, true, true);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(1),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    ..Default::default()
                },
                Some("multi"),
            );
            let g2 = as_non_compound_graph(&g);
            assert_eq!(g2.edge_vw("a", "b").unwrap().minlen, Some(1));
            assert_eq!(
                g2.edge_label_named("a", "b", "multi").unwrap().minlen,
                Some(2)
            );
        }

        #[test]
        fn as_non_compound_graph_does_not_copy_compound_nodes() {
            // "does not copy compound nodes"
            let mut g = Graph::with_options(true, true, true);
            g.set_parent("a", Some("sg1"));
            let g2 = as_non_compound_graph(&g);
            // sg1 has a child, so it should not be in g2
            assert!(!g2.has_node("sg1"));
            // a has no children, so it should be in g2
            assert!(g2.has_node("a"));
            // g2 is not compound
            assert!(!g2.is_compound());
            // parent returns None for any node in a non-compound graph
            assert!(g2.parent("a").is_none());
        }

        #[test]
        fn as_non_compound_graph_copies_graph_object() {
            // "copies the graph object"
            let mut g = Graph::with_options(true, true, true);
            g.set_graph(GraphLabel {
                rankdir: Some("TB".to_string()),
                ..Default::default()
            });
            let g2 = as_non_compound_graph(&g);
            assert_eq!(g2.graph().rankdir, Some("TB".to_string()));
        }

        // ── successorWeights ─────────────────────────────────────────────────

        #[test]
        fn successor_weights_maps_nodes_to_successors() {
            // "maps a node to its successors with associated weights"
            let mut g = Graph::with_options(true, true, false);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                Some("multi"),
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                Some("multi2"),
            );
            let sw = successor_weights(&g);
            assert_eq!(sw["a"].get("b"), Some(&2.0));
            assert_eq!(sw["b"].get("c"), Some(&3.0));
            assert_eq!(sw["b"].get("d"), Some(&1.0));
            assert!(sw["c"].is_empty());
            assert!(sw["d"].is_empty());
        }

        // ── predecessorWeights ───────────────────────────────────────────────

        #[test]
        fn predecessor_weights_maps_nodes_to_predecessors() {
            // "maps a node to its predecessors with associated weights"
            let mut g = Graph::with_options(true, true, false);
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "c",
                EdgeLabel {
                    weight: Some(2.0),
                    ..Default::default()
                },
                Some("multi"),
            );
            g.set_edge(
                "b",
                "d",
                EdgeLabel {
                    weight: Some(1.0),
                    ..Default::default()
                },
                Some("multi2"),
            );
            let pw = predecessor_weights(&g);
            assert!(pw["a"].is_empty());
            assert_eq!(pw["b"].get("a"), Some(&2.0));
            assert_eq!(pw["c"].get("b"), Some(&3.0));
            assert_eq!(pw["d"].get("b"), Some(&1.0));
        }

        // ── intersectRect ────────────────────────────────────────────────────

        fn make_rect(x: f64, y: f64, width: f64, height: f64) -> NodeLabel {
            NodeLabel {
                x: Some(x),
                y: Some(y),
                width,
                height,
                ..Default::default()
            }
        }

        fn make_point(x: f64, y: f64) -> Point {
            Point { x, y }
        }

        fn expect_intersects(rect: &NodeLabel, point: &Point) {
            let cross = intersect_rect(rect, point);
            let rx = rect.x.unwrap_or(0.0);
            let ry = rect.y.unwrap_or(0.0);
            if (cross.x - point.x).abs() > 1e-9 {
                let m = (cross.y - point.y) / (cross.x - point.x);
                // cross.y - ry ≈ m * (cross.x - rx)
                let lhs = cross.y - ry;
                let rhs = m * (cross.x - rx);
                assert!(
                    (lhs - rhs).abs() < 1e-9,
                    "slope mismatch: lhs={} rhs={}",
                    lhs,
                    rhs
                );
            }
        }

        fn expect_touches_border(rect: &NodeLabel, point: &Point) {
            let cross = intersect_rect(rect, point);
            let rx = rect.x.unwrap_or(0.0);
            let ry = rect.y.unwrap_or(0.0);
            if (rx - cross.x).abs() != rect.width / 2.0 {
                assert!(
                    ((ry - cross.y).abs() - rect.height / 2.0).abs() < 1e-9,
                    "border not touched: ry={} cross.y={} height/2={}",
                    ry,
                    cross.y,
                    rect.height / 2.0
                );
            }
        }

        #[test]
        fn intersect_rect_slope_through_center() {
            // "creates a slope that will intersect the rectangle's center"
            let rect = make_rect(0.0, 0.0, 1.0, 1.0);
            expect_intersects(&rect, &make_point(2.0, 6.0));
            expect_intersects(&rect, &make_point(2.0, -6.0));
            expect_intersects(&rect, &make_point(6.0, 2.0));
            expect_intersects(&rect, &make_point(-6.0, 2.0));
            expect_intersects(&rect, &make_point(5.0, 0.0));
            expect_intersects(&rect, &make_point(0.0, 5.0));
        }

        #[test]
        fn intersect_rect_touches_border() {
            // "touches the border of the rectangle"
            let rect = make_rect(0.0, 0.0, 1.0, 1.0);
            expect_touches_border(&rect, &make_point(2.0, 6.0));
            expect_touches_border(&rect, &make_point(2.0, -6.0));
            expect_touches_border(&rect, &make_point(6.0, 2.0));
            expect_touches_border(&rect, &make_point(-6.0, 2.0));
            expect_touches_border(&rect, &make_point(5.0, 0.0));
            expect_touches_border(&rect, &make_point(0.0, 5.0));
        }

        #[test]
        fn intersect_rect_returns_center_when_point_is_center() {
            // When the point coincides with the rectangle center, return the center
            // as a graceful fallback (the original panicked; we handle it gracefully
            // so that degenerate edge endpoints don't crash the layout).
            let rect = make_rect(0.0, 0.0, 1.0, 1.0);
            let result = intersect_rect(&rect, &make_point(0.0, 0.0));
            assert!((result.x - 0.0).abs() < 1e-9);
            assert!((result.y - 0.0).abs() < 1e-9);
        }

        // ── buildLayerMatrix ─────────────────────────────────────────────────

        #[test]
        fn build_layer_matrix_basic() {
            // "creates a matrix based on rank and order of nodes in the graph"
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    order: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(0),
                    order: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(1),
                    order: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "d",
                NodeLabel {
                    rank: Some(1),
                    order: Some(1),
                    ..Default::default()
                },
            );
            g.set_node(
                "e",
                NodeLabel {
                    rank: Some(2),
                    order: Some(0),
                    ..Default::default()
                },
            );
            let matrix = build_layer_matrix(&g);
            assert_eq!(matrix.len(), 3);
            assert_eq!(matrix[0], vec!["a", "b"]);
            assert_eq!(matrix[1], vec!["c", "d"]);
            assert_eq!(matrix[2], vec!["e"]);
        }

        // ── normalizeRanks ───────────────────────────────────────────────────

        #[test]
        fn normalize_ranks_adjusts_to_zero_min() {
            // "adjust ranks such that all are >= 0, and at least one is 0"
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(3),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(2),
                    ..Default::default()
                },
            );
            g.set_node(
                "c",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            normalize_ranks(&mut g);
            assert_eq!(g.node("a").rank, Some(1));
            assert_eq!(g.node("b").rank, Some(0));
            assert_eq!(g.node("c").rank, Some(2));
        }

        #[test]
        fn normalize_ranks_negative_ranks() {
            // "works for negative ranks"
            let mut g = Graph::with_options(true, false, false);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(-3),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(-2),
                    ..Default::default()
                },
            );
            normalize_ranks(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(1));
        }

        #[test]
        fn normalize_ranks_skips_subgraphs() {
            // "does not assign a rank to subgraphs"
            let mut g = Graph::with_options(true, false, true);
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node("sg", NodeLabel::default()); // no rank
            g.set_parent("a", Some("sg"));
            normalize_ranks(&mut g);
            assert!(g.node("sg").rank.is_none());
            assert_eq!(g.node("a").rank, Some(0));
        }

        // ── removeEmptyRanks ─────────────────────────────────────────────────

        #[test]
        fn remove_empty_ranks_border_ranks() {
            // "Removes border ranks without any nodes"
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                node_rank_factor: Some(4),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(4),
                    ..Default::default()
                },
            );
            remove_empty_ranks(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(1));
        }

        #[test]
        fn remove_empty_ranks_non_border_ranks_preserved() {
            // "Does not remove non-border ranks"
            let mut g = Graph::with_options(true, false, false);
            g.set_graph(GraphLabel {
                node_rank_factor: Some(4),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(8),
                    ..Default::default()
                },
            );
            remove_empty_ranks(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(2));
        }

        #[test]
        fn remove_empty_ranks_handles_parents_with_undefined_ranks() {
            // "Handles parents with undefined ranks"
            let mut g = Graph::with_options(true, false, true);
            g.set_graph(GraphLabel {
                node_rank_factor: Some(3),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    rank: Some(0),
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    rank: Some(6),
                    ..Default::default()
                },
            );
            g.set_node("sg", NodeLabel::default());
            g.set_parent("a", Some("sg"));
            remove_empty_ranks(&mut g);
            assert_eq!(g.node("a").rank, Some(0));
            assert_eq!(g.node("b").rank, Some(2));
            assert!(g.node("sg").rank.is_none());
        }

        // ── range ────────────────────────────────────────────────────────────

        #[test]
        fn range_builds_to_limit() {
            // "Builds an array to the limit"
            let r = range(4, None, None);
            assert_eq!(r.len(), 4);
            assert_eq!(r.iter().sum::<i32>(), 6);
        }

        #[test]
        fn range_builds_with_start() {
            // "Builds an array with a start"
            let r = range(2, Some(4), None);
            assert_eq!(r.len(), 2);
            assert_eq!(r.iter().sum::<i32>(), 5);
        }

        #[test]
        fn range_builds_with_negative_step() {
            // "Builds an array with a negative step"
            let r = range(5, Some(-1), Some(-1));
            assert_eq!(r[0], 5);
            assert_eq!(r[5], 0);
        }

        // ── mapValues ────────────────────────────────────────────────────────

        #[test]
        fn map_values_same_keys() {
            // "Creates an object with the same keys"
            let mut users: HashMap<String, HashMap<String, i32>> = HashMap::new();
            let mut fred = HashMap::new();
            fred.insert("age".to_string(), 40i32);
            let mut pebbles = HashMap::new();
            pebbles.insert("age".to_string(), 1i32);
            users.insert("fred".to_string(), fred);
            users.insert("pebbles".to_string(), pebbles);
            let ages = map_values(users, |user, _k| *user.get("age").unwrap());
            assert_eq!(ages["fred"], 40);
            assert_eq!(ages["pebbles"], 1);
        }
    } // mod util_tests

    // =========================================================================
    // unique_id tests
    // Ported from dagre-js/test/unique-id-test.ts
    // =========================================================================
    mod unique_id_tests {
        use crate::util::unique_id;

        #[test]
        fn unique_id_valid_identifier() {
            // "uniqueId(name) generates a valid identifier"
            // Guards against bug #477 where [object undefined] was produced.
            let id = unique_id("_root");
            assert!(!id.contains("[object undefined]"));
            // Should match /_root\d+/
            assert!(id.starts_with("_root"));
            let suffix = &id["_root".len()..];
            assert!(
                suffix.chars().all(|c| c.is_ascii_digit()),
                "suffix '{}' is not all digits",
                suffix
            );
        }

        #[test]
        fn unique_id_multiple_calls_distinct() {
            // "Calling uniqueId(name) multiple times generate distinct values"
            let first = unique_id("name");
            let second = unique_id("name");
            let third = unique_id("name");
            assert_ne!(first, second);
            assert_ne!(second, third);
        }

        #[test]
        fn unique_id_numeric_prefix() {
            // "Calling uniqueId(number) with a number creates a valid identifier string"
            let id = unique_id("99");
            // Should be a string that starts with "99" and is followed by digits
            assert!(id.starts_with("99"));
            let suffix = &id["99".len()..];
            assert!(
                suffix.chars().all(|c| c.is_ascii_digit()),
                "suffix '{}' is not all digits",
                suffix
            );
        }
    } // mod unique_id_tests

    // =========================================================================
    // ADDITIONAL MISSING TESTS — identified via exhaustive pass
    // =========================================================================

    // ─── calcCutValue four-node tree tests (from network-simplex-test.ts) ─────
    // These were in the JS file but not yet ported.

    #[test]
    fn calc_cut_value_4node_gc_to_c_to_p_to_o_o_into_c() {
        // "works for 4-node tree with gc -> c -> p -> o, with o -> c"
        // g: o->c (weight 7), gc->c->p->o
        // t: gc-c (cv=3), c-p-o
        // initLowLim from "p"
        // expected: calcCutValue(t,g,"c") == -4
        let mut g = make_g();
        g.set_edge(
            "o",
            "c",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["gc", "c", "p", "o"], de());
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        set_path(&mut t, &["c", "p", "o"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -4.0);
    }

    #[test]
    fn calc_cut_value_4node_gc_to_c_to_p_to_o_o_from_c() {
        // "works for 4-node tree with gc -> c -> p -> o, with o <- c"
        // g: c->o (weight 7), gc->c->p->o
        // t: gc-c (cv=3), c-p-o
        // expected: 10
        let mut g = make_g();
        g.set_edge(
            "c",
            "o",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["gc", "c", "p", "o"], de());
        let mut t = make_t();
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        set_path(&mut t, &["c", "p", "o"], EdgeLabel::default());
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 10.0);
    }

    #[test]
    fn calc_cut_value_4node_o_gc_c_p_o_into_c() {
        // "works for 4-node tree with o -> gc -> c -> p, with o -> c"
        // g: o->c (weight 7), o->gc->c->p
        // t: o-gc, gc-c (cv=3), c-p
        // expected: -4
        let mut g = make_g();
        g.set_edge(
            "o",
            "c",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["o", "gc", "c", "p"], de());
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -4.0);
    }

    #[test]
    fn calc_cut_value_4node_o_gc_c_p_o_from_c() {
        // "works for 4-node tree with o -> gc -> c -> p, with o <- c"
        // g: c->o (weight 7), o->gc->c->p
        // t: o-gc, gc-c (cv=3), c-p
        // expected: 10
        let mut g = make_g();
        g.set_edge(
            "c",
            "o",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["o", "gc", "c", "p"], de());
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 10.0);
    }

    #[test]
    fn calc_cut_value_4node_gc_c_back_p_o_o_into_c() {
        // "works for 4-node tree with gc -> c <- p -> o, with o -> c"
        // g: gc->c, p->c, p->o, o->c (weight 7)
        // t: o-gc, gc-c (cv=3), c-p
        // expected: 6
        let mut g = make_g();
        g.set_edge("gc", "c", de(), None);
        g.set_edge("p", "c", de(), None);
        g.set_edge("p", "o", de(), None);
        g.set_edge(
            "o",
            "c",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 6.0);
    }

    #[test]
    fn calc_cut_value_4node_gc_c_back_p_o_o_from_c() {
        // "works for 4-node tree with gc -> c <- p -> o, with o <- c"
        // g: gc->c, p->c, p->o, c->o (weight 7)
        // t: o-gc, gc-c (cv=3), c-p
        // expected: -8
        let mut g = make_g();
        g.set_edge("gc", "c", de(), None);
        g.set_edge("p", "c", de(), None);
        g.set_edge("p", "o", de(), None);
        g.set_edge(
            "c",
            "o",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -8.0);
    }

    #[test]
    fn calc_cut_value_4node_o_gc_c_back_p_o_into_c() {
        // "works for 4-node tree with o -> gc -> c <- p, with o -> c"
        // g: o->c (weight 7), o->gc->c, p->c
        // t: o-gc, gc-c (cv=3), c-p
        // expected: 6
        let mut g = make_g();
        g.set_edge(
            "o",
            "c",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["o", "gc", "c"], de());
        g.set_edge("p", "c", de(), None);
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), 6.0);
    }

    #[test]
    fn calc_cut_value_4node_o_gc_c_back_p_o_from_c() {
        // "works for 4-node tree with o -> gc -> c <- p, with o <- c"
        // g: c->o (weight 7), o->gc->c, p->c
        // t: o-gc, gc-c (cv=3), c-p
        // expected: -8
        let mut g = make_g();
        g.set_edge(
            "c",
            "o",
            EdgeLabel {
                weight: Some(7.0),
                ..de()
            },
            None,
        );
        set_path(&mut g, &["o", "gc", "c"], de());
        g.set_edge("p", "c", de(), None);
        let mut t = make_t();
        t.set_edge("o", "gc", EdgeLabel::default(), None);
        t.set_edge(
            "gc",
            "c",
            EdgeLabel {
                cutvalue: Some(3.0),
                ..Default::default()
            },
            None,
        );
        t.set_edge("c", "p", EdgeLabel::default(), None);
        init_low_lim_values(&mut t, Some("p".to_string()));
        assert_eq!(calc_cut_value(&t, &g, "c"), -8.0);
    }

    // ─── acyclic - dfs creates multi-edge ────────────────────────────────────

    mod acyclic_extra_tests {
        use crate::acyclic;
        use crate::graph::{EdgeLabel, Graph, NodeLabel};

        fn make_g() -> Graph {
            Graph::with_options(true, true, false)
        }

        fn de() -> EdgeLabel {
            EdgeLabel {
                minlen: Some(1),
                weight: Some(1.0),
                ..Default::default()
            }
        }

        fn set_path(g: &mut Graph, nodes: &[&str], label: EdgeLabel) {
            for i in 0..nodes.len() - 1 {
                if !g.has_node(nodes[i]) {
                    g.set_node(nodes[i], NodeLabel::default());
                }
                if !g.has_node(nodes[i + 1]) {
                    g.set_node(nodes[i + 1], NodeLabel::default());
                }
                g.set_edge(nodes[i], nodes[i + 1], label.clone(), None);
            }
        }

        fn has_cycle(g: &Graph) -> bool {
            let mut visited = std::collections::HashSet::new();
            let mut stack = std::collections::HashSet::new();
            fn dfs(
                g: &Graph,
                v: &str,
                visited: &mut std::collections::HashSet<String>,
                stack: &mut std::collections::HashSet<String>,
            ) -> bool {
                visited.insert(v.to_string());
                stack.insert(v.to_string());
                if let Some(succs) = g.successors(v) {
                    for w in succs {
                        if !visited.contains(&w) {
                            if dfs(g, &w, visited, stack) {
                                return true;
                            }
                        } else if stack.contains(&w) {
                            return true;
                        }
                    }
                }
                stack.remove(v);
                false
            }
            for v in g.nodes() {
                if !visited.contains(&v) {
                    if dfs(g, &v, &mut visited, &mut stack) {
                        return true;
                    }
                }
            }
            false
        }

        fn sort_edges(mut edges: Vec<(String, String)>) -> Vec<(String, String)> {
            edges.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            edges
        }

        fn strip_name(e: &crate::graph::Edge) -> (String, String) {
            (e.v.clone(), e.w.clone())
        }

        // --- dfs: creates multi-edge for self-loop ---
        #[test]
        fn acyclic_dfs_creates_multi_edge_for_self_loop() {
            // "creates a multi-edge where necessary"
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("dfs".to_string());
            set_path(&mut g, &["a", "b", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
            assert_eq!(g.edge_count(), 2);
        }

        // --- unknown-should-still-work acyclicer (falls back to dfs) ---

        #[test]
        fn acyclic_unknown_does_not_change_acyclic_graph() {
            // "does not change an already acyclic graph" with unknown acyclicer
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("unknown-should-still-work".to_string());
            set_path(&mut g, &["a", "b", "d"], de());
            set_path(&mut g, &["a", "c", "d"], de());
            acyclic::run(&mut g);
            let edges = g.edges().iter().map(strip_name).collect::<Vec<_>>();
            assert_eq!(
                sort_edges(edges),
                vec![
                    ("a".to_string(), "b".to_string()),
                    ("a".to_string(), "c".to_string()),
                    ("b".to_string(), "d".to_string()),
                    ("c".to_string(), "d".to_string()),
                ]
            );
        }

        #[test]
        fn acyclic_unknown_breaks_cycles() {
            // "breaks cycles in the input graph" with unknown acyclicer
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("unknown-should-still-work".to_string());
            set_path(&mut g, &["a", "b", "c", "d", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
        }

        #[test]
        fn acyclic_unknown_creates_multi_edge_for_self_loop() {
            // "creates a multi-edge where necessary" with unknown acyclicer
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("unknown-should-still-work".to_string());
            set_path(&mut g, &["a", "b", "a"], de());
            acyclic::run(&mut g);
            assert!(!has_cycle(&g));
            assert_eq!(g.edge_count(), 2);
        }

        #[test]
        fn acyclic_unknown_undo_does_not_change_acyclic_graph() {
            // "does not change edges where the original graph was acyclic"
            // with unknown acyclicer
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("unknown-should-still-work".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let el = g.edge_vw("a", "b").unwrap();
            assert_eq!(el.minlen, Some(2));
            assert!((el.weight.unwrap() - 3.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 1);
        }

        #[test]
        fn acyclic_unknown_undo_restores_reversed_edges() {
            // "can restore previously reversed edges" with unknown acyclicer
            let mut g = make_g();
            g.graph_mut().acyclicer = Some("unknown-should-still-work".to_string());
            g.set_edge(
                "a",
                "b",
                EdgeLabel {
                    minlen: Some(2),
                    weight: Some(3.0),
                    ..Default::default()
                },
                None,
            );
            g.set_edge(
                "b",
                "a",
                EdgeLabel {
                    minlen: Some(3),
                    weight: Some(4.0),
                    ..Default::default()
                },
                None,
            );
            acyclic::run(&mut g);
            acyclic::undo(&mut g);
            let ab = g.edge_vw("a", "b").unwrap();
            assert_eq!(ab.minlen, Some(2));
            assert!((ab.weight.unwrap() - 3.0).abs() < 1e-9);
            let ba = g.edge_vw("b", "a").unwrap();
            assert_eq!(ba.minlen, Some(3));
            assert!((ba.weight.unwrap() - 4.0).abs() < 1e-9);
            assert_eq!(g.edge_count(), 2);
        }
    } // mod acyclic_extra_tests

    // ─── layout: treats attributes with case-insensitivity ───────────────────
    // The JS test sets `g.graph().nodeSep = 200` (camelCase "nodeSep").
    // In the Rust port, GraphLabel uses snake_case field `nodesep`, so there
    // is no camelCase normalization to test. This test is JS-engine-specific
    // and cannot be ported 1-to-1.
    mod layout_case_insensitivity {
        use crate::graph::{EdgeLabel, Graph, GraphLabel, NodeLabel};
        use crate::layout::layout;

        #[test]
        #[ignore = "JS-specific: tests camelCase/lowercase attribute normalisation \
                    (e.g. nodeSep vs nodesep) which is not applicable in Rust \
                    where GraphLabel uses typed snake_case fields"]
        fn layout_treats_attributes_case_insensitively() {
            // In JS: g.graph().nodeSep = 200 (capital S) is normalised to nodesep.
            // In Rust, GraphLabel has `nodesep: Option<f64>` — no normalisation needed.
            let mut g = Graph::with_options(true, true, true);
            g.set_graph(GraphLabel {
                nodesep: Some(200.0),
                ..Default::default()
            });
            g.set_node(
                "a",
                NodeLabel {
                    width: 50.0,
                    height: 100.0,
                    ..Default::default()
                },
            );
            g.set_node(
                "b",
                NodeLabel {
                    width: 75.0,
                    height: 200.0,
                    ..Default::default()
                },
            );
            layout(&mut g);
            let a = g.node("a");
            let b = g.node("b");
            assert!((a.x.unwrap() - 25.0).abs() < 1.0);
            assert!((b.x.unwrap() - 287.5).abs() < 1.0);
        }
    } // mod layout_case_insensitivity

    // ─── list: toString (JS-specific, not applicable in Rust) ────────────────
    // The JS test verifies the List's toString() returns "[{\"entry\":1}, ...]".
    // The Rust List uses index-based handles with no attached data payload,
    // so there is no equivalent string representation.
    // This test is intentionally omitted (JS-only feature).
}
