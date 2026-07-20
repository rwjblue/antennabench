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
- GitHub Issues are the durable source of truth for unfinished work and open
  implementation decisions.
- `docs/superpowers/` contains temporary agent planning artifacts. These files
  may be useful while work is in progress, but they are not authoritative and
  should not be treated as current documentation or the only record of planned
  work.
- Do not implement from a plan until the user approves it.
- An explicitly handed-off, agent-ready GitHub issue is approved implementation
  scope. The `agent-ready` label alone does not authorize work to begin.
- Stop and request direction before materially expanding an approved issue's
  public behavior, durable schema, or architectural scope.
- When implementation changes project behavior, update the relevant evergreen
  doc in the same change when practical.
- Prefer documenting stable concepts and invariants over copying detailed plan
  steps or generated code.
- Tests, fixtures, and public APIs are the source of truth for implemented
  behavior when docs and code disagree.

## Issue Workflow

When explicitly handed a GitHub issue:

- Treat the issue as approved scope and confirm every blocking dependency has
  landed. `agent-ready` means the issue is executable now, not merely well
  specified.
- Replace `agent-ready` with `in-progress` and inspect the current checkout
  before planning or implementing.
- Stay within the issue contract; request direction for material expansion.
- Run the required verification and land the work before completion.
- Post completion evidence, close the issue, and update its tracking issue.
- After landing an issue, review the open issues it unblocks. Apply
  `agent-ready` only when their remaining blocking dependencies have landed and
  their contracts otherwise meet the readiness definition. Remove a stale
  `agent-ready` label whenever an unmet blocking dependency is discovered.
- If blocked by a product or architecture choice, apply `needs-decision`,
  explain the blocker, and leave the issue open.
- Treat `human-required` as a completion boundary: agents may prepare explicitly
  handed-off artifacts, but may not substitute generated evidence for owner
  action, credentials, human judgment, or real participant observations.

## Rust Conventions

- Prefer workspace-managed dependency versions: define shared third-party
  dependencies in the root `Cargo.toml` and reference them from member crates
  with `workspace = true`.
- Prefer `thiserror` for library crate error types.
- Prefer `anyhow` for application code, CLIs, test harnesses, and other
  top-level orchestration where typed public errors are not part of the API.
- Prefer `insta` for snapshot tests, using inline snapshots.

## Validation

- Match verification to the change; run the full gate only before landing:
  - Rust changes: `mise run check` (formatting, Clippy, workspace tests)
  - Desktop frontend changes: `mise run desktop:frontend-test`
  - Hosted site changes: `mise run hosted:test`
  - Embedded report HTML/CSS changes: also `mise run desktop:report-browser`
  - Before declaring an issue complete or landing work: `mise run ci`
- Use the mise tasks rather than ad-hoc cargo flag combinations: every distinct
  flag set compiles a separate artifact universe under `target/`, so matching
  the task flags keeps rebuilds warm across agent sessions.
- For documentation-only changes, inspect the rendered intent and verify the
  working-copy diff is limited to the requested files.
