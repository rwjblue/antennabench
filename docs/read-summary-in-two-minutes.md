# Read An AntennaBench Summary In Two Minutes

Use the **Summary** for an ordinary first read or share. Open the
[successful canonical Summary](https://antennabench.com/sample-report/summary/)
beside this guide. It comes from a real, sanitized WSPR comparison; your own
Summary follows the same order.

## 1. Start With The Answer

At the top, find **What did the run show?** The first paragraph states which
questions this run can answer without combining them into one stronger claim.
Each card below is marked **Available** or **Unavailable**. In the sample,
**Paired shared-path signal** is available and reports a `+5 dB` median. Read
the sign sentence before interpreting that value: it names which antenna a
positive or negative difference favors.

This is a description of one recorded run. It is not a winner, antenna-gain
measurement, confidence statement, or promise that the result will repeat.

## 2. Keep The Population And Support Attached

Under a result, **Population** says which observations were eligible for that
question. Support counts say how much recorded evidence contributed. The
sample's shared-path result uses finite-SNR reports from the same remote path,
within the same eligible alternating block and comparison condition. Its
support is 83 unique shared paths, 327 paired observations, and 7 alternating
blocks.

Do not compare two numbers until their populations match. A large count from a
different band, direction, source, or evidence question does not strengthen
the displayed shared-path result.

## 3. Read The Three Evidence Questions Separately

- **Paired shared-path signal** compares signal only where both antennas have
  usable reports for the same remote path in a nearby alternating block.
- **Controlled common-opportunity detection** asks what happened among remote
  receivers known to be active during both cycles. The sample marks this
  unavailable because it has no suitable activity census.
- **Uncontrolled observed paths** counts unique paths that appeared for either
  antenna. These paths show collected reach, not a controlled detection rate
  or a map of everywhere an antenna can reach.

One question can be available while another is unavailable. Never turn an
unmatched or missing public report into a zero-strength signal. If a supported
activity census proves a receiver was active, its non-detection can be counted
as below-threshold evidence for the detection question; otherwise it remains
missing evidence.

## 4. Read The Principal Limitation

The answer panel places the most important limitation directly below the three
questions. Treat it as part of the result. Then open the short methods or exact-
condition disclosure only if you need to confirm scope. The
[inconclusive example](https://antennabench.com/sample-report/inconclusive/)
shows a valid run with observed paths but no same-path signal comparison.
Unavailable is an evidence outcome, not an application failure.

## 5. Know When To Go Deeper

Switch to **Full evidence** when you need exact path medians, block and order
support, activity coverage, distance and direction context, exclusions,
duplicates, conflicts, acquisition gaps, planned-versus-actual history,
provenance, or the audit appendix. Use the
[Full evidence and methodology reference](reading-your-report.md) for that
walkthrough.

Summary and Full evidence are two human-readable views of one committed
snapshot. Neither replaces the [session bundle](bundle-format.md), which is the
lossless durable experiment record and the right artifact to preserve when
someone may need to regenerate or independently inspect the complete evidence.
