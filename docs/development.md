# Development

This repo uses Rust, Cargo, and Jujutsu (`jj`).

## Version Control

Use `jj` workflows. Do not create worktrees unless explicitly requested.

Project-local instructions live in `AGENTS.md`. Future agent planning artifacts
under `docs/superpowers/` are ignored and should remain local unless a user
explicitly asks to preserve them elsewhere.

## Work Tracking

GitHub Issues are the durable source of truth for unfinished work and open
implementation decisions. The roadmap describes direction; issues define
focused outcomes, scope, non-goals, and acceptance criteria.

An issue is agent-ready when its outcome is unambiguous, blocking decisions are
resolved or explicitly delegated, dependencies are identified, and its
acceptance criteria are objectively verifiable. The `agent-ready` label means
the issue can be handed to an agent; it does not authorize implementation by
itself. Explicitly instructing an agent to implement the issue approves that
issue's scope.

Agents may create ignored detailed plans while executing an approved issue. A
local plan must not be the only record of unfinished work. Material expansion
of public behavior, durable schemas, or architectural scope requires user
direction rather than an implicit issue rewrite.

Before closing an implementation issue, record the delivered behavior, the
Jujutsu change or commit, verification results, documentation updates, and any
follow-up issues discovered. Use the templates under `.github/ISSUE_TEMPLATE/`
for agent-ready implementation and decision work.

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
