# Work Tracking

GitHub Issues are AntennaBench's durable source of truth for unfinished work
and open decisions. This guide explains how a human should read and maintain
that work. [Decision 0003](decisions/0003-github-issues-are-the-durable-work-tracker.md)
records the durable rationale, while the
[Development Technical Reference](development-reference.md) defines the agent
execution rules.

## Tracking Layers

Each tracking layer answers a different question:

- The [roadmap](roadmap.md) says which product outcomes are current, next, and
  later. It is not a task list.
- A GitHub milestone groups issues that deliver one recognizable outcome and
  exposes aggregate progress.
- A tracking issue explains the dependency graph, focused child issues, and
  exit criteria for a milestone or major sub-track.
- A focused implementation, decision, owner-action, or human-validation issue
  owns one bounded outcome and its acceptance evidence.
- An ADR records a durable decision after the choice is settled. It does not
  track unfinished implementation.

An issue belongs to at most one milestone. Cross-cutting dependencies remain
linked from issue bodies and tracking checklists. Unmilestoned issues form the
later backlog; there is no catch-all backlog milestone.

## Milestones

The maintained milestones are outcome-oriented and have no due date unless a
real scheduling commitment exists:

- `Local Conductor`: validated setup, manual/no-rig operation, bounded live
  WSJT-X ingestion, coherent reports and exports, and deterministic proof.
- `Trustworthy macOS Release`: repeatable artifacts, signing and notarization,
  supply-chain gates, repository protections, and release verification.
- `Field Validation`: feedback policy, report comprehension, maintainer field
  alpha, and external operator beta.
- `Optional Hosted Sharing`: the optional hosted application, identity,
  admission, processing, lifecycle, public serving, desktop handoff, and
  moderation boundary.

Milestones describe outcomes rather than priority. The roadmap and tracker
dependency order determine which milestone receives attention now.

## Labels And Lifecycle

The project uses a small label vocabulary:

| Label | Meaning |
| --- | --- |
| `enhancement` | Product, infrastructure, documentation, or validation work. |
| `decision` | A choice must be researched and recorded before its result is known. |
| `tracking` | A parent map; implementation belongs in focused child issues. |
| `agent-ready` | The contract is bounded and every blocking dependency has landed; the issue can be handed to an agent now. |
| `in-progress` | An agent or maintainer actively owns the issue. |
| `needs-decision` | Active work reached a product or architecture choice that its contract does not delegate. |
| `human-required` | Completion requires owner action, credentials, human judgment, or real participant evidence. |

The normal implementation lifecycle is:

```text
planned -> agent-ready -> in-progress -> landed and closed
               |              |
               |              +-> needs-decision
               |
               +-> explicit handoff authorizes implementation
```

`agent-ready` is readiness, not authorization. A user must explicitly hand the
issue to an agent. A well-written issue with an open blocking dependency remains
planned and must not carry `agent-ready`.

Tracking issues are normally never agent-ready. Human-required issues may
contain preparatory work suitable for an explicit agent handoff, but generated
artifacts cannot replace the human evidence required for completion. Create a
focused agent-ready preparation issue when that work is substantial.

## Dependency Maintenance

Before beginning an issue, confirm that every `Depends on` dependency has
landed. Replace `agent-ready` with `in-progress` only after that check.

After landing and closing an issue:

1. post the completion evidence required by its contract;
2. update its parent tracking issue;
3. inspect open issues that name it as a dependency;
4. apply `agent-ready` when all remaining blockers have landed and the issue is
   otherwise bounded and verifiable; and
5. remove stale `agent-ready` whenever an unmet blocking dependency is found.

A local-only commit does not satisfy a GitHub dependency. Leave its issue open
until the change lands.

## Human-Required Work

Use `human-required` when no unattended agent can truthfully provide the final
evidence. Examples include repository settings, signing credentials and release
approval, product-owner decisions, report-comprehension interviews, and real
operator field sessions.

The issue must separate work an agent may prepare from evidence a human must
provide. Sensitive callsigns, grids, station details, schedules, bundles,
reports, credentials, or participant notes do not belong in public issue bodies
unless explicitly sanitized and approved.

## Completion Evidence

Implementation issues close only after their work lands. Their completion
record includes:

- delivered behavior;
- Jujutsu change or commit and, when applicable, pull request;
- verification commands and results;
- maintained documentation updates;
- follow-up issues and tracker changes; and
- any explicitly deferred or blocked behavior.

Decision issues record the selected option, alternatives, rationale,
consequences, ADR when warranted, and implementation follow-ups. Human-required
issues record the owner action or participant evidence in the aggregate without
exposing secrets or personal data.

## Useful Queries

These GitHub issue searches answer the most common maintenance questions:

```text
is:issue is:open label:agent-ready -label:in-progress
is:issue is:open label:in-progress
is:issue is:open label:needs-decision
is:issue is:open label:human-required
is:issue is:open milestone:"Local Conductor"
is:issue is:open no:milestone
```

The first query is the executable queue. If it contains work with an open
blocking dependency, fix the labels before handing out more work.

## Creating Work

Use the repository templates:

- `Planned implementation` for a bounded future slice whose dependencies are
  still open;
- `Agent-ready implementation` for executable implementation slices;
- `Agent-ready technical decision` for bounded research an agent may resolve;
- `Product or owner decision` for choices requiring human judgment;
- `Tracking issue` for dependency and exit-criteria maps; and
- `Human validation` for participant or real-world evidence.

Newly discovered implementation work gets a focused issue rather than being
silently added to an active issue. Material public behavior, durable schema, or
architecture expansion requires user direction before the issue contract is
changed.
