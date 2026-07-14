# 0004: Paired Descriptive Analysis Precedes Conclusions

Date: 2026-07-13

## Decision

AntennaBench will add paired, renderer-neutral descriptive comparison data
before it adds uncertainty intervals or automated antenna conclusions.

The first comparison slice may show matched SNR differences, reporter/path
overlap, unmatched observations, time order, distance, azimuth, and data-quality
diagnostics. It must not emit a winner, statistical significance, directional
evidence, practical equivalence, or "too close to call." Those claims remain
deferred until the product has a validated experimental-design and inference
contract.

Existing `EvidenceQuality` values remain descriptive coverage labels only.
They are based on usable-observation and contributing-slot counts; they are not
evidence that one antenna differs from another. Follow-up work should use
"evidence coverage" in operator-facing prose so that distinction is visible.

## Context

The current analysis validates a bundle, aligns observations to schedule slots,
and classifies them as usable or excluded. It reports counts and exclusion
reasons plus min/median/mean/max SNR by session, antenna, band, and slot.
Per-antenna coverage is `weak` at two usable observations across two slots and
`moderate` at five usable observations across three slots; session coverage is
the least coverage among scheduled antennas. The report model projects those
facts and three chart-ready datasets without comparative effect estimates or
generated conclusions.

That behavior is useful, but raw antenna-level SNR summaries are not a valid A/B
effect estimate:

- the same reporter or received station can recur, so observations are not
  independent;
- transmit-path reports and receive-path local decodes measure different
  directions of the local station and must not be pooled;
- bands, observation kinds, and record sources have different populations and
  failure modes;
- a switched schedule observes antennas at different times, so antenna, order,
  and changing propagation can be confounded;
- a missing WSPR report is not a zero-SNR measurement, and decode threshold,
  interference, receiver hopping, or source lag can make missingness
  informative; and
- the current synthetic fixtures intentionally exercise evidence and rendering,
  but do not contain repeated same-path observations suitable for a scientific
  A/B conclusion.

The NIST experimental-design guidance recommends blocking controlled nuisance
factors and randomizing remaining ones, and warns that process drift cannot be
separated from a predictor collected in the same time order. A directly
relevant WSPR antenna-comparison study instead used the same reporter in the
same time interval for both antennas; it also identified integer SNR
quantization, interference, decode failures, receiver distribution, and antenna
directivity as limitations. AntennaBench's current sequential switching model
does not satisfy that simultaneous-measurement assumption.

The ASA guidance also distinguishes effect size and scientific importance from
a thresholded p-value. AntennaBench therefore will not use a p-value or an
arbitrary sample count as a shortcut to a product claim.

## Options Considered

### Descriptive-only reports

This is the selected current product boundary. It can expose matched differences
and the limitations needed to interpret them without claiming that the observed
sample generalizes beyond the session.

### Conservative paired-effect conclusions

This remains the intended next methodological stage, but it is not approved for
implementation yet. It requires a prespecified estimand, experimental-design
gates, a practical-effect threshold, an uncertainty procedure that respects
dependence, and simulation evidence that the procedure behaves conservatively.

### Goal-specific evidence models

This is deferred. Session goals remain context and may choose views or filters,
but they must not silently change estimands, thresholds, or conclusion language.
Distance cutoffs for DX, regional, or NVIS/local operation and a detection model
for weak-signal reliability require their own justified contracts. A
single-antenna profiling goal never emits A/B conclusions.

## Paired Descriptive Contract

### Comparison shape

The first paired implementation supports a comparison-mode session with exactly
two scheduled antenna labels. Their deterministic display order establishes
only the sign convention for deltas; it does not declare a control, candidate,
or preferred antenna. Sessions with more comparison labels need an explicit,
predeclared contrast rather than automatic all-pairs comparisons.

The label that appears first in schedule order is display-left and the other is
display-right. An eligible comparison block consists of adjacent scheduled
slots on the same band with one eligible actual slot for each label. Each
uninterrupted same-band run is partitioned in sequence order into consecutive,
non-overlapping two-slot blocks. An invalid block is retained as a diagnostic;
the implementation does not slide the window or reuse a slot to manufacture a
replacement pair. The block records label order, timestamps, and elapsed time.
Adjacency reduces elapsed time but does not remove propagation or period
effects, so the result remains descriptive.

### Path identity and strata

A paired row requires the same remote path endpoint under both labels within one
eligible block:

- for a transmit-path observation, the local station is the heard station and
  the remote endpoint is `reporter_call`;
- for a receive-path observation, the local station is the reporter and the
  remote endpoint is `heard_call`; and
- observations whose direction or remote endpoint is ambiguous remain visible
  as evidence but are unavailable for pairing.

Rows are stratified by direction, band, signal mode, observation kind, and
record source. Signal mode is normalized only by trimming surrounding
whitespace and folding ASCII letters to uppercase; distinct mode names are not
aliased. Missing, blank, or control-character-bearing modes are counted and
remain available to non-comparison evidence, but are unavailable for pairing.
Rows are never pooled across those boundaries. Source normalization or
cross-source deduplication requires an explicit adapter contract; apparent
duplicates must not be counted as independent evidence. Exact duplicates may
collapse deterministically, while conflicting duplicates are reported and
excluded from paired summaries.

Both observations must already be usable under the existing alignment policy
and must contain finite SNR values. A row reports both SNR values and an
explicitly oriented `right minus left` difference in dB.

### Aggregation and repeated paths

Raw paired rows remain available for audit. Within a stratum, repeated blocks
from one remote path are summarized to one per-path median difference before
the overall median of remote paths is computed. This gives each observed remote
path equal weight in the headline descriptive center instead of allowing a
frequent reporter to dominate it.

The summary also reports paired-row, unique-path, and contributing-block counts;
the observed range; antenna-order counts; and unmatched left-only, right-only,
missing-SNR, ambiguous-path, duplicate, and excluded counts. It does not pool
strata into a single whole-session delta.

### Missing and imbalanced evidence

An observation seen under only one antenna is unmatched. It is never imputed as
zero, a decoder floor, or the weakest observed SNR. Unmatched and missing-SNR
counts remain visible by label and stratum because WSPR non-detection may depend
on signal strength, interference, receiver operation, and source behavior.

Order imbalance, unequal block coverage, repeated-path concentration, and
source imbalance are diagnostics, not correction factors. The first
implementation must expose them rather than manufacture balance through
weighting or imputation.

### Availability states and language

Comparison availability is separate from existing evidence coverage:

- `not_applicable`: the experiment mode is single-antenna profiling;
- `unsupported_comparison_shape`: the session does not have exactly two
  scheduled comparison labels;
- `no_eligible_blocks`: no adjacent same-band block has one usable actual slot
  for each label;
- `no_matched_paths`: eligible blocks exist but no same-stratum remote path has
  finite SNR under both labels; and
- `descriptive_pairs_available`: at least one paired row exists.

The last state authorizes display, not a conclusion. There is deliberately no
sample-count threshold that turns descriptive data into directional evidence.

Operator-facing text may say, for example, "the observed paired differences in
this stratum had a median of X dB across N remote paths." It must also say that
the result is descriptive and does not establish antenna superiority.
"Significant," "confident," "likely better," "winner," "equivalent," and "too
close to call" are prohibited until the deferred inference contract is approved.

## Deferred Inference Contract

Intervals, tests, and automated conclusions are explicitly deferred. A future
decision may approve them only after all of the following are specified and
validated:

- the target estimand and independent or clustered analysis unit;
- a simultaneous, randomized, or counterbalanced design whose intent and order
  are durably recorded;
- handling for reporter/path clustering, repeated time blocks, temporal drift,
  band and source strata, SNR quantization, and informative non-detection;
- a domain-justified smallest effect of practical interest, selected before
  looking at the session's result;
- minimum design and coverage gates derived from power or interval-coverage
  simulations rather than arbitrary raw-observation counts;
- a deterministic interval or resampling procedure with fixed test fixtures;
  and
- a policy for multiple contrasts and goal lenses selected before inspecting
  favorable results.

If such a contract is later approved, conclusion terms have these reserved
semantics:

- `insufficient_data`: required design or coverage gates fail, so no
  comparative conclusion is computed;
- `directional_evidence`: the entire validated uncertainty interval lies beyond
  the prespecified practical-effect bound in one direction;
- `practically_equivalent`: the entire validated interval lies inside the
  prespecified equivalence bounds;
- `too_close_to_call`: the interval includes practically meaningful effects in
  both directions, so precision is inadequate; this is not equivalence; and
- `inconclusive`: the interval satisfies none of the preceding conclusion
  regions.

"Winner" and unqualified "better antenna" remain prohibited. Any future
directional statement is scoped to the recorded station, band, direction,
remote-path population, goal lens, and session conditions.

## Chart Priorities

Future chart-ready data is prioritized as follows:

1. reporter/path overlap and unmatched-count views, plus a data-quality timeline
   showing slot status, order, exclusions, and missingness;
2. paired-difference distributions and SNR-over-time views, stratified by
   direction, band, observation kind, and source; and
3. distance and azimuth small multiples with an explicit missing-location
   category and tabular access to the same rows.

Every visualization requires an accessible table fallback. Goal-specific views
may filter these datasets only after their distance or reliability semantics are
defined; they do not alter the underlying facts.

## Test Expectations

Deterministic synthetic coverage must include:

- observed differences centered toward each label without calling either a
  winner;
- observed differences centered near zero without calling them equivalent or
  too close;
- no eligible blocks and no matched paths;
- a monotonic time trend with all blocks in the same A/B order;
- balanced A/B and B/A order;
- one prolific remote path plus several sparse paths;
- left-only, right-only, missing-SNR, ambiguous-direction, exact-duplicate, and
  conflicting-duplicate observations;
- band, direction, kind, and source separation; and
- single-antenna profiling with comparison marked not applicable.

Fixed-bundle tests must prove observation-order independence and must not turn
the existing canonical sample into comparative evidence merely because its
coverage label is `moderate`.

## Consequences

- The current runtime behavior and bundle schema do not change in this decision.
- Paired comparison facts belong in analysis and renderer-neutral report models;
  prose and visualization remain renderer responsibilities.
- Existing evidence coverage remains available but cannot gate comparative
  claims.
- The first implementation work is bounded to terminology, paired descriptive
  data, diagnostics, and rendering.
- Inferential methods, practical thresholds, and goal-specific conclusions need
  a later approved decision backed by simulations and suitable session-design
  metadata.

## Follow-up Work

- [#22](https://github.com/rwjblue/antennabench/issues/22) clarifies
  operator-facing evidence-coverage terminology without changing thresholds or
  serialized APIs.
- [#23](https://github.com/rwjblue/antennabench/issues/23) adds the
  renderer-neutral paired descriptive model and adversarial synthetic coverage.
- [#25](https://github.com/rwjblue/antennabench/issues/25) renders the initial
  overlap, quality-timeline, paired-difference, and SNR-over-time diagnostics.
- [#24](https://github.com/rwjblue/antennabench/issues/24) adds the
  lower-priority distance and azimuth views after the initial renderer.
- [#26](https://github.com/rwjblue/antennabench/issues/26) retains the deferred
  uncertainty and conclusion-policy decision; it is intentionally not
  agent-ready until paired data and suitable design evidence exist.

## References

- [NIST randomized block designs](https://www.itl.nist.gov/div898/handbook/pri/section3/pri332.htm)
- [NIST guidance on process drift and run order](https://www.itl.nist.gov/div898/handbook/pmd/section4/pmd443.htm)
- [NIST guidance on preselecting multiple comparisons](https://www.itl.nist.gov/div898/handbook/prc/section4/prc47.htm)
- [American Statistical Association statement on p-values](https://www.amstat.org/asa/files/pdfs/p-valuestatement.pdf)
- [WSJT-X user guide](https://wsjt.sourceforge.io/wsjtx-doc/wsjtx-main-2.5.4_en%20%28USLetter%29.pdf)
- [Zander, *Simple HF antenna efficiency comparisons using the WSPR system*](https://arxiv.org/abs/2209.08989)
- [Lakens, *Equivalence Tests: A Practical Primer*](https://doi.org/10.1177/1948550617697177)
