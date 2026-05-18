# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-18

### Added
- Initial release — faithful Rust port of [dagre-js](https://github.com/dagrejs/dagre)
- Full layout pipeline: acyclic, rank (network simplex), nesting graph, normalize, order (crossing minimisation), position (Brandes-Köpf), edge routing
- `Graph` compound/directed/multigraph data structure ported from [@dagrejs/graphlib](https://github.com/dagrejs/graphlib)
- Support for all layout directions: `TB`, `BT`, `LR`, `RL`
- Support for compound (nested) graphs
- Support for multigraphs (multiple edges between same node pair)
- 298 unit tests ported from the dagre-js test suite (93.45% line coverage)
- Full public API documentation
- MIT licence

[Unreleased]: https://github.com/rinfimate/dagre-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rinfimate/dagre-rs/releases/tag/v0.1.0
