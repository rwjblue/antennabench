# 0025: Use Checkpointed Runtime Contexts And Operational Diagnostics

Date: 2026-07-19

## Status

Accepted.

## Context

A schema-v5 field session retained successfully committed WSPR.live source
responses but remained `running` after final collection. The decisive backend
failure was that one proposed `adapter-records.jsonl` member exceeded the
256 KiB physical-line limit. The bundle did not retain the rejected operation,
its typed resource facts, the build that performed it, or the runtime platform.
Diagnosing the copied bundle required source access and a custom replay.

`manifest.json` currently identifies only the application version that created
the bundle. Provider provenance identifies an adapter, not a later AntennaBench
process that recovered, imported into, or mutated the session. In-memory errors
also use several unrelated types and sometimes flatten causes into display
strings. A portable session therefore cannot explain material operational
failures after the UI and process are gone.

The bundle must remain the canonical portable record. Operational metadata must
not become radio, adapter-source, or operator evidence; must remain bounded and
private by default; and cannot promise persistence when storage itself is lost
or unsafe to write.

## Decision

Bundle schema version 6 adds two dedicated append-only streams:

- `runtime-contexts.jsonl` records the bounded AntennaBench build and runtime
  platform contexts that create or materially act on the session; and
- `diagnostics.jsonl` records bounded typed outcomes for material operational
  failures, partial successes, recovery, and other explicitly retained
  operational states.

Both streams participate in the existing `session-state.json` checkpoint
protocol selected by [Decision 0010](0010-checkpoint-append-only-live-session-mutations.md).
They are operational metadata, never experiment evidence. There is no second
checkpoint, external log, telemetry service, or mutable diagnostics database.

### Topology And References

The schema-v6 manifest remains immutable bootstrap metadata. It declares both
streams and contains `creator_runtime_context_id`. The referenced creator
context is the first committed runtime-context record. This preserves an
immutable creator reference while using one context type and avoiding a second
copy of build/platform fields in the manifest.

Every schema-v6 operator, adapter, observation, rig, propagation, and diagnostic
record carries `runtime_context_id` in its record metadata. Static plan members
are attributable through the manifest's creator reference. A diagnostic refers
to lower-level evidence by bounded typed IDs; it never embeds another stream's
record or raw payload.

The checkpoint records committed heads for both streams and a small
`diagnostics_status` value:

- `complete`: no known diagnostic record was omitted;
- `saturated`: a semantic count/byte bound prevented a new diagnostic record,
  with a saturating omitted count, first-omitted time, and stable reason code;
- `gap`: a diagnostic could not be represented safely but a later independent
  checkpoint could durably record that fact; or
- absent on schema-v1 through schema-v5, which means legacy/unknown rather than
  “no failures.”

`complete` describes only the guarantees in this decision. It cannot rule out
abrupt death or storage failure before a record could be committed.

### Runtime Context Contract

A runtime context has these versioned fields:

| Field | Contract |
| --- | --- |
| `contextId` | `ctx_` plus lowercase SHA-256 of the canonical versioned build/platform fields. |
| `schema` | Fixed record contract identifier, initially `runtime_context.v1`. |
| `firstRecordedAt` | Trusted UTC time when this context was first committed to this bundle; excluded from the context digest. |
| `mutation` | Ordinary checkpoint mutation membership; excluded from the context digest. |
| `build.appVersion` | AntennaBench semantic-version string when known; null means legacy/unknown. |
| `build.sourceCommit` | Full 40- or 64-character lowercase hexadecimal source commit when available; otherwise null. This is the primary build identity. |
| `build.sourceState` | `clean`, `dirty`, or `unknown`; dirty and unknown builds cannot claim official status. |
| `build.buildChannel` | `official_release`, `development`, `local`, or `unknown`. |
| `build.releaseTag` | Exact release tag only for a verified official release; otherwise null. |
| `build.targetTriple` | Compile-time Rust target triple, or null when unavailable. |
| `build.buildArchitecture` | Compile-time executable architecture, or null when unavailable. |
| `build.buildTimestamp` | Optional UTC value with source `source_date_epoch`; otherwise null. |
| `platform.osFamily` | Bounded normalized family such as `macos`, `linux`, or `windows`; null means unknown. |
| `platform.osVersion` | Bounded product version or null; no kernel build, device model, or serial. |
| `platform.runtimeArchitecture` | Runtime process architecture or null. |
| `platform.applicationId` | Fixed application/package identity when available; otherwise null. |

The canonical digest excludes `contextId`, `firstRecordedAt`, and `mutation`.
An identical build/platform tuple reuses its existing context and does not append another
line. A changed build, source state, target, OS version, runtime architecture,
or application identity creates a distinct context. Two processes with the
same tuple intentionally share a context; the contract identifies the acting
build/platform, not a device or process fingerprint.

The digest input is domain-separated and independent from serializer map order.
It begins with the UTF-8 bytes `antennabench.runtime-context.v1`, followed by one
zero byte. It then encodes, in the table's build/platform field order,
`app_version`, `source_commit`, `source_state`, `build_channel`, `release_tag`,
`target_triple`, `build_architecture`, build-timestamp value, build-timestamp
source, `os_family`, `os_version`, `runtime_architecture`, and `application_id`.
Each present UTF-8 value is prefixed by its unsigned 32-bit big-endian byte
length; null is encoded as `0xffffffff`. SHA-256 covers that exact byte string.

Official release construction must inject version, tag, clean source commit,
target, architecture, and channel from the same authoritative inputs used by
the release manifest, and fail when they disagree. `build_timestamp` is omitted
unless it derives from an explicit `SOURCE_DATE_EPOCH`; a wall-clock compile
time is never invented. Local and development builds use their actual commit
and dirty state when available and remain explicitly local/development.

The source commit is primary because it connects the bundle to reviewable
source while remaining available before the final executable exists. An
embedded digest of the executable that contains that digest is self-referential
and is not required. Release archives retain their external artifact digests
and provenance separately.

WebView/browser versions are omitted. They are not currently decisive at the
Rust-owned filesystem, validation, adapter, and checkpoint boundaries, vary in
availability and precision across platforms, and add fingerprinting cost. A
future failure that demonstrates specific value requires a new decision.

### Diagnostic Contract

Each diagnostic is a complete newline-terminated record with:

| Field | Contract |
| --- | --- |
| `diagnosticId` | Collision-resistant record identity. |
| `correlationId` | Groups one multi-stage logical operation across retries or follow-up stages. |
| `attemptId` | Idempotence identity for one submission; an exact transport retry reuses it. |
| `mutation` | Ordinary checkpoint mutation membership for this diagnostic-only or post-effect commit. |
| `runtimeContextId` | Actor context, verified against the committed context stream. |
| `occurredAt` | Trusted UTC occurrence time; stream order remains authoritative. |
| `operation` | Stable operation kind such as `wspr_live_acquisition`, `conductor_mutation`, `checkpoint_recovery`, or `report_render`. |
| `phase` | Stable phase such as `preflight`, `acquire`, `normalize`, `checkpoint`, `finalize`, `recover`, `render`, or `write_destination`. |
| `code` | Stable machine code; display text is not the contract. |
| `summary` | One bounded path-free sentence selected by the code, without arbitrary interpolation. |
| `outcome` | `failed`, `partial`, `recovered`, `cancelled`, `completed_idempotently`, or `unknown`. |
| `severity` | `info`, `warning`, or `error`. |
| `revisionBefore` | Session revision when the primary operation began, when known. |
| `revisionAfter` | Revision containing the primary operation's evidence effect, or the same revision when none committed. |
| `diagnosticRevision` | Revision that commits this diagnostic record. |
| `evidenceEffect` | `none_committed`, `earlier_evidence_retained`, `primary_evidence_committed`, `prior_commit_reused`, `cancelled_before_effect`, or `unknown`. |
| `retry` | `retryable`, `requires_state_change`, `requires_input_change`, `not_retryable`, or `unknown`, plus a stable guidance code. |
| `targets` | At most eight typed references such as adapter/source, mutation, slot/intent, acquisition window, or existing rig-record IDs. |
| `causes` | Ordered outer-to-inner stable cause codes with bounded typed facts. |
| `detailStatus` | `complete` or `truncated`, with an omitted-fact count when optional detail was dropped. |

Cause depth is four and a record contains at most 24 facts in total. Fact names
are bounded lowercase machine identifiers. Values are booleans, bounded
identifiers, signed/unsigned integers, UTC timestamps, or fixed enums. General
strings, filesystem paths, URLs, HTTP bodies, stack traces, environment values,
and Rust `Display`/`to_string()` output are not diagnostic facts. A safe static
summary helps older readers, but code, phase, facts, and effect are authoritative.

The taxonomy supports more outcomes than the initial retention policy. Schema
v6 retains:

- every safely writable material failure, partial-success outcome, recovery,
  unknown-effect outcome, and post-effect cancellation;
- completed-idempotent recovery when it is needed to explain why bytes were
  reused rather than appended; and
- no routine polling, successful report reads, ordinary successful mutations,
  or cancellation before any material operation begins.

Successful evidence records already carry their actor context and mutation
identity. Omitting routine success diagnostics prevents a second noisy history
that merely restates evidence.

The initial operation enum is `session_mutation`, `checkpoint_recovery`,
`wspr_live_acquisition`, `adapter_file_import`, `wsjtx_intake`,
`antenna_controller_attach`, `antenna_controller_switch`,
`antenna_controller_verify`, `report_render`, `report_export`, and
`bundle_export`. The initial phase enum is `admission`, `preflight`, `acquire`,
`parse`, `normalize`, `serialize`, `checkpoint`, `finalize`, `recover`,
`render`, and `write_destination`. New variants require a schema-owned additive
change and unknown-variant compatibility behavior; implementations do not place
freeform values in either field. Codes and guidance codes are lowercase
dot-separated schema-owned identifiers.

Target kinds are `adapter`, `source`, `mutation`, `record`, `slot`, `intent`,
and `acquisition_window`. The first six contain one bounded ID; an acquisition
window contains exact UTC start/end fields. Cause facts retain schema-declared
order. Targets use the enum order above and then their encoded ID/time tuple, so
optional-detail truncation is deterministic across input order.

### Revision, Ordering, And Durability

A new runtime context is appended as the first member of the same checkpointed
mutation as the first evidence or diagnostic record that references it.
Diagnostics follow any primary evidence members. No operation commits a dangling
context reference.

Primary operations and diagnostics are ordered as follows:

1. The writer acquires and verifies the ordinary exclusive lease and current
   checkpoint.
2. It performs bounded preflight before changing committed bytes.
3. A successful primary mutation commits its complete evidence batch, including
   a new context when needed.
4. If a later stage fails, a separate diagnostic mutation commits afterward and
   states the primary revision before/after and evidence effect.
5. If the primary operation fails before committing evidence, a diagnostic-only
   mutation advances the bundle revision and records identical
   `revision_before`/`revision_after` values for the primary effect.

The diagnostic commit's own revision is distinct from the primary evidence
effect. Readers therefore never mistake a revision advance caused only by
operational metadata for new radio/operator evidence.

Exact transport retry reuses `attempt_id`. If the primary mutation or its
diagnostic already committed, the writer returns that existing result and adds
nothing. An operator-requested retry after changing state or input receives a
new attempt ID and may retain the correlation ID. Diagnostic mutations use
their own storage mutation identity, so they do not overwrite the primary
operation's idempotence result. Implementations must index or scan the bounded
diagnostic stream rather than relying only on “last mutation ID.”

Stale revision, external modification, unsafe recovery state, or loss of writer
capability blocks any attempted diagnostic append to the affected bundle. The
in-memory/UI result must state `not_persisted` and its reason. AntennaBench does
not write around an unverified checkpoint just to log an error.

The guarantee is precise: after a logical, validation, policy, adapter, or
resource failure, AntennaBench attempts one bounded diagnostic commit only when
the bundle remains verified and safely writable. Success acknowledges the new
diagnostic revision. There is no guarantee for disk full, lost/inaccessible
storage, abrupt process kill, power loss outside the platform durability
boundary, an unsafe/external modification state, or failure of the diagnostic
write/checkpoint itself. Failure to persist a diagnostic never recursively
attempts to diagnose itself.

### Bounds And Retention

These schema-semantic limits are narrower than `local-standard-v1`:

| Boundary | Limit |
| --- | ---: |
| Runtime-context record, including newline | 4 KiB |
| Runtime contexts per bundle | 256 |
| Runtime-context stream | 1 MiB |
| Diagnostic record, including newline | 8 KiB |
| Diagnostic records per bundle | 2,048 |
| Diagnostic stream | 16 MiB |
| Diagnostic summary | 256 UTF-8 bytes |
| Code, operation, phase, guidance code, or fact name | 64 lowercase ASCII bytes |
| Typed identifier value | 128 UTF-8 bytes |
| Target references | 8 |
| Cause depth | 4 |
| Typed facts per record | 24 |

Serialization preflights complete records. Required identity, code, outcome,
effect, revision, and retry fields are never truncated. Optional targets/causes
use deterministic declared order; if they cannot fit, the record sets
`detail_status: truncated` and counts omitted facts. It never stores a partial
JSON line.

Retention is first-committed append order. There is no sampling, eviction,
rewriting, or “latest” replacement. When a diagnostic count/byte limit is
reached, later primary operations may still proceed if their own budgets allow,
but the checkpoint must become `saturated` when that small status update can be
committed. The UI/report must show that later outcomes may be absent. If even
the status update cannot commit, only the ephemeral `not_persisted` result is
possible. Exhausting runtime-context capacity blocks a mutation that requires a
new actor context; reusing an already committed identical context remains
allowed.

### Privacy And Sharing

Persisted diagnostics use an allowlist, not after-the-fact redaction:

| Field family | Bundle default | Support summary | Full report | Compact/public/hosted default |
| --- | --- | --- | --- | --- |
| App version, source commit/state, channel, release tag | Included | Included | Explicit opt-in | Excluded |
| Target triple, build/runtime architecture | Included | Included | Explicit opt-in | Excluded |
| OS family/product version, fixed application ID | Included | Included | Explicit opt-in | Excluded |
| Reproducible build timestamp and operation times | Included when known | Included | Explicit opt-in | Excluded |
| Stable code, operation, phase, outcome, revisions, effect, retry | Included | Included | Explicit opt-in | Excluded |
| Bounded source/slot/intent/mutation/window IDs already present in the bundle | Included when relevant | Included only by the support-summary allowlist | Explicit opt-in | Excluded |
| Hostname, username, device serial/model, hardware fingerprint | Prohibited | Prohibited | Prohibited | Prohibited |
| Raw coordinates or newly derived precise location | Prohibited | Prohibited | Prohibited | Prohibited |
| Environment variables, secrets, credentials | Prohibited | Prohibited | Prohibited | Prohibited |
| Arbitrary/native/home paths, general URLs or query strings | Prohibited | Prohibited | Prohibited | Prohibited |
| HTTP bodies, stack traces, controller output, arbitrary logs | Prohibited; existing separately approved evidence is referenced only by ID | Prohibited | Governed by its existing explicit evidence policy, never copied into diagnostics | Prohibited |

Lossless bundle copy/export necessarily preserves the approved context and
diagnostic streams byte for byte. The future support-summary command uses the
fixed whitelist above, is deterministic and bounded, and declares legacy,
truncated, omitted, or persistence-gap state. It never claims “no failures.” A
schema-v6 `complete` checkpoint permits only “no recorded material diagnostics
within the format's guarantees,” alongside the storage/process non-guarantees.

Full evidence HTML excludes runtime/diagnostic history by default and may
include the safe structured view only through a separate explicit choice.
Compact summary HTML never includes it. Future hosted sharing follows the
compact/public default unless a separately approved explicit disclosure
contract is added. Existing controller-evidence inclusion remains a separate
choice and never authorizes copying controller output into diagnostics.

### Compatibility And Upgrade

This contract requires schema version 6 and version-owned `v6` core APIs under
[Decision 0024](0024-use-version-owned-core-schema-modules.md). It is not added
as optional fields to schema v5.

Schema-v1 through schema-v5 bundles remain readable, reportable, and losslessly
copyable without modification. Their absent context/diagnostic streams are
reported as legacy/unknown, never as a clean diagnostic history. A live mutation
requires an explicit non-destructive upgrade to a new schema-v6 destination.

Upgrade creates:

- a `legacy_creator` context containing only the old manifest's actual
  `app_version` and creation time, with all unavailable build/platform fields
  null/unknown;
- a complete context for the upgrade process; and
- an upgrade/recovery diagnostic that names the source schema and states that
  earlier operational history is unavailable.

No commit, platform, channel, or timestamp is inferred. Validation verifies
manifest/context references, context digests, diagnostic codes/facts, checkpoint
heads, and saturation/gap status. Fixed resource accounting includes both
streams. Recovery, checkpointed export, inspection, and lossless copy treat them
as modeled data. Analysis and scientific conclusions do not read them.

## Wire Examples

The following examples omit unrelated manifest/checkpoint fields but are valid
shapes for the selected contract.

### New Session Creation

```json
{
  "schemaVersion": 6,
  "sessionId": "018fbf58-8ec2-7a43-a7a0-7b331b1ce201",
  "createdAt": "2026-07-19T14:00:00Z",
  "creatorRuntimeContextId": "ctx_68b55d43ba3113d787296c2b5995a45e4d4644d3e2c3a69ef8fc11c823cfaf13",
  "files": {
    "runtimeContexts": "runtime-contexts.jsonl",
    "diagnostics": "diagnostics.jsonl",
    "sessionState": "session-state.json"
  }
}
```

```json
{"schema":"runtime_context.v1","contextId":"ctx_68b55d43ba3113d787296c2b5995a45e4d4644d3e2c3a69ef8fc11c823cfaf13","firstRecordedAt":"2026-07-19T14:00:00Z","mutation":{"mutationId":"018fbf58-a4f9-77ac-9233-f29b0444ad70","memberIndex":1,"memberCount":1},"build":{"appVersion":"0.1.0","sourceCommit":"7d9f4fef52e831b1e5689ca7d2a93b0a56122fd4","sourceState":"clean","buildChannel":"official_release","releaseTag":"v0.1.0","targetTriple":"aarch64-apple-darwin","buildArchitecture":"aarch64","buildTimestamp":{"value":"2026-07-19T12:00:00Z","source":"source_date_epoch"}},"platform":{"osFamily":"macos","osVersion":"15.5","runtimeArchitecture":"aarch64","applicationId":"com.rwjblue.antennabench"}}
```

The initial checkpoint commits that context, references it as creator, and uses
`diagnosticsStatus.state: complete` with zero diagnostic records.

```json
{
  "revision": 0,
  "streams": {
    "runtimeContexts": { "records": 1, "lastRecordId": "ctx_68b55d43ba3113d787296c2b5995a45e4d4644d3e2c3a69ef8fc11c823cfaf13" },
    "diagnostics": { "records": 0, "lastRecordId": null }
  },
  "diagnosticsStatus": { "state": "complete", "omittedCount": 0 }
}
```

### Reopen And Mutation By A Different Build

```json
{"schema":"runtime_context.v1","contextId":"ctx_c74c37b66aed1d6a0ee67a259dc8a41052c5a72a0acfea81ad7c4bb3bdeeae23","firstRecordedAt":"2026-08-02T09:30:00Z","mutation":{"mutationId":"018fc736-c7d0-7f29-9a39-46fcab0317fd","memberIndex":1,"memberCount":2},"build":{"appVersion":"0.2.0-dev","sourceCommit":"8a20c50815e324e088b5cb85282f17cefbf35f15","sourceState":"dirty","buildChannel":"development","releaseTag":null,"targetTriple":"x86_64-apple-darwin","buildArchitecture":"x86_64","buildTimestamp":null},"platform":{"osFamily":"macos","osVersion":"15.6","runtimeArchitecture":"x86_64","applicationId":"com.rwjblue.antennabench"}}
```

The first v6 evidence record produced by that actor includes:

```json
{"meta":{"schemaVersion":6,"sessionId":"018fbf58-8ec2-7a43-a7a0-7b331b1ce201","recordedAt":"2026-08-02T09:31:00Z","runtimeContextId":"ctx_c74c37b66aed1d6a0ee67a259dc8a41052c5a72a0acfea81ad7c4bb3bdeeae23","mutation":{"mutationId":"018fc736-c7d0-7f29-9a39-46fcab0317fd","memberIndex":2,"memberCount":2}},"payload":{"type":"note_added","text":"Checked feedline before resuming."}}
```

The context and evidence record are members of the same checkpointed mutation.

### Failed WSPR.live Import With No Evidence Commit

```json
{"schema":"operational_diagnostic.v1","diagnosticId":"018fc741-07e5-770f-a74e-f868c2b9305f","correlationId":"018fc740-c28d-76af-b8cf-78c9ad804cb9","attemptId":"018fc740-ea78-72eb-b893-e29b43194230","mutation":{"mutationId":"018fc741-3443-7d38-bf97-711a589c2081","memberIndex":1,"memberCount":1},"runtimeContextId":"ctx_c74c37b66aed1d6a0ee67a259dc8a41052c5a72a0acfea81ad7c4bb3bdeeae23","occurredAt":"2026-08-02T09:35:00Z","operation":"wspr_live_acquisition","phase":"preflight","code":"resource.jsonl_line_bytes","summary":"The captured adapter record exceeded the bounded JSONL line size.","outcome":"failed","severity":"error","revisionBefore":14,"revisionAfter":14,"diagnosticRevision":15,"evidenceEffect":"none_committed","retry":{"disposition":"requires_input_change","guidanceCode":"reduce_or_update_adapter_batch"},"targets":[{"kind":"source","id":"wspr-live"},{"kind":"acquisition_window","start":"2026-08-02T09:30:01Z","end":"2026-08-02T09:32:00Z"}],"causes":[{"code":"resource.jsonl_line_bytes","phase":"serialize","facts":[{"name":"stream","value":{"type":"enum","value":"adapter_records"}},{"name":"observed_bytes","value":{"type":"u64","value":263410}},{"name":"limit_bytes","value":{"type":"u64","value":262144}}]}],"detailStatus":{"state":"complete","omittedFactCount":0}}
```

The diagnostic line is the only new stream member. Its checkpoint advances the
bundle to revision 15, while the primary evidence effect correctly remains
revision 14. This is sufficient to explain the motivating field failure without
source replay.

### Evidence Commit Followed By Finalization Failure

```json
{"schema":"operational_diagnostic.v1","diagnosticId":"018fc745-dfbf-7ecf-bb34-b08ccf34df09","correlationId":"018fc744-a527-73a0-b95b-43da174a6855","attemptId":"018fc744-d542-7c61-9531-ad8b7c346a08","mutation":{"mutationId":"018fc745-fae7-7457-876a-7b22464f3e36","memberIndex":1,"memberCount":1},"runtimeContextId":"ctx_c74c37b66aed1d6a0ee67a259dc8a41052c5a72a0acfea81ad7c4bb3bdeeae23","occurredAt":"2026-08-02T09:40:00Z","operation":"wspr_live_acquisition","phase":"finalize","code":"session.finalization_rejected","summary":"Public evidence committed, but the session could not be finalized.","outcome":"partial","severity":"error","revisionBefore":20,"revisionAfter":21,"diagnosticRevision":22,"evidenceEffect":"primary_evidence_committed","retry":{"disposition":"requires_state_change","guidanceCode":"refresh_and_retry_finalization"},"targets":[{"kind":"source","id":"wspr-live"},{"kind":"mutation","id":"018fc744-edab-7a5d-83b5-4f020ec413d2"}],"causes":[{"code":"session.stale_revision","phase":"finalize","facts":[{"name":"expected_revision","value":{"type":"u64","value":20}},{"name":"observed_revision","value":{"type":"u64","value":21}}]}],"detailStatus":{"state":"complete","omittedFactCount":0}}
```

Revision 21 contains the capture. Revision 22 contains only the diagnostic, so
the copied bundle truthfully says the evidence was retained.

### Diagnostic Persistence Failure

If the diagnostic append or checkpoint cannot be made safe, no diagnostic JSONL
record is claimed. The command returns an ephemeral typed companion result:

```json
{
  "operationOutcome": {
    "code": "resource.jsonl_line_bytes",
    "evidenceEffect": "none_committed"
  },
  "diagnosticPersistence": {
    "status": "not_persisted",
    "reasonCode": "storage.no_safe_diagnostic_append",
    "guarantee": "none"
  }
}
```

This object is UI/process state, not bundle evidence. AntennaBench makes no
recursive write attempt and a later reader cannot infer a missing record. If a
later independently safe checkpoint can establish the gap, it sets
`diagnostics_status: gap`; otherwise the absence remains an explicit
non-guarantee of the format.

## Rollout

Implementation follows the dependency order in tracking issue #177:

1. #180 introduces schema-v6/version-owned models, release-derived build
   identity, context digest/deduplication, creator/legacy contexts, context
   references on v6 records, checkpoint heads, upgrades, bounds, and fixtures.
2. #181 introduces the diagnostic taxonomy, typed in-memory causes, material
   retention matrix, transaction/idempotence behavior, saturation/gap state,
   the motivating field regression, and partial-success tests.
3. #179 adds local historical presentation and the deterministic safe support
   summary, with legacy/gap/saturation states and explicit full-report inclusion.

Each slice updates the same bundle, architecture, event/resource, and operator
references. No child may weaken the privacy allowlist, make contexts optional on
new v6 records, or treat operational diagnostics as scientific evidence without
revising this ADR.

## Consequences

- A copied v6 bundle can identify every distinct build/platform that materially
  acted on it and explain safely persisted failures without source replay.
- Diagnostic-only commits advance the bundle revision, so UI and retry code must
  refresh the checkpoint after a failed material operation.
- Two new bounded streams and explicit v6 upgrades add storage and validation
  work, but reuse the proven checkpoint protocol.
- The contract honestly exposes legacy, saturation, and persistence gaps instead
  of fabricating “no failures.”
- Default reports and hosted/public output remain free of operational metadata.

## Alternatives Rejected

### One Unified Operations Stream

Repeating build/platform fields on every outcome wastes bounded space, while
mixing context declarations and outcomes complicates reference validation. Two
streams give each record one role and still share one atomic checkpoint.

### Store Diagnostics In Operator Or Adapter Evidence

Operational failures are not things the operator did or radio/source
observations. Mixing them would make analysis eligibility and privacy behavior
ambiguous.

### A Separate Diagnostic Checkpoint

Independent revisions can disagree about which runtime context or evidence
operation a diagnostic describes. The existing single checkpoint already gives
all modeled streams one coherent committed interpretation.

### Pre-Log Every Attempt

Writing an “attempt started” record before every operation adds noise, changes
revisions before useful work, and still cannot guarantee a completion record
after process/storage loss. The selected outcome-first policy records material
states after their evidence effect is known.

### Mutable Ring Buffer Or External Log

Eviction would rewrite portable history and an external log would not travel
with the canonical bundle. Fixed append bounds plus explicit saturation are
deterministic and honest.

### Include WebView, Device, Or Executable Fingerprints

These fields add instability or privacy cost without explaining the motivating
Rust-owned failure. Source/build identity and bounded OS/runtime facts are the
approved diagnostic boundary.
