# 0014: Use One Pinned Rust Toolchain

Date: 2026-07-14

## Decision

AntennaBench supports one exact Rust toolchain for development, continuous
integration, and release builds. The workspace `rust-version`,
`rust-toolchain.toml`, and Mise pin agree on Rust 1.96.1, and CI checks that
invariant before running the full quality suite.

The project does not maintain a separate older minimum-supported-Rust-version
compatibility floor. Its workspace crates are internal application components,
not a published library compatibility surface, and the project controls the
build and release environment.

## Context

Issue #42 corrected a false Rust 1.78 compatibility claim by measuring the
locked dependency graph, raising the declaration to Rust 1.89, pinning routine
work to Rust 1.96.1, and adding a separate Rust 1.89 CI job. That made the
metadata truthful, but it also created a continuing compatibility promise that
the application did not need.

Maintaining two compilers makes dependency updates and required-check policy
more complex without improving the installed application's compatibility.
Reproducible compiler selection, consistent local/CI/release inputs, and
intentional compiler upgrades remain valuable.

## Consequences

- Compiler updates change the workspace declaration, `rust-toolchain.toml`,
  and Mise together in one focused, reviewed change.
- Dependency updates record any new compiler requirement and update the single
  pin when appropriate instead of preserving an arbitrary older floor.
- CI no longer installs or checks Rust 1.89 separately.
- Release manifests still record the exact compiler used for each artifact.
- Publishing any workspace crate as a supported Rust library requires a new
  compatibility decision before publication.

## References

- [Truthful compatibility issue #42](https://github.com/rwjblue/antennabench/issues/42)
- [Development workflow](../development.md)
- [Release contract](0007-ship-separate-signed-macos-release-archives.md)
- [Supply-chain policy](0012-use-combined-supply-chain-maintenance-gates.md)
