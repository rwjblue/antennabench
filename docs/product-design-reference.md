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

The first optional rig-integration slice uses fresh status from the configured
WSJT-X client only to warn about a WSPR mode, band, Enable Tx, or unexpected
receive-period transmission mismatch. Warnings are advisory and companion
status is not proof of physical radio or antenna state. Missing or stale status
leaves the ordinary manual workflow unchanged. Direct frequency, mode, PTT,
keying, and antenna control are deferred by
[Decision 0019](decisions/0019-observe-rig-state-before-control.md).

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

Report answerability is modeled per conditional question rather than by one
global success gate. Shared-path signal, detection among receivers active in
both cycles, observed reach, observed distance and direction profile, and
repeatability across blocks each retain a typed availability or limitation.
The renderer names answered families in the headline, omits unavailable
families from primary navigation and full empty panels, and keeps limitations
in one secondary disclosure. Full evidence and Summary reports consume the same
renderer-neutral projection. The original paired-comparison availability field
remains only as a compatibility view of finite-SNR shared-path analysis.

Transmit-path reports and receive-path local decodes answer different questions
and must not be pooled. Missing decodes are not zero-SNR observations. Goal
lenses may choose documented views or filters, but they do not change effect or
conclusion rules. Single-antenna profiling never invents an A/B comparison.

Distance and azimuth report views are observed-session path context, not maps,
radiation patterns, propagation models, or causal conclusions. They remain
separate for every comparison stratum. The primary profile answers
`distance/bearing | antenna decoded`: it counts every unique usable remote path
once per antenna, retaining observation, block, and slot support separately.
Missing-SNR decodes remain paths, while SNR summaries use only finite values.
The shared-path context separately answers `distance/bearing | both antennas
decoded` and uses one paired-path median at most once per aggregate. Neither is
the common-opportunity `detection outcome | receiver active during both cycles`
estimand. Receiver/transmitter availability may differ across antenna periods,
so the all-path view is descriptive and is not a controlled detection
comparison. Missing or inconsistent location evidence remains visible.

The comparative geographic view projects the common-opportunity detection
estimand without changing its denominator. Within each exact stratum and
eligible block, receivers active during both cycles remain partitioned into
first-only, both, second-only, and neither outcomes, then are grouped by the
fixed distance and azimuth taxonomy. Each aggregate retains unique receivers,
receiver-block opportunities, per-antenna heard rates, coverage qualification,
and explicit location-unavailable counts. Repeated receivers remain visible as
repeated opportunities, and block order stays auditable. The accessible table
is numerically equivalent to the station-centered visual. Unsupported receive
direction and inaccessible census evidence produce explicit unavailability,
not an empty map or a zero denominator.

Coverage overlap and repeatability use exact counts, never a composite score.
Observed complementarity partitions the union of unique usable paths into
first-only, shared, and second-only membership and reports the two-antenna
total. Opportunity-conditioned complementarity separately preserves the four
common-active receiver outcomes. Per-antenna repeatability counts unique paths
seen in one versus multiple eligible blocks, retains the path-block count
distribution, and keeps raw observations and antenna order in audit detail so
a prolific endpoint cannot dominate the headline. A single eligible block is
explicitly insufficient for multi-block repeatability. These facts are
descriptive and do not imply signal strength, confidence, reliability, or an
automatic recommendation.

The fixed policy is one operator-facing taxonomy: near /
local proxy under 500 km, regional from 500 km to less than 1500 km, longer
path from 1500 km to less than 3000 km, and DX-oriented at 3000 km and above.
Distance is not propagation-mode proof. Maps may use nonlinear radial
geometry, but their ring boundaries and accessible labels use the same
categories. Azimuth uses eight 45° compass sectors, with North spanning 337.5°
through 22.5°. Each populated cell
shows unique located paths, paired-row support, and the available median path
delta. Missing or inconsistent location evidence remains visible and exact
paired-row values stay available in the audit tables.

The predeclared session goal selects only a presentation lens. The fixed lens
contract may reorder available question families and emphasize prespecified
distance categories, but it does not change facts, estimands, thresholds,
strata, or conclusion rules, and it leaves contrary evidence accessible.
Rust projects the typed priority order, emphasized categories, and practical
meaning once into the report model; Full evidence and Summary renderers consume that
same metadata rather than interpreting the goal independently.
NVIS/local wording always calls near distance a proxy; single-antenna profiling
uses no A/B conclusions. See [Decision
0027](decisions/0027-use-predeclared-goal-lenses-and-one-distance-taxonomy.md).

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

WSJT-X companion mode is the first integration path. Its next narrow slice is
passive setup-warning support; native WSPR, mobile operation, command-capable
rig control, public search, and hosted publishing are later layers.

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
3. Validated setup creates and opens a new checkpointed schema-v6 bundle from
   an exact normalized review (#61). The manual/no-rig conductor runs it without
   depending on any optional adapter, with current/next slot guidance, explicit
   actual-antenna confirmation, missed/bad/note facts, append-only correction,
   durable lifecycle transitions, and restart recovery (#62).
4. Bounded WSJT-X ingress and desktop orchestration add live evidence (#56 and
   #63). The receiver is a start prerequisite for schema-v4 plans containing
   receive periods and optional for transmit-only plans. It binds only a numeric
   loopback address, admits one expected client,
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
7. Optional local antenna-controller profiles normalize to one direct program
   plus ordered argv, remain outside portable bundles, and commit every bounded
   switch/verification attempt as schema-v5 rig evidence (#109). Rust-owned
   automatic sequencing and optional atomic command-verified readiness retain
   explicit Start/Resume and the manual-ready fallback (#110).

Setup begins with four disposable operator questions that map deterministically
to the existing whole-station BOTH, TX-focused, RX-focused, and single-antenna
profiling modes. The single-antenna choice selects its required existing goal;
other questions default the separate goal to general coverage. Mode and goal
remain visible and editable, and no question or expected-result field enters the
draft or bundle. Setup then accepts callsign, grid, transmit power, ordered
antenna definitions, and complete repetitions. BOTH and four repetitions are
the defaults. Each repetition visits every configured antenna once in each
selected direction. Rust projects the exact directed order, WSPR cycle count,
required WSPR cycle time, antenna-versus-direction transitions, counterbalance
rationale, and mode-shaped can-describe/cannot-establish statements from the
same normalized candidate retained for creation. The frontend renders that
projection and does not maintain a second schedule or capability model.
Automatic bidirectional WSPR.live public spots remain a normal attributed
section before advanced controlled-signal setup. The online path requires
WSJT-X **Upload spots** and network access, and
collects returned rows for configured request windows on a best-effort basis;
the upstream mirror does not provide an independent completeness guarantee. The offline opt-out remains
secondary behavior; a receive-capable offline run requires the direct/local UDP
receiver, while UDP is optional and separately attributed when WSPR.live is on. The routine
form keeps station identity plus antenna labels/descriptions visible and places
optional metadata behind disclosure. A user-triggered native macOS Core
Location request can prompt for foreground permission and fill a six-character
Maidenhead grid from one transient coordinate pair. Denied, restricted,
unavailable, and timeout outcomes leave manual entry available; raw coordinates
are never persisted or logged. Rust trims and types values, uppercases
callsigns, constructs stable ordered cycle intentions,
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

Controller-assisted setup is explicitly opt-in. It selects a frozen portable
invocation and manual-review policy, a revisioned app-data profile,
and one opaque target per scheduled antenna. Review shows the canonical
program/indexed arguments for every mode/direction intention and warns that
resolved commands and output become portable evidence. Reopening a saved or
imported bundle restores no executable authority; local association and arming
are explicit. Automatic authority begins only after Start/Resume, waits for the
complete prior transmission interval, and remains independent of webview
timers. Active Run offers switch, verification status, explicit retry, profile
edit/reattach, blocked and awaiting-review states, and the unchanged manual-ready
fallback without automatic retry. Review-disabled success atomically records
both commands and command-verified readiness. See
[Local Antenna Controller Profiles](antenna-controller-profiles.md).

Controlled non-WSPR transmit comparisons use the schema-v3 foundation of
reusable typed signal plans with explicit per-slot frequency variants and
counterbalancing rather than freeform procedure notes. The wire model, strict
validation, persistence, migration, desktop authoring/conductor presentation,
and manual actual-state evidence are implemented. Bounded manual RBN
daily-archive import adds exact ZIP preservation and TX public reports without
network acquisition. The boundary and RBN collection constraints are selected by
[Decision 0016](decisions/0016-use-reusable-counterbalanced-transmit-signal-plans.md)
and tracked in
[#86](https://github.com/rwjblue/antennabench/issues/86). New WSPR sessions use
the current schema-v5 envelope, retaining the schema-v4 directed WSPR model
without requiring a controlled signal plan; existing schema-v3 and schema-v4
sessions remain readable.

Execution remains manual/keyer-first. AntennaBench may present the typed plan
and record operator-confirmed actual state, but it does not provide built-in CAT,
PTT, Morse generation, keyer, waveform, or automatic retry. Any future
transmitter integration requires new field evidence and a separately approved
safety boundary.

The active-run surface reads one verified checkpoint revision and derives its
phase/countdown from a Rust-owned clock plus durable readiness actions. Each
Rust response anchors a disposable frontend countdown that advances once per
second, resynchronizes on refresh/mutation/focus, and requests a new conductor
view at zero; it never timestamps evidence or selects a cycle. Active cycle
cards use concise local time, adding a short date only for another day. Routine
operation asks only for unresolved antenna, WSPR direction, band, or controlled-
signal work. When trusted open occupancy and the latest coherently armed cycle
prove the complete required state is unchanged, a Rust-owned coordinator waits
for the prior transmission to end and arms the next eligible cycle without
another ready prompt or controller command. Its explicit continued-readiness
evidence keeps the original occupancy open. A fresh operator- or command-
confirmed readiness closes the prior occupancy and opens the newly confirmed
occupancy. Historical
schema-v3 switch-start events remain readable and retain their conservative
occupancy effect. A Rust-issued action token binds the first submission time and
idempotent mutation identity;
retrying a lost response cannot duplicate evidence, while a stale revision
fails without overwrite. Opening a session left running records one durable
recovery-system interruption before resume/end actions are offered, but only
when the operator opens it to work and the conductor is loaded. Opening the
same committed session for its report does not recover it or start run
services. Saved-session actions make that `work` versus `report` intent
explicit. A stale
work request that opens a now-terminal or read-only session is safely redirected
to Reports with an explanation. No opening path starts or resumes a session;
those remain explicit operator actions. Ended and abandoned sessions are
terminal, and schema-v1 sources remain read-only.

Saved sessions is the app-level home and the startup fallback when a session-only
destination is requested before any session is active. Its
header keeps new-session, managed import, managed-folder reveal, and refresh
actions together. Import validates and atomically publishes a lossless copy,
then offers open and reveal follow-ups. Each
managed entry leads with callsign, creation time, lifecycle, compact immutable
plan facts, origin, and bundle name. Correctable evidence remains in a details
disclosure; duplicate session identities are warnings on distinct rows, not a
merge. Lifecycle-specific work/report actions, problem-only detail and reveal
actions, row-local failures, partial results, and stale-list refresh failures
keep catalog truth visible without converting catalog metadata into authority.
Successful creation also identifies the managed location on Run and offers a
Finder reveal action.

Every available Saved sessions row offers a native **Export bundle…** action
without requiring activation. Progress, cancellation, and failure stay on that
row. WSPR.live recovery and RBN archive imports remain lifecycle-constrained
under the active run's **Add evidence** disclosure because they append evidence
to one active experiment. Summary and Full evidence HTML exports remain in Local report;
there is no separate numbered Import / export destination.

Local report is a viewport-bounded reading workspace: a compact persistent
toolbar leads with an ephemeral **Back to Saved sessions** or **Back to Active
run** action based on the surface that opened the current session report. Return
restores that surface's scroll position and initiating control when it remains
available; a terminalized run falls back explicitly to Saved sessions. The
ordinary workspace and reader exchange through a brief, reversible shell
transition; a newer navigation interrupts it, and reduced-motion preference
bypasses the spatial effect. The toolbar also owns secondary navigation, coherent revision identity, refresh,
Diagnostics, and Export, while the sandboxed scientific report iframe is the
sole routine vertical scroll owner. Diagnostics contains bounded creator/subsequent runtime
cards and chronological failed, partial, and recovery outcomes with explicit
legacy, unavailable, retention-capped, and persistence-gap states. Material
warnings remain visible outside that secondary dialog. Reopened running or
interrupted sessions surface the latest relevant failure on Active run. One
clipboard action copies deterministic redacted JSON; Summary and public output omit
the history, and full evidence export requires a separate explicit inclusion
choice in the Export dialog.

The routine presentation shows one prominent next action. It says whether to
keep or switch the named antenna and, independently, whether to turn WSJT-X
Enable Tx on or off before pressing ready. While a WSPR period is active, the
prompt keeps the current antenna connected until completion and offers no
early-switch timing action. Skipping an unarmed cycle is a durable, correctable
missed-cycle fact that advances the intended order. Notes and corrections are task-level
shortcuts. The secondary run surface shows operator-useful lifecycle and current
antenna state without checkpoint revisions, backend clock values, or opaque
identifiers. Public-spot status uses short waiting, collecting, collected,
disabled, and retry language; detailed request windows and provenance remain in
durable evidence and reports. Receiver configuration, diagnostics, and action
history use progressive disclosure. Every mutation shows a pending state
followed by explicit success or typed failure.

Running and interrupted sessions also keep a first-class **Abort run** action on
the primary surface in every conductor phase, including finalization and
adapter-error recovery. Its application-owned confirmation is bound to the
presented session, Rust-issued action token, and revision; identifies the
current or next cycle without exposing opaque IDs; starts on the safe action;
and distinguishes terminal abandonment from resumable Pause, normal End, and
single-cycle Skip. A confirmed Abort preserves committed evidence, preempts a
matching AntennaBench-owned controller process, stops further local intake only
after the terminal checkpoint commits, and never claims to disable WSJT-X
**Enable Tx**, release PTT, or stop an active radio transmission.

Selected setup and Active Run concepts use one restrained contextual-help
pattern. A real `?` button exposes at most two short operator-facing sentences,
advertises its controlled description to assistive technology, and supports
keyboard activation, Escape with focus return, outside-click close, and narrow
layout reflow. Help remains presentation-only: validation, consent, safety,
active instructions, and recovery errors always stay visible without opening a
disclosure.

The active-run surface can start or stop one WSJT-X receiver while the session
is ready or running. It must be running before a receive-capable schema-v4
session starts. Rust owns the socket, expected-client filter, bounded
adapter state, raw hex preservation, conservative actual-cycle annotation,
retry identity, and checkpoint append. Interruption, terminal lifecycle, active-session
replacement, adapter resource exhaustion, or an unrecordable persistence error
stops affected intake. Receiver absence blocks only the initial start of a
receive-capable schema-v4 run; it does not block other operator evidence,
lifecycle actions, or lossless export.

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

Hosted sharing is not an active product commitment. Capture, inspection,
analysis, report rendering, standalone HTML export, and lossless bundle export
remain the complete product without an account or hosted service. The deployed
public site is informational and accepts no user uploads.

The repository retains an admission-disabled Cloudflare prototype and ADRs 0013
and 0014 as prior design research. The former ZIP admission, Container
processing, account, lifecycle, cached report, desktop/web client, and moderation
issues are closed as not planned. Their detailed topology must not be treated as
approved implementation scope.

Issue #10 gates any reassessment on signed external-beta evidence that identifies
a repeated sharing problem local standalone HTML cannot solve. A later
experiment must choose the smallest useful mechanism and one first client rather
than assume the former end-to-end account service. Hosted state would remain a
derived explicit copy, never synchronization or session evidence.
