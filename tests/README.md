# Tests and policy hosts

`self_policy.rs` is an integration-test composition root, compiled separately
from the production library. Its `self_support/` module owns actual `src/`
discovery, source and manifest reads, explicit source/module/package policy, and
the locked offline `cargo metadata` call. It checks those observed facts through
the same public APIs available to consumers. Missing input and observation
failures fail the test.

The other integration tests exercise scanner forms, coverage, module edges,
metadata parsing and package evaluation using literal fixtures or constructed
facts. Those tests establish evaluator behavior, not another consumer's
filesystem adapter or assembled CI wiring. The fixtures under `fixtures/` are
small planted examples; the actual-source host checks the current production
tree.

See [the architecture guide](../docs/architecture.md) for the policy table,
proof classes and nonclaims. Run `just check` for the local formatter,
all-target Clippy and locked test gate.
