# AntennaBench Agent Instructions

jj-commit-default: auto

## Project Context

- AntennaBench is a local-first antenna comparison and profiling app for WSPR
  experiments.
- The durable source of truth is the session bundle: JSON and JSONL files for
  station details, antennas, schedules, operator events, observations, adapter
  inputs, propagation snapshots, and analysis metadata.
- SQLite indexes, UI state, generated reports, charts, and hosted publishing
  artifacts are derived from the session bundle.

## Planning

- Evergreen project docs live under `docs/` and are the maintained source of
  truth for architecture, bundle format, development workflow, and roadmap.
- `docs/superpowers/` contains temporary agent planning artifacts. These files
  may be useful while work is in progress, but they are not authoritative and
  should not be treated as current documentation.
- Do not implement from a plan until the user approves it.
- When implementation changes project behavior, update the relevant evergreen
  doc in the same change when practical.
- Prefer documenting stable concepts and invariants over copying detailed plan
  steps or generated code.
- Tests, fixtures, and public APIs are the source of truth for implemented
  behavior when docs and code disagree.

## Rust Conventions

- Prefer workspace-managed dependency versions: define shared third-party
  dependencies in the root `Cargo.toml` and reference them from member crates
  with `workspace = true`.
- Prefer `thiserror` for library crate error types.
- Prefer `anyhow` for application code, CLIs, test harnesses, and other
  top-level orchestration where typed public errors are not part of the API.
- Prefer `insta` for snapshot tests, using inline snapshots.

## Validation

- Before declaring Rust changes complete, run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
- For documentation-only changes, inspect the rendered intent and verify the
  working-copy diff is limited to the requested files.
