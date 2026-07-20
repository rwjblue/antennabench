# Agent-ready work handoff — 2026-07-19

This handoff records the requested linear pass through #202, #207, #208, and
#199, followed by the shared geographic foundation split from #203 and the
#213 → #202 → #203 dependency chain. The implementation commits through #203
are present on `main`; no pull request was opened and no issue was closed by
the agent.

## #213 — Make activity census rows band-specific

- **Status:** Implemented and left open/in progress for owner review.
- **Delivered:** WSPR.live activity census queries, persistence, and durable
  record identity now include the typed band. Rows are keyed by
  `(cycle_time, band, reporter)`, so downstream analysis can form an honest
  per-band active-reporter population. Existing bandless rows are treated as
  unsupported rather than guessed or migrated; preserving prior local-only
  test data was explicitly not required.
- **jj change / commit:** `rlworpslwwwr` —
  `feat(wsjtx): key activity census rows by band (#213)`.
- **Verification:** Focused WSPR.live adapter and persistence tests passed; the
  complete `mise run ci` passed before the main bookmark was advanced.
- **Documentation:** Updated the activity-census contract to describe the
  band-qualified durable row shape and unsupported bandless data.
- **Follow-up:** Owner review and normal publication/closure flow only.

## #202 — Add census-conditioned hearing rates

- **Status:** Implemented after the owner selected band-qualified census rows
  and #213 supplied that prerequisite; left open/in progress for owner review.
- **Delivered:** The analysis crate joins band-qualified census records to
  attributed observations without pooling strata or imputing activity. Reports
  expose per-cycle active/heard counts and rates, paired active-in-both rates,
  and explicit complete/partial/truncated/unknown coverage. Bandless or absent
  census evidence stays coverage-unknown rather than becoming zero. Desktop
  report loading preserves the adapter records needed for this derivation.
- **Decision record:** Initial blocker and owner choices are recorded at
  https://github.com/rwjblue/antennabench/issues/202#issuecomment-5017446681;
  #213 implements the selected band-binding model.
- **jj change / commit:** `qmlmuuqqyllo` —
  `feat(report): condition hearing rates on active reporters (#202)`.
- **Verification:** Focused three-cycle field-shape, truncation, and absence
  tests passed; the complete `mise run ci` passed before the main bookmark was
  advanced.
- **Documentation:** Updated `docs/reading-your-report.md` with denominator,
  pairing, coverage, and missingness semantics.
- **Follow-up:** Owner review and normal publication/closure flow only.

## #207 — Label same-path comparison chart sides

- **Status:** Implemented and left open/in progress for owner review.
- **Delivered:** The same-path chart now labels both antenna sides, marks the
  center as `0 dB`, and provides exact path values in a collapsed table for
  full and compact reports. Browser and hosted-report mirrors cover the same
  semantic labels.
- **jj change / commit:** `kzupzxlmyvxw` —
  `feat(report): label same-path chart sides (#207)`.
- **Verification:** Targeted report HTML, browser accessibility/style, and
  hosted mirror checks passed; the complete `mise run ci` passed before the
  main bookmark was advanced.
- **Documentation:** No standalone documentation change was needed; visible
  chart/table language is self-explanatory and covered by rendered fixtures.
- **Follow-up:** Owner review and normal publication/closure flow only.

## #208 — Make local support history secondary

- **Status:** Implemented and left open/in progress for owner review.
- **Delivered:** Local support history is now a native collapsed disclosure;
  compact reports retain a visible material-alert summary, and full reports
  default to the first report path. DOM/browser tests pin keyboard focus,
  disclosure state, accessibility, and full/compact behavior.
- **jj change / commit:** `rtwznwvtkwwx` —
  `feat(desktop): make support history secondary (#208)`.
- **Verification:** Desktop frontend tests (86 tests), browser checks, Rust
  checks, and the complete `mise run ci` passed before the main bookmark was
  advanced. One no-progress macOS Rust process was restarted; the clean rerun
  passed.
- **Documentation:** No standalone documentation change was required.
- **Follow-up:** Owner review and normal publication/closure flow only.

## #199 — Move bundle import/export into Saved sessions

- **Status:** Implemented and left open/in progress for owner review.
- **Delivered:** Saved sessions now owns native bundle import and per-row export.
  Import copies into bounded sibling staging, validates before atomic publish,
  creates collision-safe locations, preserves duplicate IDs separately, and
  cleans up cancellation/failure without changing the source. Export uses an
  opaque saved-session locator, revalidates before selection and checkpointed
  copy, never activates or mutates the session, and redacts filesystem paths
  from UI errors. WSPR.live/RBN file evidence moved under Active run; local
  report export remains under Local report. The obsolete transfer route,
  commands, and permissions were removed.
- **jj change / commit:** `wnvtqsprqtyr` —
  `feat(desktop): move bundle transfer to saved sessions (#199)`.
- **Verification:** Eight native managed-session transfer tests, 84 desktop
  frontend tests, real-browser checks, lint, and the complete `mise run ci`
  passed before the main bookmark was advanced.
- **Documentation:** Updated `docs/quickstart.md`, `docs/product.md`, and
  `docs/architecture-reference.md` for the new workflow and trust boundary.
- **Follow-up:** Owner review and normal publication/closure flow only.

## #211 — Add a shared geographic report foundation

- **Status:** Filed from approved #203 owner decision 5, linked back to #203 as
  its shared-geometry dependency, implemented, and left open/in progress for
  owner review.
- **Delivered:** The report crate now exposes normalized 4/6/8-character
  Maidenhead cell-center coordinates, great-circle distance and initial
  bearing, a station-centered azimuthal-equidistant projection, a reusable
  square-root polar ring frame, and deterministic coastline projection with
  antipode removal and large-jump path splitting. A quantized Natural Earth
  1:110m coastline is compiled once as a 46,306-byte asset under a compile-time
  60 KiB hard cap. This foundation intentionally renders no end-user figure.
- **Issue/dependency records:** https://github.com/rwjblue/antennabench/issues/211
  and https://github.com/rwjblue/antennabench/issues/203#issuecomment-5017797439.
- **jj change / commit:** `zpzmorvwsnrk` —
  `feat(report): add shared geographic foundation (#211)`.
- **Verification:** Seven geographic unit tests pass across the report test
  suites (50 filtered); report-crate Clippy passes with warnings denied. The
  final repository tree, including this handoff, passes `mise run ci`.
- **Documentation:** Added Natural Earth source, public-domain terms, source
  blob, quantization, exact checked-in size, and hard-cap details to
  `docs/attribution.md`.
- **Follow-up:** #203 now consumes these helpers and asset. Companion
  directionality figures can reuse the same foundation.

## #203 — Add three-state receiver coverage maps

- **Status:** Implemented after #213, #202, and #211 satisfied its data and
  geometry prerequisites; left open/in progress for owner review.
- **Delivered:** Full reports render side-by-side antenna coverage with a
  script-free grid/polar toggle, grid view by default, and grid view in print.
  Four-character cells distinguish heard, active-not-heard, and no active
  coverage while retaining exact six-character reporter grids for audit.
  Station-centered polar views and compact 8 × 4 square-root distance/azimuth
  cells use the shared geographic foundation; unmapped reporters and bounded
  detail are disclosed rather than dropped silently. The hosted Why WSPR
  reference implementation, desktop stylesheet mirrors, and canonical sample
  were refreshed to the same production contract.
- **Design record:** The final production palette and 46,306-byte compiled
  coastline confirmation are recorded at
  https://github.com/rwjblue/antennabench/issues/203#issuecomment-5018031630.
- **jj change / commit:** `qvwvmukyquqw` —
  `feat(report): map active-receiver coverage (#203)`.
- **Verification:** Focused report and field-shape suites, hosted contracts,
  desktop real-browser CSP/style checks, and the complete `mise run ci` passed
  before the main bookmark was advanced.
- **Documentation:** Updated `docs/reading-your-report.md` for coverage-state,
  map-view, compact-cell, missing-grid, and audit semantics.
- **Follow-up:** Owner review and normal publication/closure flow only.
