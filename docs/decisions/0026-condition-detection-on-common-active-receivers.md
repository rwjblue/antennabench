# 0026: Condition Detection On Common Active Receivers

Date: 2026-07-20

## Status

Accepted.

## Context

A receiver can be active during both antenna cycles yet decode the session
callsign during only one cycle or neither cycle. Conversely, a finite-SNR
comparison exists only when both antennas were decoded for the same path in
one eligible block. Treating these as one estimand discards useful
below-threshold evidence and makes a run with one-sided detection look
unanswerable.

Per-cycle hearing rates already used the live, band-qualified receiver census,
but they did not retain the joint outcome for each receiver. Pooling those
rates across blocks would also count a receiver repeatedly without saying so,
and pooling across direction, band, mode, observation kind, or source would
change the population being described.

## Decision

For every eligible transmit-side WSPR A/B block with band-qualified activity
coverage, AntennaBench intersects the two cycle censuses by stable receiver
identity. Each receiver active in both cycles contributes exactly one of four
receiver-block outcomes:

- heard both antennas;
- heard only the left/A antenna;
- heard only the right/B antenna; or
- heard neither antenna.

A receiver absent from either cycle census is excluded. An active-in-both
receiver with no session report in a cycle is retained as below-threshold
evidence for that cycle; no SNR value is fabricated. Receiver locator evidence
is retained when the two census records agree or only one has a locator, and
is left unknown when they conflict.

The report's deterministic headline is computed separately for each exact
comparison stratum. It shows both unique receivers and receiver-block
opportunities. The four outcome counts and per-antenna detection rates use
receiver-block opportunities as their denominator; unique receivers count
each identity once within the stratum. Eligible block count, known-coverage
block count, antenna order balance, and per-block rows remain visible. A
receiver appearing in multiple blocks therefore contributes multiple
opportunities, which are explicitly descriptive and not independent samples.

This detection estimand is:

> detection outcome | receiver active during both cycles

The existing matched-signal estimand remains separate:

> SNR difference | both antennas decoded

Live receiver activity is transmit-side WSPR evidence. Receive-direction rows
and non-WSPR rows remain explicitly unsupported rather than being assigned a
zero denominator. Complete, partial, truncated, and unknown census coverage
remain visible and are never silently pooled.

## Consequences

A run can now retain useful left-only, right-only, or neither evidence even
when it has no finite-SNR matched path. The joint partition is auditable to the
receiver and block, and aggregation no longer hides repeated receiver
opportunities.

The result remains descriptive. This decision adds no inferential winner,
decoder-threshold model, independence claim, geography estimand, or synthetic
SNR floor.
