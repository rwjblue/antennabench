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

The planned conductor keeps planned and actual state distinct. A slot points to
the schedule's intended antenna, while each switch confirmation records the
actual antenna independently. Missed/bad marks and later corrections append to
the evidence history instead of rewriting it. Draft, ready, running,
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

The policy is approved but not yet implemented. Storage, adapter, and
report/desktop enforcement are tracked by
[#55](https://github.com/rwjblue/antennabench/issues/55),
[#56](https://github.com/rwjblue/antennabench/issues/56), and
[#57](https://github.com/rwjblue/antennabench/issues/57).

## Planned Local Conductor Delivery

The local conductor is tracked by
[#45](https://github.com/rwjblue/antennabench/issues/45) and remains planned
work. Its implementation is intentionally split so the UI cannot outrun the
durable, validation, and resource boundaries:

1. Schema-v2, layered validation, strict write preflight, and bounded storage
   establish what can be created and mutated (#46, #50, #51, and #55).
2. Checkpointed persistence/recovery and pure lifecycle/correction semantics
   establish the auditable session state (#53 and #54).
3. Validated setup creates a new bundle, then the manual/no-rig conductor runs
   it without depending on any optional adapter (#61 and #62).
4. Bounded WSJT-X ingress and desktop orchestration add live evidence without
   making adapter health a lifecycle prerequisite (#56 and #63).
5. Granular evidence eligibility and bounded report/IPC behavior feed coherent
   live and final reports and exports (#52, #57, and #64).
6. A deterministic unattended scenario proves creation through interruption,
   recovery, completion, reporting, export, and reopen (#65).

All setup, mutation, adapter, clock/identity, filesystem, and network authority
stays behind focused Rust-owned commands. JavaScript presents typed drafts,
actions, diagnostics, and derived report documents; it does not become a
second experiment model or receive general host authority. Reports and exports
consume one committed checkpoint revision, and rendered output never becomes
source evidence.

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
Authentication, callsign claims, ownership, visibility defaults, raw-download
exposure, and moderation remain deliberately unsettled and tracked by
[#12](https://github.com/rwjblue/antennabench/issues/12).
