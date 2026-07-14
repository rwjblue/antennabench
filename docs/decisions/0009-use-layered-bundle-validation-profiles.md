# 0009: Use Layered Bundle Validation Profiles

Date: 2026-07-14

## Decision

AntennaBench will separate bundle inspection, structural integrity, semantic
diagnostics, operation-specific eligibility, and strict creation into layered
validation profiles.

Schema versions describe persisted wire shape. They do not imply that every
bundle expressible in that version is suitable for every operation. Adding a
new domain diagnostic therefore does not itself require a schema revision.

Version 1 remains readable, inspectable, reportable when its evidence is
eligible, and losslessly copyable. Existing version 1 bytes are never changed
merely because validation found a warning or produced a normalized in-memory
projection. Newly authored bundles, including version 2 bundles introduced by
[Decision 0008](0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md),
must pass the strict creation profile before any durable write is promoted.

No validation path silently repairs source evidence. Derived alignment fields
may be regenerated in memory, but the original values and bytes remain
available for diagnostics and lossless export.

## Validation Layers

Validation produces one deterministic report and applies different operation
profiles to it. It does not collapse every concern into one valid/invalid bit.

### Wire And Storage Integrity

This layer establishes whether persisted bytes can be interpreted
unambiguously and safely. It covers:

- supported schema dispatch;
- required files and safe, unique manifest paths;
- valid JSON or one complete JSON value per nonblank JSONL line;
- duplicate object member names in modeled envelopes;
- required field presence and wire types;
- schema and session identity agreement;
- unique durable record and slot identities;
- resolvable modeled references; and
- a schedule order and window layout that alignment can interpret
  deterministically.

A blocker here prevents normalized typed projection and analysis. It does not
prevent an independent storage-safe lossless copy of the original bundle.
Opaque attachment and source payload bytes are not parsed as modeled JSON.

Duplicate member names inside a modeled JSON object are blockers because JSON
implementations disagree about which value is authoritative. Duplicate names
inside a version 1 `raw` value are diagnosed as legacy source ambiguity rather
than interpreted as normalized truth; lossless copying preserves the original
JSONL bytes. Version 2 adapters preserve exact source input inline or as an
attachment before projecting it into normalized fields.

### Bundle Semantic Diagnostics

This layer checks whether values have coherent domain meaning without
pretending every questionable legacy value makes the entire bundle unreadable.
Diagnostics distinguish:

- **semantic blockers**, where a named operation cannot be honest or
  deterministic; and
- **semantic warnings**, where the value remains inspectable but may be
  incomplete, implausible, source-specific, or unusable for a narrower
  purpose.

Semantic blockers stop only operations identified by the diagnostic. For
example, ambiguous antenna labels block slot alignment and paired comparison,
but not station inspection or lossless export.

### Analysis Eligibility

Analysis consumes explicit eligibility outcomes. An invalid observation field
excludes the smallest honest scope: the field, observation, slot, comparison
stratum, or comparison block. One unusable SNR value must not make unrelated
evidence disappear or fail the entire bundle summary.

Exclusions retain stable reasons and counts in analysis output and reports.
Missing evidence remains distinguishable from malformed, contradictory,
unsupported, and deliberately excluded evidence.

### Strict Creation And Promotion

New-session creation, adapter normalization, bundle upgrade, and durable write
promotion validate before persistence. A newly authored normalized value may
not carry a structural blocker or semantic warning. Input that cannot be
normalized stays in attributed adapter evidence with a disposition; it is not
coerced into a plausible normalized value.

Every floating-point value is checked with `is_finite()` before serialization.
JSON does not permit NaN or infinity, and the selected JSON serializer can
otherwise encode a non-finite Rust float as `null`, silently changing
"invalid" into "missing."

## Diagnostic Contract

Diagnostics are data, not formatted error strings. Each diagnostic contains:

- a stable machine code independent of Rust type and variant names;
- category: wire, structural, semantic, or eligibility;
- severity and operation impact;
- bundle file or stream role;
- record kind, stable ID when available, and physical line or record index;
- a JSON Pointer-style field path when applicable;
- an operator-facing message; and
- related record or field identities when the problem is cross-record.

Codes use namespaces such as `bundle.structure.duplicate_id`,
`bundle.semantic.invalid_range`, and
`analysis.observation.ineligible.band_mismatch`. Tests assert codes, paths,
and impact rather than complete prose. Diagnostics are returned in stable
file, record, field, and code order.

The current `validate_bundle()` all-or-nothing API remains available as a
compatibility wrapper while callers migrate to a report plus explicit
operation profile.

## Durable Field Policy

The following rules apply to normalized modeled fields. Resource limits for
files, record counts, text, nesting, and attachments are selected by issue
#40 and compose with these semantic rules.

### Manifest, Identity, And Paths

- Schema versions must be supported explicitly; unknown versions block typed
  interpretation.
- Root and record session IDs must agree with the manifest.
- Newly generated session, slot, event, observation, adapter, rig, and
  propagation IDs are nonempty, at most 128 ASCII bytes, and unique within
  their identity domain.
- A unique but empty legacy ID is a warning and blocks mutation or upgrade of
  the affected record. A duplicate ID is a structural blocker because a
  reference cannot select one record unambiguously.
- Manifest paths remain single-component, unique, relative names with the
  existing symlink and containment protections.
- `created_at`, application version, and source timestamps must parse. Their
  cross-record plausibility is a warning unless an operation depends on the
  ordering.

### Station And Antennas

- A newly authored station callsign and grid are trimmed and nonempty.
  Protocol-specific workflows validate and canonicalize the values they need
  before the user confirms creation.
- Core validation does not impose the WSJT-X WSPR callsign grammar or one
  jurisdiction's operating rules on every source. Legacy literals are
  preserved exactly and receive adapter- or workflow-specific diagnostics.
- Maidenhead syntax is required when a workflow claims that a value is a
  Maidenhead locator. An invalid legacy grid is a warning and makes dependent
  location analysis ineligible; it does not rewrite the literal.
- Present station or observation transmitter power must be finite and greater
  than zero. Unknown or inapplicable power is absent rather than encoded as
  zero.
- Antenna labels are trimmed, nonempty, at most 128 UTF-8 bytes, free of
  control characters, and unique. Ambiguous labels block schedule alignment.
- Present height and radial length are finite and nonnegative. Present
  orientation is finite and in `[0, 360)` degrees. Facets and descriptive
  strings remain operator-authored metadata rather than controlled semantic
  identities.

### Schedule

- A newly authored schedule contains at least one slot.
- Slot IDs are unique. Sequence numbers are unique, strictly increasing in
  persisted slot order, and need not be contiguous; gaps do not change slot
  identity.
- Slot start times are strictly increasing, durations are greater than zero,
  windows do not overlap, and guard time is less than duration.
- Every scheduled antenna label resolves to exactly one antenna.
- Whole-station, transmit-focused, and receive-focused comparison schedules
  use at least two distinct scheduled antenna labels. Single-antenna profiling
  uses exactly one scheduled label and pairs the profiling mode and goal.
- A legacy experiment-shape mismatch is a semantic warning and makes only the
  unsupported comparison ineligible. It does not change the stored mode,
  goal, labels, or order.

### Operator Events

- Event IDs and present slot references follow the identity and reference
  rules above.
- Event timestamps, slot-relative timing, incompatible event combinations,
  session lifecycle, observed actual antenna state, and corrections produce
  diagnostics but are not silently reordered or collapsed.
- Decision #39 defines the future authoritative event payload, correction,
  replay, and lifecycle semantics. This decision does not preempt that durable
  model.

### Observations

- Observation IDs and session metadata follow the identity rules above.
- Present frequency is greater than zero. Exact band edges and permitted
  frequencies remain workflow-, mode-, and jurisdiction-specific; adapters
  validate their source contract.
- A band/frequency disagreement is visible and excludes the affected evidence
  from any analysis that depends on agreement. It does not rewrite either
  field.
- Present mode and callsign/grid fields are validated by the adapter or
  analysis that understands their protocol. Core preserves their literals.
- Present distance is finite and nonnegative. Present azimuth is finite and in
  `[0, 360)` degrees. Present SNR, drift, and power are finite, with power
  greater than zero.
- Slot confidence is finite and in `[0, 1]`.
- Slot ID, label, and confidence are derived alignment annotations. A
  normalized projection may regenerate them in memory. A mismatch is
  diagnosed, source bytes remain unchanged, and analysis uses the regenerated
  projection rather than stale annotations.
- `raw` remains attributed source evidence and is not promoted to normalized
  truth merely because it contains a similarly named field.

### Adapter, Rig, And Propagation Records

- Record IDs, session metadata, and timestamps follow the common identity
  rules.
- Provider record types, rig status, and rig mode remain adapter-defined
  strings. Empty or unsupported values are diagnosed by their consumer.
- Present rig frequency is greater than zero.
- Every normalized propagation float is finite. Universally defined ranges,
  such as planetary Kp in `[0, 9]`, are enforced for normalized fields.
  Product-specific quality, status, interval, location, and acceptable-range
  rules remain adapter responsibilities.
- A source value that lacks the semantics required by a normalized field stays
  in raw or typed adapter evidence. It is not copied into a similarly named
  core field.
- Alerts, daylight state, and other legacy free-form values are preserved but
  do not become controlled lifecycle or derived-context values without a
  separately defined model.

### Analysis Metadata And Free-Form Text

- `generated` analysis status requires a generation timestamp for newly
  authored metadata. A legacy mismatch is a warning; analysis output is
  regenerated from bundle evidence rather than trusted as canonical input.
- Operator notes, antenna descriptions, alerts, and other free-form text are
  untrusted display content. They are escaped by renderers and bounded by the
  resource policy, but core validation does not infer facts from their text.

## Unknown And Additional Fields

Version 1 readers do not enable `deny_unknown_fields` as a blanket policy.
Unknown members in a supported schema version receive a stable diagnostic,
are ignored by normalized analysis, and remain in the original bytes for
lossless copy. This preserves evidence and exposes likely typos without
inventing semantics.

The AntennaBench writer never emits unmodeled fields. Strict authored-bundle
ingress rejects them. A new wire shape uses a new explicitly dispatched schema
version rather than relying on ignored fields for compatibility.

## Version 1 Compatibility And Version 2 Upgrade

Opening a version 1 bundle runs the compatibility profile:

- storage-safe source inspection and lossless copy remain available;
- structural blockers prevent only typed operations that require an
  unambiguous model;
- semantic warnings remain visible without changing source bytes;
- analysis uses granular eligibility and regenerated derived annotations; and
- reports disclose exclusions and warnings relevant to their conclusions.

The explicit version 1 to version 2 upgrade from Decision 0008 writes a new
destination and never mutates the source. Structural ambiguity blocks the
upgrade. Warning-bearing evidence may upgrade only when its normalized meaning
and source evidence can both be retained and the warning remains representable;
otherwise the upgrade stops with actionable diagnostics. The upgrader never
guesses, clamps, renumbers, normalizes, or drops evidence to make a bundle pass.

All checked-in version 1 fixtures remain compatibility, semantic-equivalence,
lossless-copy, and migration fixtures. The proposed identity, label, schedule,
guard, and normalized observation range rules do not reject any current
fixture.

## Hosted And Local Reuse

Local creation and hosted ingress call the same strict semantic profiles.
Hosted ingress additionally applies the archive, decompression, request, and
resource limits selected by issues #11 and #40. Hosting does not gain a second
interpretation of bundle meaning, and local operation never depends on the
hosted service.

## Verification

Implementation includes deterministic adversarial fixtures for every stable
diagnostic code and operation impact, including:

- duplicate JSON members, IDs, labels, and sequence numbers;
- missing and dangling references;
- empty and boundary-length identities;
- zero duration, consumed guard windows, overlap, and order ambiguity;
- non-finite in-memory values before serialization and finite range edges;
- unknown fields and unsupported schema versions;
- protocol-invalid callsigns, grids, modes, and band/frequency combinations;
- contradictory event evidence and stale derived annotations;
- warning-only legacy read, granular analysis exclusion, strict write refusal,
  and blocked/non-destructive upgrade; and
- byte-identical lossless copy of warning-bearing version 1 evidence.

Property tests should cover identifier and numeric boundaries where they add
confidence. Mutation tests should start from checked-in valid fixtures so each
failure changes one condition and produces deterministic diagnostics.

## Alternatives Considered

### Keep Version 1 Structurally Permissive

Leaving all stronger checks to adapters and analyses minimizes immediate
compatibility risk. It was rejected because each consumer would rediscover
different validity rules, hosted ingress and local creation could disagree,
and ambiguous evidence could reach conclusions without a common diagnostic.

### Layered Backward-Compatible Validation

This selected approach preserves old evidence while making ambiguity,
warnings, and eligibility explicit. It adds API and diagnostic design work,
but that complexity reflects real differences in operation impact.

### Make Strict Semantics A Version 2 Wire Invariant

Requiring every stronger rule solely through schema version 2 creates a clean
new boundary. It was rejected because wire evolution and domain-policy
evolution have different cadences, and it would force harmless legacy
warnings either to block migration or to be silently repaired. Version 2 new
writes are strict, but the strictness comes from the creation profile rather
than the version number alone.

## Consequences

- Core and storage gain a stable diagnostic report and explicit operation
  profiles before new session creation is exposed.
- Analysis and reports move from whole-bundle failure toward the smallest
  honest exclusion scope.
- Newly authored bundles cannot persist non-finite, ambiguous, or warning-level
  normalized values.
- Version 1 remains non-destructive and useful without being treated as
  semantically perfect.
- Version 2 implementation can reuse the same internal validation report while
  retaining distinct wire models.
- Adapter-specific and provider-specific constraints stay out of universal
  core rules.
- Decision #39 remains responsible for event correction and conductor
  lifecycle semantics, and issue #40 remains responsible for resource bounds.
- Hosted validation can reuse local semantics without coupling local workflows
  to hosting.

## References

- [Decision issue #38](https://github.com/rwjblue/antennabench/issues/38)
- [Local conductor tracker #45](https://github.com/rwjblue/antennabench/issues/45)
- [Bundle schema version 2 implementation #46](https://github.com/rwjblue/antennabench/issues/46)
- [Crash-safe mutation and event decision #39](https://github.com/rwjblue/antennabench/issues/39)
- [Resource-limit decision #40](https://github.com/rwjblue/antennabench/issues/40)
- [Hosted boundary decision #11](https://github.com/rwjblue/antennabench/issues/11)
- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0008](0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md)
- [RFC 8259: JSON](https://www.rfc-editor.org/rfc/rfc8259.html)
- [Serde container attributes](https://serde.rs/container-attrs.html)
- [serde_json number
  representation](https://docs.rs/serde_json/latest/serde_json/value/struct.Number.html)
