# 0002: Superpowers Docs Are Ephemeral

Date: 2026-07-10

## Decision

`docs/superpowers/` is local agent planning output, not maintained project
documentation.

Evergreen project docs live directly under `docs/`, and stable decisions live
under `docs/decisions/`. GitHub Issues are the durable work tracker as recorded
in [0003](0003-github-issues-are-the-durable-work-tracker.md).

## Context

Agent-generated specs and implementation plans are useful while work is in
progress, but they become stale quickly. Keeping them tracked made it easy to
mistake dated execution notes for the current source of truth.

## Consequences

- `docs/superpowers/` is ignored by git.
- A local plan must not be the only durable record of unfinished work or an open
  implementation decision.
- Before plan execution, the approved outcome, scope, non-goals, and acceptance
  criteria should exist in a GitHub issue.
- Agents should update evergreen docs when behavior changes.
- Agents should record completion evidence on the corresponding issue before it
  is closed.
- Tests, fixtures, and public APIs remain authoritative for implemented
  behavior when docs and code disagree.
