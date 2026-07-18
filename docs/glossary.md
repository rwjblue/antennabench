# Operator Glossary

These are the canonical plain-language terms used by the AntennaBench app,
reports, and operator guides. Technical references may also name the internal
model term when precision requires it.

## Session

A session is one antenna experiment, from its reviewed plan through the recorded
run and any later report or export. Its history stays attached to one stable
session identity.

## Session Bundle

A session bundle is the portable directory that holds the durable record of a
session. After this full name is introduced, **bundle** is the preferred short
form; it is not the same as an HTML report derived from the bundle.

## Antenna Label

An antenna label is the recorded name for an antenna in one session, such as
“Vertical” or “Inverted V.” Operator text uses **antenna label** rather than the
ambiguous shorthand **label**, and reports show these names instead of internal
left/right positions.

## WSPR Cycle

A WSPR cycle is one timed WSPR opportunity at an eligible even-minute boundary.
**Cycle** is the canonical operator term for this opportunity; **period** may
describe elapsed time, while **slot** is reserved for a planned schedule window.

## Slot

A slot is a planned time window in a fixed schedule or an older session format.
Current operator-paced WSPR guidance uses **WSPR cycle** for the timed
opportunity, so **slot** is not its canonical operator-facing synonym.

## Block

A block is a back-to-back pair of cycles on the same band, one per antenna. A
block remains visible for audit even when it cannot be used in a comparison.

## Readiness

Readiness records that the named antenna is ready before AntennaBench selects a
WSPR cycle; it never backdates a cycle. The basis is either an operator
confirmation or a successful switch plus independent command verification.

## Lifecycle

Lifecycle is the recorded session state: draft, ready, running, interrupted,
ended, or abandoned. Start, interruption, resume, end, and abandon actions move
the session only through allowed states and remain in its history.

## Checkpoint

A checkpoint is a durably committed session revision. It lets recovery, reports,
and exports identify the exact saved evidence they use without treating an
unfinished write as committed.

## Spot

A spot is a reception report produced by a network receiver, such as a WSPR or
Reverse Beacon Network receiver. **Spot** is the canonical word for that network
fact; **report** alone is avoided when it could mean an AntennaBench HTML report.

## Local Decode

A local decode is a signal decoded by WSJT-X at the operator’s own station and
captured directly by AntennaBench. It remains distinct from evidence obtained
through a public service or later file import.

## Public Report

A public report is a spot retrieved from a configured public source, currently
the WSPR.live mirror for automatic WSPR collection. Collection is best effort,
and the mirror does not independently guarantee completeness.

## Imported Spot

An imported spot is a spot loaded from a supported file or archive rather than
collected during the live session. Its original source and import provenance
remain recorded.

## Observation

An observation is the canonical general word for a normalized evidence fact
that analysis can inspect. A spot, local decode, public report, or imported spot
names a more specific fact and should be used when that distinction matters.

## Evidence Kind

Evidence kind classifies an observation by what it represents: a local decode,
public report, or imported spot. It describes the fact’s meaning, not where the
record came from.

## Source

A source identifies where evidence came from, such as WSJT-X UDP, a WSJT-X log,
WSPRnet, or an imported file. AntennaBench keeps sources separate unless an
explicit adapter contract says they can be combined.

## Attribution

Attribution records whether a WSPR cycle is pending, skipped, tied to a known
actual antenna, or left unknown. A completed transmission is attributable only
when recorded antenna occupancy covers its whole interval.

## Alignment

Alignment matches an observation to the applicable planned timing, band, and
recorded antenna state. Boundary timing, missing actual state, or conflicting
facts can prevent trustworthy alignment.

## Exclusion

An exclusion keeps an observation visible but leaves it out of a calculation
whose rules it does not satisfy. Excluded evidence is not silently deleted or
converted into a zero.

## Eligible Block

An eligible block has two same-band cycles with unambiguous recorded actual
antenna labels, one for each scheduled antenna in either order. An ineligible
block remains a diagnostic and its cycles are not rearranged to manufacture a
replacement pair.

## Comparison Group (Internally: Stratum)

A comparison group keeps observations separate by transmit/receive direction,
band, signal mode, evidence kind, and source. Results from different comparison
groups are never combined into one overall difference.

## Remote Path

A remote path is one observed remote endpoint, identified by the relevant
callsign for the transmit or receive direction. It groups that endpoint within
one comparison group and makes no claim about unobserved coverage.

## Matched Pair (Internally: Paired Row)

A matched pair contains usable signal observations for the same remote path in
one eligible block and comparison group, one observation per antenna. Repeated
matched pairs from one remote path are summarized before paths are summarized
together.

## Comparison Availability

Comparison availability says whether an A/B display applies and whether eligible
blocks and matched pairs exist. It is a recorded state—not a score, confidence
claim, or conclusion about which antenna is better.

## Evidence Coverage

Evidence coverage describes the amount of usable observations and contributing
cycle evidence in the session. It is not an antenna-quality grade and does not
show that one antenna differs from another.

## SNR

SNR is the reported signal-to-noise ratio in decibels (dB). A missing SNR is not
zero, and SNR values from different networks are not interchangeable calibrated
measurements.

## Difference (Also Shown As Delta)

A difference is the signed SNR change between the two antenna labels in a
matched pair or its summary. The report states which antenna label a positive
or negative value favors; **delta** is the shorter table and chart term, not a
separate concept.
