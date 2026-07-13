# 0003: GitHub Issues Are The Durable Work Tracker

Date: 2026-07-13

## Decision

GitHub Issues are the durable source of truth for unfinished work and open
implementation decisions.

Evergreen docs continue to describe stable product intent, architecture,
formats, and high-level direction. Detailed agent plans may be created under
`docs/superpowers/` while work is active, but they are ignored scratch artifacts
and are not authoritative.

An implementation issue is agent-ready when it has an unambiguous outcome,
bounded scope and non-goals, resolved or explicitly delegated decisions,
identified dependencies, and objectively verifiable acceptance criteria.
Applying an `agent-ready` label records readiness only; it does not authorize
work to begin. Explicitly handing the issue to an agent approves implementation
within that issue's scope.

The agent may choose internal implementation details that preserve the issue's
public and architectural contract. Material expansion of public behavior,
durable schemas, or architectural scope requires user direction.

## Context

Agent-generated implementation plans are useful for detailed execution, but
they become stale quickly and are intentionally excluded from version control.
Without a durable counterpart, unfinished intent and unresolved decisions can
be lost even when the code and evergreen docs remain accurate.

The project is primarily implemented through agent-driven slices. A focused,
agent-ready issue provides a stable handoff that survives local plan deletion
without requiring a large generated plan to become maintained documentation.

## Consequences

- The roadmap remains high-level and does not become a second task tracker.
- Focused GitHub issues record unfinished outcomes, decisions, dependencies,
  non-goals, and acceptance criteria.
- Local plans may elaborate on an issue but must not silently redefine it.
- An agent should stop for direction when implementation requires material
  scope expansion or a blocking decision not delegated by the issue.
- Before an issue is closed, its completion record should include delivered
  behavior, the Jujutsu change or commit, verification results, documentation
  updates, and follow-up issues.
- Completed historical work does not require retroactive issues. Migration from
  local plans should preserve unfinished intent and unresolved questions only.
- Issue templates and labels are operational conventions and may evolve without
  revising this decision.
