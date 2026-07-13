# Product

AntennaBench is a local-first app for comparing and profiling antennas using
WSPR observations.

The first product target is a desktop workflow that helps an operator run a
controlled WSPR session, preserve the evidence, and generate conservative
reports. The app should favor honest evidence quality over simple winner claims.
Local collection, analysis, reporting, and export must remain useful without an
account or network connection.

## Core Workflow

The intended workflow is:

1. Record station basics such as callsign, grid, and power.
2. Define one or more antennas with freeform labels and optional installation
   details.
3. Define a schedule of WSPR slots across bands and antenna labels.
4. Record operator events such as switched, missed slot, bad slot, notes, and
   session end.
5. Ingest local and external observations.
6. Align observations to planned slots, preserving confidence and uncertainty.
7. Export a portable session bundle.
8. Generate reports from the bundle.

## Evidence And Report Honesty

The session bundle preserves the experiment record so later analysis can be
regenerated and audited. Adapters should retain raw or near-raw inputs,
provenance, timestamps, operator events, exclusions, and uncertainty rather than
only keeping values that support a conclusion.

Reports should distinguish what was planned, what actually happened, what was
observed, what was inferred, and how strong the evidence is. Missing,
imbalanced, or low-quality evidence must stay visible. `Insufficient data` and
`too close to call` are valid outcomes; the product must not manufacture a
winner when a method does not justify one.

The implemented analysis and report layers are currently descriptive and do
not select a winner. Future effect-size, uncertainty, conclusion-language, and
advanced-chart behavior requires the methodological decision tracked in
[#15](https://github.com/rwjblue/antennabench/issues/15).

## V1 Bias

V1 should prioritize collecting trustworthy local evidence over building a
large public community surface.

Default mode is whole-station A/B testing. TX-focused, RX-focused, and
single-antenna profiling modes are part of the data model and can grow from the
same bundle shape.

WSJT-X companion mode is the expected first integration path. Native WSPR,
mobile operation, deeper rig control, public search, and hosted publishing are
later layers.

## Hosted Sharing

Hosted sharing is an optional extension of the local workflow, not a dependency
of it. A publishing surface should accept bounded session-bundle data, apply
strict structural, semantic, and size validation, and render reports entirely
with trusted application code. It must not accept or execute bundle-provided
HTML, JavaScript, or templates; operator-authored and imported text remains
untrusted content.

The uploaded bundle remains the evidence input. Normalized copies, metadata,
report pages, charts, and discovery indexes are derived artifacts. Architecture,
storage, validation, and abuse-control choices are tracked by
[#11](https://github.com/rwjblue/antennabench/issues/11). Authentication,
callsign claims, ownership, visibility defaults, raw-download exposure, and
moderation are deliberately unsettled and tracked by
[#12](https://github.com/rwjblue/antennabench/issues/12).
