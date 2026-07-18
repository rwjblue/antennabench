# Roadmap

Last reviewed: 2026-07-18

This page summarizes product outcomes. GitHub Issues contain the implementation
scope, dependencies, and acceptance criteria for individual changes.

## Available Now

AntennaBench has a complete local, manual workflow for repeatable WSPR sessions:

- review the station, antennas, direction, and cycle order before creating a
  session;
- conduct operator-paced cycles with actual WSPR timing, notes, missed or bad
  cycles, and corrections;
- recover an interrupted run without rewriting its evidence history;
- optionally collect local WSJT-X evidence and delayed WSPR.live public spots;
- import supported WSPR.live JSON and Reverse Beacon Network archives;
- inspect conservative descriptive reports; and
- export standalone HTML or a verified copy of the complete session bundle.

The repository can also build verified macOS release inputs for Apple silicon and
Intel. There is not yet a signed public download. `antennabench.com` is the
public information site and canonical sample; it does not offer accounts,
uploads, or report publishing. The separate hosted-sharing foundation remains a
non-public, admission-disabled prototype and is not part of the current product.

## Next: A Trustworthy macOS Preview

The next product outcome is an installable preview that operators can use in real
antenna sessions. That work includes:

- signing, notarization, publication, and independent release verification;
- report-comprehension review, maintainer field sessions, and external operator
  feedback under the [reporter-directed privacy policy](field-testing.md);
- clearer in-product guidance and report-comprehension checks; and
- fixes discovered through real use without weakening the evidence model.

Manual operation remains the baseline. Optional integrations should improve data
collection without making an account, network service, rig, or controller a
requirement.

## Later Possibilities

Later work may include calibrated uncertainty and carefully validated
comparative conclusions, broader rig observation or control, live or scheduled
RBN acquisition, cross-session search, optional report sharing, native WSPR
operation, and mobile-specific workflows. These are possibilities, not active
implementation tracks.

Automatic “winner” language remains out of scope until the project has a
validated experiment-design and inference contract plus enough real-session
evidence to justify it.

Rich propagation capture, built-in non-WSPR keying, and the previously designed
hosted account/publishing service are not planned. Their prior issues and ADRs
remain research history. Hosted sharing will be reconsidered only after the
signed external beta identifies a repeated problem that local standalone HTML
cannot solve; any later experiment will remain optional and will not replace the
local session bundle.
