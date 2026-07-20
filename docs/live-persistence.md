# Live Persistence And Recovery

This technical reference defines how an active session becomes durable and how
AntennaBench recovers after interruption. Schema v2 owns the complete live
mutation and recovery protocol; schema v3 through v5 reuse its checkpoint,
locking, and verified snapshot boundaries. For the bundle mental
model, start with [Session Bundles](bundle-format.md).

Live mutation is owned by `antennabench-storage`. A `LiveSessionV2` or
`LiveSessionV3` holds one OS-backed exclusive lock for its lifetime. Callers
provide typed records and an expected checkpoint revision; the writer assigns
the trusted capture time and mutation envelope, validates the complete batch,
appends in a fixed order, synchronizes every changed stream, and promotes one
checkpoint.

The public live-session facade and its hook, failpoint, mutation, receipt,
recovery-report, and typed-error surface remain in `storage::live`. Private
implementation ownership is split by invariant: `mutation` prepares, validates,
accounts for, appends, and recognizes idempotent v2/v3/v5 mutations;
`checkpoint` loads committed prefixes and constructs, publishes, reopens, and
exactly verifies checkpoints; `recovery` resolves pending plan generations,
stream tails, checkpoint temporary files, and recovery artifacts;
`attachments` verifies and durably copies content-addressed files;
`durability` owns the platform-specific sync, replace, publish, and capability
probe primitives; and `lock` owns advisory-lock acquisition. These modules are internal seams behind the same synchronous
facade, not additional persistence authorities.

Schema-v1 bundles remain immutable inputs. They must be explicitly upgraded to
a checkpointed schema in a new `.session.antennabundle` before these APIs will mutate
them.

## Durable Layout

Static v2 bundles retain root plan files for compatibility. A live draft edit
creates a complete generation without rewriting those roots:

```text
example.session.antennabundle/
  manifest.json
  session-state.json                 current commit point
  session-state.previous.json        prior verified checkpoint
  plan-generations/
    <generation-id>/
      station.json
      antennas.json
      schedule.json
      generation.json                base revision and plan digests
  events.jsonl
  adapter-records.jsonl
  observations.jsonl
  rig.jsonl
  propagation.jsonl
  attachments/sha256/<digest>
  .antennabench.lock                 transient lock inode
  .session-state.next.json           transient checkpoint staging path
```

`session-state.json` selects either the compatible root plan or one generation
directory. A generation becomes visible only when the checkpoint naming its
identity and file digests is promoted. A selected plan is immutable after the
session starts.

The lock file's existence has no liveness meaning. The open file description
and its OS lock are the lease. Because spawning a child process clones the
descriptor table, a lock being released can stay transiently pinned by the
child until it execs; acquisition absorbs that with a brief bounded retry
before failing closed with `WriterBusy`, and never waits on a genuine
concurrent writer beyond that budget.

Completed checkpointed exports exclude the lock, temporary checkpoint,
previous checkpoint, uncommitted stream tails, and orphan generations.

## Append And Acknowledgement Boundary

`LiveMutationV2` declares one mutation ID, the caller's expected revision, and
all members. The writer overwrites untrusted record envelope values with schema
version 2, the bundle session ID, its clock value, the mutation ID, and the
complete member count. Member indexes must be contiguous. UUID-v4 IDs from
trusted Rust are available through `LiveSessionV2::allocate_id`; tests replace
the clock and ID source through `LivePersistenceHooks`.

Physical append order is adapter evidence, normalized observations, operator
events, rig records, then propagation records. Thus raw adapter evidence is
written before a normalized record that depends on it. Every line is complete
and newline terminated before serialization leaves memory. Evidence may append
only while running; lifecycle events use the event reducer and expected
revision contract. Before writing, the storage profile checks every serialized
line, next per-stream byte/record head, and aggregate modeled byte/record total;
an over-budget mutation never advances the checkpoint.

After all changed streams synchronize, the writer serializes and synchronizes
`.session-state.next.json`, retains the old checkpoint as
`session-state.previous.json`, atomically replaces the current checkpoint,
synchronizes the bundle directory, reopens the checkpoint, and only then
acknowledges. The current checkpoint therefore describes either the prior
prefixes or the complete next batch. A retry with an already committed mutation
ID returns the existing revision when the content matches and fails on a
conflicting reuse.

## Snapshots And Export

`BundleStore::read_v2_checkpointed()` takes a shared OS lock and reads exactly
the selected plan plus each committed stream prefix. It does not parse or expose
uncommitted tail bytes. `export_v2_checkpointed_to()` holds the same shared lock
while it creates and verifies a new static v2 destination, copies all durable
content-addressed attachments, and excludes transient/recovery-working files.

Static `read_v2()` remains useful for already quiescent bundles. Active report
and export code should use the checkpointed APIs so one derived result cannot
mix revisions.

Opening a bundle for activation or report is an observational checkpointed
read. It does not acquire a writer, clean recovery artifacts, append an
interruption, or otherwise change bundle bytes. The desktop loads only the
committed report for report intent. Work intent crosses the recovery boundary
only when it subsequently requests the active-session conductor; that first
conductor load owns recovery and returns the authoritative post-recovery
lifecycle and revision. The frontend reconciles those fields into its session
summary and refreshes a report that recovery made stale. It never issues an
implicit Start or Resume action.

Schema v3 provides corresponding `read_v3_checkpointed()` and
`export_v3_checkpointed_to()` boundaries. Its writer appends correctable
operator events and attachment-backed adapter evidence plus observations while
preserving the same expected-revision, digest, lock, and checkpoint rules.
The same versioned writer carries schema v4 through v6. Schema v5 adds
`append_antenna_control`: a failed attempt commits one or two rig records with
no event; successful command-authorized readiness commits switch, verification,
and armed event as one ordered mutation. Injected failure before checkpoint
promotion exposes the prior revision, while failure after promotion exposes
the complete next revision. Exact retry returns the committed receipt and a
conflicting mutation-ID reuse fails. Once an automatic switch process has
completed, its captured records and one stable mutation ID wait for the desktop's
single foreground permit. Admission contention cannot rerun the process, rewrite
its exit result as a switch failure, or disarm the association; persistence then
uses the ordinary current-revision and idempotent-mutation checks. A real stale
authority, lifecycle change, validation error, or durable write failure still
blocks automation and requires operator review.

Active Run keeps a compact Rust-owned projection of the current manifest,
checkpoint, schedule, operator events, and controller evidence. The five-second
conductor, controller, and WSJT-X status polls validate the small checkpoint
document and committed stream lengths, then read that projection; they do not
reparse adapter, observation, propagation, runtime-context, or diagnostic
streams. A changed checkpoint or unexpected stream length fails closed instead
of serving a mixed revision. Full checkpointed reads remain at open, recovery,
report/export, and explicit integrity boundaries. Successful conductor,
controller, WSJT-X, and WSPR.live commits refresh the projection from the
writer's committed snapshot, so evidence growth cannot make routine polling
starve final acquisition or session completion.

Schema v6 adds checkpointed `runtime-contexts.jsonl` and `diagnostics.jsonl`.
`append_diagnostic` commits the active/new runtime context before its first
reference and then one typed diagnostic under the same checkpoint. Exact
attempt retries reuse the existing diagnostic; conflicting reuse fails. The
stream stops at 2,048 records, 16 MiB, or 8 KiB per record and records a
checkpoint-level saturated status instead of rewriting history. Diagnostic
persistence is attempted once and never logs its own failure recursively.

## Recovery

`BundleStore::recover_v2()` acquires the exclusive lock. It selects the highest
valid current/previous checkpoint whose plan and committed prefixes verify.
Malformed records or digest changes inside that prefix are fatal; recovery does
not skip an interior line.

Bytes beyond committed heads are handled as one declared mutation:

- one complete, valid batch rolls forward by promoting the next checkpoint;
- an exact repeated committed mutation is truncated as an idempotent retry;
- torn, malformed, incomplete, or conflicting bytes are first copied exactly
  into `attachments/sha256/` with a separate JSON metadata attachment naming
  source stream, committed offset, detection time, and diagnosis, then the
  stream returns to the committed length;
- a complete pending plan generation rolls forward, while partial or ambiguous
  generations are preserved file-by-file before cleanup; and
- cleanup, synchronization, permission, or capacity failure returns an exact
  error instead of claiming recovery succeeded.

If the selected/recovered lifecycle is `running`, recovery appends one
idempotent recovery-system `interruption_detected` event and checkpoints the
session as `interrupted` before returning. Schema-v6 recovery then attempts one
typed recovery diagnostic that references the recovery runtime context and
states whether earlier evidence was retained. The recovery report independently
returns `persisted`, `not persisted`, or `not applicable` for that companion
record, so diagnostic failure does not undo or disguise completed recovery.

Recovery scans and truncates uncommitted tails in the runtime-context and
diagnostic streams as well as scientific/evidence streams. There is no durable
diagnostic guarantee when storage is full or inaccessible, the writer cannot
establish a verified safe head, another process modified the bundle, or the
process dies before checkpoint promotion. Those cases remain explicit
non-guarantees; no alternate log or telemetry channel is created.

## Platform And Filesystem Boundary

On Unix targets the adapter uses standard file locks, `sync_all`, same-directory
rename replacement, and directory synchronization. Windows does not expose a
supported directory-`fsync` equivalent, so the adapter synchronizes every
regular file before it becomes reachable and promotes the prior and current
checkpoints with `MoveFileExW(MOVEFILE_REPLACE_EXISTING |
MOVEFILE_WRITE_THROUGH)`. Opening a Windows writer or recovery handle first
probes that exact synchronized replacement operation in the bundle directory.
The real integration tests execute lock, append, replacement, reopen, and
recovery behavior on every supported CI target.

The guarantee is capability-based, not a claim about every device or power-loss
mode. Opening a live writer fails with a typed capability error when the bundle
is not a regular directory or the host cannot provide OS locking and the
platform durability barrier: directory synchronization on Unix or synchronized
write-through replacement on Windows. Advisory locks cannot stop a
non-cooperating editor, so every mutation also rechecks checkpoint, plan,
length, and prefix digests and freezes the handle without overwriting unexpected
bytes. Provider-managed, network, removable, or virtual filesystems are not
supported merely because they expose a path; only a filesystem on which these
required operations actually succeed is inside the acknowledged-write boundary.

## Deterministic Verification

`crates/storage/tests/live_persistence.rs` injects failures before, within, and
after stream writes; around every stream/checkpoint synchronization and
replacement boundary; after durable commit but before response; and throughout
all four plan-generation files. Each case reopens as the previous or complete
next revision. The suite also covers two cooperative writers, a writer that
ignores the advisory lock, stale revisions, idempotent retry behavior,
raw-plus-normalized batches, complete/torn/incomplete/duplicate/conflicting
tails, current/previous checkpoint selection, committed corruption, plan
freeze, recovery attachments, interruption detection, and checkpointed export.

`crates/storage/tests/v3_live_persistence.rs` separately verifies schema-v3-v5
creation, checkpointed reads and exports, event and evidence appends,
attachment rollback, stale revisions, conflicting mutation reuse, and external
modification detection. Schema-v5 cases inject every rig/event/checkpoint
failure boundary and verify that failed attempts remain non-arming while a
verified pair and armed event are all-or-nothing.
