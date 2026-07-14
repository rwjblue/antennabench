# Architecture

AntennaBench is organized around a durable session bundle. The bundle is the
source of truth; everything else is derived from it.

## Crates

Current crates:

- `crates/core`: serializable bundle model, schedule alignment, normalization,
  and validation.
- `crates/storage`: filesystem read/write APIs for `.session.wsprabundle`
  directories.
- `crates/wsjtx`: offline WSPR `ALL_WSPR.TXT` import plus a live WSJT-X UDP
  companion boundary, producing preserved adapter records and eligible local
  decode observations.
- `crates/analysis`: conservative, descriptive A/B evidence summaries derived
  in memory from validated bundle contents and core schedule alignment.
- `crates/report`: deterministic, renderer-neutral report data derived in
  memory from one bundle and its analysis summary, plus standalone local HTML
  rendering from that report boundary.
- `apps/desktop`: a Tauri 2 application shell with a static HTML/CSS/JavaScript
  frontend and explicit workflow navigation for setup, active runs, bundle
  transfer, and local reports.

Planned crates and apps:

- `apps/web`: hosted report viewer and publishing surface.
- `crates/rig`: optional rig-observation or control adapters.
- `crates/public-spots`: source-neutral public and imported spot adapters.

## Data Flow

```text
operator + adapters
        |
        v
session bundle JSON/JSONL
        |
        +--> normalization
        +--> validation
        +--> local indexes
        +--> analysis
        +--> reports
        +--> hosted rendering
```

The current storage API exposes three read modes:

- `BundleStore::read()`: parse-only filesystem read.
- `BundleStore::read_validated()`: strict read and validation.
- `BundleStore::read_normalized_validated()`: tolerant read, normalization, and
  validation.

Analysis accepts normalized bundle contents, validates them without mutation,
and reuses core alignment to derive slot status and evidence eligibility. It
returns observation counts, exclusions, per-antenna/band/slot evidence, SNR
descriptive statistics, and conservative evidence-coverage labels. It does not
select a winner or perform effect-size, confidence, or significance analysis.
Those labels measure descriptive evidence coverage from usable-observation and
contributing-slot counts; they are not comparative evidence.

[Decision 0004](decisions/0004-paired-descriptive-analysis-precedes-conclusions.md)
defines the paired descriptive boundary. Analysis partitions uninterrupted
same-band runs into non-overlapping adjacent two-slot blocks, fixes delta
orientation from the first two scheduled labels, and distinguishes transmit and
receive paths. Paired rows remain stratified by band, observation kind, and
record source; unmatched sides, missing SNR, ambiguous paths, exact duplicates,
conflicts, invalid blocks, time, and order stay explicit. Repeated rows are
reduced to a per-path median before the stratum median so prolific paths do not
receive extra headline weight. Uncertainty intervals and automated conclusions
remain deferred.

Report construction accepts one `BundleContents` value and invokes analysis
internally, preventing callers from pairing bundle context with a summary from
another bundle. It deterministically projects session context, conservative
evidence sections, typed notices, paired comparison availability and
diagnostics, and concrete chart-ready rows for antenna SNR, band evidence, slot
evidence, overlap, data-quality timelines, paired differences, and SNR over
time. The model is serializable but renderer-neutral: it contains no generated
prose, winner logic, generic chart configuration, or rendering output. The
renderer may explain and visualize those typed facts, but it must not infer a
conclusion from chart shape, raw antenna summaries, paired descriptive centers,
or evidence-coverage labels.

`render_standalone_html()` accepts only a `SessionReport`. It does not read or
reanalyze a bundle and does not persist output into one. The renderer produces a
complete deterministic HTML document with embedded CSS, a restrictive content
security policy, no scripts or external resources, and accessible tables beside
its CSS visualizations. Every report-provided string is HTML-escaped; bundle
text is never accepted as markup, script, a template, or a style value.

## Alignment

Schedule alignment is pure core logic. It derives actual slot state from planned
slots plus operator events, then assigns observations to slots with labels and
confidence.

Alignment is deterministic. Validation uses the same alignment logic to detect
stale persisted observation annotations.

## Derived State

SQLite indexes, UI state, generated reports, charts, and hosted publishing
artifacts are derived. They can be rebuilt from the bundle and should not become
the canonical record of a session.

Analysis summaries, session reports, and rendered HTML are derived and are not
persisted in the bundle. `analysis.json` remains bundle metadata rather than a
serialized analysis summary or report. Report construction and rendering do
not change the bundle format or schema version.

## Desktop Shell Boundary

The desktop application is a thin Tauri host around static, framework-free web
assets. Its JavaScript owns disposable workflow and loading state plus the small
summary returned for an active session. It does not model bundle contents,
normalize evidence, analyze observations, render report markup, or persist UI
state.

The allowlisted `open_session_bundle` application command owns the native
directory picker and composes storage, normalization, validation, report
construction, and standalone HTML rendering in Rust. It returns only a small
session summary. The read-only `active_session_report` command supplies the
already-derived document to the report surface. `export_active_session` owns a
native save dialog and asks storage to create and verify a lossless copy of the
active source. It returns only the destination bundle name and does not replace
the active session.

Lossless export copies the original durable root-file bytes and complete nested
attachments tree rather than serializing normalized in-memory state. Existing
destinations, symbolic links, and unsupported filesystem entries are rejected;
an incomplete new destination is rolled back safely after copy or verification
failure. The frontend receives no paths and has no general filesystem or dialog
command permission. The dialog plugin is registered for native Rust use, but
its frontend permissions are not granted. The only retained backend state is
the selected source reference and derived active-session presentation; opening
and exporting do not write to the source bundle.

Native open/save pickers are thin path-selection adapters around private Rust
orchestration functions. The unattended desktop integration test substitutes
only that selection result, then exercises the same storage, validation,
analysis, report, active-state, export-verification, and reopen code used by the
Tauri commands. This seam adds no webview command, permission, path argument, or
release-only behavior. Native picker presentation and OS path handoff remain a
small optional interactive platform smoke; domain and workflow regression
coverage runs without a window or foreground input.

The report document is displayed through a sandboxed `srcdoc` frame without
script, same-origin, navigation, or network authority. The trusted report
renderer already emits no scripts or external resources and supplies its own
restrictive content security policy. The containing shell also denies network
connections and grants the frame only the access needed to display this local
document.

## Integration Seams

External systems should enter through narrow adapters so their availability,
payload shape, and failure behavior do not become experiment-model invariants.
The durable boundaries are:

- WSPR integration produces preserved adapter records and eligible
  observations; WSJT-X companion mode is first, while native implementations
  may be added later.
- Rig integration is optional. A session remains runnable with manual switching
  and no rig adapter.
- Public-spot and propagation sources preserve provenance and raw or near-raw
  inputs before normalizing supported values into bundle records.
- Local stores, disposable indexes, and publishers consume the session bundle;
  they do not replace it as the evidence source of truth.

These seams describe responsibilities, not approved provider or library
choices. Public-spot source and polling policy is tracked by
[#13](https://github.com/rwjblue/antennabench/issues/13), and the first optional
rig-control milestone by
[#14](https://github.com/rwjblue/antennabench/issues/14).

## Hosted Trust Boundary

A hosted viewer may accept only a bounded session-bundle representation. The
ingress boundary must enforce structural, semantic, path/archive, and size
limits before analysis or rendering. Original uploaded evidence remains
auditable; normalized data, metadata, report pages, charts, and indexes remain
derived and replaceable.

Hosted output is rendered by trusted application code. Bundle-provided HTML,
JavaScript, templates, and other executable content are never rendering inputs,
and all operator-authored or imported text is treated as untrusted. Fixed-bundle
rendering tests and malformed, hostile, oversized, and archive-abuse cases belong
at this boundary. Platform services and the exact upload/storage lifecycle are
deferred to [#11](https://github.com/rwjblue/antennabench/issues/11); identity,
authorization, visibility, and moderation are deferred to
[#12](https://github.com/rwjblue/antennabench/issues/12).

## Live WSJT-X Boundary

The live adapter accepts official WSJT-X network-message schemas 2 and 3. It
parses heartbeat, the status prefix through station identity, WSPRDecode, and
close messages. Unknown message types and compatible trailing fields are
ignored for behavior while supported datagrams are retained exactly as hex in
bundle-ready `WsjtXRecord` values.

The parser is pure. `LiveWsjtxIngest` owns the small per-client state machine
for schema/version identity, current status, duplicate suppression, and client
lifecycle. A close message or a gap longer than three heartbeat periods resets
status and duplicate state. The synchronous UDP receiver only binds, receives,
timestamps, and exposes explicit shutdown; orchestration remains the future
desktop application's responsibility.

WSPRDecode carries a time-of-day rather than a date. The adapter reconstructs
UTC by choosing the closest of the receipt date and its adjacent dates, using
the supplied session start only as a deterministic tie-breaker. This handles
midnight rollover without inventing a durable clock source. Decode and receipt
times remain available in the preserved raw data.

Observation production is deliberately conservative: `New` must be true,
`Off air` false, the datagram must not be a duplicate in the current client
generation, and current status must identify the configured station in WSPR
mode. Status transmitting/receiving/decoding values are tracked and preserved
but do not gate a decode because WSJT-X status transitions and completed decode
delivery need not occur in the same instant.
