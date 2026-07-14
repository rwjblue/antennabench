# AntennaBench

AntennaBench is a local-first antenna comparison and profiling app for WSPR
experiments.

The project is currently building the bundle-first Rust foundation and local
report data model. The durable source of truth is a portable session bundle
made from JSON and JSONL files. SQLite indexes, UI state, generated reports,
charts, and hosted publishing artifacts are derived from that bundle.

## Current Status

Implemented:

- Rust workspace with core, storage, WSJT-X import, analysis, and report library
  crates.
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
  and insufficient/weak/moderate evidence-coverage labels.
- Deterministic, in-memory report data derived from one bundle, with session
  context, conservative evidence sections, typed notices, and concrete
  renderer-neutral rows for antenna SNR, band evidence, and slot evidence.
- Standalone offline HTML report rendering with embedded styling, accessible
  chart tables, restrictive content security policy, and hostile-text escaping.
- A Tauri desktop shell that opens existing directory bundles, validates and
  analyzes them without mutation, shows the local report in an isolated frame,
  and exports verified lossless copies through narrow Rust commands.
- An unattended desktop integration path that verifies canonical open, report,
  lossless export, cancellation, typed failure, and reopen behavior without
  launching a window or taking foreground input.
- Fixture coverage for minimal, WSJT-X import-hardening, and balanced
  analysis-rich whole-station sessions.

Not implemented yet:

- WSJT-X live UDP adapter.
- Rig control.
- Public spot fetching.
- Winner selection and advanced statistical analysis.
- Markdown, PDF, and image rendering.
- Hosted report viewing or publishing.

## Documentation

Evergreen project docs live in [docs/](docs/README.md). Start there for the
product shape, architecture, bundle format, development workflow, roadmap, and
project decisions.

Agent planning files under `docs/superpowers/` are intentionally ignored. They
are useful working notes while agents execute tasks, but they are not maintained
as project documentation.
