# 0020: Defer Local SQLite Until A Measured Query Requires It

## Status

Accepted.

## Context

Decision 0001 makes each portable session bundle the durable source of truth
and names local indexes as disposable projections. The shipped product has no
index implementation or database dependency, but that absence did not yet have
an explicit product and lifecycle rationale.

The current query inventory is bundle-shaped rather than database-shaped:

- open selects one operator-chosen bundle and inspects or reads one complete
  coherent checkpoint;
- the conductor projects current and next actions from the selected bundle's
  schedule, events, and checkpoint revision;
- live adapters append to that same bundle and verify the resulting revision;
- imports are explicit, bounded operations against one selected session;
- analysis and report generation intentionally consume the complete validated
  bundle so exclusions, duplicates, provenance, and missingness remain visible;
  and
- export copies or renders one verified revision.

No shipped surface lists an application-managed session library, searches
across bundles, performs full-text lookup, or repeatedly sorts a large
cross-session result. The public-preview roadmap likewise prioritizes release
and field validation before rebuildable local search. Repeated active-session
reads may justify retaining a verified in-memory projection or improving the
streaming reader before they justify a persistent database.

The local resource profile permits at most 250,000 records in one JSONL stream
and 500,000 modeled records across a bundle. Those are adversarial safety
ceilings, not expected session sizes or a requirement that every interactive
view query the ceiling. Existing fixtures, end-to-end scenarios, and CI expose
no measured index-sensitive latency. Direct reads are therefore sufficient for
the known product queries. They are likely to become insufficient first for a
cross-session catalog containing hundreds or thousands of bundles, repeated
callsign/path/full-text lookup, or interactive filtering over a demonstrated
large-session corpus.

## Decision

AntennaBench will **not add SQLite or another persistent local index yet**. A
production index requires a focused issue naming the exact query, a
representative privacy-safe corpus, and a repeatable benchmark that measures
direct-bundle latency and memory against a candidate projection. The issue must
show a material user-visible benefit that simpler in-memory or streaming
changes do not provide.

The decision is revisited when at least one of these exists:

- an approved cross-session browser or search workflow;
- a measured active-session or report query that misses its documented
  interaction budget on representative bundles; or
- a bounded query that cannot be implemented without repeatedly reparsing a
  large unchanged committed revision.

The benchmark, query contract, expected corpus shape, update frequency, and
platform targets must be recorded before choosing a Rust database library. No
implementation issue is created by this decision because no current query
meets that bar.

## Future Disposable Cache Contract

If a measured query later warrants SQLite, these rules apply unless superseded
by another ADR:

### Ownership and authority

- A Rust-owned cache service is the only layer allowed to open the database.
  The webview receives typed query results, never paths, SQL, a connection, or
  general filesystem authority.
- Cache files live under the platform application cache directory, never
  inside a bundle, its export, or its attachment tree.
- Every cache row is reproducible from a supported, successfully inspected
  bundle revision. The database is never an input to bundle write, recovery,
  validation, lossless export, or authoritative report generation.
- Deleting the complete cache tree while AntennaBench is stopped must lose no
  session, evidence, operator action, report meaning, preference needed to use
  a bundle, or hosted ownership fact.

### Identity and external changes

- A filesystem path and session ID are locators, not sufficient cache keys.
  Copies can share an ID and later diverge; a bundle can move without changing
  its evidence.
- A cache generation is keyed by an index-format version plus a strong identity
  of the exact committed source revision: schema and session identity,
  checkpoint generation where applicable, and digests of the committed modeled
  files or prefixes that feed the projection.
- Size, modification time, and file inventory may reject an obviously stale
  entry cheaply, but they cannot establish a cache hit without the strong
  revision identity. Uncheckpointed tails are never indexed as committed data.
- A moved or byte-identical copy may reuse a generation after strong identity
  verification. A divergent copy builds a separate generation even when its
  session ID or former path matches.

### Build, invalidation, and failure

- A builder reads through the same bounded storage inspection/projection APIs
  as the direct path. It writes a unique staging database in one transaction,
  records the complete source identity and index-format version, closes it, and
  publishes it atomically. A partial build is never queryable.
- Source-identity mismatch, unsupported bundle schema, index-format mismatch,
  corruption, or incomplete build causes deletion and rebuild. Disposable
  index schemas are replaced, not migrated in place.
- A live checkpoint revision invalidates the prior active generation. An
  implementation may build revision-keyed snapshots or refresh atomically, but
  must not combine rows from different revisions.
- Open, validation, conductor, report, and export operations fall back to the
  direct bundle path when the cache is absent, stale, corrupt, rebuilding, over
  budget, or unwritable. Cache failure is scoped and cannot make a readable
  bundle unavailable.
- Contract tests delete, corrupt, truncate, move, copy, externally modify, and
  concurrently rebuild caches, then compare every typed result with the direct
  bundle path.

### Privacy, retention, and noncanonical local state

- A cache can reproduce callsigns, locations, notes, and raw-source facts and
  therefore receives the same local privacy treatment as the source. It uses
  user-only permissions, no telemetry or synchronization, and bounded
  least-recently-used cleanup. Documentation tells backup tools that it is
  disposable.
- Disposable navigation, window, selected-tab, sort, filter, pagination, and
  recent-locator state may live outside a bundle. Reusable station form
  preferences and platform credentials may also live in their existing
  purpose-specific stores because they are not session evidence.
- A tag, note, correction, antenna fact, import disposition, publication
  ownership fact, or other value that must survive cache deletion is not UI or
  index state. It must use an approved canonical bundle or service boundary.

## Options Considered

| Option | Result | Rationale |
| --- | --- | --- |
| No index until demonstrated | Selected | Every current operation consumes one bounded coherent bundle, and no measured query shows a user-visible database benefit. |
| Per-bundle disposable index | Deferred | It may help repeated large-session filtering, but it adds revision identity, rebuild, cleanup, and native dependency work while authoritative operations still need bundle validation. |
| Application catalog plus per-session data | Deferred | It becomes useful only with an approved cross-session library/search workflow and carries the greatest stale-path, divergent-copy, and privacy surface. |

## Rust Library Evaluation

No library is selected or added. For a future focused spike:

- `rusqlite` is the leading small synchronous candidate for a desktop-owned
  disposable cache. It is a direct SQLite wrapper and its documented `bundled`
  feature pins and links SQLite rather than depending on a possibly absent or
  old system library.
- SQLx supports SQLite, asynchronous runtimes, and optional compile-time query
  checking. That machinery is useful only if the measured cache service needs
  an async pool or shares a broader SQL toolkit; it is not justified by the
  current synchronous local query shape.
- Diesel supplies a typed query builder/ORM and migration infrastructure with a
  SQLite backend. Its schema and migration strengths are unnecessary when an
  index-format change deliberately deletes and rebuilds the entire cache.

The eventual issue must re-evaluate maintained versions, minimum Rust version,
native build and cross-compilation behavior, licenses, vulnerability policy,
binary size, cancellation, connection threading, and Tauri packaging rather
than treating this candidate order as permanent approval.

## Consequences

- The bundle remains the only local session evidence store and no shadow
  persistence model is introduced for an unmeasured benefit.
- Current open, conductor, adapter, analysis, report, and export paths retain
  their direct bounded behavior.
- A future index has a concrete identity, rebuild, fallback, privacy, and proof
  contract before implementation begins.
- Cross-session discovery remains a later product decision, not an accidental
  consequence of choosing a database library.

## References

- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Issue #7](https://github.com/rwjblue/antennabench/issues/7)
- [rusqlite](https://github.com/rusqlite/rusqlite)
- [SQLx](https://github.com/transact-rs/sqlx)
- [Diesel SQLite documentation](https://docs.diesel.rs/main/diesel/sqlite/index.html)
