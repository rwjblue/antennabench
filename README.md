# AntennaBench

AntennaBench is a local-first antenna comparison and profiling app for WSPR
experiments.

The project is currently building the core bundle model and validation
pipeline. The durable source of truth is a portable session bundle made from
JSON and JSONL files. SQLite indexes, UI state, generated reports, charts, and
hosted publishing artifacts are derived from that bundle.

## Current Status

Implemented:

- Rust workspace with core, storage, WSJT-X import, and analysis library crates.
- Canonical bundle model for station, antennas, schedules, operator events,
  observations, adapter records, propagation snapshots, and analysis metadata.
- Filesystem read/write support for `.session.wsprabundle` directories.
- Deterministic schedule slot alignment and observation annotation.
- Strict bundle validation for schema/session drift, duplicate IDs, references,
  slot windows, confidence ranges, and stale alignment annotations.
- Bundle normalization that repairs missing or stale observation slot
  annotations before validation.
- Offline WSJT-X WSPR log import crate for `ALL_WSPR.TXT`-style rows, raw
  `wsjtx.jsonl` preservation, and local decode observation conversion.
- Conservative in-memory A/B evidence summaries with observation eligibility,
  exclusion reasons, per-antenna/band/slot counts, SNR descriptive statistics,
  and insufficient/weak/moderate evidence-quality labels.
- Golden fixture coverage for a minimal whole-station A/B session.

Not implemented yet:

- Desktop UI.
- WSJT-X live UDP adapter.
- Rig control.
- Public spot fetching.
- Winner selection, advanced statistical analysis, and report generation.
- Hosted report viewing or publishing.

## Documentation

Evergreen project docs live in [docs/](docs/README.md). Start there for the
product shape, architecture, bundle format, development workflow, roadmap, and
project decisions.

Agent planning files under `docs/superpowers/` are intentionally ignored. They
are useful working notes while agents execute tasks, but they are not maintained
as project documentation.
