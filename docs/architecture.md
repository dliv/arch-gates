# Architecture and local policy

## Responsibility and build roles

`arch-gates` has one production responsibility: evaluate caller-supplied Rust
source and package facts against architecture rules chosen by the caller. Its
single production library owns the deterministic checks. The consumer owns
source selection, filesystem and Cargo observation, policy data, native
compiler controls, invocation and gate wiring. Keeping this engine independent
of consumer policy lets each repository choose the scope and rules it can
prove.

The library's Rust modules form one unit with explicit local directions:

| Module | Responsibility | Permitted local direction |
| --- | --- | --- |
| `lib.rs` (`crate`) | Public facade, contextual Pure violations and API exposure | May expose APIs from the other modules |
| `tokens` | Private Rust token and import-tree reader | No other module |
| `manifest` | Bounded readers over caller-supplied manifest text | No other module |
| `metadata` | Parse caller-supplied Cargo metadata and attach manifest lint facts | `manifest` |
| `scan` | Pure ambient-effect, external-reference and trait-name checks | `tokens` |
| `graph` | Pure source coverage, module identity and import-direction checks | `tokens` |
| `packages` | Pure package-class and dependency evaluation | `metadata` |

The actual-source policy in `tests/self_policy.rs` owns the explicit seven-file
Pure source map and this module-direction matrix. Its support module traverses
`src/`, reads source and manifests, and invokes Cargo. These effects belong to
that separately compiled integration-test host; they are not library APIs.

Source and module checks are intentionally layered. The Pure scanner sees
source forms; the module graph sees a bounded set of qualified internal paths.
The root facade may expose any API. The current path reader does not fully
follow root, child, or bare paths, inline module ownership, or macro expansion.
Passing the direction matrix therefore does not prove a complete Rust import
graph.

## Package and source policy homes

The authoritative engine policy is local to the test host:

- `PURE_SOURCES` names every production `.rs` file under `src/`; the host
  discovers the actual tree and `graph::check_coverage` rejects missing,
  stale, duplicate and invalid classifications.
- The external allowance is `serde_json` with no explicitly declared
  features. Actual source references must use the allowance, and every
  allowance must be used.
- `CRATES` contains exactly `arch-gates` as `Pure { feature:
  "architecture-validation" }`. `packages::check_edges` evaluates actual
  workspace members, build-script presence, lint inheritance, internal edges,
  and declared dependency names and features. The host also pins this package's
  full declared dependency set to one normal, non-path `serde_json` dependency
  with no rename or explicitly declared features; this rejects unapproved dev
  and build dependencies too. Empty Effectful and Edge lists are still checked
  for staleness.
- Cargo target facts require one ordinary library rooted at `src/lib.rs`, plus
  integration-test targets. Added binaries (including targets outside `src/`),
  examples, benches, procedural macros or changed library roots require an
  explicit role/source-policy decision; they cannot silently escape the source map.
- Workspace lint configuration in `Cargo.toml` forbids `unsafe` and denies
  unfulfilled lint expectations; the package inherits those lints.

The test host invokes Cargo with an explicit root manifest path:

```sh
cargo metadata --locked --offline --no-deps --format-version 1 --manifest-path Cargo.toml
```

It parses the observed workspace member set with `metadata::parse_members` and
reads each listed manifest before `with_manifest` supplies its lint fact. Read,
enumeration, command, UTF-8 and parse failures fail the integration test.

The library is classified Pure because its production decisions are
deterministic over supplied values. Its algorithms serve architecture tooling,
but the engine cannot enforce where a consumer uses the package: consumers own
their separate Tooling-use boundary and must prevent production packages from
depending on tooling where their local policy requires it.

## Proof and enforcement

`just check` is the local gate for formatting, all-target Clippy and locked
tests. CI invokes that same command. The integration policy host checks actual
source and Cargo metadata. Existing scanner, graph, metadata and package tests
exercise the shared evaluators with literal controls and constructed policy
facts. Consumer repositories must also test their actual adapters and
assembled validation command; engine tests do not prove consumer wiring.

The source scan is bounded token recognition, not Rust parsing or an effect
system. It does not resolve types, expand macros, inspect dependency internals,
or prove transitive purity. Path-name matching may report same-named methods on
unrelated types. The query-name scan exempts unambiguous namespace components,
including components followed by `::`, a brace group, or a glob, and explicit
grouped `self` imports. Ambiguous terminal query names remain fail-closed: for
example, a terminal `metadata` import, value, call or turbofish is still
reported even if its path could name a module. This avoids turning the bounded
name scan into a type or import resolver. The crate-edge reader does not fully
follow root/child/bare paths or prove the complete module graph or absence of
cycles. Declared features are checked as Cargo reports them, but the engine does
not prove `serde_json` default-feature behavior, feature unification or
transitive capabilities. The checks target accidental drift by ordinary code,
not a malicious author. A clean self-check is evidence about these checks and
this source snapshot, not a security boundary or runtime-correctness proof.

## Enforcement and proof classes

| Concern | Class | Evidence/check | Limit |
| --- | --- | --- | --- |
| Production source inventory, Pure forms, explicit module directions, workspace members, declared dependency policy and inherited Rust lints | Gated | `tests/self_policy.rs` invokes `check_pure_source`, `graph::check_coverage`, `graph::check_direction`, `metadata::parse_members` and `packages::check_edges`; `just check` compiles and runs it | Source and import readers are bounded; Cargo checks declared members/dependencies, not dependency internals or complete semantic reachability |
| Observation stays in the separately compiled test host | Checkable | Review `tests/self_policy.rs` and `tests/self_support/mod.rs` for file reads and Cargo invocation; review `src/` for production ownership | The source scan blocks its listed forms but is not a general effect system |
| Consumer Tooling-use boundary | Judgment | Each consumer classifies where it uses the engine and checks that production packages do not depend on tooling under its local rules | This engine has no consumer workspace or deployment graph to inspect |
| Default-feature, feature-unification and transitive `serde_json` capability safety | Judgment | Review the dependency declaration, lock and resolved feature/capability evidence when those claims matter | This gate checks only declared names and explicit features; it does not establish those capabilities |

The classes distinguish enforcement from review and judgment; they do not
upgrade one proof type into another. In particular, the engine's local Pure
classification does not establish a consumer's package-use boundary.
