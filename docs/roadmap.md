# Roadmap

Last reviewed: 2026-07-13

## Current

The current implementation focus is the bundle-first Rust foundation:

- canonical bundle model
- filesystem bundle read/write
- schedule slot alignment
- strict validation
- normalization before validation
- offline WSJT-X WSPR log import from `ALL_WSPR.TXT`-style logs
- live WSJT-X schema 2/3 UDP heartbeat, status, and WSPR decode ingestion with
  auditable raw datagrams and conservative observation production
- in-memory conservative A/B summaries with descriptive SNR statistics and
  insufficient/weak/moderate evidence-coverage labels
- deterministic, renderer-neutral report data with session context,
  conservative evidence sections, typed notices, and concrete chart-ready rows
- deterministic standalone local HTML rendering with embedded visualizations,
  table fallbacks, escaped report text, and no network-loaded runtime
- a minimal Tauri 2 desktop shell that opens local bundles, renders their report,
  and exports verified lossless copies through narrowly allowlisted commands
- a realistic, purpose-built synthetic canonical sample-report session spanning
  two antennas, two bands, usable and excluded evidence, and missing data
- golden fixture coverage
- a durable paired-analysis contract that permits stratified descriptive
  comparisons while deferring uncertainty and automated conclusions
- paired descriptive comparison data with explicit availability, overlap,
  missingness, order, duplicate/conflict, repeated-path, and stratum facts
- standalone paired diagnostics for data quality, differences, SNR over time,
  and distance/azimuth path context with accessible table equivalents
- optional NOAA SWPC F10.7 and provisional estimated-Kp acquisition with pure
  captured-fixture parsing, source envelopes, freshness and polling policy,
  typed best-effort failures, and duplicate suppression

## Later

Later tracks:

- A rebuildable local SQLite/index boundary
  ([#7](https://github.com/rwjblue/antennabench/issues/7)).
- Calibrated uncertainty and comparative conclusion semantics, after paired
  data and suitable design evidence exist
  ([#26](https://github.com/rwjblue/antennabench/issues/26)).
- Optional rig observation or control
  ([#14](https://github.com/rwjblue/antennabench/issues/14)).
- Public spot source and polling integration
  ([#13](https://github.com/rwjblue/antennabench/issues/13)).
- Safe hosted report viewing, upload validation, identity, and publishing
  ([#10](https://github.com/rwjblue/antennabench/issues/10)).
- Native WSPR or mobile-specific operation.
- Public discovery and callsign-oriented browsing.

The roadmap should stay high-level. Detailed task plans belong in local agent
planning artifacts or focused implementation issues, not in evergreen docs.
