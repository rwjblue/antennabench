# AntennaBench

AntennaBench is a local-first antenna comparison and profiling app for WSPR experiments.

The first implementation slice focuses on the durable session bundle: JSON and JSONL files that preserve station details, antennas, schedules, operator events, observations, adapter inputs, propagation snapshots, and analysis metadata. SQLite, UI state, reports, and hosted publishing are derived from the bundle rather than being the source of truth.

First build slice scope:

- Rust workspace foundation.
- Core bundle schema crate.
- Filesystem bundle import/export crate.
- Golden fixture and round-trip tests for a minimal whole-station A/B session.

Planned later slices include the desktop app, WSJT-X companion adapter, rig-control adapters, public spot imports, analysis/report generation, and hosted report viewing.
