# 0027: Use Predeclared Goal Lenses And One Distance Taxonomy

Date: 2026-07-20

## Status

Accepted.

## Context

The session plan durably records an operator goal before evidence collection,
but reports currently give that choice little presentational effect. At the
same time, matched-path context uses boundaries at 500, 1500, and 3000 km while
the active-receiver polar view uses 1000, 3000, and 8000 km. Adding more
geographic comparisons on top of those incompatible categories would make
apparently similar labels describe different populations.

A goal can help an operator find the prespecified evidence relevant to the
question they intended to ask. It must not become a post-hoc filter, change an
estimand or threshold, suppress an inconvenient result, or make distance stand
in for a measured propagation mode.

## Decision

### Distance Taxonomy

All report projections, tables, prose, accessible equivalents, and map legends
use these half-open great-circle-distance categories:

| Category | Exact boundary | Operator meaning |
| --- | --- | --- |
| Near / local proxy | 0 km to less than 500 km | Nearby observed paths and a prespecified local/NVIS-oriented proxy. It is not proof of NVIS. |
| Regional | 500 km to less than 1500 km | Paths within a broad regional operating scale. |
| Longer path | 1500 km to less than 3000 km | Longer observed paths below the DX-oriented category. “Longer” does not assert long-path propagation around the globe. |
| DX-oriented | 3000 km and above | A prespecified distant-contact category, not a universal DX award or band-independent propagation threshold. |

These boundaries preserve the report's original matched-path policy rather
than optimizing bins around the current fixtures. They are practical,
round-number amateur-radio presentation categories. Their interpretation
depends on band, station, time, and propagation; distance alone does not prove
ground wave, NVIS, skip count, short-path, or long-path propagation.

A map may use a square-root or other documented monotonic radial transform to
make near and distant evidence legible. Geometry does not create semantic
bins. Ring boundaries, labels, tooltips, and accessible tables must use the
four categories above, and exact distances remain available in audit detail.

### Goal Lenses

The immutable schedule goal selects one renderer-neutral presentation lens.
The lens is projected from Rust and is identical in full and compact reports.
It may reorder available question families, name one or more prespecified
distance categories for emphasis, choose bounded explanatory wording and
next-run advice, and choose which secondary disclosure begins expanded. It
does not mutate any analysis fact.

The fixed priority contract is:

| Predeclared goal | Primary priority | Prespecified emphasis and permitted wording |
| --- | --- | --- |
| General coverage | Shared-path signal; common-opportunity detection; observed reach; distance/direction; repeatability | “General coverage” may summarize overlap, distance, and bearing without a universal winner. |
| DX | Distance/direction; shared-path signal; common-opportunity detection; repeatability; observed reach | May say “DX-oriented” and emphasize the 3000 km-and-above category; may not call every such path DX or hide nearer evidence. |
| Regional | Distance/direction; common-opportunity detection; observed reach; repeatability; shared-path signal | May emphasize the near, regional, and longer-path categories and observed bearing distribution. |
| NVIS / local | Distance/direction; common-opportunity detection; repeatability; observed reach; shared-path signal | Must say “near / local proxy” or “NVIS-oriented proxy”; must state that distance does not establish NVIS propagation. |
| Weak-signal reliability | Common-opportunity detection; repeatability; shared-path signal; observed reach; distance/direction | May emphasize one-sided outcomes, repeated decode opportunities, and observed coverage edges; may not fabricate decoder-floor SNR. |
| Single-antenna profiling | Observed reach; distance/direction; repeatability | Uses footprint and repeatability language only. A/B, left/right, winner, and comparative detection or SNR claims are not applicable. |

Only available question families enter primary navigation. Every other
available family remains accessible after reordering, and every typed
limitation remains in the shared disclosure. A lens cannot pool strata, alter
matched-path or active-receiver populations, choose favorable bins after the
run, change conclusion rules, or conceal materially contrary evidence.

The report shows the recorded goal and a concise statement of what its lens
prioritizes near the summary. The schedule remains the durable source of the
choice: setup records it before the run, report generation reads it without
writing back to the bundle, and changing derived presentation requires no
session-bundle migration.

## Consequences

Matched-path context, active-receiver maps, and subsequent geographic analysis
share one testable category contract. Visual geometry may remain nonlinear,
but labels and populations can no longer drift.

Goal-specific reports become easier to scan without changing their evidence.
Tests must prove that lens changes affect only typed presentation metadata and
prespecified grouping, that contrary evidence remains reachable, and that
single-antenna profiling contains no comparative conclusions.

This decision adds no goal-specific statistical estimand, uncertainty method,
propagation-mode classifier, post-hoc favorable view, or universal antenna
winner.
