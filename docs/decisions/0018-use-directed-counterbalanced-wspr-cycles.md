# 0018: Use Directed Counterbalanced WSPR Cycles

## Status

Accepted.

## Decision

Every newly authored WSPR cycle intention records whether the operator is to
receive or transmit. This is schema v4. Schema-v3 bundles remain readable, but
their direction is unknown; AntennaBench does not guess a direction while
reading or upgrading historical evidence.

WSPR occupies a two-minute sequence and a conventional single-radio station
cannot receive its own WSPR decode while transmitting. A BOTH repetition with
two antennas therefore contains four directed periods: receive on each antenna
and transmit on each antenna. Setup defaults to BOTH and four repetitions,
which produces 16 periods and an ideal minimum of 32 minutes.

The order is counterbalanced while avoiding unnecessary physical changes. The
first BOTH repetition is `RX A, RX B, TX B, TX A`; the next is
`RX B, RX A, TX A, TX B`, and later repetitions alternate those orientations.
TX-only and RX-only sessions similarly alternate `A, B` and `B, A`, producing
`A, B, B, A` across two repetitions. This keeps each antenna equally represented
in early and late positions without forcing an antenna change at every period.

A schema-v4 session containing receive periods requires the local WSJT-X UDP
receiver to be running before the session can start. The receiver may be armed
while the session is still ready so the prerequisite does not lose the first
decode. WSPR.live collection remains available for sessions with transmit
periods and is unavailable for receive-only sessions.

The conductor treats direction changes as operator actions even when the
antenna stays connected. Before a receive period it tells the operator to turn
WSJT-X Enable Tx off and keep Monitor on. Before a transmit period it tells the
operator to set Tx Pct to 100 percent and turn Enable Tx on. Antenna changes and
WSJT-X changes are presented independently, because either, both, or neither
may be needed at a boundary.

Evidence admission follows the same direction boundary. Local WSJT-X decodes
can align only to receive intentions, while WSPR.live public reception reports
can align only to transmit intentions. Schema-v3 intentions with no recorded
direction retain their legacy behavior.

## Consequences

- Transmit and receive evidence cannot be accidentally attributed to the same
  two-minute period.
- A repetition means one observation in every selected direction on every
  participating antenna, rather than one undirected visit per antenna.
- Four default BOTH repetitions take 32 ideal minutes for two antennas, not 24.
- Reduced switch count does not sacrifice order balance; consecutive periods
  on one antenna occur at counterbalanced direction boundaries.
- Reports and setup review disclose each cycle's direction.
- Schema v4 validation rejects missing directions and direction sets that do
  not match the experiment mode.
