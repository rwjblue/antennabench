# Bundle Format

A session bundle is a directory ending in `.session.wsprabundle`. It contains
JSON root files, JSONL streams, and an attachments directory.

Default file layout:

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

The implemented schema version is `1`.

Schema version 2 is approved but not yet implemented. In addition to the
provider-neutral adapter evidence in
[Decision 0008](decisions/0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md),
new mutable sessions will use versioned plan generations and a
`session-state.json` checkpoint that commits one lifecycle state and coherent
prefix of every append-only stream. Mutation IDs, committed byte lengths,
record heads, and digests make retry, recovery, active report refresh, and
export deterministic. The complete persistence and operator-event contract is
[Decision 0010](decisions/0010-checkpoint-append-only-live-session-mutations.md).

Version 1 is never silently rewritten to gain these semantics. It remains
readable and losslessly copyable; live mutation requires an explicit v2 upgrade
to a new `.session.antennabundle` destination.

## Planned Local Resource Profile

[Decision 0011](decisions/0011-use-a-fixed-bounded-local-resource-profile.md)
selects one fixed first-product profile, `local-standard-v1`. It is an
operational policy rather than a schema invariant, and its implementation is
tracked by [#55](https://github.com/rwjblue/antennabench/issues/55),
[#56](https://github.com/rwjblue/antennabench/issues/56), and
[#57](https://github.com/rwjblue/antennabench/issues/57).

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

These limits are not yet enforced by the current schema-v1 reader. Until the
implementation issues land, operator-selected input should still be treated as
potentially unbounded.

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
- `wsjtx.jsonl`: raw or near-raw WSJT-X adapter records, including
  `all_wspr_decode` for parsed decode rows and `all_wspr_malformed` for
  preserved lines that could not become observations. Live companion records
  use `udp_heartbeat`, `udp_status`, `udp_wspr_decode`, and `udp_close`.
- `rig.jsonl`: rig adapter state.
- `propagation.jsonl`: time-scoped propagation context. The implemented NOAA
  SWPC adapter writes observed F10.7 and provisional estimated planetary Kp as
  separate sparse records because the products have different source times.
  `meta.timestamp` is response receipt time, `observed_at` is the upstream UTC
  `time_tag`, and `raw` retains the exact endpoint, selected near-raw object,
  retrieval time, provider/semantics attribution, and available HTTP metadata.
  Repeated unchanged source observations are not appended.

Every JSONL record includes `meta` with schema version, session id, timestamp,
and source.

Offline WSJT-X WSPR log import preserves every nonblank imported line in
`wsjtx.jsonl`. Valid `ALL_WSPR.TXT`-style decode rows also produce
`observations.jsonl` local decodes. Malformed rows are retained as adapter
records with issue details and do not produce observations.

Live WSJT-X ingestion preserves the complete UDP datagram as lowercase hex plus
its parsed known fields. Supported heartbeat, status, WSPR decode, and close
messages become `wsjtx.jsonl` records. Unknown message types are ignored, and
compatible fields appended after the known message layout are left in the
preserved datagram without changing bundle schema version 1.

Only new, on-air, nonduplicate WSPR decodes from a client whose latest status is
in WSPR mode and matches the session station callsign and grid become local
observations. Replayed, off-air, duplicate, semantically invalid, or
insufficiently identified decodes remain auditable WSJT-X records without an
observation.

## Observation Slot Annotations

Observations store computed alignment fields:

- `slot_id`
- `slot_label`
- `slot_confidence`

These fields are derived from schedule, events, and observation timestamps. They
are persisted for auditability and easy downstream use, but they can be
regenerated by normalization.

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
- analysis additionally rejects semantic facts that would make evidence
  interpretation unsafe; persisted alignment annotations may be regenerated
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
- planned slot, event, observation, WSJT-X, rig, and propagation IDs are unique
- planned slot antenna labels exist in `antennas.json`
- planned slot windows are sorted and non-overlapping
- event and observation slot references point to known planned slots
- observation slot confidence values are in `0.0..=1.0`
- persisted slot annotations match regenerated alignment output

Use `BundleStore::inspect()` to retain the report beside an optional all-or-none
typed bundle. `read()` applies the compatibility profile,
`read_validated()` requires a completely clean report, and
`read_normalized_validated()` applies the analysis profile after regenerating
observation alignment annotations. The complete policy is recorded in
[Decision 0009](decisions/0009-use-layered-bundle-validation-profiles.md).

## Lossless Copies

`BundleStore::copy_losslessly_to()` creates a new bundle directory by copying
the source representation instead of serializing a typed in-memory model.
Manifest-declared durable root files and the complete nested `attachments/`
tree retain their original bytes. The source need not be safe for typed
interpretation: duplicate modeled members, duplicate legacy raw members, and
unknown fields remain preservable. The manifest and filesystem layout must
still be safe to traverse. The source is never modified.

The destination must not already exist or be inside the source; lossless copy
never overwrites or merges. Symbolic links and non-file/non-directory
filesystem entries are rejected. If copying or verification fails, the newly
created destination is removed when it remains safe to do so. Verification
reopens the copied manifest and checks the storage layout; it does not require
typed projection of the preserved evidence.

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
