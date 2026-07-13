# Architecture

AntennaBench is organized around a durable session bundle. The bundle is the
source of truth; everything else is derived from it.

## Crates

Current crates:

- `crates/core`: serializable bundle model, schedule alignment, normalization,
  and validation.
- `crates/storage`: filesystem read/write APIs for `.session.wsprabundle`
  directories.
- `crates/wsjtx`: offline WSPR `ALL_WSPR.TXT` import plus a live WSJT-X UDP
  companion boundary, producing preserved adapter records and eligible local
  decode observations.
- `crates/analysis`: conservative, descriptive A/B evidence summaries derived
  in memory from validated bundle contents and core schedule alignment.
- `crates/report`: deterministic, renderer-neutral report data derived in
  memory from one bundle and its analysis summary.

Planned crates and apps:

- `apps/desktop`: desktop application shell.
- `apps/web`: hosted report viewer and publishing surface.
- `crates/rig`: rig-control adapters.
- `crates/public-spots`: WSPRnet, WSPR.live, and imported spot adapters.

## Data Flow

```text
operator + adapters
        |
        v
session bundle JSON/JSONL
        |
        +--> normalization
        +--> validation
        +--> local indexes
        +--> analysis
        +--> reports
        +--> hosted rendering
```

The current storage API exposes three read modes:

- `BundleStore::read()`: parse-only filesystem read.
- `BundleStore::read_validated()`: strict read and validation.
- `BundleStore::read_normalized_validated()`: tolerant read, normalization, and
  validation.

Analysis accepts normalized bundle contents, validates them without mutation,
and reuses core alignment to derive slot status and evidence eligibility. It
returns observation counts, exclusions, per-antenna/band/slot evidence, SNR
descriptive statistics, and conservative evidence-quality labels. It does not
select a winner or perform effect-size, confidence, or significance analysis.

Report construction accepts one `BundleContents` value and invokes analysis
internally, preventing callers from pairing bundle context with a summary from
another bundle. It deterministically projects session context, conservative
evidence sections, typed notices, and concrete chart-ready rows for antenna SNR,
band evidence counts, and slot usable/excluded counts. The model is serializable
but renderer-neutral: it contains no generated prose, winner logic, generic
chart configuration, or rendering output.

## Alignment

Schedule alignment is pure core logic. It derives actual slot state from planned
slots plus operator events, then assigns observations to slots with labels and
confidence.

Alignment is deterministic. Validation uses the same alignment logic to detect
stale persisted observation annotations.

## Derived State

SQLite indexes, UI state, generated reports, charts, and hosted publishing
artifacts are derived. They can be rebuilt from the bundle and should not become
the canonical record of a session.

Analysis summaries and session reports are currently derived in memory and are
not persisted. `analysis.json` remains bundle metadata rather than a serialized
analysis summary or report. Report construction does not change the bundle
format or schema version.

## Live WSJT-X Boundary

The live adapter accepts official WSJT-X network-message schemas 2 and 3. It
parses heartbeat, the status prefix through station identity, WSPRDecode, and
close messages. Unknown message types and compatible trailing fields are
ignored for behavior while supported datagrams are retained exactly as hex in
bundle-ready `WsjtXRecord` values.

The parser is pure. `LiveWsjtxIngest` owns the small per-client state machine
for schema/version identity, current status, duplicate suppression, and client
lifecycle. A close message or a gap longer than three heartbeat periods resets
status and duplicate state. The synchronous UDP receiver only binds, receives,
timestamps, and exposes explicit shutdown; orchestration remains the future
desktop application's responsibility.

WSPRDecode carries a time-of-day rather than a date. The adapter reconstructs
UTC by choosing the closest of the receipt date and its adjacent dates, using
the supplied session start only as a deterministic tie-breaker. This handles
midnight rollover without inventing a durable clock source. Decode and receipt
times remain available in the preserved raw data.

Observation production is deliberately conservative: `New` must be true,
`Off air` false, the datagram must not be a duplicate in the current client
generation, and current status must identify the configured station in WSPR
mode. Status transmitting/receiving/decoding values are tracked and preserved
but do not gate a decode because WSJT-X status transitions and completed decode
delivery need not occur in the same instant.
