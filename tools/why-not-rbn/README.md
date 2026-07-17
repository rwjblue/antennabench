# Receiver comparison refresh tool

This directory contains the data and script behind
`docs/why-not-just-use-rbn.md`.

## Rebuild without network access

```sh
python3 tools/why-not-rbn/refresh_receiver_comparison.py --offline
```

This uses the included aggregate snapshots and regenerates the article table,
SVG maps, optional PNG copies, and the interactive HTML explorer.

## Fetch a fresh snapshot

```sh
python3 tools/why-not-rbn/refresh_receiver_comparison.py --refresh
```

The refresh performs four bounded WSPR.live aggregate queries:

1. all supported HF bands over 24 hours;
2. all supported HF bands over 72 hours;
3. all supported HF bands over seven days; and
4. one combined 40/20/15-meter query containing conditional counts for all
   three windows.

It then fetches the RBN active-node endpoint once. The default six-second pause
between WSPR.live requests is intentionally conservative. Use `--end` to anchor
an exact UTC snapshot and `--cooldown` only when necessary.

## Definitions

- A **WSPR reporting call** is a distinct upper-cased `rx_sign` observed in the
  selected time and band window.
- A **WSPR call/grid pair** distinguishes the same callsign reporting from more
  than one locator.
- An **RBN node** is a usable call/grid record returned by the current detailed
  active-node endpoint.
- RBN per-band counts use the bands advertised in each returned node record.
- Locations are plotted at reported Maidenhead locator centers.

The two populations are not calibrated receiver networks. Use the figures to
understand likely path availability and geographic distribution, not to rank
individual receiver quality.
