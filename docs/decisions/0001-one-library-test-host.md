# ADR 0001: One library with an integration-test policy host

## Context

`arch-gates` evaluates source, module, metadata and package facts for callers
that own filesystem and Cargo observation and choose their own rules. These
checks share one deterministic responsibility. Splitting them into empty
effect, edge or application packages would add build boundaries without
moving an actual runtime responsibility. The engine also needs to check its
own production source and package declarations without adding I/O to that
library.

## Decision

Keep one production library for architecture-validation decisions. Keep the
consumer's Tooling-use boundary separate from the algorithm's Pure behavior.
Compile `tests/self_policy.rs` as a distinct integration-test policy host; it
owns actual source enumeration, file reads, explicit local policy, and the
`cargo metadata` observation needed to evaluate the real package graph through
the public library APIs.

The host fails closed on missing or unreadable source, unclassified files,
Cargo command or parse errors, stale source classifications or external
allowances, forbidden module directions, and package-policy violations.

## Consequences

- Production source remains independent of process and filesystem effects.
- The integration test proves the policy against the checked-out source and
  metadata, while the existing focused tests separately exercise shared
  evaluators and bounded scanner controls.
- The self-policy shares implementation with the library under evaluation;
  it is not independent semantic verification. Consumer adapters and assembled
  gates still need their own proof.
- Declared dependency features do not establish default-feature,
  feature-unification or transitive capability safety. Source and module scans
  remain bounded and are not a security boundary.

Reconsider this shape when the engine acquires a real effectful production
workflow or independently owned protocol boundary, or when a second production
responsibility needs a separately enforced build boundary.
