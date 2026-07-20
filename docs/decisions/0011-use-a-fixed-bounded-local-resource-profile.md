# 0011: Use A Fixed Bounded Local Resource Profile

Date: 2026-07-14

## Decision

AntennaBench will apply one versioned, fixed local resource profile to bundle
storage, adapter input, analysis, report construction, and desktop delivery.
The first profile is named local-standard-v1.

The profile is an operational safety boundary, not an evidence-quality rule and
not a schema version. Crossing a limit blocks or stops the affected operation
with a typed diagnostic. It never silently truncates input, invents a complete
session from a prefix, or changes the meaning of evidence that was accepted.

Production limits are not configurable in the first desktop workflow. Tests
may inject smaller profiles so boundary cases remain cheap and deterministic.
A future higher-capacity profile requires an explicit product decision,
measurements, and streaming or spillable implementations appropriate to its
larger bounds.

Local filesystem work has bounded bytes, records, nesting, entries, derived
rows, and cancellation checkpoints, but no wall-clock timeout. Network input
has explicit connect and total timeouts in addition to byte and state limits.

## Measurements And Planning Case

The checked-in fixture tree was measured before selecting the profile:

- the four session bundles contain 59,598 bytes in their modeled files;
- the complete session-bundle fixture tree, including its README, contains
  61,154 bytes;
- the largest bundle is the canonical sample at 28,714 bytes;
- the largest JSON or JSONL physical line is 794 bytes;
- the busiest JSONL fixture stream has 26 records; and
- its deterministic standalone HTML report is 38,594 bytes.

These are regression fixtures, not realistic capacity targets. The planning
case is a deliberately busy one-day WSPR session: 720 two-minute periods and
up to 200 admitted decodes per period produce 144,000 decode records. At the
measured 794-byte high-water line, one such stream is about 109 MiB. A raw
adapter stream plus normalized observations remains within the 256 MiB modeled
bundle and 500,000-record aggregate budgets, with headroom for events, status,
rig, and propagation records.

This calculation is a planning envelope, not a guarantee that every one-day
input fits. The exact encoded-byte, line, and record limits remain
authoritative.

## Local Standard V1 Bundle Limits

All byte limits use powers of two. A MiB is 1,048,576 bytes and a GiB is
1,073,741,824 bytes. Physical-line limits include the line terminator when it
is present.

### Modeled Files

| Boundary | Limit |
| --- | ---: |
| One root JSON file | 4 MiB |
| One JSONL physical line | 256 KiB |
| One JSONL stream | 128 MiB |
| Records in one JSONL stream | 250,000 |
| All modeled root and stream bytes | 256 MiB |
| All JSONL records | 500,000 |
| JSON nesting | 64 containers |
| One modeled scalar string | 128 KiB |
| Entries directly below the bundle root | 64 |

Narrow semantic rules still win. For example, ADR 0009 limits newly generated
machine identities and antenna labels far below the general scalar-string
budget.

The JSONL line budget intentionally accommodates the current maximum UDP
datagram encoded as lowercase hexadecimal plus its modeled envelope. A source
payload that cannot fit safely inline must use attributed attachment evidence
in a schema that supports it; it is not split across apparently complete
records.

Blank JSONL lines remain governed by the schema and validation policy. They do
not offer a way around the byte budget and do not count as durable records.

### Opaque Files And Attachments

| Boundary | Limit |
| --- | ---: |
| One opaque root file or attachment | 512 MiB |
| Opaque root files plus attachments | 2 GiB |
| Opaque root and attachment entries | 4,096 |
| Attachment directory depth below attachments/ | 8 |
| Simultaneously open directory iterators | 1 |
| Simultaneously open copy files | 1 source and 1 destination |

The total accepted bundle is therefore bounded by the 256 MiB modeled pool and
the 2 GiB opaque pool, plus negligible directory metadata.

Symbolic links and non-file/non-directory entries remain unsupported. Every
entry counts before its type is accepted. Strict creation refuses unmodeled
root entries. A legacy storage-safe copy that claims to be lossless preserves
safe opaque root entries and attachments byte-for-byte within the opaque pool;
it does not silently omit them.

The current attachment tree does not persist a portable path encoding, so this
decision does not invent a cross-platform byte limit for legacy native names.
Host filesystem path rules apply during same-platform inspection and copy, and
a target-platform failure is explicit. Schema-v2 referenced attachment names
must use the portable path grammar selected with that schema. Entry count,
depth, and byte budgets bound legacy traversal in the meantime.

### Preflight And Streaming Enforcement

Metadata is used to reject an oversized regular file before parsing or copying,
but metadata is not trusted as the only check. Readers, writers, and copies
count bytes as they stream. A file that is replaced or grows after preflight
still stops at the same limit.

Root JSON must be read through a bounded reader. JSONL parsing must cap the
buffer before searching for a terminator and count the record before inserting
it. Strict serialization uses a checked writer rather than constructing an
unbounded String and checking afterward.

Tree traversal does not follow links, holds one directory iterator at a time,
and counts directories as well as files. Copy uses bounded chunks and never
keeps more than one source and destination file open.

## Operation Profiles

### Storage-Safe Inspection And Copy

Storage-safe inspection establishes paths, entry types, and resource bounds
without interpreting modeled meaning. If that succeeds, a byte-preserving copy
can remain available even when typed parsing, semantic validation, analysis, or
reporting fails.

The copy is explicitly marked unvalidated when typed interpretation did not
succeed. This separation follows ADR 0009 and prevents a damaged but
storage-safe source from becoming impossible to preserve.

### Typed Read And Strict Write

A typed read returns the complete typed bundle or no bundle. It never returns a
prefix. The modeled-file, JSON, JSONL, and aggregate counters apply before
objects enter long-lived collections.

A newly authored root file, record, attachment, or plan generation is checked
before promotion. The live schema-v2 checkpoint from ADR 0010 may advance only
when the next mutation remains within the profile. The old checkpoint remains
the committed truth if the new bytes would cross a limit.

### Live Growth

Budget accounting is part of the checkpoint state or is deterministically
reconstructible from its committed lengths. Concurrent or external growth
detected through length, digest, or revision comparison stops the writer.

When adapter growth reaches a hard bundle limit, the adapter stops before
accepting more input. If there is room to commit a small acquisition-gap
mutation, the conductor records the resource code, source, time, and affected
interval. If even that cannot be committed, the in-memory operator state must
still show that completeness is unknown and require an explicit recovery or
end decision. Manual/no-rig conduction remains available.

## Adapter Limits

### Offline WSJT-X Text Import

An offline source file is limited to 128 MiB, 250,000 nonblank source lines,
and 64 KiB per physical line. The importer streams input and preserves every
malformed line that remains within those limits using the existing explicit
adapter-record disposition.

Crossing a file or line budget fails the complete import. A retained prefix may
exist only as quarantined incomplete evidence with its byte count and failure
reason. It cannot produce a complete normalized import.

### Live WSJT-X UDP

The direct/local receiver retains these same limits whether it is the required
offline receive source or runs optionally beside WSPR.live. Concurrent source
use does not combine queues, deduplication identities, or evidence strata.

| Boundary | Limit |
| --- | ---: |
| UDP datagram | 65,535 bytes |
| Receiver/orchestrator queue | 256 datagrams and 8 MiB |
| Aggregate admission rate | 64 datagrams/second refill |
| Admission burst | 512 datagrams |
| Tracked clients | 32 |
| Client identifier | 128 UTF-8 bytes |
| Duplicate fingerprints per client | 4,096 |
| Duplicate window | 10 minutes |
| Idle client eviction threshold | 5 minutes |

The queue stops at whichever item or byte limit is reached first. Admission
uses a deterministic token bucket. The implementation stores a fixed-size
fingerprint and receipt time for duplicate suppression, never a second copy of
each full datagram.

Only a least-recent client idle for at least five minutes may be evicted to
admit a new client. Active state is not discarded merely to hide overflow.

A queue, rate, client, or duplicate-state breach creates one resource/acquisition
gap and stops the affected receiver. UDP cannot apply backpressure, so silently
dropping datagrams and continuing as if the session were complete is forbidden.
Already committed evidence remains usable and the rest of the conductor does
not have to stop.

### NOAA SWPC HTTP

NOAA acquisition remains limited to adapter-owned HTTPS endpoints. Redirects
are limited to three hops and must remain HTTPS on the same host.

| Boundary | Limit |
| --- | ---: |
| Connect timeout | 5 seconds |
| Total request timeout | 20 seconds |
| Response headers | 64 |
| One normalized header field | 8 KiB |
| All normalized response headers | 32 KiB |
| Complete decoded response body | 2 MiB |

Content-Length over the body limit is rejected before reading. Missing,
incorrect, compressed, or chunked length does not bypass a streaming decoded
byte counter. A successful product response must use an expected JSON media
type. Unsupported media types or content encodings are typed acquisition
failures.

Only a complete, in-budget response can be parsed or promoted. A bounded
partial body may be quarantined with endpoint, receipt time, byte count, and
failure reason; it cannot become a propagation record. Existing polling,
freshness, retry, conditional-request, and duplicate rules still apply.

## Analysis, Reports, And Desktop Delivery

Analysis starts only from a typed bundle accepted by the local profile. Every
intermediate collection is capped at 500,000 entries, and all simultaneously
live intermediate collections are capped at 1,000,000 entries. Algorithms may
index and sort accepted records, but may not materialize an unbounded
cross-product.

| Derived boundary | Limit |
| --- | ---: |
| Repeated rows in a full-detail report model | 25,000 |
| Deterministically serialized report model | 8 MiB |
| Standalone HTML, including escaped expansion | 16 MiB |
| Desktop session-summary payload | 64 KiB |
| Desktop report-document IPC payload | 16 MiB |
| Concurrent foreground open/analyze/render/export operations | 1 |
| Retained active reports | 1 |

Budget counters run before inserting an intermediate or report row. HTML uses a
checked writer so malicious text expansion cannot allocate beyond the output
limit before failing.

If full detail exceeds its row or model budget, report construction may produce
a bounded overview only when it contains complete aggregate counts and an
explicit resource.report.detail_omitted notice for every omitted detail family,
including its full row count. It never samples rows. The document labels itself
as an overview rather than a full-detail report. If even the overview exceeds a
model or HTML budget, report generation fails.

A report failure does not block storage-safe lossless export. Reports, overview
projections, and their omission notices remain derived artifacts and never
modify the bundle.

The desktop backend retains one source reference, one small summary, and one
derived report document. A second memory-heavy foreground command receives a
typed busy result instead of duplicating the bundle and report pipeline.
Frontend authority remains unchanged. Network wait time is not a foreground
operation: automatic WSPR.live acquisition takes the single permit to snapshot
authority, releases it while each bounded HTTP request is in flight, then waits
to reacquire it before revalidating the active source, lifecycle, checkpoint
revision, and acquisition plan and committing a still-authorized response.
Explicit conductor mutations and persistence of an already completed controller
attempt also wait for that one permit rather than losing the action to transient
`resource.operation.busy` contention. They remain serialized; this admission
rule does not increase the foreground-operation limit.

## Diagnostics

Resource diagnostics are structured operational data. Each contains:

- a stable machine code;
- profile name and version;
- operation and stage;
- path, stream, adapter, or output role;
- configured limit and unit;
- observed value when it is safely known;
- whether retry without changing input can help;
- whether a complete typed result was produced; and
- whether an acquisition or presentation gap exists.

Code namespaces include:

- resource.bundle.root_entries;
- resource.bundle.modeled_bytes;
- resource.json.depth;
- resource.json.scalar_bytes;
- resource.jsonl.line_bytes;
- resource.jsonl.records;
- resource.attachments.entries;
- resource.attachments.depth;
- resource.attachments.total_bytes;
- resource.adapter.offline_source_bytes;
- resource.adapter.udp.queue_bytes;
- resource.adapter.udp.rate;
- resource.adapter.udp.clients;
- resource.adapter.udp.dedup_entries;
- resource.adapter.http.headers;
- resource.adapter.http.body_bytes;
- resource.analysis.live_entries;
- resource.report.rows;
- resource.report.model_bytes;
- resource.report.html_bytes;
- resource.report.detail_omitted;
- resource.desktop.ipc_bytes; and
- resource.operation.busy.

Messages may improve without changing codes. Tests assert code, stage, role,
limit, observed value, and completeness effect.

Resource diagnostics do not claim that evidence is scientifically weak or
invalid. They say that a named operation did not safely consume or present all
requested input under this product profile.

## Failure, Cancellation, Cleanup, And Retry

Local operations check cancellation at every directory entry, JSONL record,
analysis phase, and no less often than every 64 KiB of copied or rendered
output. Long analysis loops check at least every 1,000 work entries.

Cancellation returns no new typed bundle or partial report, leaves the source
unchanged, and leaves the previous active desktop presentation in place.
Temporary output created by the operation is removed best-effort.

Copy and export create a new destination and never overwrite or merge. A byte,
entry, cancellation, or I/O failure rolls back only that new destination. If
cleanup fails, the diagnostic names the incomplete path and does not call it a
successful export.

Disk-full and write failures preserve the last committed schema-v2 checkpoint.
Uncommitted tail bytes follow ADR 0010 recovery and quarantine rules; they are
not silently discarded or treated as committed. Retrying uses the same
mutation ID and is idempotent.

No local file, analysis, or rendering timeout is selected because wall time is
platform- and device-dependent. Bounded work plus cooperative cancellation is
the portable contract. Network acquisition uses the explicit timeouts above.

A deterministic hard-limit error is not automatically retryable. Retry is
useful after the input changes, disk space is restored, or a later explicitly
selected profile supports more capacity. Transient transport and busy outcomes
retain their own retry policy.

## Hosted Relationship

The hosted boundary in issue #11 reuses core semantic validation and may reuse
these diagnostic shapes. It does not inherit these numeric limits
automatically.

Hosted ingress must account for request concurrency, archive entry names,
compressed and expanded byte ratios, persistent storage cost, abuse, and
tenant isolation. Its limits are likely lower and may reject inputs accepted
locally. Local operation never depends on hosted capacity.

## Verification

Implementation uses injected tiny test profiles and covers N-1, N, and N+1 for
each numeric boundary without allocating production-sized inputs.

Required adversarial coverage includes:

- misleading metadata and growth after preflight;
- an unterminated or oversized JSONL line;
- JSON nesting and scalar-string boundaries;
- per-stream and aggregate record/byte limits;
- wide and deep attachment trees, links, and special entries;
- cancellation and injected disk-full failures during read, append, and copy;
- false Content-Length, missing length, chunked data, slow response, excessive
  headers, redirects, media type, and decoded HTTP body size;
- UDP bursts, sustained floods, queue bytes, too many clients, idle eviction,
  and duplicate-window saturation;
- analysis collection and aggregate-live-entry limits;
- worst-case HTML escaping and checked-writer failure;
- full-detail to explicit-overview transition and overview failure;
- concurrent desktop operations and active-state preservation; and
- exact stable diagnostic fields and deterministic ordering.

Network tests use captured or synthetic transports. File and disk failures use
injected readers/writers or compact sparse fixtures. No test depends on a live
service or consumes the production budget.

## Alternatives Considered

### Fixed Conservative Product Limits

This is the selected approach. One visible, versioned profile makes behavior
deterministic across desktop installations and allows the initial conductor to
ship without an unreviewed expert-mode escape hatch.

### Configurable Profiles

Safe defaults plus higher limits sound flexible, but the current readers,
analysis, renderer, and desktop IPC materialize substantial data in memory.
An environment variable or hidden preference would let an operator select a
capacity the implementation has not proven safe. Higher profiles remain
possible after measurements and streaming work.

### Streaming First Without Product Limits

Streaming is required at storage, copy, HTTP, and checked-output boundaries,
but it does not bound record collections, dedup state, report rows, or CPU by
itself. Deferring all numbers until every subsystem is spillable would leave
the first conductor unsafe and delay the core product.

### Trust All Local Input

Operator selection is not proof that a downloaded bundle, adapter response, or
stale directory tree is benign. Local-first describes ownership and
availability, not infinite trust or capacity.

## Consequences

- Initial large-session capacity is finite, explicit, and testable.
- Storage, adapters, analysis, reports, and desktop IPC share one profile
  vocabulary without conflating their operations.
- Some locally selected evidence will be storage-safe to copy but too large to
  interpret or report under the first profile.
- Live adapter overflow becomes an auditable completeness gap instead of a
  silent drop.
- Reports may become explicit aggregate overviews rather than sampling detail.
- Higher-capacity workflows require future evidence and design rather than a
  hidden override.
- Hosted limits remain a separate abuse and operations decision.

## References

- [Decision issue #40](https://github.com/rwjblue/antennabench/issues/40)
- [Local conductor tracker #45](https://github.com/rwjblue/antennabench/issues/45)
- [Bounded storage implementation #55](https://github.com/rwjblue/antennabench/issues/55)
- [Bounded adapter implementation #56](https://github.com/rwjblue/antennabench/issues/56)
- [Bounded report implementation #57](https://github.com/rwjblue/antennabench/issues/57)
- [Hosted boundary decision #11](https://github.com/rwjblue/antennabench/issues/11)
- [Decision 0009](0009-use-layered-bundle-validation-profiles.md)
- [Decision 0010](0010-checkpoint-append-only-live-session-mutations.md)
