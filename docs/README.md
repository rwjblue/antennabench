# AntennaBench Docs

These are the maintained project docs. They should describe current behavior,
stable intent, and near-term direction without copying dated implementation
plans.

- [Product](product.md): what AntennaBench is for and what v1 is aiming at.
- [Architecture](architecture.md): crates, data flow, and source-of-truth
  boundaries.
- [Bundle Format](bundle-format.md): session bundle files, records, and
  validation rules.
- [Operator Event Semantics](event-semantics.md): schema-v2 lifecycle,
  correction, actual-state, and conservative alignment rules.
- [Schema-V2 Live Persistence And Recovery](live-persistence.md): durable
  append, locking, checkpoint promotion, snapshots, export, and recovery.
- [Development](development.md): local workflow, tests, and repo conventions.
- [Supply-Chain Updates](supply-chain.md): dependency, Action, tool, and runner
  review procedure.
- [Work Tracking](work-tracking.md): how milestones, tracking issues, labels,
  dependencies, human handoffs, and completion evidence fit together.
- [Roadmap](roadmap.md): current, next, and later work.
- [Decisions](decisions/): short records of project decisions that should remain
  easy to find.

## Work Tracking

The roadmap stays intentionally high-level. GitHub Issues are the durable source
of truth for unfinished work and open implementation decisions. Templates for
planned, agent-ready, tracking, decision, and human-validation work live under
`.github/ISSUE_TEMPLATE/`.
See [Work Tracking](work-tracking.md) for the human-facing issue lifecycle,
milestone map, label meanings, and useful backlog queries.

Detailed agent plans under `docs/superpowers/` may elaborate on an approved
issue while work is active, but they are ignored scratch artifacts and must not
be the only record of intended work.

## Source Of Truth

Code, tests, fixtures, and public APIs are authoritative for implemented
behavior. These docs summarize that behavior and should be updated when project
behavior changes.

Agent planning artifacts under `docs/superpowers/` are ignored and ephemeral.
Do not treat them as current documentation or as the durable work tracker.
