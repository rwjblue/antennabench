# Session Bundle Fixtures

`canonical-sample-report.session.wsprabundle/` is the canonical input for local
and hosted sample-report rendering. It is a publication-clean compatibility
projection of a real interleaved WSPR A/B session recorded by N1RWJ on 2026-07-20.
The owner approved publication of the callsign, six-character station grid,
timestamps, antenna labels, and normalized WSPR.live observations.

The private schema-v6 source remains the lossless station record. The checked-in
schema-v1 projection retains every report input and produces byte-identical full
report HTML, while removing operator notes, adapter captures and attachments,
controller commands and output, runtime identity, diagnostics, and raw provider
payloads. The public fixture therefore preserves the measured evidence and
report result without publishing unrelated operational metadata.

The canonical sample demonstrates:

- two antennas alternating across sixteen 20 m WSPR cycles;
- 1,025 usable WSPR.live observations;
- 83 shared paths, 327 matched pairs, and seven eligible blocks;
- a descriptive +5 dB median for the DX Commander across shared paths in this
  run, without making a universal antenna ranking; and
- uneven per-cycle evidence and unmatched-path limitations.

Regenerate a publication-clean fixture from an owner-approved private bundle:

```bash
cargo run -p antennabench-report --example sanitize_canonical_sample -- \
  /path/to/source.session.antennabundle \
  fixtures/session-bundles/canonical-sample-report.session.wsprabundle
```

The destination must not already exist. Render the private source and sanitized
destination separately and byte-compare their full HTML before replacing the
canonical fixture. `crates/report/tests/canonical_sample.rs` separately pins the
public fixture's sanitation and evidence invariants.

`inconclusive-sample-report.session.wsprabundle/` retains the former
purpose-built synthetic whole-station A/B sample. Its callsign-shaped
identifiers, coarse grid, propagation values, and source-shaped raw fields are
invented solely for deterministic test coverage. It intentionally has no
matched paths and remains the teaching example for an honest insufficient-data
outcome.

The other bundles remain focused fixtures:

- `minimal-whole-station.session.wsprabundle/` keeps the smallest useful
  alignment and round-trip scenario.
- `analysis-rich-whole-station.session.wsprabundle/` keeps balanced synthetic
  analysis assertions.
- `wsjtx-import-hardening.session.wsprabundle/` preserves parser edge cases.
