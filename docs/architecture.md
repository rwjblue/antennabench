# Architecture

AntennaBench is organized around a durable session bundle. The bundle is the
source of truth; everything else is derived from it.

## Crates

Current crates:

- `crates/core`: distinct schema-v1 and schema-v2 wire types, a shared current
  projection, schedule alignment, normalization, and validation.
- `crates/storage`: dispatched read/write, non-destructive upgrade, verified
  attachment, and lossless-copy APIs for `.session.wsprabundle` v1 and
  `.session.antennabundle` v2 directories.
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

The current storage API exposes inspection plus three profiled read modes:

- `BundleStore::inspect()`: deterministic diagnostics plus an optional
  all-or-none typed bundle.
- `BundleStore::read()`: compatibility read; warnings may remain visible via
  inspection while ambiguous or structurally unsafe input is rejected.
- `BundleStore::read_validated()`: strict clean-report read.
- `BundleStore::read_normalized_validated()`: analysis-profile read followed by
  deterministic alignment normalization and validation.
- `BundleStore::read_current()`: the shared projection with v2 structured
  provenance, generic adapter evidence, and checkpoint sidecars retained.
- `BundleStore::read_v2()`: the explicitly versioned v2 wire representation.

`BundleStore::upgrade_v1_to_v2()` is the only migration boundary. It creates a
new neutral-suffix destination, preserves the v1 source bytes, maps all legacy
source and WSJT-X evidence, and verifies semantic equivalence and the v2
checkpoint. New v2 writes use `write_v2()` or
`write_v2_with_attachments()`; `write()` remains the explicit v1 compatibility
writer for legacy fixtures and integrations.

The diagnostic contract separates wire, structural, and semantic failures and
states which of compatibility read, analysis, strict creation, or upgrade each
diagnostic blocks. It detects duplicate JSON object members before ordinary
deserialization can collapse them. Unknown fields and duplicate members inside
legacy `raw` evidence stay reportable without granting typed code permission to
rewrite the source. [Decision 0009](decisions/0009-use-layered-bundle-validation-profiles.md)
defines this boundary.

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
receive paths. Paired rows remain stratified by band, normalized signal mode,
observation kind, and record source; unmatched sides, missing or invalid mode,
missing SNR, ambiguous paths, exact duplicates, conflicts, invalid blocks, time,
and order stay explicit. Duplicate schedule sequence numbers make the ordering
ambiguous, so no block in that schedule is eligible for paired evidence. Signal
mode normalization trims surrounding whitespace
and folds ASCII letters to uppercase without aliasing distinct mode names.
Repeated rows are
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
its CSS visualizations. Comparison availability precedes overlap and missingness,
slot data-quality, paired-difference, SNR-over-time, stratum-summary, and
distance/azimuth path-context views. Geographic views consume the already
paired rows, preserve every comparison stratum, retain incomplete grid,
distance, or azimuth facts as `location unavailable`, and show unique-path and
45-degree display-sector concentration counts. The sectors are presentation
bins, not goal thresholds or estimates of unobserved directions. The report
states the fixed right-minus-left orientation and warns that adjacent slots do
not remove propagation or time confounding. Every report-provided string is
HTML-escaped; bundle text is never accepted as markup, script, a template, or a
style value.

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

## Planned Local Resource Boundary

[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md)
defines the fixed `local-standard-v1` safety envelope for future bundle
inspection, writes, adapters, analysis, reports, copies, and desktop delivery.
The profile separates a 256 MiB modeled-data pool from a 2 GiB opaque
attachment pool, bounds records, lines, JSON nesting, tree traversal, UDP/HTTP
state, analysis intermediates, report rows, HTML, and IPC, and uses
cooperative cancellation instead of a local wall-clock timeout.

Resource failures are typed operational outcomes, not evidence-quality
judgments. They never return a complete typed bundle from a prefix. A
storage-safe lossless copy remains independent from typed interpretation, and
a live adapter that cannot accept more data records an explicit completeness
gap before it stops. Full report detail may become an explicitly labeled
aggregate overview with complete omission counts, but it is never silently
sampled.

This boundary is approved design rather than current enforcement. Implementation
is split across [#55](https://github.com/rwjblue/antennabench/issues/55),
[#56](https://github.com/rwjblue/antennabench/issues/56), and
[#57](https://github.com/rwjblue/antennabench/issues/57). Hosted upload and
archive limits remain a separate decision in
[#11](https://github.com/rwjblue/antennabench/issues/11).

## Schema-V2 Foundation And Planned Live Persistence

[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md)
defines the schema-v2 mutation boundary. The implemented v2 wire foundation
includes provider-neutral evidence, mutation member envelopes, and the complete
`session-state.json` checkpoint shape. Complete draft plan generations
will be staged and validated before one checkpoint selects them. During a run,
operator, adapter, observation, rig, and propagation evidence will append; a
small atomically replaced `session-state.json` will identify the one committed
plan generation and coherent prefix of every stream.

One Rust-owned writer lock, checkpoint/digest comparison, durable mutation IDs,
and explicit tail recovery prevent cooperative concurrent writes, retry
duplication, and silent overwrite of external changes. Reports and active
exports will consume one checkpoint revision rather than racing live files.
Static v2 creation, read, attachment verification, lossless copy, and explicit
v1 upgrade are implemented. Atomic append/promotion, locking, recovery,
checkpoint-aware active export, and lifecycle/correction reduction remain in
#53 and #54. Schema-v1 bundles remain static read/report/export inputs and must
be upgraded non-destructively before a future conductor mutates them.

## Planned Conductor Delivery

The conductor tracker
([#45](https://github.com/rwjblue/antennabench/issues/45)) turns the approved
validation, persistence, and resource decisions into a dependency-ordered local
product path:

```text
schema v2 + validation + bounded storage
                  |
                  v
       checkpoint persistence + event reducers
                  |
                  v
        setup/create -> manual conductor
                              |
                              v
               bounded WSJT-X orchestration
                              |
                              v
        coherent report/export -> end-to-end proof
```

Schema and safety prerequisites are #46 and #50 through #57. Focused product
slices are #61 for validated setup and bundle creation, #62 for the
manual/no-rig conductor, #63 for live WSJT-X orchestration, #64 for coherent
live/final report and export, and #65 for deterministic end-to-end coverage.
The slices consume the checkpoint and event contracts; they do not define
competing persistence, lifecycle, correction, or resource semantics.

The trusted boundary remains Rust-owned throughout. Setup and conductor
commands accept typed intent plus an expected checkpoint revision, create
trusted mutation IDs and timestamps, validate before durable promotion, and
return typed outcomes. The frontend owns presentation and disposable input
state only. It receives no general path, filesystem, socket, clock, identity,
or network authority.

Manual/no-rig operation is the first complete vertical path. Live WSJT-X is an
optional bounded producer: admitted raw evidence and normalized observations
commit together, and a resource or acquisition gap is explicit before affected
intake stops. Reports, report export, and lossless bundle export select one
verified checkpoint revision so derived views cannot mix live generations.
None of this is implemented conductor behavior until the focused issues land.

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
- `crates/propagation` implements the first optional NOAA/NWS SWPC boundary. It
  selects observed F10.7 and provisional `estimated_kp` from two fixed endpoints,
  emits separate sparse schema-version-1 records, preserves the selected source
  object and HTTP metadata, and exposes freshness, polling, retry, conditional
  request, duplicate-suppression, and best-effort two-product outcomes. Captured
  fixtures and transport substitution keep tests independent of live networks.
  Source times up to five minutes ahead of capture are tolerated as clock skew
  and reported with zero age; later values are discarded explicitly and cannot
  displace a valid current observation.
- Local stores, disposable indexes, and publishers consume the session bundle;
  they do not replace it as the evidence source of truth.

These seams describe responsibilities. Public-spot source and polling policy is tracked by
[#13](https://github.com/rwjblue/antennabench/issues/13), and the first optional
rig-control milestone by
[#14](https://github.com/rwjblue/antennabench/issues/14).

## Hosted Trust Boundary

AntennaBench's hosted surface is an optional sharing adapter. Local session
capture, inspection, analysis, rendering, and export have no hosted dependency.
An explicit publish operation may send one bounded ZIP transport to a minimal
Worker API; the service is not a synchronization peer and hosted state never
replaces the local bundle as evidence.

The selected service uses static application assets, private R2 quarantine and
original storage, D1 control metadata, an at-least-once Queue, and the canonical
Rust pipeline in an egress-disabled scale-to-zero Container. A separate public
R2 boundary holds only trusted immutable standalone HTML that the visibility
policy permits to be public. Public views use a custom domain and cache without
ordinary Worker, D1, Queue, or processor execution.

Hosted ingress applies the fixed `hosted-standard-v1` profile before analysis or
rendering. It limits the HTTP body, archive entries and paths, compressed and
expanded bytes, compression ratio, bundle files and records, attachments,
analysis/report projections, output bytes, and processor time. The hosted
profile is deliberately lower than local-standard-v1 and may reject a locally
valid bundle without changing local behavior.

Hosted output is rendered by trusted application code. Bundle-provided HTML,
JavaScript, templates, and other executable content are never rendering inputs,
and all operator-authored or imported text is treated as untrusted. Fixed-bundle
rendering tests and malformed, hostile, oversized, and archive-abuse cases belong
at this boundary. Exact accepted archive bytes and their entry digests remain
private and auditable; metadata, diagnostics, report models, and HTML remain
derived. Write-once objects, idempotency keys, explicit states, reconciliation,
and cache purge define retry and deletion across services that do not share a
transaction.

[ADR 0013](decisions/0013-use-an-optional-static-hosted-sharing-adapter.md)
defines the platform, profile, lifecycle, cost, and verification boundary.
[ADR 0014](decisions/0014-require-account-owned-private-to-unlisted-publishing.md)
defines the identity and policy boundary. Every upload is owned by one
verified-email account and begins private. Desktop and web clients share the
same account and report service while remaining independently complete.

Web sessions use narrow secure cookies. Desktop enrollment and recovery occur
inside the application with email codes; Rust stores a separately revocable
bearer session through a macOS, Windows, or Linux platform credential-store
adapter and performs authenticated hosted requests. The webview receives no
credential or general network authority. A missing or expired hosted session
never affects local bundle operations.

Only an explicit previewed transition creates immutable unlisted HTML in the
public bucket. Callsigns have no authorization meaning, raw accepted archives
remain private without an initial download endpoint, and owner or moderator
lifecycle actions pass through the authenticated Worker boundary. Unpublishing
retires the public URL permanently; republishing creates a new immutable URL.

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
