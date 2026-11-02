# Production source

`src/` is the single production library. It evaluates values and source text
supplied by callers and performs no filesystem or process observation.

- `lib.rs` exposes the public API and contextual Pure violations.
- `scan.rs` applies source-level Pure and Root checks and discovers external
  crate references and declared trait names.
- `tokens.rs` is the private bounded lexer and import-tree reader shared by
  source and graph checks.
- `graph.rs` checks source coverage and caller-owned local module direction.
- `manifest.rs` and `metadata.rs` read supplied manifest text and Cargo
  metadata JSON; they do not open or execute either source.
- `packages.rs` evaluates supplied members, classes, dependencies and external
  allowances.

The [architecture guide](../docs/architecture.md) owns the exact dependency
matrix, local policy, evidence and limitations. The separately compiled
integration-test host owns actual filesystem and Cargo observation.
