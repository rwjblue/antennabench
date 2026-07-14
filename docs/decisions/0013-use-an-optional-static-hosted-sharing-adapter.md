# 0013: Use An Optional Static Hosted Sharing Adapter

Date: 2026-07-14

## Decision

AntennaBench will treat hosted report sharing as an optional adapter around the
complete local workflow. Capture, bundle inspection, analysis, report rendering,
standalone HTML export, and lossless evidence export remain available without an
account, network connection, or hosted service. Publishing is an explicit copy
operation; it is not synchronization and the hosted service never becomes the
source of truth for a session.

The first hosted product shape is a static viewer and explanatory site plus the
smallest practical publishing API. It uses Cloudflare-native managed services:

- Workers Static Assets for the maintained site and sample-report entry point;
- a small Worker for upload admission, status, lifecycle commands, and private
  access selected by the later identity decision;
- private R2 buckets for quarantined uploads and retained original archives;
- a separate public R2 bucket and custom domain for immutable published report
  HTML that the identity decision permits to be public;
- D1 for hosted control-plane metadata and processing state, never evidence;
- Queues for asynchronous, at-least-once processing;
- a scale-to-zero Cloudflare Container running the canonical Rust validation,
  analysis, and report pipeline; and
- Turnstile, application quotas, platform rate limits, bounded concurrency, and
  resource limits as layered abuse controls.

The processor is a conventional OCI image with a narrow job contract. The first
implementation uses a `basic` Container with no general Internet egress, at most
two concurrent instances, a two-minute job deadline, and explicit shutdown as
soon as each job finishes. It does not initially port the filesystem-oriented
Rust pipeline to Workers WebAssembly.

The expected fixed operating floor is the Workers Paid minimum, currently USD
$5 per month. Static views and ordinary small-scale publishing should remain
inside the included usage. All other material cost scales with uploads, retained
bytes, or cache misses; no application processor remains running while idle.

## Local-First Boundary

The session bundle remains the durable evidence record under ADR 0001. The
hosted service accepts a transport copy only when an operator explicitly asks to
publish. Local code does not consult hosted state to open, validate, analyze,
render, export, or continue a session.

The existing Rust storage layer reads filesystem bundles and can preserve exact
files through lossless copy. The analysis and report layers consume typed bundle
data, and the standalone renderer produces deterministic HTML with inline style,
no script, no external resource, escaped untrusted text, and a restrictive CSP
meta policy. Hosted processing reuses those contracts instead of establishing a
second semantic implementation.

Hosted identifiers, processing states, indexes, validation reports, report
models, and HTML are replaceable service artifacts. Losing every hosted artifact
must not damage or reinterpret the operator's local bundle. Conversely, a hosted
copy does not automatically update when the local bundle later changes; a new
explicit publication produces a new immutable hosted report revision.

The hosted service is therefore a convenient sharing feature, not part of the
offline availability promise and not a prerequisite for product value.

## Application And Trust Boundaries

The public application is mostly static. The explanatory site, method text,
sample report, and upload UI are versioned application assets. Previously
published public reports are immutable standalone documents served through the
R2 custom domain and Cloudflare cache without invoking Worker or D1 code on an
ordinary view.

The dynamic surface is intentionally narrow:

- create an upload ticket;
- stream one admitted archive into private quarantine;
- query processing status;
- retry an explicitly retryable job;
- promote an accepted derived report according to the visibility policy;
- request authorized access to a private artifact if #12 selects that feature;
  and
- delete a hosted report and its retained evidence.

No hosted endpoint accepts report HTML, JavaScript, CSS, templates, SQL,
executables, container images, or renderer plugins from a bundle. Operator and
adapter text is data passed through the trusted Rust renderer. A content-type or
filename never grants executable authority.

The identity, authentication, ownership, callsign-claim, visibility, raw-download,
moderation, and account-deletion policies remain owned by issue #12. This
decision defines safe storage and processing capabilities but does not enable
general user publishing until that policy is selected. The maintained sample
report may use the public publication path without choosing a user identity
model.

## Hosted Standard V1 Profile

Hosted input uses a separate fixed profile named `hosted-standard-v1`. It is an
operational, abuse, and cost boundary rather than a schema or evidence-quality
rule. It may reject a bundle that local-standard-v1 can safely process. Such a
rejection does not make the local bundle invalid and never limits local use.

All byte limits use powers of two. A MiB is 1,048,576 bytes and a KiB is 1,024
bytes. Crossing a hard limit rejects the complete hosted operation with a stable
diagnostic. Input is never truncated into an apparently complete report.

### Transport And Archive Limits

The first transport is one ZIP archive containing exactly one top-level bundle
directory with a supported `.session.wsprabundle` or `.session.antennabundle`
suffix. The archive is transport packaging, not a new canonical bundle format.
The extracted directory must still satisfy its declared bundle version and the
normal bundle validator.

| Boundary | Limit |
| --- | ---: |
| HTTP request body / compressed archive | 32 MiB |
| Total expanded regular-file bytes | 128 MiB |
| Archive entries, including directories | 1,024 |
| Directory depth below the archive root | 8 |
| One entry expansion ratio | 100:1 |
| Aggregate archive expansion ratio | 100:1 |

Admission requires an exact supported media type and a `Content-Length` at or
below 32 MiB. A missing, conflicting, chunked, or oversized length is rejected
before R2 storage. The Worker still counts streamed bytes so a false length
cannot bypass the product cap.

Only stored and deflated regular files and directories are accepted. The
archive rejects:

- absolute, parent-traversing, empty-component, NUL-bearing, backslash-bearing,
  or non-portable paths;
- duplicate paths, normalized duplicates, and case-folding collisions;
- symbolic links, hard links, devices, sockets, FIFOs, and other special entries;
- encrypted entries, multi-disk archives, unsupported compression methods, and
  inconsistent central-directory/local-header metadata;
- entries whose declared or streamed bytes, CRC, count, depth, or expansion
  ratio cross the profile; and
- trailing structures or ambiguous roots that prevent a deterministic complete
  inventory.

Declared sizes allow early rejection but are never trusted alone. Extraction
streams through counters and stops before writing a byte that crosses a limit.
Temporary files live only on the processor's ephemeral disk and are removed
when the job ends.

### Bundle, Analysis, And Report Limits

| Boundary | Limit |
| --- | ---: |
| All modeled root and stream bytes | 64 MiB |
| One root JSON file | 4 MiB |
| One JSONL physical line | 256 KiB |
| One JSONL stream | 64 MiB |
| Records in one JSONL stream | 100,000 |
| Records across all JSONL streams | 200,000 |
| Opaque root files and attachments | 64 MiB |
| One opaque file or attachment | 32 MiB |
| JSON nesting | 64 containers |
| One modeled scalar string | 128 KiB |
| Repeated rows in a full-detail report model | 25,000 |
| Deterministically serialized report model | 8 MiB |
| Standalone HTML | 16 MiB |
| Complete processor job wall time | 2 minutes |

Narrower semantic limits still apply. The hosted processor uses the strict
structural and semantic profiles selected by ADR 0009 and the bounded analysis
and renderer behavior selected by ADR 0011. It does not loosen local validation
to increase upload acceptance.

If full detail exceeds the report-row budget, the same explicit bounded-overview
rules as local operation apply. The report must disclose every omitted detail
family and its complete count. If even the overview exceeds its model or HTML
budget, hosted publication fails; it never samples or silently omits detail.

Production limits are fixed and versioned. Tests may inject tiny equivalents.
Raising the profile requires measurements, an explicit review of processor
memory and cost, and a new profile version rather than an environment variable
or owner-only bypass.

## Validation And Processing Stages

An upload advances only after every preceding stage succeeds:

1. **Request admission** checks method, content type, length, ticket,
   idempotency key, Turnstile or selected authentication proof, quotas, and rate
   policy.
2. **Private quarantine** streams the exact request body to a new write-once R2
   key. An incomplete request never becomes eligible for processing.
3. **Archive inventory** validates container structure, paths, types, counts,
   declared sizes, compression methods, and root shape without trusting entry
   content.
4. **Bounded extraction** verifies streamed compressed/expanded counters, CRCs,
   collisions, and final inventory on ephemeral disk.
5. **Bundle storage-safe inspection** applies entry/type/path/resource rules
   before typed interpretation.
6. **Structural and semantic validation** applies the canonical Rust profiles
   for the declared bundle version and preserves ordered diagnostics.
7. **Analysis eligibility and report projection** use the canonical Rust
   analysis and bounded report model.
8. **Trusted rendering** produces the deterministic standalone HTML and checks
   the final byte limit and security invariants.
9. **Publication** writes derived artifacts under new immutable keys and changes
   hosted metadata to published only after every required object is readable.

A failure returns no partial typed bundle, report model, HTML document, or
published URL. Stable diagnostics record stage, code, profile, limit, observed
value when known, evidence-completeness effect, and retryability without logging
raw operator content.

## Original And Derived Artifact Ownership

The service uses physically separate private and public storage boundaries.

### Private Upload And Evidence Storage

The exact admitted ZIP bytes are retained in private R2 after successful
processing. The processor records their SHA-256 digest, byte count, archive
inventory, and exact digest of every extracted regular file. This permits a
later audit to prove which transport bytes and bundle files produced a report.

The ZIP wrapper is not itself canonical evidence; the exact extracted session
bundle files inside it are. Retaining the exact wrapper plus entry manifest is a
reversible evidence copy and avoids storing a second expanded tree indefinitely.

Rejected, abandoned, and incomplete quarantine objects expire after two days.
Lifecycle deletion is a backstop; the application also deletes known failed
objects promptly. Accepted original archives remain private until the owning
report is deleted or a later retention decision explicitly says otherwise.
Issue #12 decides who may download an accepted original. It is never placed in
the public report bucket.

### Derived Private Storage

The ordered validation result, processor and renderer versions, report model,
standalone HTML, publication metadata, and cache state are derived. They may be
recomputed from an accepted original with the recorded software version. No
temporary extracted files or analysis intermediates persist after the job.

D1 stores only hosted metadata: random identifiers, object roles and digests,
state, idempotency keys, software/profile versions, timestamps, size counters,
diagnostic summaries, visibility selected under #12, deletion state, and audit
events. D1 is never the evidence record and never stores the complete bundle or
report HTML.

### Public Published Storage

Only trusted, derived, standalone HTML that the #12 policy marks public or
unlisted enters the public R2 bucket. Object keys are immutable and are never
overwritten. Public bucket listing remains disabled; knowledge of an identifier
does not grant access to any private object.

The public custom domain uses a short browser cache lifetime and a long shared
edge lifetime. Publishing purges any cached negative response before exposing
the URL. Deletion removes the object and purges its exact URL before the state
becomes deleted. The system cannot retract an already downloaded copy, and a
browser may retain a copy until its bounded client cache expires; user-facing
deletion language must state that limit honestly.

Private report viewing, if selected by #12, goes through the Worker and does not
use the public bucket. The included Worker request allowance is sufficient for
the expected initial scale, but private access remains an explicitly metered
path rather than being described as static hosting.

## Stable Identifiers And URLs

Each hosted report receives a cryptographically random identifier with at least
128 bits of entropy, encoded in a URL-safe, case-insensitive alphabet. It is not
derived from a callsign, station location, bundle digest, filename, account, or
session identity. Identifiers are never reused, even after rejection or
deletion.

One successful immutable report revision has one stable public URL. Publishing
an updated local session creates a new revision and URL; it does not silently
replace bytes behind an existing report. A future product may add an explicitly
mutable human-facing collection page that points at revisions, but that pointer
is hosted metadata and not selected here.

An identifier is not authorization. Public and unlisted access semantics,
owner capabilities, and private links remain governed by #12.

## State, Idempotency, Retry, And Reconciliation

The D1 control record moves through explicit states:

`created`, `uploading`, `uploaded`, `queued`, `processing`, `published`,
`rejected`, `retryable_failed`, `deleting`, and `deleted`.

R2, D1, Queue, cache, and Container operations do not form a distributed
transaction. The implementation therefore uses write-once objects, monotonic
state transitions, idempotency keys, and reconciliation rather than claiming
atomic cross-service publication.

- Creating an upload ticket with the same valid idempotency key returns the same
  ticket and never allocates a second report ID.
- Completing the same upload twice observes the existing digest or reports a
  conflict; it never overwrites accepted bytes.
- Queue messages carry a stable job ID and expected upload digest. At-least-once
  delivery may run the same job again, but write-once derived keys and a
  compare-before-transition rule make the result idempotent.
- A retryable processor failure retains the exact accepted upload and advances
  neither the public pointer nor the published state.
- A deterministic archive, profile, schema, or semantic failure is rejected and
  is not retried without new input.
- A dead-lettered job becomes `retryable_failed` with an operator-visible stable
  code; it never remains falsely `processing`.
- A small scheduled Worker reconciles stale `uploaded`, `queued`, `processing`,
  and `deleting` records. It re-enqueues or completes only idempotent work and
  has no continuously running process.

Deletion first prevents new application access, then deletes private and public
objects, purges the public URL, and finally records a non-reusable tombstone. A
partial deletion remains `deleting` and is retried by the reconciler. Cache
purge failure is not reported as completed deletion.

## Processor Isolation And Portability

The current storage code and lossless-copy contract are filesystem-oriented.
Workers WebAssembly supports Rust but shares the Worker isolate's memory limit,
does not provide native threads, and exposes only experimental WASI filesystem
support. Rewriting archive and bundle storage around those constraints would
create a second ingress adapter before it demonstrates product value.

The first processor therefore runs the canonical Rust crates in a `basic`
Container with 1 GiB memory, one-quarter vCPU, and 4 GB ephemeral disk. It uses
one container per active job, at most two concurrent containers initially, and
explicitly stops in a `finally` path after success, rejection, cancellation, or
failure. The platform's default idle timeout is not the cost boundary.

General outbound Internet access is disabled. The processor receives only a
narrow job description and accesses selected Cloudflare bindings through the
trusted Worker boundary. It holds no R2, D1, account, DNS, or publication
credential and cannot fetch bundle-named URLs. Its output is bounded structured
diagnostics, digests, report data, and HTML.

The OCI image and job contract preserve an escape hatch to another container
runtime if platform cost, limits, or availability change. Cloudflare-specific
state coordination stays outside the Rust core. A future measured prototype may
move the processor to Workers WebAssembly, but only if it preserves canonical
behavior and materially reduces operations or cost.

## Cost Profile And Guardrails

At the pricing reviewed for this decision, the Workers Paid plan has a USD $5
monthly minimum and includes 10 million Worker requests, 30 million Worker CPU
milliseconds, 25 GiB-hours of Container memory, 375 vCPU-minutes, and 200
GB-hours of Container disk. Static Asset requests are free and unlimited.

R2 Standard includes 10 GB-month of storage, one million Class A operations,
10 million Class B operations, and free Internet egress. D1, Queues, Durable
Objects, and Workers Logs also have included allowances far above the expected
initial workload.

A fully busy two-minute `basic` job consumes about 0.033 GiB-hours of memory and
0.5 vCPU-minutes. Approximately 750 such jobs fit in both included Container
dimensions. Above the allowance, the reviewed memory, CPU, and disk rates put
one additional fully busy two-minute job below USD $0.001 before negligible
queue and object operations.

The expected monthly shape is therefore:

| Workload | Planning estimate |
| --- | ---: |
| Idle or view-only month | about $5 |
| 100 two-minute uploads and 100,000 public views | about $5 |
| 1,000 two-minute uploads, ordinary retained sizes, and 1 million views | about $5.25 |
| 1,000 uploads retaining the maximum archive, model, and HTML sizes | about $6 |

These are planning estimates rather than a billing guarantee. Pricing changes
require documentation review before provisioning or materially increasing
limits.

The implementation must preserve that envelope:

- public report views fetch one immutable cached HTML object and do not invoke
  Worker, D1, Queue, or Container code;
- R2 Standard is used initially because its free tier is cheaper at this scale
  than Infrequent Access and its minimum-duration/retrieval costs;
- `max_instances` starts at two and excess jobs remain queued;
- every processor stops immediately after its job rather than waiting for the
  default ten-minute idle timeout;
- the two-minute job deadline and every byte/record/entry limit are enforced;
- failed quarantine objects expire and extracted working trees never persist;
- ordinary public views do not emit application logs, while lifecycle logs are
  structured, redacted, and sampled where appropriate;
- a low account budget alert is configured, with the understanding that alerts
  notify rather than hard-stop spend; and
- application quotas, rate limits, Turnstile, bounded concurrency, and admission
  backpressure remain the hard denial-of-wallet controls.

Turnstile is currently free for the selected scale. It supplements, but does
not replace, server-side quotas or the authentication decision.

## Abuse, Privacy, And Observability

Upload admission verifies Turnstile server-side and consumes its single-use
token. When #12 selects authentication, authenticated ownership and per-owner
quotas are evaluated at the same boundary. The Worker applies conservative
per-source and global admission limits; platform rate limiting is an additional
eventually consistent signal, not an accounting ledger.

The Worker proxies the bounded upload into private R2 rather than issuing a
reusable presigned write URL. This keeps the exact byte counter, ticket state,
idempotency rule, and overwrite prevention in one boundary. The 32 MiB product
cap stays below the platform request-body limit reviewed for the selected plan.

Logs and metrics may contain random report/job IDs, stage, stable diagnostic
code, profile and software version, byte/entry/record counters, duration,
retry count, and coarse outcome. They must not contain raw bundle bytes,
callsigns, grid locators, station coordinates, operator notes, attachment names,
Turnstile tokens, authentication tokens, deletion capabilities, or complete
public URLs. Debugging sensitive evidence requires an explicit authorized path,
not broader production logging.

Metrics alert on queue age, repeated processor failure, reconciliation backlog,
deletion backlog, rejected-input classes, container duration, retained bytes,
cache-miss rate, and current billable usage. A service degradation may delay
sharing, but it does not impair local operation.

## Verification

The hosted implementation uses deterministic, network-independent core tests
wherever practical.

Archive fixtures cover:

- absolute and parent traversal, separators, NULs, and non-portable names;
- duplicate, normalized-duplicate, and case-colliding paths;
- symbolic/hard links and every unsupported special type;
- encrypted, multi-disk, unsupported-compression, corrupt-CRC, truncated, and
  inconsistent-header archives;
- misleading declared sizes, individual and aggregate expansion bombs;
- entry count, path depth, compressed/expanded bytes, and exact N-1/N/N+1
  boundaries; and
- cancellation and injected disk-full during quarantine and extraction.

Canonical v1 and v2 bundle fixtures run through local and hosted entry points.
Given the same accepted bundle, profile, and software version, their ordered
diagnostics, report model, and standalone HTML are byte-identical. Hostile text
fixtures prove escaping, no script or external resource, correct media types,
CSP, `nosniff`, referrer, and framing policy.

State-machine tests inject failure after every D1, R2, Queue, processor, public
copy, and purge step. They cover duplicate and reordered Queue delivery,
dead-letter handling, stale reconciliation, upload conflict, publication/delete
races, immutable-object conflicts, and repeated deletion. No failure may expose
an unaccepted report, overwrite an immutable revision, lose a retained accepted
original, or report deletion while a known public cache entry remains.

Binding contract tests use local or fake Cloudflare services. A narrow deployed
smoke test may verify the configured bindings, cache, and scale-to-zero lifecycle,
but the main acceptance suite does not depend on a live account or network.

Cost-envelope tests assert configured instance type, concurrency, explicit
shutdown, time and input limits, lifecycle rules, cache policy, and absence of
per-view dynamic execution. Operational documentation records how to inspect
usage, set budget alerts, stop admission, drain the Queue, and delete retained
objects.

## Alternatives Considered

### Cloudflare-Native Dynamic Application

A Worker-rendered page for every view would keep authorization and deletion
checks centralized and remains inexpensive at initial scale. It was rejected as
the default public path because immutable trusted HTML already exists, static
cached views are faster and cheaper, and ordinary sharing does not need a live
database or application invocation. Private viewing may still use this path if
#12 requires it.

### Portable Always-On Rust Service

A conventional Rust web service maximizes immediate filesystem reuse and avoids
Container-specific coordination. It was rejected for the first deployment
because it introduces another origin, credential and network surface, deployment
system, and idle-cost floor. The selected OCI processor and narrow job contract
retain most of its portability without requiring an always-running service.

### Workers WebAssembly Processor

Running Rust directly in the Worker could remove Container cold starts and make
small jobs even cheaper. It was rejected initially because the canonical storage
layer expects a filesystem, WASI filesystem support is experimental, and the
entire isolate shares a 128 MB memory limit. A measured future adapter remains
possible after the hosted contract is proven.

### Presigned Direct R2 Upload

Presigned uploads avoid proxying request bytes through the Worker. They were
rejected for the first 32 MiB profile because a presigned PUT is a reusable
bearer capability until expiry and complicates overwrite, exact-length,
idempotency, and completion races. The bounded Worker stream is simpler and its
request usage fits the included plan.

### No Hosted Sharing

Standalone local HTML already satisfies offline reporting and can be shared
manually. This remains a valid user workflow, but it does not provide a stable
managed link, safe optional upload flow, deletion lifecycle, or maintained
sample/explanatory site. The selected adapter adds that convenience without
weakening the offline product.

## Consequences

- AntennaBench remains complete and useful offline; hosted outages affect only
  optional sharing.
- Public report views are static, globally cached, and normally have no dynamic
  compute or database cost.
- The expected fixed service floor is about USD $5 per month rather than an
  always-running server bill.
- The canonical Rust validation, analysis, and renderer remain the behavioral
  authority across local and hosted entry points.
- A new archive adapter and hostile-input corpus are required even though ZIP is
  only transport packaging.
- Hosted-standard-v1 rejects some locally valid bundles and requires explicit
  versioning when limits change.
- Retaining exact accepted archives plus derived output consumes more storage
  than retaining reports alone, but preserves auditability at a low current R2
  price.
- Cloudflare service coordination is eventually reconciled rather than
  transactionally atomic.
- Public cache deletion is an explicit purge workflow and cannot retract copies
  already downloaded by another party.
- The Container runtime adds a platform dependency and cold-start path, while
  the OCI image and job contract preserve a portable-service escape hatch.
- Identity, ownership, visibility, raw download, moderation, and account
  deletion remain blocked on #12 rather than being smuggled into infrastructure.

## References

- [Hosted boundary decision #11](https://github.com/rwjblue/antennabench/issues/11)
- [Hosted sharing tracker #10](https://github.com/rwjblue/antennabench/issues/10)
- [Hosted identity decision #12](https://github.com/rwjblue/antennabench/issues/12)
- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0009](0009-use-layered-bundle-validation-profiles.md)
- [Decision 0011](0011-use-a-fixed-bounded-local-resource-profile.md)
- [Cloudflare Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/)
- [Cloudflare Workers limits](https://developers.cloudflare.com/workers/platform/limits/)
- [Cloudflare Static Assets](https://developers.cloudflare.com/workers/static-assets/)
- [Cloudflare Containers pricing](https://developers.cloudflare.com/containers/pricing/)
- [Cloudflare Container limits](https://developers.cloudflare.com/containers/platform-details/limits/)
- [Cloudflare Container outbound controls](https://developers.cloudflare.com/containers/platform-details/outbound-traffic/)
- [Cloudflare R2 pricing](https://developers.cloudflare.com/r2/pricing/)
- [Cloudflare R2 consistency](https://developers.cloudflare.com/r2/reference/consistency/)
- [Cloudflare R2 cache integration](https://developers.cloudflare.com/cache/interaction-cloudflare-products/r2/)
- [Cloudflare D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/)
- [Cloudflare Queues delivery guarantees](https://developers.cloudflare.com/queues/reference/delivery-guarantees/)
- [Cloudflare Queues pricing](https://developers.cloudflare.com/queues/platform/pricing/)
- [Cloudflare Turnstile plans](https://developers.cloudflare.com/turnstile/plans/)
- [Cloudflare Turnstile server-side validation](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/)
- [Cloudflare usage-based billing](https://developers.cloudflare.com/billing/understand/usage-based-billing/)
