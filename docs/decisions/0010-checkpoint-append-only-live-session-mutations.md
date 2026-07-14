# 0010: Checkpoint Append-Only Live Session Mutations

Date: 2026-07-14

## Decision

Live AntennaBench sessions will use a hybrid of versioned plan snapshots,
append-only evidence streams, and one atomically replaced checkpoint file as
the transaction commit point. The session bundle remains the only durable
source of truth.

This contract belongs to bundle schema version 2. Version 1 remains readable,
normalizable, analyzable, reportable, and losslessly copyable, but the
conductor will not mutate a version 1 bundle. Resuming or extending version 1
evidence first requires the explicit, non-destructive v1-to-v2 upgrade selected
by [Decision 0008](0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md).

The persistence implementation must not claim that any portable filesystem
API makes data literally immune to every device, controller, kernel, or sudden
power failure. It must provide a precise acknowledged-write boundary, use the
strongest supported flush and same-filesystem replacement operations, retain a
recoverable previous checkpoint, and fail with a typed diagnostic when the
required local-filesystem behavior is unavailable.

## Current Boundary Inventory

The implemented schema-v1 boundary is safe for bounded read/copy workflows but
is not a live-session store:

- `BundleStore::write()` creates or truncates ten modeled files in sequence.
  It does not validate the complete value first, stage a generation, lock the
  bundle, synchronize file contents, or expose an atomic multi-file commit.
- JSONL output rewrites each complete stream. There is no durable append,
  committed offset, tail recovery, retry token, or external-change check.
- Offline and live WSJT-X helpers append only to an in-memory
  `BundleContents`. `LiveWsjtxIngest` sequence numbers and duplicate state
  restart with the process, so they cannot provide durable replay identity.
- `OperatorEventType::Switched` names a planned slot but carries no independent
  actual antenna. Alignment currently assumes the planned label was actual.
  Events have one timestamp, no recorded-versus-occurred distinction, no
  correction/retraction relation, and no lifecycle validation.
- Normalization rewrites only derived observation slot annotations in memory.
  Those annotations cannot be the durable record of what the operator did.
- Desktop open retains a source path and one derived report in memory. Lossless
  export validates, then copies source files without a writer lock or coherent
  live-stream checkpoint.

These behaviors remain valid for existing static v1 fixtures. They are not an
implicit mutation API.

## Version-2 Durable Layout Responsibilities

The schema-v2 foundation issue must reserve a versioned `session-state.json`
checkpoint and mutation metadata in addition to the provider-neutral adapter
stream selected by Decision 0008. The exact Rust module layout is an
implementation detail, but these durable responsibilities are fixed:

| Durable area | Ownership and mutation rule |
| --- | --- |
| `manifest.json` | Storage-owned schema/bootstrap metadata. Created once and not used as a frequently rewritten transaction file. |
| Plan generations | Complete station, antenna, and schedule snapshots. A new draft generation is staged and validated before one checkpoint makes it current. |
| `session-state.json` | Storage-owned commit point: revision, lifecycle, active plan generation, and the committed head of every stream. |
| `events.jsonl` | Append-only lifecycle and operator evidence. Existing committed lines are never edited or deleted. |
| `adapter-records.jsonl` | Append-only exact/near-raw adapter evidence and dispositions selected by Decision 0008. |
| `observations.jsonl` | Append-only normalized observations linked to adapter evidence. Persisted slot annotations remain derived and regenerable. |
| Rig and propagation streams | Append-only observed/adapter evidence under the same checkpoint protocol. |
| `analysis.json` | Replaceable metadata only. Reports, charts, and analysis results remain derived and outside the transaction source of truth. |
| Attachments | Immutable, content-addressed evidence plus explicitly identified recovery artifacts. |
| Lock and temporary files | Storage-owned transient state. They are excluded from reports and completed exports and never interpreted as evidence. |

Each checkpoint records at least:

- a monotonically increasing revision;
- lifecycle state and current plan-generation identity;
- for every stream, committed byte length, record count, last record ID, and
  digest of the committed prefix;
- the last committed mutation ID; and
- digests of every file in the referenced plan generation.

Each appended record carries a collision-resistant mutation ID plus its member
index and total member count. A one-event operator action is a one-member
mutation. An adapter acquisition that produces raw evidence and a normalized
observation is one multi-member mutation across the two streams. Stream order,
not timestamp or lexicographic ID order, decides replay and correction
precedence.

## Draft And Plan Snapshot Commits

Draft edits never replace several active root files in place. The writer:

1. creates a new generation in the same bundle/filesystem;
2. writes the complete station, antenna, and schedule files under create-new
   names;
3. validates the complete proposed plan under the schema/semantic policy;
4. synchronizes every generation file and its directory metadata where the
   platform supports it;
5. writes and synchronizes a new checkpoint in the bundle directory;
6. atomically replaces `session-state.json`, retaining the prior valid
   checkpoint until the new revision is reopened and verified; and
7. acknowledges the edit only after the checkpoint and required directory
   metadata synchronization succeed.

An unreferenced generation is never current. Recovery may remove a verified
orphan only after it is either proven redundant or preserved as a diagnosed
recovery artifact. Once a session starts, its selected plan generation is
frozen. A later discovery that the plan was wrong becomes operator evidence,
a correction to operator evidence, or a new session; it does not rewrite what
was originally planned.

## Append And Checkpoint Protocol

The writer holds the exclusive bundle lock and verifies that the active
checkpoint, plan digests, stream lengths, and committed-prefix digests still
match the state it opened. It then:

1. allocates or reuses the mutation ID before any write;
2. serializes every complete newline-terminated record in memory;
3. appends mutation members in deterministic stream order, with raw adapter
   evidence before any normalized observation derived from it;
4. synchronizes every changed stream;
5. writes, synchronizes, and atomically replaces the checkpoint with the new
   stream heads and revision;
6. synchronizes the containing directory through the platform durability
   adapter where supported; and
7. returns success only after reopening or otherwise verifying the new
   checkpoint.

Ordinary `Write::flush` is insufficient because Rust files are unbuffered at
that layer and the method is currently a no-op on Unix and Windows. The
implementation uses `File::sync_all`/the platform equivalent for durable
boundaries and a same-directory, same-filesystem replace. Rust documents that
`rename` maps to different OS facilities and that replacement details differ,
so the implementation must own and test a narrow platform adapter instead of
assuming identical behavior everywhere.

The prior checkpoint always describes a coherent prefix. A crash before the
new checkpoint becomes durable cannot make a partially appended batch visible
to normal readers. A crash after a verified checkpoint may expose the whole
new batch. A successful operation is therefore visible as either the previous
revision or the complete next revision, never as a mixed committed revision.

## Locking And External Modification

One process may own an exclusive writer lock for setup, conduction, recovery,
or checkpointed export. Read-only operations use a shared lock or an immutable
checkpoint snapshot. The lock is an OS file lock whose handle remains open for
the lease; lock-file existence alone never means a writer is alive, and a
process crash releases the OS lock.

Rust explicitly documents that its file locks may be advisory and that other
processes can still modify files. Therefore every mutation rechecks the
checkpoint revision, plan digests, stream sizes, and prefix digests. Any
unexpected change freezes mutation, preserves the unexpected bytes, and
returns a typed `external_modification` recovery diagnostic. AntennaBench never
overwrites or silently merges evidence written by a non-cooperating editor.

The supported mutation boundary is a regular local filesystem with working
same-volume replacement, file synchronization, and OS locking. Read-only open
and lossless copy may remain available elsewhere, but a network share,
virtualized provider, removable medium, or filesystem that cannot satisfy the
writer preflight is not accepted for a live run. The UI reports the exact
capability failure rather than claiming crash safety it cannot provide.

## Recovery Contract

Recovery acquires the exclusive lock before changing anything and selects the
highest valid checkpoint whose referenced plan generation and committed stream
prefixes verify. It then inspects bytes after each committed head:

- complete, valid, newline-terminated members that together form one whole
  mutation may be committed forward exactly once;
- a repeated already-committed mutation ID is an idempotent retry and returns
  the existing result without appending;
- a complete raw adapter member without all declared mutation members is not
  silently normalized or presented as committed evidence;
- torn, malformed, incomplete, or conflicting tail bytes are copied exactly to
  a content-addressed recovery attachment with their source stream, offsets,
  detection time, and diagnostic before the live stream is restored to its
  last checkpointed prefix; and
- malformed bytes inside a committed prefix are fatal corruption. Recovery
  does not skip an interior record and pretend the session is complete.

Temporary checkpoints and orphan plan generations are treated similarly:
verify, recover forward when the transaction is complete, otherwise preserve a
diagnostic artifact before cleanup. Disk-full or cleanup failure leaves the
bundle read-only and reports the exact remaining artifacts. Retry never
destroys the last verified checkpoint.

If the recovered lifecycle was `running` and no live writer still owns the
lock, recovery appends one idempotent `interruption_detected` lifecycle event
and moves the checkpoint to `interrupted` before offering resume, end, or
abandon actions. An explicit operator stop uses the same interrupted state but
records that it was requested rather than crash-detected.

## Session Lifecycle

The durable lifecycle is:

```text
draft --validate--> ready --start--> running
  |                    |               |
  +----abandon---------+---------------+----> abandoned
                                       |
                                       +----> interrupted --resume--> running
                                       |             |
                                       +-------------+----> ended
```

- `draft`: plan generations may be committed; no observations are acquired.
- `ready`: one complete plan generation passed creation validation and is
  frozen for the prospective run. Returning to draft creates a new generation.
- `running`: a durable `session_started` or `session_resumed` event is
  checkpointed. Adapter and operator evidence may append.
- `interrupted`: an explicit stop or recovery-detected loss of the prior writer
  is durable. Existing evidence remains reportable; acquisition is stopped.
- resume is a durable transition event back to `running`, not an erasure of the
  interruption.
- `ended`: the operator intentionally finalized the evidence, including an
  optional early-end reason. It is terminal.
- `abandoned`: the operator intentionally retained an incomplete/non-run draft
  or session without claiming normal completion. It is terminal.

Lifecycle transitions use recorded stream order and expected checkpoint
revision. Duplicate starts, resume without interruption, events after a
terminal state, or a stale UI revision fail without mutation. Terminal events
are confirmed explicitly and are not correctable; continuing requires a new
session rather than rewriting a finalized experiment.

## Operator Event Semantics

Schema-v2 events distinguish capture from occurrence. Every event has:

- stable event and mutation IDs;
- `recorded_at`, assigned by the trusted writer;
- `occurred_at`, the best time for the operator action or observed state;
- a time basis (`observed_now`, `operator_reported`, or recovery/system) and
  optional uncertainty;
- an optional planned slot reference; and
- a typed payload.

The minimum payloads are:

- lifecycle start, interruption, resume, end, and abandon;
- `antenna_state_confirmed` with an explicit actual antenna label, even when it
  differs from the slot's planned label;
- `slot_missed`, meaning no trustworthy slot action/actual state was confirmed;
- `slot_bad`, meaning the slot occurred but its evidence is intentionally
  ineligible, with a reason while any separate actual-antenna fact remains;
- `note_added`, which never changes eligibility by itself; and
- `event_corrected`, targeting one prior correctable event and either
  retracting it or supplying a typed replacement plus reason.

The schedule remains the planned state. No consumer infers the actual antenna
from `slot_id`. A switch confirmation without an explicit actual antenna is not
a valid v2 switch fact. Unknown actual state remains unknown.

Corrections append; they never edit the original line. A correction may target
only an earlier operator evidence event in the same session, not itself, a
future event, or a terminal lifecycle transition. Reduction follows append
order. The latest valid correction in a chain determines the effective view,
while reports retain the original and correction history. Competing active
missed/bad/switch facts without a valid correction produce a semantic
diagnostic and conservative exclusion rather than arbitrary timestamp
precedence.

## IDs, Retry, And Adapter Restarts

Session, mutation, event, adapter-run, adapter-record, and observation IDs use
collision-resistant UUID-based identities generated in trusted Rust code.
Ordering never depends on UUID lexical order. Tests inject deterministic clocks
and ID generators.

The command layer issues an opaque mutation token before an operator action is
submitted and reuses it across transport retries. If durable commit succeeded
but the reply was lost, retry finds the same mutation and returns its recorded
result. A different mutation ID with the same human-visible content is a new
fact, not silently deduplicated.

Each WSJT-X receiver start/resume has a durable adapter-run ID; records use that
run plus a sequence or independent UUID so process restart cannot collide with
the previous in-memory `000001` sequence. Duplicate-datagram policy remains a
bounded adapter concern under the resource decision, and duplicate inputs stay
auditable through their disposition.

## Validation, Reports, And Export

Creation validation runs before a draft becomes ready. Mutation validation
checks the proposed event/record and lifecycle transition before bytes are
written. Recovery validates the selected checkpoint and complete recovered
mutation. Full semantic/analysis eligibility validation runs against a
checkpointed snapshot; a failure does not delete raw adapter evidence.

Reports and exports use exactly one checkpoint revision:

- a report refresh briefly coordinates with the writer, captures the plan
  generation and committed stream heads, releases the writer, and derives only
  from those prefixes;
- the UI identifies the checkpoint revision/time and whether the session is
  live, interrupted, or final;
- an active export pauses mutations, synchronizes and verifies the current
  checkpoint, copies exactly its referenced plan generation, committed stream
  prefixes, and durable attachments to a new staged destination, verifies that
  destination, then resumes; and
- lock files, temporary files, uncommitted tails, and orphan generations are
  never smuggled into a completed export. A bundle needing recovery must be
  recovered or explicitly copied as diagnosed evidence before normal export.

This changes the current static-source `copy_losslessly_to()` contract only for
schema-v2 active sessions. Version-1 lossless copies remain byte-preserving and
non-mutating.

## Deterministic Verification Contract

The implementation issues must provide hardware-free tests with injected
clocks, IDs, filesystem operations, and write failpoints. Failpoints cover:

- every plan file write and synchronization;
- stream writes before, within, and after a line;
- every changed-stream synchronization;
- checkpoint temporary write, synchronization, replacement, platform metadata
  durability barrier, reopen/verification, and post-commit response;
- disk full, permission loss, cleanup failure, and cancellation;
- process restart before and after each boundary;
- two cooperative writers, an ignored advisory lock, and external file edits;
- retry with the same and different mutation IDs;
- complete, partial, torn, duplicate, and conflicting tails;
- every valid and invalid lifecycle transition;
- correction chains, wrong-switch correction, missed/bad conflicts, timestamp
  uncertainty, and actual labels differing from planned labels; and
- report/export while acquisition attempts to append.

For every injected interruption, reopen must yield the previous verified
revision or the complete next revision. No test may observe a mixed committed
revision, silently discarded tail, duplicate acknowledged mutation, or report
built from unmatched stream heads. Cross-platform integration tests exercise
real locking, replace, flush, and recovery behavior on each supported runner.

## Alternatives Considered

### Atomic Root Rewrites Plus Direct JSONL Append

This is the smallest change. It was rejected as the complete contract because
separate root replacements and stream appends provide no single coherent
revision for multi-record adapter mutations, reports, or active export.

### Whole-Bundle Transactional Directory Swap

Staging and promoting an entire bundle is useful for initial creation and
explicit v1-to-v2 upgrade. Rewriting/copying every growing stream for each live
operator action is expensive, and replacing a non-empty live directory has
different cross-platform constraints. It remains a creation/upgrade tool, not
the live append protocol.

### Full Application Write-Ahead Journal

A second journal containing complete mutation payloads could replay into every
canonical stream. It was rejected for the first conductor because it would
duplicate evidence ownership and require reconciliation between the journal
and bundle streams. The selected checkpoint plus mutation-member metadata gives
complete batches and committed prefixes one durable interpretation. A future
journal is justified only if implementation fault tests prove this protocol
cannot recover a required multi-stream operation.

### Checkpointed Append-Only Streams And Versioned Plans

This selected approach keeps evidence append-only, makes one small file the
commit point, avoids rewriting growing streams, preserves a prior coherent
revision, and gives report/export/recovery an exact snapshot boundary. Its cost
is a schema-v2 state file, mutation metadata, platform-specific durability
adapter, and explicit tail recovery.

## Consequences

- The schema-v2 foundation must include checkpoint and mutation metadata before
  new-session writing is considered stable.
- A focused storage issue implements locking, plan generations, append/checkpoint,
  recovery, and fault injection before any conductor UI writes evidence.
- A separate operator-event issue implements the v2 event payloads and pure
  lifecycle/correction reducer before the manual conductor uses them.
- Resource budgets from #40 bound stream sizes, recovery scans, checkpoints,
  attachment preservation, receiver state, report snapshots, and export time.
- Semantic validation from #38 classifies legacy-v1 evidence and v2 lifecycle,
  correction, plan, and checkpoint diagnostics.
- Manual/no-rig operation remains first-class because actual operator state is
  explicit evidence rather than inferred from a rig adapter.
- The desktop webview retains narrow commands; path, lock, filesystem, clock,
  ID, and durability authority remain in Rust.

## References

- [Decision issue #39](https://github.com/rwjblue/antennabench/issues/39)
- [Local conductor tracker #45](https://github.com/rwjblue/antennabench/issues/45)
- [Schema-v2 foundation #46](https://github.com/rwjblue/antennabench/issues/46)
- [Checkpointed persistence and recovery #53](https://github.com/rwjblue/antennabench/issues/53)
- [Operator lifecycle and correction semantics #54](https://github.com/rwjblue/antennabench/issues/54)
- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0008](0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md)
- [Rust `File` synchronization and locking](https://doc.rust-lang.org/stable/std/fs/struct.File.html)
- [Rust `rename` platform behavior](https://doc.rust-lang.org/stable/std/fs/fn.rename.html)
- [Windows `MoveFileExW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
- [Windows flushing system-buffered I/O](https://learn.microsoft.com/en-us/windows/win32/fileio/flushing-system-buffered-i-o-data-to-disk)
- [Apple write-barrier and full-sync guidance](https://developer.apple.com/documentation/xcode/reducing-disk-writes)
