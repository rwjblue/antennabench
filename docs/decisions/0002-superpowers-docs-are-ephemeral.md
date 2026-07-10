# 0002: Superpowers Docs Are Ephemeral

Date: 2026-07-10

## Decision

`docs/superpowers/` is local agent planning output, not maintained project
documentation.

Evergreen project docs live directly under `docs/`, and stable decisions live
under `docs/decisions/`.

## Context

Agent-generated specs and implementation plans are useful while work is in
progress, but they become stale quickly. Keeping them tracked made it easy to
mistake dated execution notes for the current source of truth.

## Consequences

- `docs/superpowers/` is ignored by git.
- Agents should update evergreen docs when behavior changes.
- Tests, fixtures, and public APIs remain authoritative for implemented
  behavior when docs and code disagree.
