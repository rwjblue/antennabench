# 0016: Use Reusable Counterbalanced Transmit Signal Plans

Date: 2026-07-15

## Decision

Controlled non-WSPR transmit experiments use reusable typed signal-plan
definitions plus explicit per-slot allocations. AntennaBench does not put every
signal fact directly on every slot, and it does not leave the protocol in notes.

A signal plan owns the facts intended to remain stable across a comparable run:

- typed mode, initially CW or RTTY;
- planned transmitter power when known;
- the exact transmitted callsign or indicator, distinct from the station's
  base/licensed identity;
- deterministic message and cadence instructions, including repetition count,
  key speed when applicable, transmit duration, and interval; and
- a typed evidence-collection profile.

Each participating slot references one signal plan and records its exact
planned frequency plus a stable frequency-variant identity. Frequency belongs
on the slot because changing and counterbalancing it is part of the experiment,
not merely a reusable radio configuration.

The first collection profile is a pinned RBN CW profile. RBN's official
CW guidance says a node that already spotted a station can ignore it for ten
minutes unless the next transmission moves at least 300 Hz. Strict creation of
an RBN-profile plan therefore rejects potentially comparable transmissions
with the same exact transmitted identity and mode when they are less than ten
minutes apart and less than 300 Hz apart. The boundary is inclusive: a 300 Hz
move or a full ten-minute separation satisfies this particular collection
constraint. RTTY remains a typed signal and observation stratum, but AntennaBench
does not label an RTTY plan RBN-comparison-ready until equivalent provider
collection behavior is separately verified. This rule mitigates a documented
collection effect; it does not remove propagation, receiver, QRM, timing, or
other confounding.

Frequency variants and antenna order must be counterbalanced. For a two-antenna,
two-frequency comparison, complete blocks rotate both allocations so neither
antenna permanently owns one offset or one order position. Strict creation
rejects a plan whose comparison strata are structurally confounded. Runtime
misses, deviations, and incomplete blocks remain durable evidence and make the
affected comparison unavailable or explicitly incomplete rather than silently
reassigning observations.

## Planned, Confirmed, Observed, And Commanded Facts

These fact classes remain separate:

- `schedule.json` owns planned signal definitions and slot allocations.
- A correctable typed operator event owns manual confirmation of the actual
  frequency, mode, exact transmitted identity, power when known, and whether
  the planned cadence was followed.
- `rig.jsonl` owns optional adapter observations and readbacks. Its future
  signal fields are observations, not proof that transmission occurred.
- Public observations own the frequency, mode, heard identity, and SNR actually
  reported by the source. RBN's absence of transmitter power remains absence.
- Command, keying, watchdog, and abort records belong to the execution boundary
  decided by #32 and are not implied by a plan or a rig readback.

Manual/no-rig operation remains first class. Missing actual frequency, mode,
power, identity, or cadence confirmation does not block raw evidence import or
lossless export. It does block a protocol-adherence claim and any comparison
that requires the missing fact. Reports show planned, confirmed, rig-observed,
and public-observed mismatches without selecting a convenient source as truth.

## Callsign Identity

The station record retains the configured base/licensed callsign. A signal plan
retains the exact identity intended to be transmitted, and public-source
matching uses that exact identity. AntennaBench does not silently strip,
canonicalize, invent, or rotate indicators.

If the transmitted identity differs from the station identity, setup requires
the operator to affirm that the exact indicator is valid for the station,
location, and jurisdiction. That affirmation is provenance, not a legal ruling.
Recognized portable forms observed in archives do not justify arbitrary
per-segment suffixes. RBN's CW Skimmer also requires more repetitions for
unfamiliar call patterns, so identity changes are not treated as a reliable
duplicate-suppression workaround.

## WSPR And Provider Boundaries

Existing WSPR schedules do not acquire synthetic signal plans. WSJT-X remains
the WSPR timing and transmit engine, and current WSPR setup, conduction,
acquisition, and analysis continue unchanged. A future WSPR signal-plan profile
may reuse the general shape, but RBN collection rules are never applied to WSPR
or another provider implicitly.

The RBN archive adapter in #29 remains import-first and provider-specific. It
can preserve and normalize raw observations independently, but
comparison-readiness and plan-adherence reporting consume this typed experiment
contract. Evidence from WSPR, RBN CW, and RBN RTTY remains stratified.

## Schema And Migration

This contract requires schema version 3. Adding safety- and interpretation-
relevant fields as optional schema-v2 members would let an older conductor
silently ignore them and run a materially different experiment. A new schema
version makes unsupported readers fail closed.

The v3 implementation must retain v1/v2 compatibility reads and lossless copy.
Explicit upgrade from v1 or v2 creates no signal plan and invents no actual
signal facts; existing WSPR behavior remains the default interpretation.
Signal-plan sessions are created directly as v3. Downgrade is not supported.

## Validation And Reporting

The implementation validates at least:

- bounded unique plan and frequency-variant identities and valid references;
- supported typed modes, positive finite power, positive frequency, internally
  coherent cadence, and frequency/band agreement;
- the RBN CW 300 Hz / ten-minute boundary for the exact transmitted identity;
- balanced antenna, frequency-variant, and order allocations across complete
  comparison blocks;
- exact transmitted-identity matching and explicit affirmation for an
  indicator that differs from station identity; and
- planned-versus-confirmed-versus-rig-observed-versus-public-observed
  mismatches, missing actual facts, missed slots, and incomplete blocks.

Worldwide band-plan and callsign legality cannot be inferred from a generic
schema. Setup presents the exact plan and requires operator responsibility for
frequency, power, identity, and operating authority before conduction. That
affirmation never converts absent actual evidence into confirmation.

## Consequences

- The protocol is reproducible and machine-checkable without requiring a rig
  adapter.
- Reusable stable facts avoid slot duplication while per-slot frequency makes
  counterbalancing explicit.
- RBN's documented collection suppression cannot silently masquerade as an
  antenna result in a plan AntennaBench labels RBN-compatible.
- Schema v3 adds migration and fixture work, but prevents older software from
  silently dropping a safety-relevant experiment contract.
- Manual operation remains possible, with visible uncertainty when actual
  signal facts are not confirmed.
- #86 implements the schema and manual audit foundation. #32 decides execution
  and keying; #14 independently decides the first optional rig milestone.

## Alternatives Rejected

### Put all signal fields directly on each slot

This is mechanically simple but repeats stable identity, mode, power, and
cadence data and makes inconsistent edits easy. Frequency still needs explicit
per-slot allocation, so full denormalization provides little benefit.

### Keep the schedule unchanged and document the procedure

Notes cannot validate the suppression window, counterbalancing, identity,
planned-versus-actual mismatches, or protocol eligibility. They are inadequate
for a defensible comparison contract.

### Rotate callsign suffixes instead of frequency

RBN recommends frequency movement. Arbitrary indicators are not established as
valid, consistently recognized, or jurisdictionally permitted, and unfamiliar
patterns may require more repetitions. AntennaBench does not use identity
rotation as its cache strategy.

### Extend schema v2 with optional fields

Older v2 conductors could ignore those fields and still treat the schedule as
executable. A fail-closed version boundary is preferable for a contract that
affects transmission instructions and evidence interpretation.

## References

- [RBN: How to get spotted](https://beta.reversebeacon.net/pages/How%2Bto%2Bget%2Bspotted%2Bby%2Bthe%2BRBN%2B44)
- [RBN raw data](https://www.reversebeacon.net/raw_data/)
- [Decision issue #30](https://github.com/rwjblue/antennabench/issues/30)
- [Schema-v3 implementation #86](https://github.com/rwjblue/antennabench/issues/86)
- [RBN archive adapter #29](https://github.com/rwjblue/antennabench/issues/29)
- [Transmit execution decision #32](https://github.com/rwjblue/antennabench/issues/32)
- [Optional rig decision #14](https://github.com/rwjblue/antennabench/issues/14)
