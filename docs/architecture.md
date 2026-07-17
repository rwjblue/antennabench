# Architecture Overview

AntennaBench is built around one rule: the session bundle is the durable
experiment record. User interfaces, reports, charts, indexes, and optional
hosted copies are projections of that bundle and can be rebuilt.

## System Shape

```text
operator + optional data sources
               |
               v
        desktop application
               |
               v
      session bundle on disk
          /           \
         v             v
 validation +       analysis +
 normalization      local report
         |
         v
 optional explicit hosted copy
```

The desktop frontend presents forms and results. Rust owns filesystem access,
bundle interpretation, validation, clocks and identities, adapter input,
analysis, and report rendering. This keeps browser code from becoming a second
experiment model or receiving general access to the host computer.

## Main Components

- **Core and storage** define bundle versions, validation, normalization,
  upgrades, checkpointed writes, recovery, and verified exports.
- **Adapters** translate WSJT-X, WSPR.live, Reverse Beacon Network, and NOAA
  inputs into attributed evidence without making any source mandatory.
- **Analysis** aligns observations with the schedule and derives conservative,
  descriptive comparison data.
- **Reporting** turns that typed analysis into a deterministic standalone HTML
  document with accessible table equivalents.
- **Desktop** guides setup and conduction, coordinates adapters, and exposes
  reports and exports through narrowly scoped Rust commands.
- **Hosted foundation** is an optional, currently admission-disabled sharing
  boundary. Local sessions do not depend on it.

## Important Boundaries

Planned settings, confirmed operator actions, adapter observations, and public
reports are different fact classes. AntennaBench does not silently substitute
one for another.

Schema-v5 antenna-control evidence follows the same boundary. Portable bundles
may describe command-control policy and retain resolved invocation evidence,
but never contain executable profiles, target mappings, or timeouts that grant
local authority. This foundation performs no process execution. Later local
controller work must attach and arm machine-local configuration explicitly.
Command attempt records and an optional command-verified ready event cross one
storage checkpoint; a failed attempt can remain auditable without advancing
antenna occupancy.

An active session is committed in checkpointed revisions. A report and its
exports use one verified revision, so they cannot accidentally combine files
from different moments in a live run.

External inputs are bounded and attributed. A failing or oversized optional
adapter stops its own intake and records a completeness gap; it does not stop
manual conduction or lossless export.

The hosted system receives a copy only after an explicit publishing action. It
is not synchronization, and hosted state never becomes session evidence.

## Technical Reference

See the [Architecture Technical Reference](architecture-reference.md) for
crate-level APIs, data flow, validation and resource behavior, desktop command
boundaries, adapter contracts, and hosted trust details.
