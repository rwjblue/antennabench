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

- Specs live under `docs/superpowers/specs/`.
- Implementation plans live under `docs/superpowers/plans/`.
- Do not implement from a plan until the user approves it.

## Rust Conventions

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
