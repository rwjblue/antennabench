# Bundle Format Technical Reference

This page specifies the on-disk format, compatibility behavior, validation,
and resource limits for AntennaBench session bundles. For the user-facing
mental model, start with [Session Bundles](bundle-format.md).

A session bundle is a directory containing JSON root files, JSONL streams, and
an attachments directory. Schema version 1 uses `.session.wsprabundle`; schema
versions 2 through 5 use the provider-neutral `.session.antennabundle` suffix.

Version 1 layout:

```text
example.session.wsprabundle/
  manifest.json
  station.json
  antennas.json
  schedule.json
  events.jsonl
  observations.jsonl
  wsjtx.jsonl
  rig.jsonl
  propagation.jsonl
  analysis.json
  attachments/
```

Versions 2 through 5 share this physical layout:

```text
example.session.antennabundle/
  manifest.json
  session-state.json
  session-state.previous.json
  station.json
  antennas.json
  schedule.json
  events.jsonl
  observations.jsonl
  adapter-records.jsonl
  rig.jsonl
  propagation.jsonl
  analysis.json
  attachments/
    sha256/
  plan-generations/
    <generation-id>/
      station.json
      antennas.json
      schedule.json
      generation.json
```

Storage reads `manifest.schema_version` first and dispatches into distinct v1,
v2, and v3-v5 wire models. Unknown versions fail with a typed unsupported-version
error. All versions project into one current analysis model; v2/v3 projection retains a
provider-neutral provenance sidecar, generic adapter records, and the session
checkpoint. V3-only planned and confirmed signal facts are not coerced into
legacy analysis fields.

Version 2 implements the static wire foundation selected by
[Decision 0008](decisions/0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md).
Its `session-state.json` reserves the checkpoint selected by
[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md):
revision, lifecycle, a persisted WSPR.live automatic-acquisition choice,
active-plan/root digests, committed byte
length/count/last-ID/digest for every stream, and the last mutation ID. Every
v2 stream record carries mutation ID and member index/count. The live writer,
atomic promotion, locking, checkpoint snapshots/export, and recovery contract
are implemented by #53. Typed lifecycle/correction reduction is implemented by
#54 and is shared by v2 inspection, mutation, analysis, and reports. See
[Schema-V2 Live Persistence And Recovery](live-persistence.md).

Newly reviewed desktop sessions use
`BundleStore::create_v2_checkpointed()`. The strict bundle is written and
synchronized under a uniquely named sibling staging directory, reopened
through the live-writer capability boundary, compared with its reviewed model,
and only then published at the absent `.session.antennabundle` destination.
The initial checkpoint is lifecycle `ready`, revision zero, and contains exact
root and empty-stream digests. A missing
`wspr_live_acquisition_enabled` field is false for compatibility with existing
bundles. New desktop setup presents the disclosed choice checked and persists
true unless the operator opts out. Failed validation or preparation removes the
staging directory; an existing destination is never an overwrite target.

Version 1 is never silently rewritten to gain these semantics. It remains
readable and losslessly copyable; live mutation requires an explicit v2 upgrade
to a new `.session.antennabundle` destination.

[Decision 0016](decisions/0016-use-reusable-counterbalanced-transmit-signal-plans.md)
selected schema v3 for typed non-WSPR transmit signal plans. V3 adds reusable
CW/RTTY plan definitions containing planned power, exact transmitted identity,
cadence, and collection profile; each participating slot carries an exact
frequency, frequency-variant identity, counterbalance block, and position. The
pinned RBN CW profile rejects allocations less than 300 Hz apart when their
transmissions are less than ten minutes apart, and RTTY cannot claim that CW
profile. Strict validation also requires complete, position-balanced antenna ×
frequency-variant blocks and explicit validation when transmitted identity
differs from the station callsign.

V3 events add correctable `signal_state_confirmed` evidence. Actual frequency,
mode, optional power, exact transmitted identity, and cadence adherence remain
separate from the plan; missing actual facts stay missing. Rig records add only
optional observed/read-back power. Static checkpoint persistence, verified
attachments, manifest dispatch, lossless export, and deterministic direct
v1-to-v3 and v2-to-v3 upgrades are implemented. Upgrades preserve legacy
evidence but create no signal plans, slot allocations, confirmations, or rig
power facts. Existing v1/v2 read and lossless-copy behavior is unchanged.
The schema-v3 writer can also append attachment-backed adapter records and
normalized observations in one checkpoint revision. It admits this evidence
after a session has started, including post-session import into ended or
interrupted sessions, without changing lifecycle state.

[Decision 0017](decisions/0017-use-operator-paced-wspr-cycles.md) also makes v3
the authoring format for new WSPR sessions. `schedule.json` stores ordered
`wspr_cycle_intents` without timestamps. Append-only `antenna_switch_started`
and `wspr_cycle_armed` events record actual manual progress, backend-selected
protocol boundaries, and half-open antenna occupancy. Legacy `slots` remain a
versioned compatibility field but are empty in newly authored WSPR bundles.
Readers project actual armed cycles when a slot-oriented analysis path is
required. Attribution requires one antenna occupancy interval to cover the
complete 110.592-second transmission.

[Decision 0018](decisions/0018-use-directed-counterbalanced-wspr-cycles.md)
selects schema v4 for newly authored sessions. V4 reuses the checkpointed v3
envelope and requires every WSPR cycle intention to carry `direction` as
`receive` or `transmit`. Validation requires transmit-only and receive-only
modes to contain only their named direction, while BOTH and single-antenna
profiling schedules contain both. Existing schema-v3 cycles deserialize with
unknown direction and remain readable; no migration guesses whether a past
period transmitted or received.

[Decision 0021](decisions/0021-use-command-verified-antenna-control.md)
selects schema v5 for antenna-control policy and evidence. `schedule.json`
records only the portable policy: manual or command-controlled,
operator-triggered or automatic invocation, and whether manual review is
required. Executable paths, argument templates, antenna target mappings, and
timeouts are local application configuration and are never plan fields.

[Decision 0024](decisions/0024-use-version-owned-core-schema-modules.md)
separates durable schema numbers from Rust API ownership. Public version-owned
types live under the `antennabench_core::v2`, `v3`, and `v5` modules and keep
their explicit version suffixes; shared APIs use a curated crate-root surface.
Schema v4 intentionally uses the v3 Rust representation because it added a
validation and authoring requirement for the optional v3 intention-direction
field rather than a distinct wire model. Future revisions must document the
same ownership choice and may not enter the crate root through glob exports.

Each attempted switch or verification may add one typed `rig.jsonl` record.
It contains the controller profile name/revision, original command templates,
resolved program and indexed argument array, complete intention context
(including the durable experiment mode used for `{mode}` interpolation),
timing, exit/spawn/signal/timeout disposition, and independently bounded
stdout/stderr. Output is UTF-8 or base64 with explicit truncation state and a
64 KiB decoded-byte limit per stream. A complete rig line remains subject to
the 256 KiB JSONL line limit.

Every schema-v5 `wspr_cycle_armed` event names its readiness basis.
`operator_confirmed` is the default manual meaning. `command_verified`
references one successful switch record and one successful verification record
for the same intention and target. Strict validation requires the two rig
records and event to be members 0, 1, and 2 of one checkpointed mutation;
roles, order, session, intention, antenna, target, direction, band, frequency,
sequence, callsign, and exit-zero outcomes must agree. Failed attempts may be
committed alone and never arm a cycle or change antenna occupancy.

## Provider-Neutral Evidence

Every v2 provenance contains `provider_id`, `source_id`,
`acquisition_channel`, `adapter_id`, and a separate `adapter_version`. The four
identities are lowercase ASCII, at most 128 bytes, and consist of alphanumeric
segments separated by a single `.`, `_`, or `-`. They are validated strings,
not closed provider enums, so an unknown valid provider remains readable.

`adapter-records.jsonl` replaces v1's provider-specific `wsjtx.jsonl`. Records
contain capture/source times, provider record type, typed accepted, malformed,
filtered, duplicate, conflict, unsupported, or partially-normalized disposition, stable
reason identity, normalized-record links, and attributed input. Input is
exact/near-raw inline text or an attachment reference containing lowercase
SHA-256, byte size, media type, encoding/container, and source locator.

Attachment bytes live at `attachments/sha256/<digest>`. Reads verify size and
digest. `BundleAttachment` with `write_v2_with_attachments()` creates a new v2
destination containing referenced attachment bytes; `write_attachment()` and
`read_attachment()` expose the same verified content-addressed store.

The selected first WSPR public-spot boundary uses these generic records rather
than adding another provider-specific stream. A bounded operator-supplied or
automatically acquired WSPR.live ClickHouse JSON response is retained as an
exact attachment; row records preserve near-raw values and link accepted TX or
RX `ImportedSpot` observations. The persisted
`wspr_live_acquisition_enabled` field is unchanged, but now means automatic
bidirectional collection. Callsign role, UTC window, band, WSPR mode, and known
schema-v4 cycle direction are repeated in both paths. RX observations use the
station as reporter, the remote transmitter as heard identity, and the
provider's receiver-side incoming azimuth; both provider azimuths remain in raw
evidence. Missing public reports remain missing evidence. See
[Decision 0015](decisions/0015-use-an-import-first-wspr-public-spot-boundary.md).

Each automatic HTTPS acquisition also requests a server-aggregated reporter
activity census for the same half-open UTC window and selected bands. The
`wspr_live_activity_census_capture` record references the exact ClickHouse JSON
attachment; `wspr_live_activity_census_summary` records source, accepted,
duplicate, and malformed row counts, the 10,000-row retained limit, and an
explicit truncation marker. Accepted `wspr_live_activity_census` records contain
one `(cycle_time, reporter)` key with optional normalized reporter grid,
decoded-spot and distinct-station counts, and maximum and median SNR. Overlapping
cumulative queries do not append a key already present in the bundle. Invalid
grids are omitted without discarding otherwise valid activity. Query or response
failure is a typed unsupported census-summary disposition and does not change
the independently committed public-spot result. Census records are contextual
adapter evidence: they have no slot attribution or normalized-record links and
do not create observations or affect current analysis and reports.

The RBN daily-archive boundary uses provider `reverse-beacon-network`, source
`rbn-daily-archive`, acquisition channel `file-import`, and adapter
`antennabench.rbn-daily-archive`. One `rbn_archive_capture` record references
the exact selected ZIP at `attachments/sha256/<digest>`, one summary records
the pinned header, scope, member name, and disposition counts, and one
`rbn_archive_row` record retains each bounded near-raw field array. Accepted
rows link to TX `PublicReport` observations with reporter/heard callsigns,
timestamp, band, exact Hz frequency, CW or RTTY mode, and SNR. Reporter/heard
grids, distance, azimuth, drift, and transmitter power are absent because the
archive does not provide them.

Malformed, wrong-callsign, out-of-window, unselected/unsupported-band,
unsupported-mode, within-archive duplicate, replay duplicate, and replay
conflict outcomes remain adapter records and never become observations. Exact
replay uses the RBN spot identity and content fingerprint only within this
adapter; no cross-file/provider analysis deduplication is added. CW and RTTY
remain different mode strata.

## Explicit V1 Upgrade

`BundleStore::upgrade_v1_to_v2()` accepts only an upgrade-eligible v1 source and
an absent v2 destination. It never writes the source. Every legacy
`RecordSource` maps conservatively to structured provenance; provider-only
sources use `legacy-unspecified` acquisition instead of an invented channel.
Every legacy WSJT-X record becomes generic adapter evidence and retains its
original physical JSONL line exactly. Normalized observations, rig records,
and propagation records receive adapter-evidence backlinks and deterministic
migration mutation membership.

The upgrader verifies projected semantic equivalence, checkpoint/stream
digests, retained evidence counts, destination reopen, and a before/after
snapshot of every source file. `upgrade_v1_to_v3()` applies the same lossless
legacy conversion directly, while `upgrade_v2_to_v3()` preserves v2 evidence
and attachments. The direct and two-step v3 models are deterministic and
equivalent. Explicit `upgrade_v1_to_v5()`, `upgrade_v2_to_v5()`, and
`upgrade_v3_to_v5()` create a new v5 destination. Schema-v3 and schema-v4 armed
cycles become `operator_confirmed`; no command, target, invocation, or
verification fact is invented. Historical schema-v3 WSPR intentions retain an
unknown direction under the manual policy rather than guessing RX or TX. There
is no downgrade to an older schema.

## Schema-V2 Operator Events

Every schema-v2 event separates trusted capture time (`meta.recorded_at`) from
the best-known occurrence time (`occurred_at`). `time_basis` records whether
the occurrence was observed now, reported by the operator, or generated by
recovery/system behavior; optional uncertainty is measured in seconds.

The payload is a tagged value. Lifecycle payloads cover start, explicit
interruption, recovery-detected interruption, resume, end, and abandon.
Correctable operator evidence covers explicit actual-antenna confirmation,
missed slots, bad slots with reasons, and notes. An antenna confirmation always
names the actual antenna independently of the planned slot label.

Corrections are new `event_corrected` records. They target one earlier
correctable event and either retract it or provide a complete typed replacement
with its own occurrence time, time basis, uncertainty, slot, and payload.
Committed stream order decides correction precedence; timestamps and UUID text
never do. Invalid, future, self, correction-to-correction, lifecycle, and
terminal targets leave the prior effective view unchanged and produce a typed
diagnostic.

The current-model projection uses only the effective append-ordered view.
Unknown actual state remains unknown. Multiple active switch, missed, or bad
facts for one slot produce a stable conflict diagnostic, no actual label, and
conservative observation exclusion. See
[Operator Event Semantics](event-semantics.md) for the transition and alignment
contract.

## Local Resource Profile

[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md)
selects one fixed first-product profile, `local-standard-v1`. It is an
operational policy rather than a schema invariant. Bundle storage enforces its
filesystem portion; adapter, analysis/report, and desktop-delivery enforcement
are tracked separately by [#56](https://github.com/rwjblue/antennabench/issues/56)
and [#57](https://github.com/rwjblue/antennabench/issues/57).

The selected modeled-data limits are 4 MiB per root JSON file, 256 KiB per
JSONL line, 128 MiB and 250,000 records per JSONL stream, and 256 MiB plus
500,000 records across modeled files. JSON nesting stops at 64 containers and
a general modeled scalar string stops at 128 KiB; narrower semantic rules
still apply.

Opaque root files and attachments use a separate pool: 512 MiB per file, 2 GiB
total, 4,096 entries, and eight directory levels below `attachments/`.
Strict creation refuses unmodeled root entries. A legacy operation that claims
to be lossless must preserve safe opaque entries within this pool rather than
silently skipping them.

Readers preflight metadata and enforce the same counters while streaming, so a
growing or replaced file cannot bypass the limits. Typed read returns the
whole bundle or no typed bundle. Storage-safe preservation remains separate
from parsing and analysis, and strict writes or live checkpoints never promote
bytes that cross the profile.

Schema-v1 through schema-v6 reads, strict writes, upgrades, attachment access, and
lossless copies use this same fixed profile. Production callers cannot override
it. Tests can inject a tiny equivalent to exercise exact boundaries and
mid-operation failures deterministically. Resource failures expose a stable
code plus profile/version, operation, stage, path, limit, observed value, unit,
retryability, completeness, and evidence-gap fields. Cancellation is checked
during directory traversal, between JSONL records, and at each 64 KiB file-copy
chunk.

## Schema-V6 Runtime Context Layout

[Decision 0025](decisions/0025-use-checkpointed-runtime-contexts-and-operational-diagnostics.md)
defines checkpointed `runtime-contexts.jsonl` and `diagnostics.jsonl` streams.
The runtime-context stream records the content-deduplicated
app build and bounded runtime platform that created or materially acted on the
session. The diagnostics stream records bounded typed material-operation
outcomes and participates in checkpoint verification, recovery, upgrade,
resource accounting, and lossless copy. Both remain distinct from operator,
adapter-source, radio, and normalized scientific evidence.

The immutable manifest references the creator runtime context. Every v6
evidence record names the responsible context, and the existing checkpoint
commits a new context before its first referencing primary evidence
in coherent mutation order. A failure before evidence commits may create a
diagnostic-only revision; the record separately states the primary
`revision_before`, `revision_after`, and `evidence_effect` so consumers do not
infer new scientific evidence from that revision advance.

Runtime contexts are limited to 256 records and 1 MiB, with 4 KiB per line.
Diagnostics are limited to 2,048 records and 16 MiB, with 8 KiB per line,
four cause levels, and 24 typed facts. Required identity/effect fields are never
truncated. Optional details declare deterministic truncation, and checkpoint
status exposes saturation or a safely recorded persistence gap. The writer
makes only one diagnostic-persistence attempt and promises nothing when disk,
process, writer, or verified-checkpoint safety prevents that commit.

The initial operation taxonomy covers session mutation and recovery,
WSPR.live/file acquisition, WSJT-X intake, controller attach/switch/verify,
report rendering/export, and bundle export. Routine polling and routine
successes are omitted. A rejected WSPR.live JSONL member records
`resource.jsonl_line_bytes`, stream, observed and limit bytes, final acquisition
window, unchanged primary revision, `none_committed`, and input/profile/code
change guidance. A capture followed by failed automatic finalization records a
separate `partial` outcome with `primary_evidence_committed`.

Schema-v1 through schema-v5 absence means legacy/unknown, not “no failures.”
They remain unchanged and losslessly copyable; live v6 mutation requires an
explicit non-destructive upgrade. Upgrade retains only the legacy creator app
version actually present, records all unavailable identity as unknown, and
adds the real upgrade-process context. Analysis ignores both new streams.

Build identity is compiled from the package version, source commit/state,
channel, target triple, architecture, optional release tag, and optional
`SOURCE_DATE_EPOCH`. Official release builds require all authoritative inputs
to agree. Runtime identity is limited to OS family/product version, process
architecture, and the fixed application identifier; it contains no host, user,
device, location, home-path, environment, secret, or WebView fingerprint.

## Root Files

- `manifest.json`: schema version, session id, creation time, app version, and
  declared bundle file paths.
- `station.json`: callsign, grid, optional power, and operator notes.
- `antennas.json`: freeform antenna labels plus optional facets and installation
  details.
- `schedule.json`: experiment mode, goal, and planned slots.
- `analysis.json`: analysis generation status and notes.

## Streams

- `events.jsonl`: operator events such as session start, switched, missed slot,
  bad slot, note added, and session end.
- `observations.jsonl`: local decodes, public reports, and imported spots.
- v1 `wsjtx.jsonl`: raw or near-raw WSJT-X adapter records, including
  `all_wspr_decode` for parsed decode rows and `all_wspr_malformed` for
  preserved lines that could not become observations. Live companion records
  use `udp_heartbeat`, `udp_status`, `udp_wspr_decode`, and `udp_close`.
- `rig.jsonl`: rig adapter state and, in schema v5, typed bounded
  antenna-control invocation evidence.
- `propagation.jsonl`: time-scoped propagation context. The implemented NOAA
  SWPC adapter writes observed F10.7 and provisional estimated planetary Kp as
  separate sparse records because the products have different source times.
  `meta.timestamp` is response receipt time, `observed_at` is the upstream UTC
  `time_tag`, and `raw` retains the exact endpoint, selected near-raw object,
  retrieval time, provider/semantics attribution, and available HTTP metadata.
  Repeated unchanged source observations are not appended.

Every v1 JSONL record includes `meta` with schema version, session id,
timestamp, and legacy source. Every v2 record instead uses structured
provenance and mutation membership. V3 through v5 retain that envelope for
every stream.

Offline WSJT-X WSPR log import preserves every nonblank imported line in
`wsjtx.jsonl`. Valid `ALL_WSPR.TXT`-style decode rows also produce
`observations.jsonl` local decodes. Malformed rows are retained as adapter
records with issue details and do not produce observations.

Live WSJT-X parsing preserves the complete UDP datagram as lowercase hex plus
its parsed known fields. In schema v1, supported heartbeat, status, WSPR decode,
and close messages become `wsjtx.jsonl` records and unknown message types are
ignored. In the shipped schema-v2 desktop orchestrator, every admitted datagram
becomes generic `adapter-records.jsonl` evidence: supported messages are
accepted, unknown messages are unsupported, malformed inputs are malformed,
wrong-client inputs are filtered, and duplicate/partial outcomes retain their
declared dispositions. Compatible trailing fields remain in the preserved
datagram without changing the bundle schema.

Only new, on-air, nonduplicate WSPR decodes from a client whose latest status is
in WSPR mode and matches the session station callsign and grid become local
observations. Replayed, off-air, duplicate, semantically invalid, or
insufficiently identified decodes remain auditable WSJT-X records without an
observation.

An accepted decode and its observation share one mutation envelope and carry
bidirectional adapter/normalized-record links. Acquisition resource gaps are
durable partial adapter records when persistence remains possible; affected
intake stops rather than silently treating a prefix as complete.

## Observation Slot Annotations

Observations store computed alignment fields:

- `slot_id`
- `slot_label`
- `slot_confidence`

These fields are derived from schedule, events, and scientific observation
times. They are persisted for auditability and easy downstream use, but they
can be regenerated by normalization. Most observation sources use their
normalized record timestamp directly. Confirmed schema-v4+ WSPR.live rows are
the exception: the durable observation keeps trusted local capture time in
`meta.recorded_at`, its linked adapter record keeps the provider's exact source
time in `source_time`, and analysis projects that source time onto the matching
AntennaBench cycle. The projection accounts for WSPR.live's canonical
even-minute timestamp and AntennaBench's one-second cycle start. It repairs
older affected bundles deterministically only when band, direction, source
time, and fully confirmed antenna occupancy identify one cycle; historical or
unconfirmed imports do not gain inferred attribution.

## Validation

Bundle inspection produces deterministic typed diagnostics. Each diagnostic has
a stable code, wire/structural/semantic/eligibility category, severity, file
and record location, JSON field path, affected operation profiles, and related locations
where useful. JSONL locations include both the zero-based logical record index
and one-based physical line number. Diagnostics are sorted by durable location
and code so callers and tests do not depend on filesystem or hash iteration.

The operation profiles are deliberately different:

- compatibility read rejects ambiguous modeled JSON, unsupported schemas, and
  structural identity/reference failures, but retains semantic warnings
- analysis maps field-, observation-, event-, and slot-scoped semantic problems
  to granular evidence exclusions; only ambiguity that prevents deterministic
  typed interpretation rejects the whole analysis, and persisted alignment
  annotations may be regenerated while their diagnostics remain disclosed
- strict creation rejects warning-bearing authored values; upgrade may accept a
  regenerable derived annotation only when the old source evidence remains
  retained, while warnings that cannot be represented losslessly still block it

Modeled duplicate JSON members are errors because ordinary JSON projection
would silently choose one value. Duplicate members below legacy `raw` evidence
are reported as warnings: typed code does not interpret their meaning, while
lossless copy preserves the original bytes. Unknown schema-v1 fields are also
warnings; compatibility and analysis ignore them in the typed projection, but
strict creation and upgrade must not rewrite them implicitly.

Structural and semantic validation checks:

- root files and records use the expected schema version
- root files and records use the manifest session id
- session, slot, event, observation, adapter, rig, and propagation machine IDs
  are nonempty ASCII of at most 128 bytes for new writes and unique in their
  identity domain
- station callsign/grid text is trimmed and nonempty; antenna labels are
  trimmed, nonempty, unique, control-free, and at most 128 UTF-8 bytes
- planned slot antenna labels resolve exactly once in `antennas.json`
- schedule sequence numbers are unique and strictly increasing in persisted
  order (gaps are valid); start times strictly increase and windows do not
  overlap
- schedules contain a slot, durations are positive, guards consume less than
  the duration, and experiment mode/goal/distinct-antenna shape is coherent
- event and observation slot references point to known planned slots
- present station/observation power is finite and positive; antenna dimensions
  are finite and nonnegative; antenna/observation headings are finite in
  `[0, 360)`
- present observation distance is finite and nonnegative; SNR and drift are
  finite; observation/rig frequencies are positive; slot confidence is finite
  in `[0, 1]`
- normalized propagation floats are finite, nonnegative where the modeled
  quantity cannot be negative, and planetary Kp is in `[0, 9]`
- generated analysis metadata includes its generation timestamp
- persisted slot annotations match regenerated alignment output
- v2 mutation membership and record schema/session identities agree
- normalized v2 records link to existing generic adapter evidence
- attachment digests/sizes, active-plan digests, and committed stream heads
  match `session-state.json`
- schema-v5 command invocations satisfy command/argument/output bounds and
  command-verified readiness resolves to one matching, successful switch and
  verification pair in the same ordered mutation

Use `BundleStore::inspect()` to retain the report beside an optional all-or-none
current projection. `read_for_analysis()` returns a normalized compatibility-safe
projection together with that report so analysis and reporting can preserve
stable reason codes while excluding only affected evidence. `read_current()` retains provider-neutral sidecars and
`read_v2()` exposes the v2 wire model. `read()` applies the compatibility profile,
`read_validated()` requires a completely clean report, and
`read_normalized_validated()` applies the analysis profile after regenerating
observation alignment annotations. The complete policy is recorded in
[Decision 0009](decisions/0009-use-layered-bundle-validation-profiles.md).

Core deliberately does not impose WSPR callsign/grid grammar, exact band-plan
edges, jurisdiction rules, provider quality flags, rig status values, or
provider-specific propagation acceptance ranges. WSJT-X offline and live
adapters validate callsign, locator, power, frequency/band, and supported
message semantics before emitting normalized observations; other adapters and
workflows own equivalent source-specific diagnostics.

`BundleStore::write()` and the v2 authored writers apply the strict-creation
profile before creating any destination file. This catches warning-level
authored values and every modeled non-finite float before `serde_json` could
turn it into `null`. The v1 upgrader instead applies the upgrade profile, so a
representable warning can retain both normalized meaning and source evidence;
it never clamps, trims, renumbers, or otherwise repairs the source.

## Lossless Copies

`BundleStore::copy_losslessly_to()` creates a new bundle directory by copying
the source representation instead of serializing a typed in-memory model.
The complete safe root tree, including unmodeled opaque root entries and the
nested `attachments/` tree, retains its original bytes. The source need not be safe for typed
interpretation: duplicate modeled members, duplicate legacy raw members, and
unknown fields remain preservable. The manifest and filesystem layout must
still be safe to traverse. The source is never modified.

The destination must not already exist or be inside the source; lossless copy
never overwrites or merges. Symbolic links and non-file/non-directory
filesystem entries are rejected. If copying or verification fails, the newly
created destination is removed when it remains safe to do so. Verification
reopens the copied manifest and checks the storage layout; it does not require
typed projection of the preserved evidence.
The same operation supports v1 and v2 and requires the destination suffix to
match the source schema.

## Fixtures

The canonical sample-report input is:

```text
fixtures/session-bundles/canonical-sample-report.session.wsprabundle/
```

It is a purpose-built, redistribution-safe synthetic whole-station A/B session
with two antennas and two bands. It retains source-shaped local and imported
inputs, representative operator events, exclusions, and missing optional data.
It is the designated input for sample rendering; it is not evidence for an
antenna winner or a scientifically valid comparison.

Fixture provenance, synthetic-data policy, demonstrated cases, and the roles of
the smaller focused fixtures are maintained in
`fixtures/session-bundles/README.md`.
