# 0006: Capture Rich Typed Propagation Evidence

Date: 2026-07-13

## Decision

AntennaBench will expand propagation context by capturing broad, bounded source
evidence while normalizing only values with explicit and stable semantics.

The expansion will use an additive typed propagation-evidence stream in a
future bundle revision. It will not replace the existing `PropagationRecord`
in one migration. Existing version 1 records remain readable, and the narrow
F10.7 and provisional estimated Kp adapter selected in
[Decision 0005](0005-use-a-narrow-noaa-propagation-snapshot.md) remains the
core profile and initial implementation boundary.

Expanded acquisition must not populate the current generic sunspot, A-index,
solar-wind, Bz, alert, or daylight fields merely because a source exposes a
similarly named value. Those fields do not express the source, unit, interval,
location, coordinate frame, quality, status, or revision semantics needed to
make richer evidence unambiguous.

## Typed Evidence Contract

Each extended evidence item will distinguish at least:

- provider, product, endpoint, and upstream source identity;
- evidence class: observation, operational estimate, model output, forecast,
  or operational notification;
- capture time and the source observation time or interval;
- forecast or notification validity interval when applicable;
- spacecraft, station, spatial location, or modeled boundary when applicable;
- metric name, numeric value, unit, energy band, and coordinate frame as
  applicable;
- provisional, final, corrected, or otherwise source-defined status;
- source quality flags, sample counts, gaps, and active-sensor selection;
- adapter/parser version and HTTP retrieval metadata;
- a stable upstream identity or payload digest; and
- the selected near-raw source payload, either inline or through a
  content-addressed attachment reference with media type and encoding.

Source corrections and later final values append new evidence. They do not
overwrite the payload captured during a session. A later item may identify the
item it revises or supersedes, allowing analysis to choose either the evidence
known during the session or the best subsequently finalized value.

Complete rolling endpoint responses are not durable bundle evidence. Adapters
select and append unseen source rows, preserving enough near-raw content to
reparse them without making an upstream rolling-window format a core bundle
invariant. Large spatial products use compressed, content-addressed
attachments so identical payloads can be deduplicated.

## Capture Profiles

### Core

The core profile remains the Decision 0005 snapshot:

- observed F10.7 solar flux; and
- provisional NOAA estimated planetary Kp.

It remains optional and offline-safe. Issue #20 may implement it without
waiting for the typed evidence schema.

### Extended Observational

The first expansion profile will retain the following source evidence:

- the SWPC operational estimated daily sunspot number, explicitly labeled as
  an estimate rather than the official international sunspot number;
- NOAA end-of-period planetary Kp, its three-hour equivalent-amplitude value,
  and contributing station count;
- active real-time solar-wind plasma observations, including speed, density,
  temperature, spacecraft identity, sample counts, and quality flags;
- active interplanetary magnetic-field observations, retaining Bt, vector
  components, GSE or GSM coordinate frame, spacecraft identity, and quality;
- ballistically propagated solar-wind values as separate model/arrival
  evidence, with both source and propagated times;
- both GOES X-ray passbands with satellite and correction metadata;
- GOES integral proton flux thresholds with their energy bands and satellite;
  and
- observed/current NOAA R, S, and G scale state, kept distinct from future
  probabilities or forecasts in the same upstream product.

The NOAA field named `a_running` must not be normalized into the current
generic `a_index`. It represents the equivalent amplitude associated with a
three-hour Kp interval, while A and Ap are daily aggregates with different
semantics.

L1 solar-wind measurements are upstream spacecraft observations rather than
contemporaneous conditions at a ground station. Propagated solar wind is a
model input with an estimated boundary-arrival time. These are separate
evidence classes and must never be merged into a scalar record with a shared
observation time.

### HF Impact And Operational Notifications

A separately selectable profile will capture:

- D-Region Absorption Prediction data as compressed, timestamped spatial
  attachments, including the highest affected frequency and model status; and
- NOAA alerts, watches, warnings, continuations, cancellations, and summaries
  as typed operational notifications with their original message retained.

D-RAP is an empirical model driven by GOES X-ray and proton observations. It
is useful HF context, but it is not an independent ground observation. Alert
text is also not a durable normalized lifecycle model by itself; product
identity, issue time, validity, message type, and revision relationship remain
explicit.

### Forecast

Forecast capture is a separate, initially disabled profile. Forecast evidence
always retains its issue time, valid interval, and forecast status. It is never
substituted for a missing observation or presented as conditions measured
during the session.

### Experimental Ionosphere

GloTEC capture is an explicit experimental opt-in. Its TEC, anomaly, hmF2,
NmF2, and spatial quality evidence may prove useful for exploratory analysis,
but GloTEC is a data-assimilative model primarily intended for
trans-ionospheric and GNSS use. Its WSPR interpretation is less direct, and its
grids are materially larger than the other selected evidence.

### Finalization And Reconciliation

A later post-session reconciliation step may append official GFZ Kp, ap, and
Ap values. GFZ distinguishes nowcast from monthly definitive values, and the
typed evidence status must preserve that distinction. GFZ attribution and its
CC BY 4.0 license accompany captured values.

The official WDC-SILSO sunspot series is deferred from automated bundle
capture. It provides useful provisional/final status, uncertainty, and
observation counts, but its CC BY-NC 4.0 terms require a separate product and
redistribution decision. Until then, the extended live profile may retain only
the clearly labeled SWPC operational estimate.

Station-specific K/A indices, Dst, OVATION output, broad edited-event catalogs,
and solar radio event tables are also deferred. They may be reconsidered after
the selected profiles have analysis consumers or when they add information not
already represented by the higher-priority evidence.

## Polling And Size Policy

Acquisition remains bounded by an active AntennaBench session. Adapters honor
source cadence, use response compression and conditional requests when useful,
append only unseen or changed evidence, back off on failure, and never perform
catch-up request bursts after an offline period.

Rolling one-minute and five-minute feeds should normally be fetched in batches
no more often than every ten minutes, with every unseen source sample retained.
Products with slower source cadences are fetched at session start, on their
natural cadence during long sessions, and once best-effort at session end.
D-RAP may be sampled every five minutes. GloTEC, when explicitly enabled, may
be sampled every ten minutes. Exact freshness thresholds and retry behavior
remain source-specific adapter contracts.

Disposable samples measured on 2026-07-13 and 2026-07-14 bound a representative
two-hour session before typed-envelope overhead as follows:

| Evidence | Approximate selected payload size |
| --- | ---: |
| Active RTSW wind and magnetic rows at five-minute cadence | 25 KB |
| Propagated solar-wind rows at five-minute cadence | 2.5 KB |
| Both GOES X-ray channels at one-minute cadence | 51 KB |
| GOES proton thresholds at five-minute cadence | 19 KB |
| D-RAP at five-minute cadence | 1.0 MB raw / 51 KB compressed |
| GloTEC at ten-minute cadence | 30 MB raw / 3.3 MB compressed |

The expected extended observational bundle budget is approximately 0.2 to
0.4 MB for a two-hour session. D-RAP adds approximately 0.05 MB when compressed.
GloTEC is the clear outlier and therefore remains opt-in. At the measured
endpoint sizes, ten-minute compressed retrieval of the rolling RTSW and GOES
feeds transfers approximately 3 MB during a two-hour session; storing their
complete rolling responses would be substantially larger and is prohibited by
this decision.

These figures are planning bounds rather than durable format limits. Adapters
and tests should measure representative fixtures and expose unexpected growth.

## Daylight, Twilight, And Path Context

Solar elevation, daylight, twilight, and gray-line context are deterministic
derivations from time and location, not source-adapter observations. They stay
outside the propagation evidence stream.

Analysis and reports should derive numeric solar elevation for the session
station and, when coordinates are available, the reporter or other path
endpoint at the observation time. Categorical daylight and twilight labels are
derived from documented elevation thresholds. Path-level summaries must state
which endpoints or sampled path points they describe; a free-form value such
as `mixed_path` is not sufficient.

If derived values are persisted for reproducibility, they belong in versioned
analysis metadata with the algorithm and inputs identified. Missing coordinates
produce missing path context rather than an inferred default.

The implemented derived boundary uses the NOAA GML fractional-year equations
as `noaa-gml-fractional-year` version 1 and geometric elevation without an
atmospheric-refraction correction. Explicit 4-, 6-, and 8-character Maidenhead
locators are converted to their cell centers by
`maidenhead-cell-center-v1`; absent and malformed locators stay separately
typed missing results. Daylight begins at 0 degrees elevation, civil twilight
at -6 degrees, nautical twilight at -12 degrees, astronomical twilight at -18
degrees, and night below -18 degrees. Gray line denotes any of those three
twilight categories. Analysis/report rows retain the exact locator, derived
coordinates, UTC time, endpoint identity, and algorithm identifiers without
adding any record to the bundle evidence streams.

## Report And Analysis Boundary

Propagation evidence remains attributed session context. Reports may present
source age, status, quality, missingness, and evidence class, but must not use
it to adjust antenna scores, select a winner, or claim a causal explanation for
observed WSPR differences. Exploratory analysis may associate evidence with
observation times while preserving that limitation.

Network acquisition is always optional. A session can start, run, analyze,
export, and report without any propagation source being available.

## Alternatives Considered

### Add Normalized Fields Only

Adding a field only when a current report consumes it minimizes schema and
storage work. It was rejected because it discards source evidence that may be
valuable for future reanalysis and makes today's normalization choices the
limit of tomorrow's analysis.

### Store Broad Raw Blobs In Existing Records

Placing additional responses only in `PropagationRecord::raw` avoids a schema
revision. It was rejected because important interval, status, quality,
location, and revision semantics would remain implicit and difficult to query
or validate.

### Replace The Flat Record Immediately

A complete typed-schema replacement offers clear semantics but requires a
big-bang migration before the extended sources have consumers. It was rejected
in favor of an additive stream that preserves version 1 compatibility and
allows consumers to migrate deliberately.

### Capture Every Available Space-Weather Product

Archiving every NOAA or third-party product would maximize volume, not useful
evidence. It was rejected because AntennaBench sessions are not a general
space-weather archive. The selected profiles prioritize plausible HF relevance,
bounded storage, explicit semantics, and future reanalysis.

## Consequences

- Issue #20 and the version 1 core adapter remain narrowly scoped.
- Extended acquisition requires a typed bundle-schema addition before its
  adapters are implemented.
- Rich selected evidence is retained without promoting ambiguous generic
  fields to authoritative core concepts.
- Observations, upstream measurements, model outputs, forecasts, and
  operational messages remain distinguishable.
- Append-only revision evidence supports both as-captured and subsequently
  finalized analysis.
- Content-addressed compressed attachments bound spatial-product growth and
  preserve lossless bundle copying.
- Offline behavior and deterministic fixture-driven tests remain mandatory.
- GloTEC and non-NOAA data introduce explicit profile, attribution, and license
  decisions rather than hidden dependencies.
- Reports gain better context without changing the descriptive-analysis and
  no-causal-claim boundaries.

## References

- [Decision issue #21](https://github.com/rwjblue/antennabench/issues/21)
- [Core propagation implementation #20](https://github.com/rwjblue/antennabench/issues/20)
- [SWPC data access](https://www.spaceweather.gov/content/data-access)
- [SWPC solar-wind observations](https://www.spaceweather.gov/products/solar-wind)
- [SWPC GOES X-ray flux](https://www.spaceweather.gov/products/goes-x-ray-flux)
- [SWPC GOES proton flux](https://www.spaceweather.gov/products/goes-proton-flux)
- [SWPC D-RAP documentation](https://www.spaceweather.gov/content/global-d-region-absorption-prediction-documentation)
- [SWPC GloTEC](https://www.spaceweather.gov/products/glotec)
- [GFZ Kp data and status](https://kp.gfz.de/en/data)
- [WDC-SILSO daily sunspot data](https://www.sidc.be/SILSO/infosndtot)
- [NWS disclaimer and appropriate-use guidance](https://www.weather.gov/index.php/disclaimer)
