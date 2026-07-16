# Roadmap

Last reviewed: 2026-07-15

The roadmap describes product outcomes, not every implementation task. GitHub
Issues carry the detailed scope, dependencies, and acceptance criteria.

## Available Today

AntennaBench has a complete local manual/no-rig workflow for repeatable WSPR
sessions:

- review station, antenna, and intended cycle order before creating a bundle;
- conduct operator-paced cycles with one readiness action after each antenna
  change, actual WSPR timing, occupancy, notes, and corrections;
- recover an interrupted run without rewriting its evidence history;
- optionally collect bounded direct/local WSJT-X evidence and automatically
  gather default-on bidirectional WSPR.live public spots;
- import supported WSPR.live JSON and Reverse Beacon Network archives;
- inspect conservative descriptive reports; and
- export standalone HTML or a verified copy of the complete session bundle.

The repository also builds verified macOS release inputs for Apple silicon and
Intel, and contains an admission-disabled hosted foundation. Neither is yet a
public end-user service.

## Toward A Public Preview

The next outcome is a trustworthy macOS preview that people can install and use
in real antenna sessions. That includes:

- signing, notarization, publication, and release verification;
- maintainer field sessions and external operator feedback;
- clearer in-product guidance and report comprehension checks; and
- fixes discovered through real-world use without weakening the evidence
  model.

The manual workflow remains the baseline. Optional integrations should improve
collection without making a network service, rig, or account necessary.

## Later

Possible later tracks include:

- calibrated uncertainty and carefully validated comparative conclusions;
- optional rig observation or control;
- live or scheduled RBN acquisition if an appropriate filtered source exists;
- rebuildable local search and indexes;
- optional private-to-unlisted hosted report sharing;
- native WSPR or mobile-specific operation; and
- public discovery and callsign-oriented browsing.

“Winner” language remains out of scope until the project has a validated
experimental-design and inference contract. Hosted sharing remains optional and
never replaces the local session bundle as the source of truth.
