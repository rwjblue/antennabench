# 0001: Bundle Is The Source Of Truth

Date: 2026-07-10

## Decision

The portable session bundle is the durable source of truth for AntennaBench
sessions.

SQLite indexes, UI state, generated reports, charts, hosted publishing records,
and other derived artifacts must be rebuildable from the bundle.

## Context

Antenna comparisons need enough raw and near-raw evidence to support future
analysis improvements. If reports or local indexes became canonical, old
sessions would be harder to reanalyze and audit.

## Consequences

- Core types must preserve station, antenna, schedule, event, observation,
  adapter, propagation, and analysis metadata in JSON/JSONL bundle files.
- Storage code should treat bundle I/O as the durable boundary.
- Future reports and hosted rendering should consume validated bundles rather
  than accept arbitrary rendered output.
