# AntennaBench Documentation

Choose the section that matches why you are here. Operator guides explain the
product without requiring repository knowledge. Contributor and maintainer pages
are intentionally technical.

## Use Or Evaluate AntennaBench

- [Your First Antenna Comparison](quickstart.md) is a start-to-finish walkthrough
  of one manual WSPR session and its three export choices.
- [How To Read Your AntennaBench Report](reading-your-report.md) explains each
  report section, empty state, descriptive number, and audit boundary.
- [How AntennaBench Works](product.md) walks through planning, running, and
  reviewing an antenna experiment.
- [Session Bundles](bundle-format.md) explains the portable experiment record and
  when to export a bundle versus an HTML report.
- [Operator Glossary](glossary.md) defines the canonical terms used by the app,
  reports, and operator guides.
- [Why Not Just Use The Reverse Beacon Network?](why-not-just-use-rbn.md) compares
  WSPR and RBN as antenna-testing tools, with current receiver-population data.
- [Roadmap](roadmap.md) summarizes what works today, what is next, and what remains
  exploratory.
- [Local Antenna Controller Profiles](antenna-controller-profiles.md) is an
  advanced operator guide for optional local switch and verification programs.
- [Field Testing And Feedback](field-testing.md) explains how to report public
  or private findings without requiring station or session evidence.
- [Examples](../examples/README.md) contains reviewed starting points for optional
  station integrations.

## Contribute To The Project

- [Development Guide](development.md) covers setup, routine commands, repository
  layout, and contribution expectations.
- [Architecture Overview](architecture.md) explains the major components and the
  boundaries that protect experiment evidence.
- [Architecture Technical Reference](architecture-reference.md) records
  crate-level responsibilities, APIs, data flow, and trust boundaries.
- [Bundle Format Technical Reference](bundle-format-reference.md) specifies the
  on-disk formats, upgrades, validation rules, and resource limits.
- [Operator Event Semantics](event-semantics.md) defines lifecycle, corrections,
  actual state, and observation eligibility.
- [Live Persistence And Recovery](live-persistence.md) defines checkpointing,
  locking, snapshots, export, and crash recovery.
- [Product Design Reference](product-design-reference.md) records detailed product
  invariants and selected future direction.

## Internal Maintainer And Automation Guides

> [!NOTE]
> The pages in this section are repository operations material. They are not
> end-user documentation. Agent-specific authority and execution rules are
> scoped to [AGENTS.md](../AGENTS.md) and the work-tracking references below.

- [Internal Work Tracking And Agent Handoffs](work-tracking.md) defines issue
  layers, labels, authorization boundaries, dependencies, and completion
  evidence.
- [Development Technical Reference](development-reference.md) records repository
  policy, verification coverage, CI behavior, desktop internals, and agent
  execution conventions.
- [Desktop Releases](releasing.md) is the owner runbook for signing, notarization,
  draft verification, promotion, recovery, and credentials.
- [Supply-Chain Updates](supply-chain.md) is the dependency, tool, Action, runner,
  and exception-maintenance procedure.
- [Hosted Site And Foundation Operations](hosted-operations.md) covers static
  site deployment, domain and rollback verification, and the separate
  admission-disabled hosted prototype.

## Historical And Legal References

- [Architecture Decisions](decisions/) preserve the reasoning behind durable
  product and engineering choices. They are historical records, not tutorials or
  work trackers.
- [Attribution And External Data](attribution.md) records third-party data and
  service attribution plus fixture redistribution policy.

Code, tests, fixtures, and public APIs are authoritative when maintained
documentation and implemented behavior disagree. GitHub Issues track unfinished
work and open decisions. Ignored `docs/superpowers/` files are temporary agent
planning artifacts and are not project documentation.
