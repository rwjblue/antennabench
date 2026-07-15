# 0015: Use An Import-First WSPR Public-Spot Boundary

Date: 2026-07-14

## Decision

AntennaBench will introduce transmit-path WSPR public reports through a
deterministic file importer before it adds live public-spot polling. The first
source-specific input is a bounded WSPR.live ClickHouse `FORMAT JSON` result
document supplied by the operator. Parsing and normalization perform no hidden
network access.

This decision does not authorize AntennaBench to query WSPR.live in production.
The service permits personal research and projects only when results remain
freely accessible, and it prohibits commercial or profit-oriented use. Those
terms do not clearly cover a generally distributed Apache-licensed desktop app,
private local reports, or every downstream use that the license permits. A live
WSPR.live transport therefore requires written clarification from its operator
covering installed applications, retained local evidence, private reports,
attribution, and commercial-capable distribution.

AntennaBench will not scrape WSPRnet's HTML database pages. WSPRnet is the
authoritative upload destination used by WSJT-X, but its anonymous public
surfaces do not currently provide a documented, bounded application API that
meets this product's reliability and usage-policy requirements. The JSON route
observed during this decision returned `403 Access denied`; the supported web
interfaces are human-oriented recent-result pages and very large monthly
archives. Direct WSPRnet acquisition may be reconsidered if its administrators
document and grant an appropriate API contract.

Network polling is therefore deliberately `none` in the first slice. Sessions
remain complete local workflows without spot import, and failure or omission of
public reports never stops the conductor.

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

Technically, this is the best candidate for a later live transport. Operational
and licensing terms prevent selecting that transport today: availability and
stability are explicitly not guaranteed, commercial/profit-oriented use is
forbidden, and the requirement that results remain freely accessible is
ambiguous for private local reports. AntennaBench will not turn those
constraints into hidden application assumptions.

### Import First

Import-first is selected because it separates three responsibilities:

1. the operator obtains a bounded result under terms and authority applicable
   to that operator;
2. a deterministic adapter parses and normalizes exact supplied bytes; and
3. later network orchestration can reuse the same parser only after its source
   and polling contract is authorized.

The first format is WSPR.live's documented ClickHouse `FORMAT JSON` response,
not an AntennaBench-invented spot interchange format. This keeps the file useful
with existing WSPR.live query/export tools and exercises the same response
shape a future HTTPS transport would receive.

## Import Contract

The import operation accepts one complete JSON response plus explicit operator
inputs. It records:

- normalized session transmitter callsign;
- half-open UTC source window `[window_start, window_end)`;
- selected WSPR bands and WSPR-2 mode;
- import capture time and source locator when supplied;
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
`time, id`, and use `FORMAT JSON`. The importer repeats every safety filter; it
never trusts the query description or filename to have selected correctly.

The normal import window is the schedule's earliest slot start through latest
slot end. A caller may deliberately supply a wider bounded source result, but
rows outside the schedule window remain filtered evidence and cannot become
observations. Source timestamps identify the WSPR receive period and continue
through the existing slot-alignment and guard-time policy; import time is not
used as the observation time.

Structural JSON failure, missing required columns, duplicate column names, an
unsupported response shape, or a resource-limit breach fails the complete
import. Individual bounded rows with invalid values remain auditable adapter
dispositions and do not become normalized observations.

## Provenance And Normalization

Imported evidence uses schema-v2 provider-neutral provenance:

- provider: `wspr-live`;
- source: `wsprnet-spots-mirror`;
- acquisition channel: `file-import`;
- adapter: `antennabench.wspr-live-json`; and
- the AntennaBench adapter version.

The import summary references the exact response attachment. Row-level adapter
records preserve each selected near-raw object and link to any normalized
observation. This avoids one oversized summary record while retaining exact
reproduction input.

An accepted row becomes an `ImportedSpot` observation with:

- source event time from `time`;
- remote receiver call/grid as reporter identity;
- the session transmitter call/grid as heard identity;
- WSPR band, receive frequency in Hz, and mode `WSPR`;
- reported SNR and drift;
- dBm converted to watts using the existing WSPR conversion;
- provider distance and transmitter-to-receiver azimuth when valid; and
- upstream WSPRnet spot ID, WSPR.live mode code, receiver software version,
  import capture time, and near-raw values retained as adapter evidence.

The provider's `id` is the primary replay identity within this source. Repeated
imports of an identical ID and row are duplicates and do not append a second
observation. Reuse of one ID with conflicting semantic values is retained as a
conflict and produces no observation. AntennaBench does not deduplicate across
WSPR.live, a future direct WSPRnet source, RBN, or another provider merely
because callsign, time, and band happen to match.

WSPR, RBN CW, RBN RTTY, and other provider/mode strata remain separate. Missing
public reports are missing evidence, never zero-SNR observations. Provider
coordinates, distance, azimuth, grid completion, and transmitter power are not
recomputed or inferred when absent or invalid.

## Freshness, Completeness, And Errors

A file import has no polling interval, retries, backoff, conditional requests,
or cache revalidation. The exact file attachment is its immutable cache. A
failed import does not mutate the bundle.

The report presents the import capture time, source window, accepted count, and
all disposition counts. It labels completeness as unknown because WSPR.live
documents scrape lag, later reconciliation, duplicates, false spots, outages,
and no availability guarantee. Importing a later result may add newly arrived
provider IDs, but it does not rewrite earlier evidence or imply that the final
set is complete.

If a future authorized live transport is added, its issue must define a bounded
poll cadence below the provider's published rate, overlap/window watermarks for
late rows, retry and `Retry-After` behavior, stop/finalization timing, network
timeouts, source-error adapter records, and explicit partial/stale UI. This ADR
does not silently pre-authorize those network choices.

## Offline And Privacy Behavior

Public-spot import is optional. Setup, conduction, local WSJT-X ingestion,
analysis, reports, and export work without it. The desktop never uploads the
session bundle or local operator events as part of import.

The source callsigns, grids, signal reports, and times are already publicly
reported data, but their inclusion in a session still receives source
attribution and remains under the operator's control. AntennaBench does not
republish imported rows automatically. The import UI must identify WSPR.live as
a volunteer WSPRnet mirror, link its current usage terms, and require the
operator to confirm that the supplied file may be used for the intended
purpose.

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
- exact attachment digest/round-trip and lossless bundle export; and
- stable partial/completeness and attribution presentation.

A disposable, minimal query may be run manually against WSPR.live when its
current terms authorize that check. Its data is not retained in the repository
and its success is not a release or CI prerequisite.

## Consequences

- AntennaBench gains a useful TX public-report boundary without making network
  availability part of a session.
- The parser and normalization contract can be reused by a later authorized
  WSPR.live HTTPS adapter.
- Near-real-time automatic fetching remains unavailable in the first slice.
- Operators must obtain and retain authority for imported WSPR.live results.
- WSPRnet remains the authoritative upstream publisher, while provenance
  accurately records that the imported representation came through WSPR.live.
- The RBN archive adapter in #29 remains a separate provider-specific import
  that shares generic adapter evidence and TX observation analysis, not WSPR
  parsing or acquisition policy.

## Alternatives Rejected

### Scrape WSPRnet HTML

Rejected because the markup is a presentation surface with no documented
automation, rate, or stability contract.

### Query WSPR.live Directly Now

Rejected until source terms clearly authorize installed applications, private
local reports, retained evidence, and commercial-capable distribution.

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
