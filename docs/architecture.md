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
- `BundleStore::create_v2_checkpointed()`: strict new-bundle creation through a
  synchronized sibling staging directory, live-capability probe, reopen
  verification, and complete-directory publication.

`BundleStore::upgrade_v1_to_v2()` is the only migration boundary. It creates a
new neutral-suffix destination, preserves the v1 source bytes, maps all legacy
source and WSJT-X evidence, and verifies semantic equivalence and the v2
checkpoint. New v2 writes use `write_v2()` or
`write_v2_with_attachments()`; `write()` remains the explicit v1 compatibility
wire writer for legacy fixtures and integrations. All authored writers run the
strict-creation diagnostic profile before creating files; the upgrade path
uses its separate non-destructive upgrade profile.

The diagnostic contract separates wire, structural, and semantic failures and
states which of compatibility read, analysis, strict creation, or upgrade each
diagnostic blocks. It detects duplicate JSON object members before ordinary
deserialization can collapse them. Unknown fields and duplicate members inside
legacy `raw` evidence stay reportable without granting typed code permission to
rewrite the source. [Decision 0009](decisions/0009-use-layered-bundle-validation-profiles.md)
defines this boundary.

Analysis accepts normalized bundle contents together with the layered validation
report and reuses core alignment to derive slot status and evidence eligibility.
Wire or structural ambiguity that prevents deterministic typed interpretation
still blocks the whole analysis. Record- and field-scoped semantic problems are
instead mapped to the smallest honest exclusion: a malformed location field is
omitted from location context, a contradictory or malformed observation needed
for eligibility is excluded from that observation, and an invalid slot or event
is removed without hiding unrelated slots. Stable validation codes are retained
with missing, malformed, contradictory, unsupported, duplicate, or deliberately
excluded categories and field, observation, or slot scope.

Analysis returns observation counts, exclusions, per-antenna/band/slot evidence,
SNR descriptive statistics, and conservative evidence-coverage labels. It does
not select a winner or perform effect-size, confidence, or significance analysis.
Those labels measure descriptive evidence coverage from usable-observation and
contributing-slot counts; they are not comparative evidence.

[Decision 0004](decisions/0004-paired-descriptive-analysis-precedes-conclusions.md)
defines the paired descriptive boundary. Analysis partitions uninterrupted
same-band runs into non-overlapping adjacent two-slot blocks, fixes delta
orientation from the first two scheduled labels, and distinguishes transmit and
receive paths. Paired rows remain stratified by band, normalized signal mode,
observation kind, and record source; unmatched sides, separately counted missing and malformed mode,
missing SNR, ambiguous paths, exact duplicates, conflicts, invalid blocks, time,
and order stay explicit. Duplicate schedule sequence numbers make the ordering
ambiguous, so no block in that schedule is eligible for paired evidence. Signal
mode normalization trims surrounding whitespace
and folds ASCII letters to uppercase without aliasing distinct mode names.
Repeated rows are
reduced to a per-path median before the stratum median so prolific paths do not
receive extra headline weight. Uncertainty intervals and automated conclusions
remain deferred.

Report construction accepts one `BundleContents` value and its matching layered
report, then invokes analysis internally, preventing callers from pairing bundle
context with a summary from another bundle. Its compatibility helper computes
that report directly for already-typed inputs. It deterministically projects session context, conservative
evidence sections, typed notices, paired comparison availability and
diagnostics, and concrete chart-ready rows for antenna SNR, band evidence, slot
evidence, overlap, data-quality timelines, paired differences, and SNR over
time. Validation-driven exclusions remain serializable structured data and render
in an operator-facing eligibility disclosure table. The model is otherwise
renderer-neutral: it contains no generated
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

## Local Resource Boundary

[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md)
defines the fixed `local-standard-v1` safety envelope for bundle inspection,
writes, upgrades, attachment access, copies, adapters, analysis, reports, and
desktop delivery.
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

The storage boundary is enforced for both durable schema versions with bounded
metadata preflight plus streaming growth checks, strict-write preflight,
cooperative cancellation, and rollback of incomplete destinations. Adapter and
network boundaries apply the same profile: offline WSJT-X input is streamed by
byte, physical-line, and nonblank-record budgets; live UDP uses a bounded queue,
deterministic token bucket, idle-only client eviction, and fixed-size timed
fingerprints; and NOAA responses use HTTPS/same-host redirect rules, bounded
headers, streamed decoded-body accounting, expected JSON media, cancellation,
and incomplete quarantine metadata. A queue, rate, client, or duplicate-state
breach produces one explicit acquisition gap and stops only that receiver.
Analysis preflights every collection and simultaneous-live-entry phase, checks
cancellation during long scans, and rejects cross-product-shaped allocation
plans. Renderer-neutral reports count repeated rows before projection, fall
back to an explicitly labeled aggregate overview with complete per-family
omission counts, and stream deterministic serialization and standalone HTML
through checked byte writers. Desktop state retains one summary/report pair,
admits one foreground operation, caps both IPC payloads, and keeps storage-safe
lossless export independent from report eligibility. Hosted upload and archive
limits remain a separate decision in
[#11](https://github.com/rwjblue/antennabench/issues/11).

## Schema-V2 Live Persistence

[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md)
defines the schema-v2 mutation boundary. The implemented v2 wire foundation
includes provider-neutral evidence, mutation member envelopes, and the complete
`session-state.json` checkpoint shape. Complete draft plan generations are
staged, validated, and synchronized before one checkpoint selects them. During
a run, operator, adapter, observation, rig, and propagation evidence append; a
small atomically replaced `session-state.json` identifies the one committed
plan generation and coherent prefix of every stream.

One Rust-owned writer lock, checkpoint/digest comparison, durable mutation IDs,
and explicit tail recovery prevent cooperative concurrent writes, retry
duplication, and silent overwrite of external changes. Checkpointed reads and
exports consume one revision rather than racing live files.
Static v2 creation, read, attachment verification, lossless copy, and explicit
v1 upgrade are implemented. Pure schema-v2 lifecycle validation, append-ordered
correction reduction, explicit actual-antenna projection, and conservative
conflict alignment are implemented by #54. Atomic append/promotion, locking,
current/previous checkpoint recovery, recovery attachments, and
checkpoint-aware export are implemented by #53. Schema-v1 bundles remain static
read/report/export inputs and must be upgraded non-destructively before a
conductor mutates them. The exact boundary and filesystem limitations are in
[Schema-V2 Live Persistence And Recovery](live-persistence.md).

The deterministic desktop E2E harness composes the same production boundaries
through setup, conductor events, captured WSJT-X datagrams, a bounded adapter
gap, lost acknowledgement, torn stream write, recovery, terminal report
refresh, exact HTML/checkpoint exports, destination collisions, and reopen. A
fixed scenario seed plus panic-time bundle/log retention makes failures
reproducible without adding test-only authority to the runtime command surface.

## Setup And Conductor Delivery

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

Schema and safety prerequisites are #46 and #50 through #57. The #61 validated
setup/bundle-creation slice, #62 manual/no-rig conductor, #63 bounded live
WSJT-X orchestration, and #64 coherent live/final report and export are
implemented. The remaining product slice is #65 for deterministic end-to-end coverage.
The slices consume the checkpoint and event contracts; they do not define
competing persistence, lifecycle, correction, or resource semantics.

The trusted boundary remains Rust-owned throughout. Setup review accepts a
typed draft, assigns trusted session/plan/slot identities and time, applies the
strict creation profile, and retains the exact normalized commit candidate.
Creation accepts only the retained review identity; Rust owns the native picker,
synchronized sibling staging, live capability probe, complete publication, and
active-session replacement. `active_session_conductor` recovers a newly active
v2 source once and projects lifecycle, current/next slots, trusted-time phase,
effective evidence, and diagnostics from one checkpoint. The focused mutation
command accepts an expected revision, Rust-issued bounded action token, and
typed operator intent. Rust assigns first-submission time/event identity and
uses the existing checkpoint writer; committed lost acknowledgements and exact
retries return idempotently. The frontend owns presentation and disposable
input state only. It receives no general path, filesystem, socket, clock,
identity, or network authority.

Manual/no-rig operation is the first complete vertical path. Live WSJT-X is an
optional bounded producer: admitted raw evidence and normalized observations
commit together, and a resource or acquisition gap is explicit before affected
intake stops. Reports, report export, and lossless bundle export select one
verified checkpoint revision so derived views cannot mix live generations.
Setup creation, manual/no-rig conduction, optional bounded WSJT-X orchestration,
and live/final report refresh and export are shipped behavior. A report publish
re-reads the checkpoint identity after rendering and retries boundedly if live
intake advanced. Only a verified candidate replaces the retained presentation;
the same revision keeps the same presentation identity. The renderer-neutral
model and HTML disclose checkpoint revision, lifecycle/interruption history,
adapter dispositions and acquisition gaps, planned versus actual slot state,
eligibility exclusions, and full-detail versus bounded-overview completeness.

## Desktop Shell Boundary

The desktop application is a thin Tauri host around static, framework-free web
assets. Its JavaScript owns disposable workflow and loading state plus the small
summary returned for an active session. It does not model bundle contents,
normalize evidence, analyze observations, render report markup, or persist UI
state.

The allowlisted `review_session_setup` command maps disposable station,
antenna, and schedule input to stable field diagnostics and an exact normalized
plan. `create_session_from_review` owns the native save dialog and checkpointed
new-bundle publication, then makes the reopened bundle active. The webview sees
only a review identity and the active-session summary.

The allowlisted `active_session_conductor` and
`mutate_active_session_conductor` commands expose the manual conductor. The
read projection is bounded to 512 KiB and includes a Rust-issued action token,
not host authority. Mutation reuses #53 expected-revision/idempotency semantics
and #54 lifecycle/correction reducers. Planned antenna labels remain guidance;
only effective explicit confirmations populate actual state. Competing
confirmation/missed/bad facts stay visibly conflicting and conservatively
unresolved.

The allowlisted `active_session_wsjtx_status`,
`start_active_session_wsjtx`, and `stop_active_session_wsjtx` commands expose
only bounded status and loopback receiver intent. A Rust-owned task holds the
UDP socket and expected-client filter. Each supported datagram becomes one
checkpoint mutation containing its generic adapter record plus any linked
observation; lost acknowledgement and stale-revision retries retain the same
mutation. Malformed, unsupported, filtered, duplicate, and partial outcomes are
also durable adapter records. Resource or persistence completeness gaps stop
intake, and lifecycle interruption/termination or active-session replacement
cannot leave an orphan receiver.

The allowlisted `open_session_bundle` application command owns the native
directory picker and selects a coherent committed snapshot in Rust. It returns
only a small session summary. `active_session_report` reads the retained
presentation, while `refresh_active_session_report` builds and verifies a
revision-keyed replacement without discarding the prior presentation on error.
`export_active_session_report` writes exactly that retained standalone HTML with
create-new semantics. `export_active_session` owns a native save dialog and asks
storage to create and verify a checkpointed lossless copy independently of
report eligibility. Neither export replaces the active session.

Lossless schema-v2 export copies one committed stream prefix, active plan, and
complete nested attachments tree rather than serializing normalized in-memory
state; schema-v1 export preserves its static source bytes. Existing
destinations, symbolic links, and unsupported filesystem entries are rejected;
an incomplete new destination is rolled back safely after copy or verification
failure. The frontend receives no paths and has no general filesystem or dialog
command permission. The dialog plugin is registered for native Rust use, but
its frontend permissions are not granted. Backend state retains at most one
exact reviewed setup candidate plus the selected source reference and derived
active-session presentation. Editing or re-reviewing replaces the candidate;
successful creation consumes it. Opening and exporting do not write to the
source bundle.

Native open/save pickers are thin path-selection adapters around private Rust
orchestration functions. The unattended desktop integration test substitutes
only that selection result and deterministic setup/conductor hooks, then
exercises the same review, checkpointed creation, manual lifecycle/evidence,
correction, interruption/recovery, storage, validation, analysis, report,
active-state, export-verification, and reopen code used by the Tauri commands.
This seam adds no webview permission, path argument, or release-only behavior.
Native picker presentation and OS path handoff remain a small optional
interactive platform smoke; domain and workflow regression coverage runs
without a window or foreground input.

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
  inputs before normalizing supported values into bundle records. The first
  WSPR public-spot boundary preserves each bounded WSPR.live ClickHouse JSON
  response as exact attachment evidence and emits TX `ImportedSpot`
  observations only after repeating callsign, UTC-window, band, and WSPR-mode
  filters. Manual file import is the offline/recovery path; an operator-opted-in
  HTTPS client reuses the same parser for automatic acquisition. Neither path
  makes public reports a session prerequisite; see
  [Decision 0015](decisions/0015-use-an-import-first-wspr-public-spot-boundary.md),
  [#84](https://github.com/rwjblue/antennabench/issues/84), and
  [#85](https://github.com/rwjblue/antennabench/issues/85).
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

These seams describe responsibilities. The selected WSPR.live import and
default-on, operator-configurable automatic-acquisition boundary is recorded by
[Decision 0015](decisions/0015-use-an-import-first-wspr-public-spot-boundary.md),
and the first optional rig-control milestone is tracked by
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
timestamps, and exposes explicit shutdown. The desktop orchestrator now owns
one loopback receiver task, admits one configured client identity, persists raw
generic adapter evidence and linked observations through the schema-v2 writer,
and exposes bounded status. Tests inject the documented datagrams directly
below the socket, so routine orchestration verification opens no network socket.

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
