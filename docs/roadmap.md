# Roadmap

Last reviewed: 2026-07-14

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
- validated schema-v2 setup/bundle creation plus a complete manual/no-rig
  conductor with durable lifecycle, explicit actual-antenna evidence,
  append-only corrections, trusted schedule guidance, and restart recovery
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

## Next Local Product Track

The next product milestone is the validated local setup and live-run conductor
tracked by [#45](https://github.com/rwjblue/antennabench/issues/45). The
validation, mutation/recovery, and bounded-resource policies are settled by
[Decision 0009](decisions/0009-use-layered-bundle-validation-profiles.md),
[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md),
and
[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md),
validated setup, checkpointed bundle creation, and the complete manual/no-rig
conductor are now shipped. Optional live adapter orchestration remains planned.

The dependency-ordered implementation path is:

1. Establish schema-v2, layered validation, strict write preflight, and bounded
   storage (#46, #50, #51, and #55).
2. Checkpointed persistence/recovery and the lifecycle/correction reducers are
   implemented (#53 and #54).
3. Validated setup/bundle creation and the complete manual/no-rig conductor are
   implemented (#61 and #62).
4. Bound adapter ingress and orchestrate live WSJT-X evidence (#56 and #63).
5. Add granular evidence eligibility, bounded report/IPC behavior, and coherent
   live/final report refresh and export (#52, #57, and #64). The same-session
   presentation fix is tracked by #41.
6. Prove the workflow with deterministic unattended end-to-end coverage (#65).

Optional rig control, public spots, hosted sharing, and stronger comparative
conclusions remain outside this milestone. Manual/no-rig operation must be
complete before optional integrations expand the workflow.

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
