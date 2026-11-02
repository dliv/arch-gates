# arch-gates

Shared source, graph, metadata and package checks for repository-owned Rust
architecture policy. Workforest and Slopid own file selection, policy,
filesystem/process observation, native compiler controls and gate wiring.

`check_pure_source(file, source)` returns contextual `PureViolation` values.
`pure_violations(source)` returns sorted, deduplicated forbidden source forms.
`scan::root_ambient_violations(source, allowed_env)` checks the common Root
profile and takes exact permitted `std::env` item names from the caller.
`scan::external_crate_references(source, dependencies)` and
`scan::trait_declarations(source)` discover production references and trait names.
These functions are deterministic over supplied source and policy data: the
engine opens no files, reads no environment or clock, and starts no process.
Use this crate as a dev-dependency of a local policy crate and invoke it from that
crate's tests. Its own tests are not automatically run by a dependent product.

Run `just check` for formatting, all-target Clippy and locked tests. The tests
cover ordinary aliases, grouped imports, path effects, raw identifiers, literals,
macros, test-module skips, exact Root environment allowances, external references
and trait declarations, plus the contextual Pure API contract.

The Pure scan flags its listed runtime `std` effects, standard input, named
filesystem path methods, source escapes and non-ASCII identifiers. The Root scan
flags filesystem, environment, network and thread paths plus the shared source
escapes; the caller supplies its exact allowed environment item names, with no
product defaults. Neither profile enforces Clippy-owned process, clock or stream
controls. Scans skip recognized inline `cfg(test)` modules and allow fixed build
inputs such as `include_str!` and `env!`. They do not expand macros, resolve types,
scan dependency internals or prove transitive purity. Same-named path methods may
be reported on other types. One private tokenizer and production-token filter
serve the Pure, Root, external-reference and trait checks.

The graph module adds source coverage and crate-local import direction checks.
Consumers keep the native compiler controls, policy data and local adapter code.

The canonical Git source is [dliv/arch-gates](https://github.com/dliv/arch-gates).
Consumers pin a full immutable Git revision over HTTPS and lock it in Cargo.lock.
They do not need an adjacent engine checkout or a local path override. Each
consumer qualifies its assembled gate independently; this crate's tests do not
establish another product's policy or platform coverage.

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

## Metadata and package dependencies

`metadata::parse_members` parses caller-supplied Cargo format-1 JSON and selects
explicit workspace IDs, rejecting missing/duplicate IDs and malformed records.
Each listed member accepts caller-read manifest text through `with_manifest`.
The engine never runs Cargo or opens those paths. `manifest` retains the donor's
bounded dependency-name/lint-inheritance readers and their spelling limits.

`packages::check_edges` evaluates supplied package classes and dependency lists,
external allowances and Pure feature pins. Consumers supply internal-edge
permission/reason callbacks and retain their layout checks. Missing/duplicate/
stale classifications, nonmember path dependencies, missing lint inheritance,
Pure build scripts, unapproved dependencies/features and stale Effectful/Edge
allowances fail. Declared/forwarded features are checked, not resolved transitive
capabilities or default feature safety.

An explicit forbidden-tooling package list is checked by package name even
through dependency renames and across normal/build/dev kinds. Production cannot
depend on those packages; Tooling may use them only as dev-dependencies. Local
workspace Tooling classes are also forbidden dependency targets. The real
consumer Cargo adapters and the native compiler inventories remain local.
