# 0015: Use WSPR.live With Import-First Evidence And Opt-In Acquisition

Date: 2026-07-14

Amended: 2026-07-15

## Decision

AntennaBench selects WSPR.live's documented, read-only ClickHouse HTTPS
interface as its first automatic source of transmit-path WSPR public reports.
The deterministic `FORMAT JSON` importer shipped first in
[#84](https://github.com/rwjblue/antennabench/issues/84), establishing exact
response preservation, filtering, normalization, replay, atomic persistence,
and reporting without network availability. The automatic acquisition in
[#85](https://github.com/rwjblue/antennabench/issues/85) reuses that complete
evidence boundary; it does not introduce a second parser or normalization path.

WSPR.live permits use for personal research and projects whose results remain
freely accessible and prohibits commercial or profit-oriented use. The
project's intended use is local, noncommercial amateur-radio research:
AntennaBench is freely available, does not gate or sell WSPR.live data or
derived results, and identifies and attributes the service. The free-results
condition prevents turning the volunteer data service into a gated product; it
does not require an operator to publish a private local research artifact.

Apache-2.0 permits downstream commercial use of AntennaBench, but that does not
make this project's use of WSPR.live commercial or transfer permission to use a
third-party service outside its terms. Downstream users remain responsible for
their own service use. Voluntary project donations do not sell WSPR.live access
or results and do not change this determination. Revisit the decision before
the project sells access or derived results, operates the workflow for profit,
materially changes redistribution, or if WSPR.live changes its terms.

Separate written permission is not a prerequisite for this intended use.
Contacting the WSPR.live operator remains worthwhile coordination: explain the
bounded client, confirm that its traffic pattern is welcome, and invite any
preferred attribution or operational guidance. That outreach is not a release
gate.

Automatic acquisition is opt-in. A session does not query WSPR.live until the
operator enables public-spot acquisition after seeing the source attribution,
usage terms, requested callsign/time/band data, and unknown-completeness
warning. Once enabled, ordinary antenna confirmations authorize and trigger
the bounded acquisitions; the operator does not repeat consent or click a
separate fetch action for each segment.

WSPR.live documents that its scraper checks WSPRnet every few minutes, while
its exporter describes real-time rows as delayed by a few minutes. It also
runs a daily reconciliation for missed or late reports. There is no published
maximum ingestion latency or completeness watermark. Automatic acquisition
therefore uses the operator's existing antenna confirmation as its authority
and trigger, without a second fetch/import action:

- confirming the antenna after a completed segment enqueues one acquisition;
- the acquisition waits a fixed five-minute ingestion grace period;
- each query overlaps cumulatively from the schedule start through the latest
  confirmed completed segment, so later fetches can recover late prior rows;
- due acquisitions are coalesced to the latest eligible segment and provider
  IDs make repeated rows deterministic duplicates;
- only one request may be in flight, with at least ten seconds between
  requests and no automatic transport retry; and
- completing the final segment enters a finalizing presentation state, runs
  the final delayed cumulative acquisition, then ends normally without a
  separate spot-fetch click. A transport failure remains visible and offers
  explicit retry or end-without-spots recovery.

Five minutes is a conservative product grace period, not a source guarantee.
Reports continue to label WSPR.live completeness unknown. Manual JSON import
remains available as an offline and recovery escape hatch rather than the
normal connected workflow.

AntennaBench will not scrape WSPRnet's HTML database pages. WSPRnet is the
authoritative upload destination used by WSJT-X, but its anonymous public
surfaces do not currently provide a documented, bounded application API that
meets this product's reliability and usage-policy requirements. The JSON route
observed during this decision returned `403 Access denied`; the supported web
interfaces are human-oriented recent-result pages and very large monthly
archives. Direct WSPRnet acquisition may be reconsidered if its administrators
document and grant an appropriate API contract.

Network access remains optional. Sessions are complete local workflows when
automatic acquisition is disabled, offline, or unavailable. Failure or
omission of public reports never invalidates local evidence or prevents an
explicit end-without-spots recovery.

## What WSJT-X Uploads

Official WSJT-X 3.0.2 does not upload spots merely because WSPR mode is active.
Its persisted `UploadSpots` setting defaults to `false`. When the operator
enables **Upload spots** and the transceiver is online, WSJT-X collects decoded
reception reports, waits a random delay of up to 20 seconds after decoding, and
form-posts each queued report to `http://wsprnet.org/post/`.

The payload identifies the receiving station and grid and carries the decoded
transmitter call/grid, receive and transmit frequencies, UTC date/time, SNR,
time offset, drift, reported power, mode code, and WSJT-X version. These are
reports of stations heard by that WSJT-X instance. Remote WSJT-X instances
perform the same upload when their operators enable it; their rows are the
public transmit evidence AntennaBench wants when the session station appears as
the heard transmitter.

The upload endpoint is not a retrieval API. AntennaBench cannot learn who heard
the local station from its existing loopback WSJT-X UDP receiver or
`ALL_WSPR.TXT`, because those inputs contain only the local station's decodes.

## Current Source Assessment

### WSPRnet First

WSPRnet is the original destination and authoritative publisher. Its current
database form supports transmitter, reporter, band, mode, recent-time, count,
and ordering filters, while the old database exposes a maximum of 10,000 rows.
Monthly CSV archives provide historical source rows.

It is not selected for the first client because:

- the documented WSJT-X endpoint is upload-only and still uses plain HTTP;
- the public query results are HTML intended for interactive use;
- the anonymous JSON route is access-controlled rather than a supported public
  contract;
- no current official rate, cache, stability, attribution, or automated-query
  policy was found;
- the old interface's CSV/XML output reports that it is disabled; and
- current monthly compressed archives can be multiple GiB, arrive after the
  session, and can exceed AntennaBench's fixed local attachment budget.

HTML scraping would couple the adapter to presentation markup and impose load
without a documented automation contract. Whole-month archive import would not
provide a practical first local-session workflow.

### WSPR.live First

WSPR.live mirrors WSPRnet spots and exposes a documented, read-only ClickHouse
HTTPS interface. It supports exact transmitter, reporter, band, mode, and
bounded UTC queries, returns structured JSON or CSV, retains the WSPRnet spot
ID, and documents its schema. The database is optimized for time-and-band
filters. Its scraper normally imports new WSPRnet rows within a few minutes and
later reconciles missed or delayed rows. The service currently limits clients
to 20 requests per minute and may change limits to protect shared capacity.

Disposable ten-minute checks on 2026-07-15 returned 4,767 WSPR-2 rows on 40 m
and 9,301 on 20 m. The newest rows were respectively 83 and 203 seconds behind
the database clock. This confirmed practical near-real-time availability for
the selected query shape; it is operational evidence, not a source guarantee.

This is the selected automatic source. Its published research use matches the
intended local workflow, and its application examples expressly support
read-only clients. Availability and correctness are not guaranteed, so the
integration is optional, bounded, attributed, and explicit about partial
evidence rather than treating the service as a session dependency.

### Import-First Evidence Boundary

Import-first is selected because it separates three responsibilities:

1. a deterministic adapter parses and normalizes exact response bytes whether
   they came from a selected file or the bounded HTTPS client;
2. durable evidence behavior is testable without a network service; and
3. network orchestration remains replaceable and cannot reinterpret source
   rows differently from offline recovery.

The first format is WSPR.live's documented ClickHouse `FORMAT JSON` response,
not an AntennaBench-invented spot interchange format. This keeps the file useful
with existing WSPR.live query/export tools and gives file import and HTTPS
acquisition exactly the same response shape.

## Shared Response Contract

The shared acquisition operation accepts one complete JSON response plus an
explicit typed query scope. It records:

- normalized session transmitter callsign;
- half-open UTC source window `[window_start, window_end)`;
- selected WSPR bands and WSPR-2 mode;
- acquisition capture time and source locator when supplied;
- the exact result bytes as a content-addressed attachment;
- parser/adapter version and the expected column contract; and
- accepted, malformed, filtered, unsupported, duplicate, and conflicting
  counts.

The expected query projection is:

`id`, `time`, `band`, `rx_sign`, `rx_loc`, `tx_sign`, `tx_loc`, `distance`,
`azimuth`, `rx_azimuth`, `frequency`, `power`, `snr`, `drift`, `version`, and
`code`.

The source query should constrain `tx_sign` to the exact normalized session
callsign, constrain `time` to the explicit half-open window, constrain `band`
to the session's supported bands, constrain `code` to WSPR-2, order by
`time, id`, and use `FORMAT JSON`. The adapter repeats every safety filter; it
never trusts the query description or filename to have selected correctly.

The normal source window is the schedule's earliest slot start through latest
slot end. A caller may deliberately supply a wider bounded source result, but
rows outside the schedule window remain filtered evidence and cannot become
observations. Source timestamps identify the WSPR receive period and continue
through the existing slot-alignment and guard-time policy; acquisition time is
not used as the observation time.

Structural JSON failure, missing required columns, duplicate column names, an
unsupported response shape, or a resource-limit breach fails the complete
acquisition. Individual bounded rows with invalid values remain auditable
adapter dispositions and do not become normalized observations.

## Provenance And Normalization

WSPR.live evidence uses schema-v2 provider-neutral provenance:

- provider: `wspr-live`;
- source: `wsprnet-spots-mirror`;
- acquisition channel: `file-import` for a selected file or `https-query` for the
  automatic client;
- adapter: `antennabench.wspr-live-json`; and
- the AntennaBench adapter version.

The acquisition summary references the exact response attachment. Row-level
adapter records preserve each selected near-raw object and link to any
normalized observation. This avoids one oversized summary record while
retaining exact reproduction input.

An accepted row becomes an `ImportedSpot` observation with:

- source event time from `time`;
- remote receiver call/grid as reporter identity;
- the session transmitter call/grid as heard identity;
- WSPR band, receive frequency in Hz, and mode `WSPR`;
- reported SNR and drift;
- dBm converted to watts using the existing WSPR conversion;
- provider distance and transmitter-to-receiver azimuth when valid; and
- upstream WSPRnet spot ID, WSPR.live mode code, receiver software version,
  acquisition capture time, and near-raw values retained as adapter evidence.

The provider's `id` is the primary replay identity within this source. Repeated
acquisitions of an identical ID and row are duplicates and do not append a
second observation. Reuse of one ID with conflicting semantic values is
retained as a conflict and produces no observation. AntennaBench does not
deduplicate across WSPR.live, a future direct WSPRnet source, RBN, or another
provider merely because callsign, time, and band happen to match.

WSPR, RBN CW, RBN RTTY, and other provider/mode strata remain separate. Missing
public reports are missing evidence, never zero-SNR observations. Provider
coordinates, distance, azimuth, grid completion, and transmitter power are not
recomputed or inferred when absent or invalid.

## Freshness, Completeness, And Errors

A file import has no polling interval, retries, backoff, conditional requests,
or cache revalidation. Automatic acquisition follows the five-minute grace,
cumulative-overlap, coalescing, single-flight, and ten-second minimum interval
defined above. It performs no automatic transport retry; later opted-in
acquisitions overlap earlier windows, while a final failure offers explicit
retry. The exact complete response attachment is the immutable cache in both
paths. A failed acquisition does not mutate the bundle.

The report presents the acquisition capture time, source window, accepted
count, and all disposition counts. It labels completeness as unknown because WSPR.live
documents scrape lag, later reconciliation, duplicates, false spots, outages,
and no availability guarantee. Acquiring a later result may add newly arrived
provider IDs, but it does not rewrite earlier evidence or imply that the final
set is complete.

## Offline And Privacy Behavior

Public-spot acquisition is optional. Setup, conduction, local WSJT-X ingestion,
analysis, reports, and export work without it. Automatic acquisition is off
until the operator opts in for the session. The desktop never uploads the
session bundle, local observations, operator notes, antenna labels, or grid.
The request contains only the public transmitter callsign, bounded UTC window,
selected bands, and fixed WSPR-2 mode needed to find public reports.

The source callsigns, grids, signal reports, and times are already publicly
reported data, but their inclusion in a session still receives source
attribution and remains under the operator's control. AntennaBench does not
republish acquired rows automatically. The opt-in UI identifies WSPR.live as a
volunteer WSPRnet mirror, links its current usage terms, explains the bounded
request and attribution, and states that completeness is unknown. Manual file
import remains available without enabling automatic network access.

## Test Policy

Tests use small synthetic `FORMAT JSON` fixtures modeled on the documented
column contract. No live network request runs in ordinary tests, and no
third-party spot row is committed without confirmed redistribution permission.

Contract coverage includes:

- valid transmit reports across supported bands;
- callsign, UTC-window, band, and WSPR-2 filtering;
- exact field/unit mapping and TX path direction;
- malformed JSON, schema drift, invalid values, and unsupported modes/bands;
- exact replay, conflicting provider IDs, and observation-link integrity;
- missing grids/location/version without inference;
- resource limits and cancellation;
- exact attachment digest/round-trip and lossless bundle export;
- stable partial/completeness and attribution presentation;
- no request before session opt-in and no per-segment consent prompt afterward;
- typed SQL/URL construction, every band mapping, and injection rejection;
- grace, cumulative overlap, coalescing, restart, and finalizing transitions;
- HTTP status, timeout, cancellation, and response-size failures without
  mutation; and
- identical normalization with distinct `file-import` and `https-query`
  provenance.

A disposable, minimal query may be run manually against WSPR.live for an
operational smoke check. Its data is not retained in the repository and its
success is not a release or CI prerequisite.

## Consequences

- AntennaBench gains a useful TX public-report boundary without making network
  availability part of a session.
- The landed parser and normalization contract is reused by the opt-in
  WSPR.live HTTPS adapter.
- Normal connected operation acquires public reports automatically after the
  documented ingestion grace period; manual import remains the offline and
  recovery path.
- AntennaBench attributes WSPR.live, links its terms, bounds every query, and
  does not gate or sell the service or derived results.
- WSPRnet remains the authoritative upstream publisher, while provenance
  accurately records that the acquired representation came through WSPR.live.
- The RBN archive adapter in #29 remains a separate provider-specific import
  that shares generic adapter evidence and TX observation analysis, not WSPR
  parsing or acquisition policy.

## Alternatives Rejected

### Scrape WSPRnet HTML

Rejected because the markup is a presentation surface with no documented
automation, rate, or stability contract.

### Make Automatic Acquisition Mandatory

Rejected because network availability and a volunteer third-party service must
not become prerequisites for conducting, preserving, or analyzing a session.

### Import Whole WSPRnet Monthly Archives First

Rejected as the first local-session path because current archives are delayed,
multi-GiB, and can exceed the fixed local attachment profile. A future
higher-capacity or remotely filtered archive workflow would require its own
resource decision.

### Invent A Provider-Neutral Spot CSV

Rejected because no existing source produces it, conversion would become an
untracked acquisition step, and provider-specific semantics would be easier to
lose. Provider neutrality belongs in durable provenance and normalized
observations, not in pretending raw formats are identical.

## References

- [Decision issue #13](https://github.com/rwjblue/antennabench/issues/13)
- [WSPR.live JSON import adapter #84](https://github.com/rwjblue/antennabench/issues/84)
- [Automatic WSPR.live acquisition #85](https://github.com/rwjblue/antennabench/issues/85)
- [RBN tracking issue #31](https://github.com/rwjblue/antennabench/issues/31)
- [RBN archive adapter #29](https://github.com/rwjblue/antennabench/issues/29)
- [Decision 0008](0008-use-provider-neutral-adapter-evidence-in-bundle-v2.md)
- [Decision 0011](0011-use-a-fixed-bounded-local-resource-profile.md)
- [WSJT-X User Guide](https://wsjt.sourceforge.io/wsjtx-main_en.html)
- [WSJT-X 3.0.2 WSPRnet client](https://github.com/WSJTX/wsjtx/blob/v3.0.2/Network/wsprnet.cpp)
- [WSJT-X 3.0.2 upload setting and
  orchestration](https://github.com/WSJTX/wsjtx/blob/v3.0.2/widgets/mainwindow.cpp)
- [WSPRnet current query form](https://www.wsprnet.org/drupal/wsprnet/spotquery)
- [WSPRnet old database](https://www.wsprnet.org/olddb)
- [WSPRnet monthly archives](https://www.wsprnet.org/archive/)
- [WSPR.live access, schema, limits, and terms](https://wspr.live/)
