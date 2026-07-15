# AntennaBench Documentation

Start with the page that matches what you are trying to do.

## Understand The Product

- [Product overview](product.md) explains the experiment workflow, local-first
  approach, and why reports stay conservative.
- [Session bundles](bundle-format.md) gives a short tour of the portable record
  AntennaBench keeps for every experiment.
- [Roadmap](roadmap.md) summarizes what is available now and what comes next.

## Build Or Contribute

- [Development](development.md) covers local setup, common commands, and
  repository conventions.
- [Architecture](architecture.md) explains the system shape, major components,
  trust boundaries, and derived-state model.
- [Desktop releases](releasing.md) covers protected signing, draft
  verification, installation, promotion, recovery, and credentials.
- [Work tracking](work-tracking.md) describes milestones, issues, labels,
  dependencies, and completion evidence.

## Technical References

- [Product design reference](product-design-reference.md) records detailed
  evidence rules, operational boundaries, and selected future direction.
- [Architecture technical reference](architecture-reference.md) records
  crate-level APIs, data flow, adapters, and trust boundaries.
- [Development technical reference](development-reference.md) records coding
  policy, verification coverage, CI, desktop internals, and release details.
- [Bundle format technical reference](bundle-format-reference.md) specifies
  layouts, records, upgrades, validation, limits, and storage APIs.
- [Operator event semantics](event-semantics.md) defines lifecycle,
  corrections, actual state, and conservative observation alignment.
- [Live persistence and recovery](live-persistence.md) defines checkpointing,
  locking, snapshots, export, and crash recovery.
- [Hosted foundation operations](hosted-operations.md) covers environment
  isolation, verification, cost controls, drain, and teardown.
- [Attribution](attribution.md) records external data and service attribution
  plus fixture redistribution policy.
- [Supply-chain updates](supply-chain.md) documents dependency, Action, tool,
  and runner review.
- [Architecture decisions](decisions/) preserve the reasoning behind durable
  product and engineering choices.

Code, tests, fixtures, and public APIs are authoritative when documentation and
implemented behavior disagree. GitHub Issues are the durable source of truth
for unfinished work and open decisions; ignored `docs/superpowers/` files are
temporary planning notes.
