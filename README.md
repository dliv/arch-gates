# arch-gates

Shared source checks for repository-owned Rust architecture policy. The first
slice extracts Workforest's Pure scanner; Workforest and Slopid own file selection,
policy, filesystem/process observation, native compiler controls and gate wiring.

`check_pure_source(file, source)` returns contextual `PureViolation` values.
`pure_violations(source)` returns sorted, deduplicated forbidden source forms.
Both are deterministic: no filesystem, environment, clock, network or process reads.
Use this crate as a dev-dependency of a local policy crate and invoke it from that
crate's tests. Its own tests are not automatically run by a dependent product.

Run `just check` for formatting, all-target Clippy and locked tests. The tests
carry Workforest's ordinary alias, import, path-effect, raw-identifier, literal,
macro and test-module controls, plus the contextual API contract.

The scanner flags listed runtime std effects, standard input, named filesystem
path methods, scan escapes and non-ASCII identifiers. It skips recognized inline
`cfg(test)` modules and allows fixed build inputs such as `include_str!` and
`env!`. It does not expand macros, resolve types, scan dependency internals or
prove transitive purity. Same-named path methods may be reported on other types.
The graph module adds source coverage and crate-local import direction checks.
Root profiles, package dependency graphs and publication remain outside this slice. The private tokenizer is retained from Workforest;
unmigrated donor checks remain in that product until separately transferred.

This checkout currently has no published remote. Consumer pins to its local Git
revision are a machine-local development checkpoint, not portable dependencies.

## Source coverage and direction

`graph::check_coverage(discovered, classified)` compares a consumer-observed set
of Rust source paths with its policy list, reporting missing, stale, duplicate
or malformed entries. Consumers discover source roots from their workspace and
own filesystem errors. `graph::module_id` interprets conventional `src` paths.

`graph::crate_edges` discovers crate/self/super paths with the donor's tested
grammar. `graph::check_direction` applies a consumer-supplied predicate and exact
`Edge` exceptions, reporting forbidden edges and stale/duplicate allowances.
No product role matrix or filesystem/process observation is in this module.
Bare child names, edges into a module's own children and macro expansions are
not followed. Unknown paths fall back to the crate root, as in the donor.
Graph discovery does not prove a complete semantic import graph or absence of
cycles. Tests cover multi-crate identities, ordinary scanner forms, valid policy
and every inventory/exception rejection class.
