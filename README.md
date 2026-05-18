# dagre-rs

[![CI](https://github.com/rinfimate/dagre-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/rinfimate/dagre-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dagre-rs.svg)](https://crates.io/crates/dagre-rs)
[![docs.rs](https://docs.rs/dagre-rs/badge.svg)](https://docs.rs/dagre-rs)
[![Coverage](https://codecov.io/gh/rinfimate/dagre-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/rinfimate/dagre-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A faithful Rust port of [dagre-js](https://github.com/dagrejs/dagre) — a directed graph layout library that produces layered, hierarchical graph drawings.

## What it does

dagre-rs takes a directed graph with node sizes and edge constraints, runs a multi-stage layout pipeline, and outputs x/y coordinates for each node and a list of waypoints for each edge. It does not render anything — it is a pure layout engine.

## Layout pipeline

The stages mirror dagre-js exactly:

1. **Acyclic** — reverse edges to make the graph a DAG
2. **Rank** — assign each node to a horizontal layer (network simplex)
3. **Nesting** — handle compound/nested graphs
4. **Normalize** — insert dummy nodes on long edges
5. **Order** — minimise edge crossings within each layer
6. **Position** — assign x/y coordinates (Brandes-Köpf)
7. **Edge routing** — compute waypoints through dummy nodes

## Usage

```rust
use dagre_rs::{Graph, GraphLabel, NodeLabel, EdgeLabel, layout};

let mut g = Graph::with_options(false, true, false);

g.set_graph(GraphLabel {
    rankdir: Some("TB".to_string()),
    nodesep: Some(50.0),
    ranksep: Some(50.0),
    ..Default::default()
});

g.set_node("a", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });
g.set_node("b", NodeLabel { width: 100.0, height: 40.0, ..Default::default() });

g.set_edge("a", "b", EdgeLabel { minlen: Some(1), weight: Some(1.0), ..Default::default() }, None);

layout(&mut g);

let a = g.node("a");
println!("a => x={:.1}, y={:.1}", a.x.unwrap(), a.y.unwrap());
```

## Graph options

`Graph::with_options(multigraph, directed, compound)`:

| Parameter | Description |
|-----------|-------------|
| `multigraph` | Allow multiple edges between the same pair of nodes |
| `directed` | Treat edges as directed (required for layout) |
| `compound` | Enable parent/child node relationships |

## GraphLabel fields

| Field | Default | Description |
|-------|---------|-------------|
| `rankdir` | `"TB"` | Layout direction: `TB`, `BT`, `LR`, `RL` |
| `nodesep` | `50` | Minimum gap between nodes in the same rank |
| `edgesep` | `10` | Minimum gap between edge splines |
| `ranksep` | `50` | Minimum gap between ranks |
| `marginx` | `0` | Graph margin (x) |
| `marginy` | `0` | Graph margin (y) |

## Output

After `layout()`, each node has `x` and `y` set to its centre coordinates, and each edge label has `points: Vec<Point>` containing the waypoints.

## Running tests

The test suite is ported from [dagre-js/test/](https://github.com/dagrejs/dagre/tree/master/test) and covers the rank, network simplex, and layout algorithms.

Run all tests:

```sh
cargo test
```

Run with output visible (useful for debugging):

```sh
cargo test -- --nocapture
```

Run a specific test by name:

```sh
cargo test network_simplex
```

Run tests matching a pattern:

```sh
cargo test rank
```

## Dependencies

- [`petgraph`](https://crates.io/crates/petgraph) — underlying graph data structure
- [`indexmap`](https://crates.io/crates/indexmap) — deterministic iteration order

## License

MIT © 2026 Rochanglien Infimate
