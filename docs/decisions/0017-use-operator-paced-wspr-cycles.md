# 0017: Use Operator-Paced WSPR Cycles

## Status

Accepted.

## Decision

New WSPR sessions persist an ordered list of cycle intentions, not setup-time
timestamps, durations, or switch guards. Starting or resuming a session leaves
the next intention unarmed. The operator begins the antenna switch and confirms
readiness when the physical change is complete; only then does AntennaBench arm
the first eligible WSPR boundary using its own clock.

WSPR uses two-minute UTC sequences. Its 162 symbols are keyed at
12000/8192 baud, so the on-air message lasts 110.592 seconds. Nominal WSPR
transmissions begin one second into an even UTC minute. AntennaBench therefore
chooses `hh:mm:01` where `mm` is even and the boundary satisfies a small
readiness lead time. If the operator is slow, the next eligible boundary is
chosen. The roughly 9.4 seconds after an ideal transmission is protocol
remainder, not a switch deadline or promised manual buffer.

Sources: the K1JT protocol description gives the 162-symbol calculation and
110.6-second duration in [MAP65: A Panoramic,
Polarization-Matching Receiver](https://wsjt.sourceforge.io/EME_Florence_2008.pdf),
and the WSPR 2.0 specification records the nominal one-second offset into even
UTC minutes in its [user guide](https://www.9h1cl.com/docs/WSPR_2.0_User.pdf).
Current WSJT-X documentation continues to describe WSPR as a two-minute
sequence mode in the [WSJT-X user
guide](https://wsjt.sourceforge.io/wsjtx-main_en.html#WSPR).

Each readiness action starts a half-open antenna-occupancy interval and closes
the prior interval at the recorded ready time. New routine operation does not
record a separate switch-start time or switch duration. Interruption, recovery
interruption, end, or abandon also closes the current occupancy.

Historical schema-v3 `AntennaSwitchStarted` events remain readable and keep
their original conservative effect of closing occupancy at their recorded
time. A spot may be assigned to a cycle only when one interval covers the
complete 110.592-second transmission window. A historical switch exactly at
the transmission end is safe; an earlier historical switch makes attribution
unknown. Reports retain every intention and show actual readiness, timing,
antenna use, and attribution separately.

These semantics are schema v3. The storage dispatcher remains version-aware so
future schema versions can coexist with older readers and migrations, but new
product behavior is not constrained by preserving unused v1/v2 authoring
semantics.

## Consequences

- Setup asks for experiment order and repetition, not a clock prediction.
- Start is immediate and visible; it does not claim that a transmission began.
- Routine antenna changes persist one readiness action, not a switch-start time
  or measured switch duration.
- Manual switching can take arbitrarily long without silently missing a
  predefined slot.
- Historical switch-start evidence remains compatible and excludes only an
  affected early-switched cycle from antenna attribution.
- WSJT-X and WSPR.live use actual, fully occupied cycles rather than planned
  labels or setup timestamps.
- Public-spot finalization waits for the final intended cycle, not merely the
  most recently observed cycle.
