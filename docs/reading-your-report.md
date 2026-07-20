# How To Read Your AntennaBench Report

An AntennaBench report describes what one recorded antenna experiment observed.
It keeps missing evidence, comparison limits, and run problems visible instead
of turning them into a cleaner-looking result.

Open the [canonical sample report](https://antennabench.com/sample-report/) in
another tab if you want a concrete report to follow. The sample is intentionally
useful as an inconclusive example: both antennas produced evidence, but it has
no [matched pairs](glossary.md#matched-pair-internally-paired-row), so it does
not manufacture an A/B difference.

This guide follows the full evidence report from top to bottom. The shorter
compact summary uses the same result facts but omits most audit detail.

## Start With The Reading Panel And Headline

The **How to read this report** panel states five rules that apply everywhere:

- A missing public report is missing evidence, not a zero-strength signal,
  unless a band-qualified activity census proves that reporter was active for
  that cycle.
- The report is descriptive. It does not select a winner or prove that one
  antenna is better.
- Each [comparison group](glossary.md#comparison-group-internally-stratum)
  stays separate by direction, band, mode,
  [evidence kind](glossary.md#evidence-kind), and
  [source](glossary.md#source).
- A [block](glossary.md#block) is a back-to-back pair of
  [WSPR cycles](glossary.md#wspr-cycle), one per antenna.
- Alternating antennas reduces time and propagation effects but cannot remove
  them.

Read the question-family headline and plain-language answer under **What did
the run show?** next. The headline names the questions with usable evidence;
the adjacent disclosure records a typed availability or limitation for every
question family. These are not extra statistical conclusions. The answer may
also suggest an experiment-quality next step, such as completing more
repetitions or concentrating on one band.

The facts immediately below identify the [session](glossary.md#session),
station, goal,
[antenna labels](glossary.md#antenna-label), bands, direction, experiment mode,
and recorded session state. Check these before interpreting any number. A
correct calculation for the wrong setup is still the wrong answer to your
question.

### Availability Is Per Question

One run can answer some questions and not others. The report therefore keeps
five conditional question families separate:

- **Shared-path signal** uses finite-SNR reports for the same remote path in
  both cycles of an eligible block.
- **Detection among receivers active in both cycles** uses the common,
  band-qualified active-receiver population as its denominator. Each
  receiver-block opportunity is classified as heard both, heard only A, heard
  only B, or heard neither.
- **Observed reach** counts unique usable paths that appeared for each antenna.
- **Observed distance and direction profile** uses every unique usable path
  decoded by each antenna, including paths that were not decoded by the other
  antenna. It does not claim a radiation pattern or unobserved coverage.
- **Repeatability across blocks** asks whether eligible opportunities repeat;
  it does not turn repetition into a confidence or significance claim.

An unavailable family is summarized once in the headline disclosure instead
of receiving a full empty primary panel. Full and compact reports consume the
same typed availability facts, and primary navigation contains only question
families with usable evidence. Shared-path signal remains the first comparison
result whenever it is available.

### Detection And Signal Are Different Estimands

The detection family answers `detection outcome | receiver active during both
cycles`. Its four outcome counts partition the receiver-block opportunities
exactly. A receiver active in both censuses but missing a session decode is
below-threshold evidence, not missing evidence, and AntennaBench does not
invent an SNR for it. A receiver absent from either census is excluded.

The headline shows unique active receivers alongside receiver-block
opportunities. The same receiver is counted once in the unique total but can
contribute one opportunity in each eligible block. Those repeated
opportunities are descriptive and are not treated as independent samples.
Results remain separated by exact direction, band, mode, observation kind,
and source, with block order and census coverage visible.

The shared-path signal family answers `SNR difference | both antennas
decoded`. It therefore uses a narrower population and may be unavailable even
when one-sided detection outcomes are useful. Live activity census evidence is
limited to transmit-side WSPR; receive-direction and non-WSPR activity remain
explicitly unsupported. See
[Decision 0026](decisions/0026-condition-detection-on-common-active-receivers.md)
for the full methodology contract.

The geographic and detection views answer three different conditional
questions. Keep their denominators attached when comparing them:

- `distance/bearing | antenna decoded` is the all-usable-path profile. Each
  remote path contributes once per antenna and stratum; repeated observations,
  blocks, and slots are retained as support rather than extra unique paths.
- `distance/bearing | both antennas decoded` is shared-path context. It is the
  narrower population where same-path signal differences can be described.
- `detection outcome | receiver active during both cycles` is the common-
  opportunity receiver population described above.

Receiver or transmitter availability may change between antenna periods, so
the first view describes collected paths rather than a controlled detection
comparison. Missing or conflicting location facts remain counted as location
unavailable. A usable decode with missing SNR still belongs in the all-path
profile, while finite-SNR summaries use only the finite values.

### Legacy Shared-Path Comparison States

The original [comparison availability](glossary.md#comparison-availability)
field remains the compatibility view for shared-path finite-SNR analysis. It
uses one of five states:

- **Not applicable** means this is a single-antenna profiling session. There is
  no A/B comparison to compute.
- **Unsupported comparison shape** means the session did not have exactly two
  scheduled antenna labels. AntennaBench does not invent pairwise contrasts
  among a different number of antennas.
- **No eligible blocks** means no back-to-back, same-band
  [eligible block](glossary.md#eligible-block) contained one usable actual
  cycle for each antenna. More completed repetitions may help, but the current
  run cannot support a matched comparison.
- **No matched paths** means eligible blocks exist, but no
  [remote path](glossary.md#remote-path) had usable finite
  [SNR](glossary.md#snr) [observations](glossary.md#observation) for both
  antennas inside the same comparison group. The canonical sample has this
  state.
- **Descriptive pairs available** means at least one usable
  [matched pair](glossary.md#matched-pair-internally-paired-row) exists. It
  authorizes the descriptive tables and charts, not a claim that one antenna is
  generally superior.

“Insufficient data” is therefore a valid result, not a software failure. It can
mean that the design did not produce eligible blocks, that observed stations did
not overlap, or that [evidence coverage](glossary.md#evidence-coverage) was too
limited for a display. Keeping those cases separate tells you what happened and
what a future run could improve.

For a concrete **No matched paths** example, suppose the 20 m public-report
group contains two Vertical-only paths, no shared paths, and two Inverted
V-only paths. The answer-first paragraph reports those three reach counts for
that group before saying that no same-path signal delta can be computed. It
does not turn a missing report into zero signal, compare the unmatched path
populations, or combine those counts with 40 m, imported, or receive-path
groups. If the run has no usable observations at all, the paragraph instead
says that no usable reach evidence is available.

## Read The Per-Group Result Table

The **Descriptive result by comparison group** table is the quickest structured
view of the comparison. Each row keeps one transmit/receive direction, band,
mode, evidence kind, and source separate. Do not combine rows mentally into one
whole-session difference; their path populations and collection behavior can
be different.

For a group with matched evidence, the table shows:

- the observed range of per-path
  [differences](glossary.md#difference-also-shown-as-delta) and the median
  across paths;
- the number of unique paths and supporting matched pairs;
- the number of contributing blocks; and
- coverage as an availability fact, not a confidence grade.

The **Signed values** sentence defines the direction for every difference in
the report. If it says positive values favor **Inverted V**, then `+2 dB` means
Inverted V had the stronger observed SNR for that displayed comparison. A
negative value favors the other named antenna.

Groups without a path difference are collapsed into one clearly named row.
They remain part of the report, but they do not repeat a full empty table and
chart. **Not available** is not `0 dB`.

The **Supported by this run** and **Not established by this run** lists are the
boundary around the headline. Read both. Counts of unmatched paths, missing
SNR, duplicate evidence, acquisition gaps, order imbalance, or
[exclusions](glossary.md#exclusion) explain limits; they are not correction
factors applied behind the scenes.

## Same-Path Signal

This section compares signal reports only when the same remote path appears for
both antennas in an eligible block and comparison group. That pairing avoids
comparing unrelated stations as though they measured the same path.

When the chart is available:

- each blue dot is one unique remote path’s median difference across its
  matched pairs;
- the purple diamond is the median across those per-path medians; and
- the vertical zero line marks no signed difference for the displayed value.

Summarizing each path before summarizing across paths prevents one frequently
reporting station from dominating the center. The table below the visual is the
accessible equivalent and contains the same path values and pair counts.

Differences use SNR in decibels. The report shows the sign
orientation beside the result. For scale, a 3 dB change is the same signal-power
change as doubling transmit power. That scale helps read the number; it does not
turn an observed difference into proof of antenna gain or superiority. WSPR SNR
is reported in whole decibels and can vary from cycle to cycle.

A dot or diamond on zero means the displayed median is zero. It does not prove
the antennas are equivalent. An empty same-path section means no usable
same-path value exists, not that the value was zero.

Open **Review same-path signal detail** when you need the underlying block,
order, missing-SNR, duplicate, conflict, or exclusion counts and the matched-pair
audit tables.

## Hearing Rate Among Active Reporters

This section answers a different question from matched-pair SNR: among stations
proven by the census to be decoding this band during this transmit cycle, what
share reported the session callsign? A row such as `43 / 180 (23.9%)` means 180
stations were present in the band-qualified activity census and 43 of those
stations reported the session callsign. The other 137 are durable
below-threshold evidence for that cycle. A station absent from the census is
still no evidence at all.

Per-cycle rows remain separate by direction, band, mode, evidence kind, and
source. Paired rows use only reporters active in both cycles of one eligible
block, so changing receiver availability cannot silently change the paired
denominator. These rates sit beside matched-pair SNR; they do not replace it,
pool groups, impute a missing census, or establish a winner.

Read the coverage column with every rate:

- **Complete band-qualified census** supports the displayed denominator for
  that cycle.
- **Partial census** means malformed rows may have reduced the denominator.
- **Truncated census** means the capture limit may have reduced the
  denominator. Every affected cycle and paired rate repeats this caveat.
- **Coverage unknown** means no supported band-qualified census covers the
  cycle. It is not zero activity and no hearing rate is invented. Older
  bandless census rows remain in this state rather than being inferred or
  migrated.

The current receiver-activity census conditions transmit-direction public
reports. It does not prove which remote transmitters were active during a
receive-direction cycle, so receive-direction activity coverage stays unknown.

## Active-Receiver Coverage Map

The primary coverage view groups the common-opportunity detection population
by station-centered bearing and the same four distance categories used by the
all-path profile. Each receiver-block opportunity remains exactly one of four
outcomes:

- **First antenna only** means the common-active receiver decoded only the
  first cycle in the eligible block.
- **Both** means it decoded both antenna cycles.
- **Second antenna only** means it decoded only the second cycle.
- **Heard neither** means it was proven active on the band during both cycles
  but did not decode the session callsign in either one. This is
  below-threshold evidence, not an invented SNR value.

The station-centered view uses 8 bearing sectors and the four
square-root-scaled distance categories defined below. Its 32-row table is the
accessible numeric equivalent of every visual cell. Distance and azimuth
marginal tables retain unique common-active receivers, receiver-block
opportunities, all four outcomes, and per-antenna heard counts. Repeated
receivers count once in each unique-receiver cell and once per eligible block
in opportunity counts.

Retained Maidenhead locators place receivers using station-centered
great-circle distance and initial bearing. Receivers with missing, invalid, or
conflicting locators remain in the common-active and outcome totals and appear
in explicit location-unavailable counts; they are not silently discarded. If
the stratum lacks supported band-qualified census coverage, the report shows
explicit unavailability instead of displaying empty cells.

Color is not the only carrier of state: the legend, SVG titles, outcome counts,
detection rates, and visible numeric tables repeat the meaning. Concise
distance-bin findings are scoped to recorded common listening opportunities;
they do not select a universal winner or imply gain, radiation angle, NVIS,
DX superiority, statistical significance, or confidence.

The full report retains the older side-by-side per-cycle activity maps inside
an audit disclosure. Those panels use each cycle's own active-receiver
population, so they are context only and must not replace the comparative
common-opportunity view.

## Reach And Unique Paths

Reach counts unique observed remote paths in three categories: the first named
antenna only, both antennas, and the second named antenna only. It answers
“which paths appeared in the recorded evidence?” rather than “which antenna
covers more of the world?”

A path heard on only one antenna is unmatched evidence. It is never converted
to zero SNR or a decoder-floor value for the other antenna. Non-detection can
also reflect interference, receiver operation, propagation timing, source lag,
or a signal below the decode threshold.

The overlap counts remain separate for every comparison group. An empty group
is collapsed and named, just as it is in the result table.

## Coverage Overlap And Repeatability

The overlap and repeatability section extends the reach counts without turning
them into a diversity or antenna-quality score. **Observed complementarity**
counts paths that appeared only on the first antenna, on both, or only on the
second antenna, plus the total unique two-antenna system reach. A path that
appeared only on one antenna is an incremental recorded contribution; it does
not prove the other antenna could never reach that endpoint.

**Opportunity-conditioned complementarity** is a separate, stronger detection
comparison. It counts first-only, both, second-only, and neither outcomes only
among receivers proven active during both cycles. Those receiver-block
opportunities are never merged with uncontrolled observed-only paths, even
when the endpoint names happen to match.

For each antenna, repeatability counts unique endpoints once and separately
shows how many path-block observations support them. The block-count
distribution distinguishes paths seen once from paths seen in multiple
eligible blocks. Raw observation counts remain in the audit table so one
prolific endpoint cannot dominate the unique-path summary. Per-path and
per-receiver rows also retain first-then-second versus second-then-first order
support.

Zero or one eligible block is labeled as limited repeatability rather than a
negative or reliable result. Repetition is descriptive support, not a
confidence interval, independence claim, or probability that a path will be
heard again.

## Distance And Azimuth

Distance and azimuth views group located paths into one fixed taxonomy and
45-degree compass sectors. The categories are near / local proxy (under 500
km), regional (500 km to less than 1500 km), longer path (1500 km to less than
3000 km), and DX-oriented (3000 km and above). Each remote path contributes
once to a category and sector; a separate count shows how many observations or
matched pairs support it. Missing or inconsistent location stays visible
instead of being silently discarded.

The near / local category is an NVIS-oriented proxy only when that goal was
chosen before the run. It does not establish NVIS propagation. Likewise,
DX-oriented is a stable report category, not a claim that every path meets a
particular award, band, or propagation definition. “Longer path” describes
distance and does not assert long-path-around-the-globe propagation.

Polar maps use square-root radial geometry so nearby evidence remains visible,
but their rings, labels, tooltips, and accessible tables use these same four
semantic categories. The transform changes drawing geometry, not membership.

The primary side-by-side tables show `distance/bearing | antenna decoded` for
all usable paths. Their per-bin composition separates paths seen by the first
antenna only, both antennas, and the second antenna only. The shared-path
disclosure separately shows `distance/bearing | both antennas decoded` and the
available same-path SNR differences. Neither view is an antenna radiation
pattern, a propagation model, or evidence about directions and distances that
the session did not observe. A concentration of paths in one sector limits how
broadly the display can be read.

The detailed disclosures retain exact paired-row location values and derived
solar context. Solar elevation and light state are geometric context derived
from recorded time and locator cells; they do not establish why an observed
difference occurred.

## Run Quality, Timeline, And Exclusions

**Run quality and answerability** restates per-question availability and the
legacy shared-path state alongside matched-pair and block counts. It is not a
score for the operator or an antenna-quality grade. Open the per-group
diagnostics to inspect antenna order, unmatched paths, missing SNR, exclusions,
duplicates, and conflicts.

**Planned versus actual** shows what the schedule requested beside what the
recorded run supports. Each cycle or older-format
[slot](glossary.md#slot) can show:

- planned and actual antenna state;
- timing and block eligibility;
- [readiness](glossary.md#readiness) and
  [attribution](glossary.md#attribution);
- usable and excluded observation counts; and
- notes, interruptions, and corrections.

Use this timeline to spot missed cycles, late switches, unknown occupancy, or
an order pattern that may be confused with changing propagation. Corrections
remain visible with the original history.

**Acquisition status** keeps three facts separate: whether the configured
workflow completed, how many explicit acquisition gaps were recorded, and
whether provider completeness is known, unknown, or unsupported. A completed
best-effort public collection retained everything returned for its requested
windows; it does not by itself prove that an upstream mirror was complete.
Interrupted, failed, skipped, or partially committed collection remains an
incomplete workflow even when no durable gap count is available.

The **Exclusion summary** groups observations by the reason they were left out
of a calculation. An exclusion does not erase the record: exact excluded
observations remain available for review, and unrelated valid evidence remains
usable.

## Audit Appendix

The audit appendix holds supporting detail that most readers do not need on a
first pass:

- the committed [checkpoint](glossary.md#checkpoint),
  [lifecycle](glossary.md#lifecycle), acquisition records, and controller
  attempts;
- reporter-activity summary and census-row record IDs behind conditioned rates;
- station, antenna, and planned schedule detail; and
- comparison blocks and the detailed data-quality timeline.

Disclosures are closed by default and stay closed in default print output. Open
only the detail needed to check a result or explain a limitation. If controller
command details were explicitly omitted during full-report export, the appendix
says so; the lossless bundle still retains them.

## Compact Summary, Full Report, Or Bundle?

All three outputs serve different purposes:

- The **compact summary HTML** keeps the reading panel, headline, result table,
  same-path and reach summaries, and a short run-quality reference. It omits
  detailed unmatched, missing-SNR, exclusion, duplicate, conflict, timeline,
  controller-output, import, solar, and raw-observation audit rows. It does not
  sample or recompute the retained facts.
- The **full evidence HTML report** is the most detailed human-readable result
  and audit presentation for one committed revision. Controller details may be
  included or explicitly omitted at export. Operational history is a separate
  export choice: it is omitted by default and, when explicitly included, adds
  only the bounded redacted support view outside the scientific findings. If a resource boundary requires a
  bounded overview, the report names the omitted detail instead of sampling it.
- The [session bundle](glossary.md#session-bundle) is the lossless durable
  record. Reports are derived from it and can be regenerated. Keep or share the
  bundle when another person needs the complete evidence rather than only a
  standalone presentation.

The in-app **Build and operational history** panel is not part of the embedded
scientific report. Its build SHA/version/channel, platform, failure/partial/
recovery outcomes, evidence effects, and retry guidance exist to support the
software and must not be read as antenna-performance conclusions. Compact/public
HTML never carries these operational details.

## What This Report Will Never Tell You Yet

The current report will not name a winner, claim statistical significance or
confidence, declare practical equivalence, say “too close to call,” or make an
unqualified claim that one antenna is better. It also will not turn observed
paths into a coverage map or say that time, solar state, or propagation caused a
difference.

Those are deliberately deferred decisions, not missing labels. They require a
validated experiment design, a preselected practical-effect threshold, explicit
handling of repeated paths and informative missingness, and simulation evidence
for an uncertainty method. Until that work is approved, the report stays scoped
to the station, antennas, bands, directions, sources, paths, and conditions that
the session actually recorded.
