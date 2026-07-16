# Product Design Reference

This page records detailed product invariants, delivery boundaries, and selected
future direction. For a short introduction, start with the
[Product Overview](product.md).

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
3. Define the intended WSPR cycle order across bands and antenna labels.
4. Record antenna readiness, armed WSPR cycles, interruptions, notes, and
   session end.
5. Ingest local and external observations.
6. Align observations only to fully occupied actual cycles, preserving
   confidence and uncertainty.
7. Export a portable session bundle.
8. Generate reports from the bundle.

The shipped manual conductor keeps intended and actual state distinct. A cycle
intention names the planned antenna without a timestamp. The operator switches
to the named antenna and records readiness once the physical change is
complete; AntennaBench then arms the next eligible WSPR cycle. Missed/bad marks
and later corrections append to the evidence history instead of rewriting it.
Draft, ready, running,
interrupted/resumed, ended, and abandoned lifecycle states remain durable and
auditable under
[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md).

Manual/no-rig operation is a first-class path. Optional adapters can add
observed state, but their absence never causes the application to invent an
actual switch or prevent an operator from conducting a local session.

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

The implemented analysis and report layers are descriptive and do not select a
winner. [Decision 0004](decisions/0004-paired-descriptive-analysis-precedes-conclusions.md)
keeps automated conclusions deferred while allowing same-path SNR differences,
overlap and unmatched counts, time/order diagnostics, stratified charts, and
distance/azimuth context for observed paths. Existing insufficient/weak/moderate
labels describe evidence coverage only; they do not say that one antenna is
better.

Transmit-path reports and receive-path local decodes answer different questions
and must not be pooled. Missing decodes are not zero-SNR observations. Goal
lenses may choose documented views or filters, but they do not change effect or
conclusion rules. Single-antenna profiling never invents an A/B comparison.

Directional evidence, practical equivalence, uncertainty intervals, and "too
close to call" require a later validated inference contract with recorded
experimental-design gates, a prespecified practical-effect bound, dependence
and missingness handling, and deterministic simulation coverage. "Winner" and
unqualified "better antenna" remain prohibited product claims. That deferred
decision is tracked by [#26](https://github.com/rwjblue/antennabench/issues/26).

## V1 Bias

V1 should prioritize collecting trustworthy local evidence over building a
large public community surface.

Default mode is whole-station A/B testing. TX-focused, RX-focused, and
single-antenna profiling modes are part of the data model and can grow from the
same bundle shape.

WSJT-X companion mode is the expected first integration path. Native WSPR,
mobile operation, deeper rig control, public search, and hosted publishing are
later layers.

## Bounded Local Operation

Local-first does not mean that every selected directory, adapter response, or
UDP sender has unlimited trust or capacity.
[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md)
defines a fixed first-product envelope for bundle bytes and records,
attachments, adapter queues and state, analysis work, report size, and desktop
IPC. Production users do not receive a hidden override for these limits.

A limit failure is explicit and scoped. It never turns a prefix into a
complete session, never silently samples report detail, and never changes the
scientific meaning of accepted evidence. Manual/no-rig conduction remains
available when an optional adapter stops. Storage-safe export remains separate
from analysis/report eligibility, and a live acquisition overflow becomes an
auditable completeness gap.

The fixed profile is implemented across local bundle storage, offline and live
WSJT-X acquisition, NOAA SWPC HTTP acquisition, analysis, report projection,
standalone HTML, and desktop IPC/orchestration. Oversized report detail becomes
an unmistakable aggregate overview with complete omission counts and no sampled
rows; if even that cannot fit, report generation fails while storage-safe
lossless export remains available.

## Local Setup And Conductor Delivery

Validated local setup, bundle creation, the complete manual/no-rig conductor,
bounded live WSJT-X evidence, coherent live/final reports and exports, and the
unattended end-to-end proof are implemented. They were delivered in layers so
the UI could not outrun the durable, validation, and resource boundaries:

1. Schema-v2, layered validation, strict write preflight, and bounded storage
   establish what can be created and mutated (#46, #50, #51, and #55).
2. Checkpointed persistence/recovery and pure lifecycle/correction semantics
   establish the auditable session state (#53 and #54). The pure event,
   lifecycle, correction, explicit-actual-state, conflict-exclusion, durable
   append, plan-generation, locking, snapshot, export, and recovery layers are
   implemented.
3. Validated setup creates and opens a new checkpointed schema-v3 bundle from
   an exact normalized review (#61). The manual/no-rig conductor runs it without
   depending on any optional adapter, with current/next slot guidance, explicit
   actual-antenna confirmation, missed/bad/note facts, append-only correction,
   durable lifecycle transitions, and restart recovery (#62).
4. Bounded WSJT-X ingress and desktop orchestration add live evidence without
   making adapter health a lifecycle prerequisite (#56 and #63). The optional
   receiver binds only a numeric loopback address, admits one expected client,
   and atomically commits raw datagram evidence with any normalized observation.
   Malformed, unsupported, filtered, duplicate, and acquisition-gap outcomes
   remain explicit; resource/persistence gaps stop intake without stopping the
   manual conductor.
5. Granular evidence eligibility and bounded report/IPC behavior feed coherent
   live and final reports and exports (#52, #57, and #64). One verified
   checkpoint revision supplies the summary, renderer-neutral model, isolated
   frame, and exact standalone HTML export. Revision changes replace the frame;
   navigation, cancellation, failed refresh, and export status do not. Lossless
   checkpoint export remains available when report generation is ineligible.
6. A deterministic unattended scenario proves creation through interruption,
   recovery, completion, reporting, export, and reopen (#65).

Setup accepts callsign, grid, transmit power, experiment mode/goal, ordered
antenna definitions, and complete-round repetitions. The routine form explains
that each round visits every configured antenna, derives the number of
individual two-minute WSPR cycles and ideal minimum cycle time, and presents
automatic WSPR.live public spots as a normal attributed section before advanced
controlled-signal setup. The offline opt-out remains available as secondary
behavior; setup leaves evidence-completeness disclosure to reports. The routine
form keeps station identity plus antenna labels/descriptions visible and places
optional metadata behind disclosure. A user-triggered system-location request can fill a
six-character Maidenhead grid; permission denial or lookup failure leaves
manual entry available, and raw coordinates are never persisted. Rust trims and
types values, uppercases callsigns, constructs stable ordered cycle intentions,
applies strict-creation diagnostics, and returns the exact normalized review
without touching a destination. Creation
accepts only that review identity, allocates a collision-safe callsign/time name
under the platform application-data directory, writes and verifies in a sibling
staging directory, probes the live filesystem capability, and publishes the
complete bundle before opening it as the active session. The last station
details are stored separately as reusable preferences; they are not session
evidence. Stale review, validation failure, and destination collision do not
replace active state or expose a partial destination. Portable placement remains
an explicit lossless export.

Controlled non-WSPR transmit comparisons use the schema-v3 foundation of
reusable typed signal plans with explicit per-slot frequency variants and
counterbalancing rather than freeform procedure notes. The wire model, strict
validation, persistence, migration, desktop authoring/conductor presentation,
and manual actual-state evidence are implemented. Bounded manual RBN
daily-archive import adds exact ZIP preservation and TX public reports without
network acquisition. The boundary and RBN collection constraints are selected by
[Decision 0016](decisions/0016-use-reusable-counterbalanced-transmit-signal-plans.md)
and tracked in
[#86](https://github.com/rwjblue/antennabench/issues/86). Existing WSPR sessions
and the WSJT-X execution path use the same v3 envelope without requiring a
controlled signal plan.

The active-run surface reads one verified checkpoint revision and derives its
phase/countdown from a Rust-owned clock plus durable readiness actions. Routine
operation tells the operator to switch to the named antenna and press that
antenna's ready button once afterward; it neither requests nor persists a
switch-start time. Each readiness action closes the prior occupancy at the
recorded ready time and opens the newly confirmed antenna occupancy. Historical
schema-v3 switch-start events remain readable and retain their conservative
occupancy effect. A Rust-issued action token binds the first submission time and
idempotent mutation identity;
retrying a lost response cannot duplicate evidence, while a stale revision
fails without overwrite. Opening a session left running records one durable
recovery-system interruption before resume/end actions are offered. Ended and
abandoned sessions are terminal, and schema-v1 sources remain read-only.

The routine presentation shows one prominent next action: switch to the named
antenna, then press that antenna's ready button after the physical change is
complete. While a WSPR transmission is active, the prompt instead keeps the
current antenna connected until completion and offers no early-switch timing
action. Skipping an unarmed cycle is a durable, correctable missed-cycle fact
that advances the intended order. Notes and corrections are task-level
shortcuts; checkpoint details, receiver configuration, diagnostics, and action
history use progressive disclosure. Every mutation shows a pending state
followed by explicit success or typed failure.

The active-run surface can start or stop one optional WSJT-X receiver while the
session is running. Rust owns the socket, expected-client filter, bounded
adapter state, raw hex preservation, conservative actual-cycle annotation,
retry identity, and checkpoint append. Interruption, terminal lifecycle, active-session
replacement, adapter resource exhaustion, or an unrecordable persistence error
stops affected intake. Receiver absence or failure never blocks operator
evidence, lifecycle actions, or lossless export.

All mutation, adapter, clock/identity, filesystem, and network authority stays
behind focused Rust-owned commands. JavaScript presents typed drafts, actions,
diagnostics, and derived report documents; it does not become a second
experiment model or receive general host authority. Reports and exports consume
one committed checkpoint revision, and rendered output never becomes source
evidence.

Routine unattended coverage composes that entire local path in one seeded
scenario: reviewed setup, manual and synthetic evidence, retry and crash
recovery, explicit acquisition incompleteness, final report refresh, both
exports, collision safety, and reopen. It uses the production reducers,
persistence, parser, analysis, renderer, and desktop command seams without
hardware, network services, native pickers, display coordinates, or timing
sleeps.

## Hosted Sharing

Hosted sharing is an optional extension of the local workflow, not a dependency
of it. Capture, inspection, analysis, report rendering, standalone HTML export,
and lossless bundle export remain complete without an account, network
connection, or hosted service. Publishing is an explicit copy operation for
convenient sharing; it is not synchronization and hosted state never becomes
session evidence.

The selected hosted shape is a static viewer and explanatory site plus a
minimal publishing API. A bounded ZIP transport is quarantined privately, the
canonical Rust pipeline validates and renders it in a scale-to-zero isolated
processor, and trusted immutable report HTML can be served through a cached
public object boundary. Previously published public reports do not require a
running application or database lookup for ordinary views.

Hosted ingress applies its own lower, versioned resource and abuse profile. It
performs strict structural, semantic, archive/path, and size validation and
renders entirely with trusted application code. It does not accept or execute
bundle-provided HTML, JavaScript, CSS, templates, or other executable content;
operator-authored and imported text remains untrusted data.

The uploaded bundle remains the evidence input. Normalized copies, metadata,
report pages, charts, and discovery indexes are derived artifacts. Architecture,
storage, validation, lifecycle, cost, and abuse-control choices follow
[ADR 0013](decisions/0013-use-an-optional-static-hosted-sharing-adapter.md).
Identity and publishing policy follow
[ADR 0014](decisions/0014-require-account-owned-private-to-unlisted-publishing.md).
Every user upload requires one verified-email account owner and begins private;
publication is an explicit previewed transition to a non-discoverable unlisted
URL. Callsigns remain unverified report content and accepted raw archives remain
private with no hosted download in the first product.

The installed application and website are complete independent clients of the
same hosted account and report lifecycle. Either can enroll, upload, preview,
publish, unpublish, and delete without requiring the other. Desktop enrollment
uses an in-app email code and stores its revocable session through a
cross-platform credential abstraction owned by Rust; the web client also
supports passkeys. None of this identity state enters a session bundle or
changes account-free offline behavior.
