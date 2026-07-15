# Live Persistence And Recovery

This technical reference defines how an active session becomes durable and how
AntennaBench recovers after interruption. Schema v2 owns the complete live
mutation and recovery protocol; schema v3 reuses its checkpoint, locking, and
verified snapshot boundaries for signal-plan sessions. For the bundle mental
model, start with [Session Bundles](bundle-format.md).

Live mutation is owned by `antennabench-storage`. A `LiveSessionV2` or
`LiveSessionV3` holds one OS-backed exclusive lock for its lifetime. Callers
provide typed records and an expected checkpoint revision; the writer assigns
the trusted capture time and mutation envelope, validates the complete batch,
appends in a fixed order, synchronizes every changed stream, and promotes one
checkpoint.

Schema-v1 bundles remain immutable inputs. They must be explicitly upgraded to
schema v2 or v3 in a new `.session.antennabundle` before these APIs will mutate
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
and its OS lock are the lease. Completed checkpointed exports exclude the lock,
temporary checkpoint, previous checkpoint, uncommitted stream tails, and
orphan generations.

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

Schema v3 provides corresponding `read_v3_checkpointed()` and
`export_v3_checkpointed_to()` boundaries. Its writer appends correctable
operator events and attachment-backed adapter evidence plus observations while
preserving the same expected-revision, digest, lock, and checkpoint rules.

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
session as `interrupted` before returning.

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

`crates/storage/tests/v3_live_persistence.rs` separately verifies schema-v3
creation, checkpointed reads and exports, event and evidence appends,
attachment rollback, stale revisions, conflicting mutation reuse, and external
modification detection.
