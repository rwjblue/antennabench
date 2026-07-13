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
- in-memory conservative A/B summaries with descriptive SNR statistics and
  insufficient/weak/moderate evidence-quality labels
- deterministic, renderer-neutral report data with session context,
  conservative evidence sections, typed notices, and concrete chart-ready rows
- golden fixture coverage

## Next

Likely next slices:

- WSJT-X live UDP status/decode companion path.
- More realistic bundle fixtures from sample WSPR sessions.
- Desktop workflow skeleton around session setup, run prompts, import/export,
  and local report viewing.

## Later

Later tracks:

- Advanced statistics and winner-selection methods.
- HTML, Markdown, PDF, image, and chart rendering.
- Rig-control adapters.
- Public spot source adapters.
- Hosted report viewer and upload validation.
- Account and publishing flow.
- Native WSPR or mobile-specific operation.
- Public discovery and callsign-oriented browsing.

The roadmap should stay high-level. Detailed task plans belong in local agent
planning artifacts or focused implementation issues, not in evergreen docs.
