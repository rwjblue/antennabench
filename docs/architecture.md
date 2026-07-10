# Architecture

AntennaBench is organized around a durable session bundle. The bundle is the
source of truth; everything else is derived from it.

## Crates

Current crates:

- `crates/core`: serializable bundle model, schedule alignment, normalization,
  and validation.
- `crates/storage`: filesystem read/write APIs for `.session.wsprabundle`
  directories.
- `crates/wsjtx`: offline WSJT-X companion import helpers for WSPR
  `ALL_WSPR.TXT`-style logs, producing raw adapter records and local decode
  observations.

Planned crates and apps:

- `apps/desktop`: desktop application shell.
- `apps/web`: hosted report viewer and publishing surface.
- `crates/analysis`: report summaries, effect sizes, confidence, and evidence
  quality.
- `crates/report`: report model and chart-ready structures.
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
