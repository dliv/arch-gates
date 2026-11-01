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
Root profiles, module/dependency graphs, policy inventories and publication are
outside this first slice. The private tokenizer is retained from Workforest;
unmigrated donor checks remain in that product until separately transferred.

This checkout currently has no published remote. Consumer pins to its local Git
revision are a machine-local development checkpoint, not portable dependencies.
