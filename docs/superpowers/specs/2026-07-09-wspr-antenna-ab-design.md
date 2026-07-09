# WSPR Antenna A/B Testing App Design

Date: 2026-07-09

## Summary

Build a local-first application for WSPR-based antenna comparison and profiling. The first version should be a desktop app that guides the operator through controlled WSPR time slots, collects local and public observations, stores a portable evidence bundle, and generates credible reports. The hosted web app should explain the method and safely render shared report bundles.

The v1 implementation should use WSJT-X companion mode rather than a native WSPR stack. The architecture should still keep WSPR interaction behind adapters so native transmit/receive, mobile operation, and deeper automation can be added later.

## Goals

- Compare two antennas using short, coordinated WSPR slots to reduce propagation drift between A and B.
- Support whole-station A/B as the default mode, while allowing TX-focused, RX-focused, and single-antenna profiling modes.
- Keep the app fully useful offline.
- Preserve enough raw data locally that future analysis can be regenerated from the original session.
- Generate honest reports that separate observations, inferences, and confidence.
- Allow users to publish validated JSON bundles to a hosted report viewer.
- Use a code architecture that can later support native WSPR and mobile apps without rewriting the experiment model.

## Non-Goals for V1

- Native WSPR transmit/receive implementation.
- Mobile field app.
- Public community search, callsign directory, aggregate browsing, or antenna leaderboards.
- Strong callsign verification.
- Anonymous listed uploads.
- Rigid antenna taxonomy.
- Uploading user-provided HTML or JavaScript.

## Local App Workflow

The local desktop app should behave like an experiment conductor:

1. Collect station basics: callsign, grid, and power if known.
2. Ask for the goal: DX, regional, NVIS/local, general coverage, weak-signal reliability, or single-antenna profiling.
3. Define antennas. The required fields are only antenna labels such as `A` and `B`. Optional fields include category facets, height, radial count, radial length, orientation, tuner, feedline, deployment notes, and other installation details.
4. Define the schedule: band or band sequence, WSPR slot duration, A/B order, guard time for manual switching, and total run target.
5. Run the session with clear visual/audio prompts. The run screen should show current antenna, current band, current slot, countdown, next action, and quick actions such as `Switched`, `Missed`, `Bad slot`, and `Add note`.
6. Collect local and external data while the session runs.
7. Generate a report, export the local bundle, and optionally publish the report.

Whole-station A/B should be the default mode because the operator is manually switching the antenna path. TX-focused and RX-focused reports can be generated from the same run when the needed data exists.

Single-antenna profiling mode should use the same collection machinery without A/B winner logic. It records how one antenna performs under known propagation conditions, enabling future comparisons against similar conditions.

## WSPR and Rig Control

V1 should integrate with WSJT-X rather than attempting to replace it. WSJT-X handles WSPR timing, transmit audio, receive decode, upload behavior, and existing radio setup. The app should interact with WSJT-X through UDP status/decode messages and logs where needed.

The app should treat WSJT-X automation as progressive:

- Required for v1: monitor status, ingest local decodes, ingest relevant logs, and align observations to planned slots.
- Desired for v1 if practical: detect frequency/mode/TX state and warn when setup does not match the schedule.
- Later: deeper control over transmit state, WSJT-X configuration switching, and band changes where reliable.

Band and rig control should be adapter-based. Hamlib should be the first rig-control path for setting frequency/mode where supported, but a session must still be runnable with no rig control.

## Data Model

The canonical local record should be a portable session bundle. SQLite should be a rebuildable local query/index layer, not the source of truth.

Proposed bundle shape:

```text
session.wsprabundle/
  manifest.json
  station.json
  antennas.json
  schedule.json
  events.jsonl
  observations.jsonl
  wsjtx.jsonl
  rig.jsonl
  propagation.jsonl
  analysis.json
  attachments/
```

Rules:

- JSON/JSONL is the durable source of truth.
- Event-like records should be append-only where practical.
- Every record should include timestamp, source, schema version, and session id.
- Raw or near-raw inputs should be preserved so analysis can improve later.
- SQLite can be deleted and rebuilt from the bundle.
- Reports are derived artifacts and should be reproducible from the bundle.

Minimum data for A/B comparison:

- callsign
- grid
- band
- antenna labels
- planned slots
- actual operator events
- observations

Observation records should support:

- local WSJT-X decodes
- public reports where others heard the station
- band, frequency, and mode
- reporter/heard callsign
- grid, distance, and azimuth when known
- SNR, drift, power, timestamp
- slot label and slot confidence
- source, such as WSJT-X UDP, WSJT-X log, WSPRnet/WSPR.live, or imported file

Propagation records should be time-scoped snapshots rather than a single global session value. Useful fields include UTC time, solar flux/F10.7, sunspot number, Kp, A-index where available, solar wind/Bz where available, NOAA alerts or storm levels, and daylight/twilight state if derived.

## Reports and Analysis

Reports should avoid naive winner claims. They should distinguish:

- what the operator planned
- what actually happened
- what was observed
- what the app inferred
- how strong the evidence is

Core report sections:

1. Executive summary with effect size and confidence when warranted.
2. Session context: callsign, grid, date/time, bands, goal, antennas, schedule, actual events, and propagation conditions.
3. TX report: who heard the station, SNR distribution, unique reporters, grid/distance/azimuth buckets, and paired A/B deltas where comparable reporters exist.
4. RX report: who the station heard locally, decode counts, SNR distribution, buckets, and repeated heard-station comparisons.
5. Goal lens: DX, regional, NVIS/local, general coverage, weak-signal reliability, or profiling.
6. Evidence quality: sample size, missed/bad slots, unbalanced counts, propagation drift, band changes, and whether the conclusion is strong enough.

V1 statistics should be practical and conservative:

- paired comparisons where possible
- median and mean SNR deltas by reporter/grid/distance bucket
- simple resampling or bootstrap confidence interval for effect size
- minimum sample thresholds before declaring a winner
- `too close to call` and `insufficient data` as first-class conclusions

Useful v1 charts:

- paired A/B SNR delta histogram
- SNR over time by slot
- distance bucket comparison
- azimuth/grid map
- reporter overlap: A only, B only, both
- data quality timeline

## Hosted Sharing

The hosted v1 should be a marketing site plus safe shared report viewer.

Required hosted features:

- marketing/process page explaining the WSPR A/B method
- sample report
- email + passkey account system
- self-claimed callsign support
- upload/publish endpoint for validated JSON bundles
- stable report URL such as `/r/<short-id>`
- report viewer rendered entirely by hosted app code
- raw bundle download

Cloudflare platform layout:

- Workers with static assets for the hosted app and API.
- R2 for original and normalized uploaded bundles.
- D1 for metadata such as report id, owner, callsign, short id, timestamps, schema version, and visibility.
- Turnstile and rate limits around signup and publishing.

Security and publishing rules:

- No raw HTML or JavaScript uploads.
- User notes are plain text in v1.
- Uploaded bundles must pass strict schema and size validation.
- Reports published by signed-in users default to public/listed metadata. In v1 this means the report has a public stable URL and metadata suitable for future discovery; the discovery UI itself is deferred.
- Anonymous publishing is deferred or limited to unlisted links later.
- The local app remains fully useful without an account or network connection.

V1 should not include community discovery UI. Callsign pages, search, aggregate filters, and public comparison browsing can be added after the report viewer and data model prove themselves, using the listed metadata already captured at publish time.

## Public Identity Model

Public discovery should eventually be callsign-first. For v1, accounts only need enough structure to own uploads.

Model:

- Account: email + passkey login that owns uploads.
- Callsign claim: one account can self-claim one or more callsigns.
- Report: tied to an owner account and callsign string.
- Verification status: self-claimed for v1; stronger verification later.
- Moderation escape hatch: admins can hide reports or correct obvious abuse.

Antenna metadata should not be a strict taxonomy. Use freeform antenna labels as primary identity, optional broad facets for coarse filtering, and notes for real-world installation nuance.

## Architecture

Use a monorepo with Rust core crates and web-based UIs.

Proposed structure:

```text
apps/
  desktop/          Tauri desktop app for Windows, macOS, and Linux
  web/              Cloudflare-hosted marketing site and report viewer

crates/
  core/             sessions, stations, antennas, slots, schedules, bundles
  analysis/         goal scoring, summaries, confidence, evidence quality
  report/           report model and chart-ready structures
  propagation/      solar/geomagnetic context capture and normalization
  storage/          JSONL bundle read/write and SQLite indexing
  wsjtx/            WSJT-X UDP/log adapter
  rig/              Hamlib and future rig-control adapters
  public-spots/     WSPRnet/WSPR.live fetch/import adapters
```

Important interfaces:

- `WsprAdapter`: WSJT-X companion first; native WSPR later.
- `RigAdapter`: no rig control, Hamlib, later direct radio APIs.
- `SpotSource`: WSPRnet, WSPR.live, imported file.
- `PropagationSource`: NOAA/SWPC snapshots and manual import fallback.
- `BundleStore`: local JSONL bundle and SQLite index.
- `Publisher`: Cloudflare upload or local export only.

Tauri is the right v1 desktop shell because it allows a web UI with native Rust capabilities. It also keeps a path open for mobile work later. Mobile should reuse the Rust core and provide mobile-specific audio and rig adapters. Android is likely the first practical mobile target for real radio control because USB host workflows are more plausible than iOS generic wired serial/audio control.

## Testing Strategy

V1 should include:

- golden bundle fixtures for known sessions
- import/export round-trip tests
- WSJT-X UDP/log parser fixtures
- analysis tests with synthetic cases: clear A, clear B, too close, and insufficient data
- schedule/slot alignment tests, including missed and bad slots
- report-model tests from fixed bundles
- Cloudflare upload validation tests, including malicious input rejection
- hosted report rendering tests from fixed bundles

## Open Questions for Implementation Planning

- Which WSJT-X control messages are reliable for WSPR mode in practice?
- Which public spot source should be the default, and what lag/polling policy should v1 enforce?
- What exact confidence language and thresholds should v1 use?
- How much Hamlib integration belongs in the first implementation milestone?
- What account/passkey library or service should the Cloudflare app use?
- What should the first sample report bundle contain?

## Recommended V1 Scope

Build the desktop data-collection/report workflow first, with WSJT-X companion mode and local bundles. Build the hosted side as a report viewer and marketing surface, not a community database.

The project should optimize for collecting real, trustworthy antenna-test data quickly. Native WSPR, mobile operation, and community discovery are important future tracks, but they should grow from the same core data model rather than block v1.
