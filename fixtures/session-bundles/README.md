# Session Bundle Fixtures

`canonical-sample-report.session.wsprabundle/` is the canonical input for local
and future hosted sample-report rendering. It is a purpose-built synthetic
whole-station A/B session, not a recording of an operator or an on-air session.
No third-party spot data is included.

The canonical sample uses callsign-shaped identifiers, a coarse Maidenhead
grid, propagation values, and spot paths invented solely for deterministic test
coverage. They do not identify or describe a station, and any resemblance to
assigned callsigns or actual propagation conditions is coincidental. Raw fields
are synthetic source-shaped examples retained to exercise auditability.

The sample demonstrates:

- two antennas alternating across 20 m and 40 m;
- local WSJT-X decodes plus synthetic public-report and imported-spot records;
- guard-time, near-boundary, late-switch, missed-slot, bad-slot, band-mismatch,
  and outside-schedule exclusions;
- session lifecycle, switch, note, and operator-error events;
- incomplete antenna, observation, rig, and propagation metadata; and
- enough usable evidence for deterministic analysis and chart-ready report
  rows without making a winner or scientific-validity claim.

The other bundles remain focused fixtures:

- `minimal-whole-station.session.wsprabundle/` keeps the smallest useful
  alignment and round-trip scenario.
- `analysis-rich-whole-station.session.wsprabundle/` keeps balanced synthetic
  analysis assertions.
- `wsjtx-import-hardening.session.wsprabundle/` preserves parser edge cases.
