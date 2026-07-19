# 0018: Use Directed Counterbalanced WSPR Cycles

## Status

Accepted.

Amended 2026-07-16 by #106.

## Decision

Every newly authored WSPR cycle intention records whether the operator is to
receive or transmit. This is schema v4. Schema-v3 bundles remain readable, but
their direction is unknown; AntennaBench does not guess a direction while
reading or upgrading historical evidence.

WSPR occupies a two-minute sequence and a conventional single-radio station
cannot receive its own WSPR decode while transmitting. A BOTH repetition with
two antennas therefore contains four directed periods: receive on each antenna
and transmit on each antenna. Setup defaults to BOTH and four repetitions,
which produces 16 periods and about 32 minutes of required cycle time.

The order is counterbalanced while avoiding unnecessary physical changes. The
first BOTH repetition is `RX A, RX B, TX B, TX A`; the next is
`RX B, RX A, TX A, TX B`, and later repetitions alternate those orientations.
TX-only and RX-only sessions similarly alternate `A, B` and `B, A`, producing
`A, B, B, A` across two repetitions. This keeps each antenna equally represented
in early and late positions without forcing an antenna change at every period.

A schema-v4 session containing receive periods requires at least one selected
receive source before it can start. Default-on WSPR.live satisfies that
preflight for TX-only, RX-only, BOTH, and single-antenna WSPR sessions. If
WSPR.live is disabled, the local WSJT-X UDP receiver must be running before a
receive-capable session starts so the first decode is not lost. UDP may also run
beside WSPR.live as separately attributed direct/local evidence.

The conductor treats direction changes as operator actions even when the
antenna stays connected. Before a receive period it tells the operator to turn
WSJT-X Enable Tx off and keep Monitor on. Before a transmit period it tells the
operator to set Tx Pct to 100 percent and turn Enable Tx on. Antenna changes and
WSJT-X changes are presented independently, because either, both, or neither
may be needed at a boundary.

Evidence admission follows the same direction boundary. Local WSJT-X decodes
can align only to receive intentions. WSPR.live rows where the station is
`rx_sign` align only to receive intentions, and rows where it is `tx_sign`
align only to transmit intentions; ambiguous self-role rows are filtered.
Schema-v3 intentions with no recorded direction retain their legacy behavior.

## Consequences

- Transmit and receive evidence cannot be accidentally attributed to the same
  two-minute period.
- A repetition means one observation in every selected direction on every
  participating antenna, rather than one undirected visit per antenna.
- Four default BOTH repetitions require about 32 minutes of WSPR cycle time for
  two antennas, not 24, plus antenna changes and boundary waits.
- Reduced switch count does not sacrifice order balance; consecutive periods
  on one antenna occur at counterbalanced direction boundaries.
- Reports and setup review disclose each cycle's direction.
- Schema v4 validation rejects missing directions and direction sets that do
  not match the experiment mode.
