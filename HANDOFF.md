# Agent-ready work handoff — 2026-07-19

This handoff records the requested linear pass through #202, #207, #208, and
#199, followed by the shared geographic foundation split from #203. No changes
were pushed, no pull request was opened, and no issue was closed.

## #202 — Add census-conditioned hearing rates

- **Status:** Blocked on a product/data-contract decision; the issue is labeled
  `needs-decision`.
- **Delivered:** Completed the issue/dependency review and identified the
  binding mismatch: persisted census rows are keyed by `(cycle_time, reporter)`
  without a band, while the required analysis filters and grouping require a
  band-specific active-reporter population. Implementing either a guessed band
  or a cross-band denominator would change the approved meaning.
- **Decision record:** Posted the blocker and concrete owner choices at
  https://github.com/rwjblue/antennabench/issues/202#issuecomment-5017446681.
- **jj change / commit:** None; no code or documentation changed for this item.
- **Verification:** Reviewed the full issue, dependency contract, persisted
  census shape, and requested hearing-rate grouping before stopping.
- **Documentation:** None.
- **Follow-up:** The owner must select the band-binding model. #203 remains
  blocked on #202. Per the queue instructions, no #203 design note was posted
  because #202 did not complete.

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
- **Follow-up:** #203 and the companion directionality figures should consume
  these helpers and asset. #203 implementation still waits for the #202 owner
  decision and its remaining design-note confirmation; this foundation does
  not relax either gate.
