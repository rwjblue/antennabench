# 0008: Use Provider-Neutral Adapter Evidence In Bundle Schema V2

Date: 2026-07-14

## Decision

AntennaBench's next durable bundle revision will separate evidence provenance
from acquisition mechanics. Bundle schema version 2 will use validated,
provider-neutral string identities and a generic adapter-evidence stream while
keeping normalized observations as the analysis boundary.

Version 1 remains readable, analyzable, reportable, and losslessly copyable.
Opening or exporting a version 1 bundle never upgrades or rewrites it. New
sessions and new adapter imports will write version 2 after the migration is
implemented. An explicit upgrade creates a new destination and never overwrites
the source.

Source adapters remain compile-time workspace components. Runtime-loaded
plugins, dynamic libraries, scripting hosts, and downloadable adapter code are
not part of this boundary.

## Structured Provenance

Version 2 replaces the closed `RecordSource` wire identity with structured
provenance containing at least:

- `provider_id`: the organization or system responsible for the evidence;
- `source_id`: the provider product or dataset;
- `acquisition_channel`: how AntennaBench received the input;
- `adapter_id`: the parser or adapter implementation; and
- the adapter/parser version.

These identities are lowercase-ASCII string newtypes with a maximum of 128
bytes. Each uses one or more alphanumeric segments separated by `.`, `_`, or
`-`; separators cannot lead, trail, or repeat. They are not provider enums.
Known identities may receive friendly display labels, but an unknown valid
identity remains readable and renders as its literal value.

Signal mode and `ObservationKind` stay separate from provenance. Paired
analysis stratifies by provider and source identity alongside mode and
observation kind; it does not substitute the acquisition channel or adapter
implementation for the evidence source.

The version 1 mapping is explicit and conservative. For example, `wsjtx_log`
maps to provider `wsjt-x` and channel `log-import`, while a provider-only legacy
value such as `wsprnet` maps to `legacy-unspecified` acquisition rather than
inventing a channel.

## Generic Adapter Evidence

Version 2 replaces provider-specific durable raw streams such as
`wsjtx.jsonl` with `adapter-records.jsonl`. Each adapter record contains:

- a stable record ID and structured provenance;
- capture time and source event time when known;
- provider record type;
- a broad disposition and stable reason identifier;
- links to any normalized observation IDs; and
- exact or near-raw input inline, or a content-addressed attachment reference.

Attachments are content-addressed by SHA-256. References include the digest,
byte size, media type, encoding/container, and source locator. Small WSJT-X
lines and datagrams may be stored inline. Large inputs such as an RBN daily ZIP
remain exact compressed attachments; a bounded import record retains the
selected archive entry, session/callsign filter, parser version, dispositions,
result counts, and reproduction metadata.

Relevant malformed, unsupported, filtered, duplicate, and partially normalized
inputs remain auditable. Rows excluded by a reproducible coarse filter may be
represented by deterministic counts when the exact source attachment is
retained. Every adapter-produced normalized observation links back to its
adapter evidence.

## Version And Migration Contract

Storage dispatches on `manifest.schema_version` into distinct persisted v1 and
v2 representations. It must not simulate a migration by adding permissive
defaults to v1 wire structures.

- V1 bundles continue to open, validate, analyze, render, and export without
  byte changes.
- New sessions and adapter imports use v2 after the migration lands.
- V1 upgrade writes a separate v2 destination, maps legacy source values and
  WSJT-X raw records, and verifies normalized semantic equivalence plus retained
  raw evidence.
- Unknown schema versions fail closed with a typed unsupported-version error.
- V2 has no downgrade to v1.

Storage projects both wire versions into a current internal model so analysis
and reporting do not carry persistence-version branches.

The version 2 directory suffix is `.session.antennabundle`. The existing
`.session.wsprabundle` suffix remains the version 1 compatibility name and is
never renamed merely because the bundle was opened or copied. The product name
remains AntennaBench. Generic product copy becomes signal/observation neutral,
while WSPR-specific workflows, adapters, fixtures, and limitations retain
accurate WSPR wording.

## Adapter Boundary

Adapters accept a reader, selected attachment, or similarly explicit input and
perform no hidden network access. Downloading, HTTP policy, archive selection,
retry, caching, and user orchestration remain outside deterministic parsing and
normalization.

A public shared Rust trait is deferred until multiple adapters demonstrate a
stable execution interface. The durable input/output contract is the common
boundary in this revision.

## Alternatives Considered

### Add Provider Variants To Version 1

Adding `RecordSource::Rbn` would be the smallest immediate patch. It was
rejected because every provider would continue changing core schema and
exhaustive consumers, while raw preservation remained provider-specific.

### Provider-Neutral Version 2

This selected option requires a focused migration but preserves source
identity, auditability, extensibility, and deterministic offline import without
making provider quirks core invariants.

### Ephemeral External Overlays

Loading public spots only in memory was rejected because a report could no
longer be regenerated from its session bundle, violating the bundle
source-of-truth decision.

### Runtime Adapter Plugins

A runtime plugin ABI was rejected because no current use case justifies its
security, compatibility, distribution, and lifecycle costs. Compile-time
workspace adapters keep the first boundary reviewable and deterministic.

## Consequences

- A focused v2 storage/core migration must land before the first RBN archive
  adapter.
- Existing v1 fixtures become compatibility, semantic-equivalence, and
  lossless-copy migration fixtures.
- The narrow NOAA adapter and typed propagation-evidence decision need explicit
  v1-to-v2 provenance mappings.
- Provider/source identity can expand without adding a new core enum variant.
- Large exact inputs can remain compressed while their bounded import and
  disposition evidence stays queryable.
- Product wording can become source-neutral without obscuring genuinely
  WSPR-specific behavior.
- Network access remains optional and outside adapter parsing.

## References

- [Decision issue #27](https://github.com/rwjblue/antennabench/issues/27)
- [RBN archive adapter #29](https://github.com/rwjblue/antennabench/issues/29)
- [Mode-stratification issue #28](https://github.com/rwjblue/antennabench/issues/28)
- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0006](0006-capture-rich-typed-propagation-evidence.md)
- [Serde enum representations](https://serde.rs/enum-representations.html)
- [Serde variant attributes](https://serde.rs/variant-attrs.html)
- [RBN raw archive](https://www.reversebeacon.net/raw_data/)
