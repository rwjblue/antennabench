# 0005: Use A Narrow NOAA Propagation Snapshot

Date: 2026-07-13

## Decision

The first live propagation adapter will acquire a narrow observational snapshot
from the NOAA/NWS Space Weather Prediction Center (SWPC). It will normalize only
two values:

- observed 10.7 cm solar radio flux (F10.7) from
  `https://services.swpc.noaa.gov/products/summary/10cm-flux.json`; and
- the provisional estimated planetary Kp value from
  `https://services.swpc.noaa.gov/json/planetary_k_index_1m.json`.

F10.7 is stored in `PropagationRecord::solar_flux_f107` in solar flux units.
The upstream `estimated_kp` value, not the integer `kp_index` field, is stored in
`PropagationRecord::kp_index` and must be presented as a provisional NOAA
estimate of the three-hour planetary Kp index.

Each selected upstream observation becomes its own sparse
`PropagationRecord`. Values from different products must not be combined into a
record with a misleading shared observation time. The other optional
propagation fields remain absent in this first adapter.

Implementation should begin with captured response fixtures and pure parsers.
Live acquisition remains optional session context and must never be required to
start, run, analyze, export, or report a local session.

## Timestamp And Raw-Input Contract

- `RecordMeta::timestamp` is the UTC instant at which AntennaBench received the
  source response.
- `PropagationRecord::observed_at` is the selected upstream `time_tag`.
- SWPC `time_tag` values from the selected endpoints are interpreted as UTC.
  Their literal representation is also retained in `raw`.
- `raw` preserves a source envelope containing the provider, exact endpoint,
  selected near-raw JSON object, retrieval time, and useful HTTP metadata such
  as `ETag`, `Last-Modified`, or `Date` when present.
- Missing, null, malformed, or out-of-range source values are not replaced with
  defaults. The adapter reports the problem and does not emit a normalized
  value for that item.

This contract keeps the source payload available for auditing and later
reanalysis without making the complete rolling response format a bundle
invariant.

## Polling, Freshness, And Failure Policy

- Fetch both products at session start and make one best-effort final fetch at
  session end.
- While a session is active, poll Kp no more often than every five minutes.
- Refresh F10.7 no more often than every six hours during an unusually long
  session.
- Append a record only when the selected source observation changes. Repeated
  responses must not create duplicate bundle records.
- Treat Kp as stale when its source observation is more than ten minutes old.
- Treat F10.7 as stale when its source observation is more than 36 hours old.
- Network, HTTP, parse, and stale-data failures do not stop the session. The
  consumer retains prior records, exposes missing or stale status, and does not
  silently substitute a forecast or cached value as a current observation.
- Automatic retries use backoff and never retry more often than once per
  minute. Polling should honor conditional-request metadata when useful and
  avoid catch-up request bursts after an offline period.

Reports may show these values only as attributed session context with source
age and provisional/stale status. Propagation data does not adjust evidence,
scoring, conclusions, or winner language.

## Attribution And Use

User-visible source attribution should identify NOAA/NWS SWPC. F10.7
attribution should also preserve NOAA's statement that the measurements are
provided by the National Research Council Canada in partnership with Natural
Resources Canada.

NWS information is generally public domain unless otherwise noted. AntennaBench
must not imply NOAA/NWS endorsement or present transformed data as official
government material. Consumers must pay attention to source time and accept
that timely internet delivery is not guaranteed.

## Context

The bundle model already has a `NoaaSwpc` record source, separate capture and
observation timestamps, optional F10.7 and Kp fields, and a raw JSON value. The
narrow selection therefore fits schema version 1 and preserves the bundle as
the durable evidence source.

The wider model also has fields for sunspot number, A index, solar-wind speed,
Bz, alerts, and daylight state. Those fields do not by themselves define the
scientific or source semantics needed for safe live normalization:

- three-hour equivalent-amplitude indices and daily A/Ap indices are distinct;
- real-time solar-wind measurements are taken upstream of Earth, may switch
  spacecraft, can have interruptions or quality caveats, and require careful
  timing interpretation;
- watches, warnings, alerts, continuations, cancellations, and forecasts have
  issue and validity semantics that cannot safely be reduced to strings; and
- daylight and twilight are deterministic functions of time and location, not
  NOAA source observations. Path state is also observation-specific rather
  than a single global session value.

These additional sources and fields remain desirable evidence candidates. A
separate decision will inventory them broadly and define staged additions once
their provenance, cadence, quality, timestamp, storage, and report semantics
are explicit.

## Alternatives Considered

### Broad Propagation Context Initially

Capturing many indices, solar-wind values, alerts, forecasts, and daylight state
would maximize immediate data volume. It was rejected for the first slice
because the current model would blur different cadences, locations, validity
intervals, and provisional/final states before their meanings are established.

### Manual Or Import Only

An import-only boundary would simplify deterministic testing and avoid a
network lifecycle. It was rejected as the product boundary because it postpones
the authoritative source and timestamp semantics this decision needs to settle.
Captured fixtures and pure parsing are retained as the implementation sequence.

## Consequences

- The first adapter has two authoritative endpoints and no bundle schema
  change.
- Offline use remains complete; propagation context is additive.
- Tests can be deterministic and network-independent.
- Initial reports receive useful solar-activity and geomagnetic context without
  implying causal adjustment.
- Additional potentially useful data is deliberately investigated in a
  separate decision rather than forgotten or forced prematurely into ambiguous
  fields.

## References

- [Decision issue #6](https://github.com/rwjblue/antennabench/issues/6)
- [SWPC data access](https://www.spaceweather.gov/content/data-access)
- [F10.7 cm radio emissions](https://www.spaceweather.gov/phenomena/f107-cm-radio-emissions)
- [Planetary K-index](https://www.spaceweather.gov/products/planetary-k-index)
- [Station K and A indices](https://www.spaceweather.gov/products/station-k-and-indices)
- [Solar-wind observations](https://www.spaceweather.gov/products/solar-wind)
- [Notifications timeline](https://www.spaceweather.gov/products/notifications-timeline)
- [NWS disclaimer and appropriate-use guidance](https://www.weather.gov/index.php/disclaimer)
- [Bundle source-of-truth decision](0001-bundle-is-source-of-truth.md)
