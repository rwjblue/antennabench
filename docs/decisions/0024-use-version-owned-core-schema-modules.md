# 0024: Use Version-Owned Core Schema Modules

Date: 2026-07-17

## Status

Accepted.

## Context

`antennabench-core` currently glob re-exports eleven implementation modules
from its crate root. Version-owned names such as `RecordMetaV2`,
`OperatorEventPayloadV3`, and `AntennaControlInvocationV5` therefore share one
flat namespace with unversioned domain, validation, and projection APIs. A new
schema family can silently enlarge that root surface or collide with an
existing export.

Rust module paths are not persisted by serde. The bundle contract comes from
field and variant names, serde attributes, version dispatch, and storage
validation. Reorganizing imports without changing those definitions has no
offline, failure-mode, migration, or wire-format effect. Golden bundles,
round-trip tests, and lossless-copy tests remain the compatibility proof.

Schema v4 explains why numeric versions and Rust model modules must not be
treated as one-to-one. [Decision 0018](0018-use-directed-counterbalanced-wspr-cycles.md)
added optional `direction` to `WsprCycleIntentV3`, then selected schema version
4 to require directed WSPR intentions and mode-compatible direction sets for
new sessions. Storage continues to deserialize v3 and v4 as
`BundleV3Contents`, use the same checkpoint envelope, and preserve unknown v3
direction rather than guessing. The v4 gates live in
`crates/storage/src/v3.rs`; upgrade and lossless-copy dispatch also accept v4
through that representation. An empty `v4` Rust module would imply distinct
type ownership that does not exist.

## Measured Current Cost

A scan of the post-#145 tree on 2026-07-17 measured the following review
surface:

| Option | Rust churn estimate | Resulting guardrail |
| --- | --- | --- |
| A: explicit flat re-exports | One core file; replace 11 globs with a list covering 169 public declarations; no consumer import edits | Reviewable root, but every future version still expands it |
| B: version-owned modules | 105 version-owned public symbols occur in 54 other Rust files, representing 545 distinct symbol/file references, plus the core facade | Version ownership is visible at every import and future versions cannot enter the root accidentally |
| C: document current globs | No Rust files; documentation only | No new guardrail against collisions or accidental public exports |

The Option B count includes 23 desktop files, 16 storage files, six report
files, four RBN files, two WSJT-X files, and three core tests or helpers. It is
a bounded mechanical rewrite now; delaying it makes every later schema family
increase the same migration surface.

## Decision

Choose Option B, with a curated root for shared APIs:

- Public version-owned schema APIs live under `antennabench_core::v2`,
  `antennabench_core::v3`, and `antennabench_core::v5`.
- Existing Rust item names and their `V2`, `V3`, or `V5` suffixes remain
  unchanged. The suffix keeps cross-version migration code explicit even after
  importing from a version module.
- Version-owned items are not also re-exported from the crate root. Consumers
  name the owner module in imports.
- Shared unversioned domain, validation, normalization, alignment, and WSPR
  operations remain available at the crate root through explicit curated
  re-exports rather than globs.
- Schema version constants remain at the root because dispatch must compare
  multiple versions together.
- There is no `latest` alias. Durable readers, writers, fixtures, and upgrades
  must state the version family they mean.
- A numeric schema revision that reuses an existing Rust representation, as v4
  reuses v3, is documented as belonging to that owner module. It does not get
  an empty facade merely to mirror the number.

For a future schema v6, every newly owned public item must be defined or
curated under `v6`, retain an explicit `V6` suffix, and stay out of the crate
root. If v6 changes validation or authoring policy without introducing a new
Rust representation, its decision must instead name the existing owner module
and the exact version gates. Either case requires bundle-reference
documentation and unchanged older-version compatibility tests.

## Consequences

- Imports become longer but immediately disclose schema ownership.
- The workspace-wide migration in
  [#146](https://github.com/rwjblue/antennabench/issues/146) is mechanical and
  intentionally follows the active feature work named by that issue.
- Removing a flat Rust re-export is an API change for Rust consumers, but this
  pre-1.0 workspace migration does not alter serialized data or runtime
  behavior.
- Explicit root exports make accidental API growth visible in review.
- v4 remains a first-class durable version with documented validation
  semantics even though it has no distinct Rust type family.

## Alternatives Rejected

### Curated flat suffixed names

This removes wildcard collisions at the lowest migration cost, but the crate
root still grows for every schema family and imports still hide ownership.

### Document the glob-exported status quo

This avoids immediate churn but leaves public-surface changes implicit and
makes the eventual migration more expensive.

### Unsuffixed names or a `latest` module

Unsuffixed aliases would add a second naming system and make code that handles
multiple bundle generations harder to audit. A moving `latest` alias is
especially unsuitable for durable storage and migration code.
