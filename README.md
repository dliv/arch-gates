# arch-gates

`arch-gates` is one Rust library for deterministic architecture checks over
source, module, Cargo metadata and package facts supplied by its caller. Its
consumer chooses the files and policy, performs local observation, and decides
where the dependency belongs. The library performs no filesystem, process,
environment, clock or network observation.

The public API is grouped by responsibility:

- `check_pure_source` and `pure_violations` check supplied Rust text.
- `scan::root_ambient_violations` checks a consumer's Root profile and receives
  the exact permitted environment names from that consumer.
- `scan::external_crate_references` and `scan::trait_declarations` inspect
  caller-supplied source.
- `graph` checks explicit source inventories and caller-owned module directions.
- `manifest` and `metadata` parse caller-supplied manifest text and Cargo
  metadata JSON.
- `packages::check_edges` evaluates caller-owned package classifications,
  dependencies and external allowances.

Read [the architecture guide](docs/architecture.md) for module ownership,
allowed dependency directions, local policy, proof ownership and limits. The
[source entrypoint](src/README.md) and [test entrypoint](tests/README.md)
navigate the main implementation and proof areas. [The ADR index](docs/decisions/README.md)
records the library and self-policy decisions.

Consumers should normally put `arch-gates` on a tooling or development-only
path and enforce that boundary with their own package policy. The algorithms
being deterministic does not make production use appropriate for every
consumer. This repository's self-policy labels the engine's one production
library as the Pure `architecture-validation` feature; it does not define the
consumer's Tooling-use rule.

Run `just check` for the local formatter, all-target Clippy and locked test
gate. CI runs the same command. The integration host in `tests/self_policy.rs`
checks actual production source and Cargo metadata; its filesystem and Cargo
observation stay in the test target.

The scanner is a bounded token-level check. It does not resolve Rust types or
macros, follow every crate/child/bare path, inspect dependencies transitively,
or prove semantic purity. Same-named path methods can be reported on other
types. See the architecture guide for the source graph, dependency feature and
proof limitations.

The canonical Git source is [dliv/arch-gates](https://github.com/dliv/arch-gates).
Consumers pin a full immutable Git revision over HTTPS and lock it in
`Cargo.lock`; they do not need an adjacent engine checkout or a local path
override. Each consumer qualifies its assembled gate independently.
