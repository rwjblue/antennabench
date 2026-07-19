# Why not just use the Reverse Beacon Network?

The operator-facing answer now lives on the AntennaBench website:

**[Why WSPR is the default—and where RBN wins](https://antennabench.com/why-wspr/)**

That page is the maintained public explanation for CW and RTTY operators. The
short answer is that AntennaBench does **not** treat the Reverse Beacon Network
(RBN) as inferior. WSPR is the default for a controlled, repeatable A/B
experiment; RBN is often the better tool for checking how the CW or RTTY signal
you actually operate is being copied.

This document retains the generated snapshot boundary, interpretation notes,
and regeneration instructions used by maintainers.

## Maintainer summary

- WSPR supplies a standardized weak-signal transmission and fixed two-minute
  cadence that make frequent, counterbalanced antenna changes practical.
- The measured WSPR receiver population and occupied four-character Maidenhead
  grid footprint were larger than the active RBN population on 40, 20, and 15
  meters.
- RBN directly measures real CW and RTTY operating performance and is especially
  useful for operational validation, beam-heading checks, and deliberately
  selected skimmer paths.
- A missing spot is not a zero in either network, and WSPR and RBN SNR values are
  not interchangeable.
- The strongest workflow is often WSPR for the controlled comparison, followed
  by RBN validation in the mode actually used on the air.

## Current bounded snapshot

The WSPR values below are distinct reporting callsigns observed during bounded
time windows. The RBN values are nodes online in one active-node snapshot and
advertising the selected band. These populations are useful for understanding
receiver opportunity, but they are not a calibrated sensitivity or availability
comparison.

<!-- BEGIN GENERATED RECEIVER SNAPSHOT -->
**Snapshot interval:** WSPR data from `2026-07-10T17:00:00Z` through `2026-07-17T17:00:00Z`; RBN active nodes fetched near the end of that interval.

| Band | WSPR calls, 24 h | WSPR calls, 72 h | WSPR calls, 7 d | RBN active nodes | 7-day WSPR / RBN |
| --- | ---: | ---: | ---: | ---: | ---: |
| 40m | 818 | 984 | 1,251 | 180 | 7.0× |
| 20m | 992 | 1,230 | 1,670 | 183 | 9.1× |
| 15m | 543 | 644 | 803 | 177 | 4.5× |

The all-HF WSPR queries found 1,466 distinct reporter calls in 24 hours, 1,825 in 72 hours, and 2,436 in seven days. The RBN endpoint returned 207 active nodes in its point-in-time snapshot.
<!-- END GENERATED RECEIVER SNAPSHOT -->

## Interpretation guardrails

The public page describes geographic footprint using occupied four-character
Maidenhead grid squares, not a percentage of the globe covered. Grid occupancy
reduces the effect of dense local receiver clusters, but it still does not
measure continuous geographic coverage.

Additional cautions:

- A callsign can move or report more than one locator. Maps therefore use
  distinct callsign/locator pairs while the headline table uses callsigns.
- Neither count measures receiver quality, antenna performance, local noise, or
  continuous availability.
- RBN duplicate suppression is part of the experiment design. For repeated CW
  tests, move at least 300 Hz between transmissions or wait ten minutes.
- Compare near-in-time A/B observations from the same receiver. Do not combine
  RBN and WSPR SNR values as though they came from one calibrated meter.

The checked-in supporting figures remain available under
[`docs/assets/why-not-rbn/`](assets/why-not-rbn/), including the receiver-count,
receiver-footprint, occupied-grid, and interactive band-explorer views.

## Sources and reproducibility

- [Public WSPR and RBN explanation](https://antennabench.com/why-wspr/)
- [AntennaBench product overview](product.md)
- [AntennaBench attribution and external-data policy](attribution.md)
- [RBN: How to get spotted](https://www.reversebeacon.net/pages/How%2Bto%2Bget%2Bspotted%2Bby%2Bthe%2BRBN%2B44)
- [RBN telnet services and mode streams](https://beta.reversebeacon.net/pages/Telnet%2Bservers%2B30)
- [WSJT-X user guide: WSPR](https://wsjt.sourceforge.io/wsjtx-main_en.html#WSPR)
- [WSPR.live database documentation](https://wspr.live/)

The exact snapshot rows, queries, current summary, map outline, and regeneration
script are installed under `tools/why-not-rbn/`. Run:

```sh
python3 tools/why-not-rbn/refresh_receiver_comparison.py --offline
```

to reproduce the charts from the included snapshot, or:

```sh
python3 tools/why-not-rbn/refresh_receiver_comparison.py --refresh
```

to make four bounded WSPR.live queries, fetch the RBN active-node list once, and
regenerate the snapshot, article table, static graphics, and interactive
explorer. The refresh path sleeps between WSPR.live requests and keeps every
query bounded by time and band.
