# Agent guide

Before changing production source, read [the architecture guide](docs/architecture.md)
and [the source entrypoint](src/README.md). Keep `src/` deterministic over
caller-supplied data; filesystem and Cargo observation belong to the separately
compiled policy host in `tests/self_policy.rs`, not production code.

Before changing tests or fixtures, read [the test entrypoint](tests/README.md).
Run `just check` for the repository gate.
