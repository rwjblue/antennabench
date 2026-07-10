# AntennaBench Docs

These are the maintained project docs. They should describe current behavior,
stable intent, and near-term direction without copying dated implementation
plans.

- [Product](product.md): what AntennaBench is for and what v1 is aiming at.
- [Architecture](architecture.md): crates, data flow, and source-of-truth
  boundaries.
- [Bundle Format](bundle-format.md): session bundle files, records, and
  validation rules.
- [Development](development.md): local workflow, tests, and repo conventions.
- [Roadmap](roadmap.md): current, next, and later work.
- [Decisions](decisions/): short records of project decisions that should remain
  easy to find.

## Source Of Truth

Code, tests, fixtures, and public APIs are authoritative for implemented
behavior. These docs summarize that behavior and should be updated when project
behavior changes.

Agent planning artifacts under `docs/superpowers/` are ignored and ephemeral.
Do not treat them as current documentation.
