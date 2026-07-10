# Development

This repo uses Rust, Cargo, and Jujutsu (`jj`).

## Version Control

Use `jj` workflows. Do not create worktrees unless explicitly requested.

Project-local instructions live in `AGENTS.md`. Future agent planning artifacts
under `docs/superpowers/` are ignored and should remain local unless a user
explicitly asks to preserve them elsewhere.

## Rust Conventions

- Define shared third-party dependencies in the root `Cargo.toml`.
- Reference workspace dependencies from member crates with `workspace = true`.
- Use `thiserror` for library crate error types.
- Use `anyhow` for application code, CLIs, test harnesses, and top-level
  orchestration where typed public errors are not part of the API.
- Prefer `insta` inline snapshots for structured test output.

## Verification

Before declaring Rust behavior complete, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For documentation-only changes, inspect the rendered intent and verify the diff
is limited to the requested files.

## Documentation Updates

When implementation changes project behavior, update the relevant evergreen doc
in the same change when practical.

Prefer documenting stable concepts, invariants, and workflows. Avoid copying
detailed implementation-plan steps or generated code into evergreen docs.
