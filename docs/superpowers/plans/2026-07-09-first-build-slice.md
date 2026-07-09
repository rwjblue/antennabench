# AntennaBench First Build Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first testable AntennaBench foundation: a Rust workspace with core session/bundle data types and JSON/JSONL bundle import/export tests.

**Architecture:** Keep the durable experiment model in `crates/core` and filesystem bundle I/O in `crates/storage`. The first slice treats JSON/JSONL bundle files as the source of truth and deliberately leaves UI, hosting, WSJT-X UDP, rig control, public spot fetching, propagation fetching, analysis statistics, and report charts for later slices.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, `chrono`, `thiserror`, Cargo tests, jj commits.

---

## Scope

This slice creates working, testable software for:

- Repo foundation for a colocated git/jj Rust workspace.
- `crates/core`: canonical schema version, session/station/antenna/schedule records, append-style event and observation records, optional raw adapter records, propagation snapshots, and an analysis stub file that records analysis status without computing reports.
- `crates/storage`: read/write support for a `.session.wsprabundle` directory containing JSON files and JSONL streams.
- Golden fixture coverage for a minimal whole-station A/B session.
- Round-trip tests proving that in-memory bundle data can be exported and imported without losing the durable fields.

This slice does not create a Tauri app, Cloudflare app, WSJT-X UDP adapter, Hamlib adapter, WSPRnet/WSPR.live client, charting code, account system, publishing endpoint, SQLite index, or analysis engine.

## File Structure

- Create `Cargo.toml`: workspace members and shared dependency versions.
- Create `rust-toolchain.toml`: stable Rust pin for reproducible local setup.
- Create `.gitignore`: Rust build output and local editor/system files.
- Create `README.md`: public repo summary, current slice status, and bundle-first architecture note.
- Create `crates/.gitkeep`: keep the crate directory present before crate manifests exist.
- Create `crates/core/Cargo.toml`: `antennabench-core` crate metadata and dependencies.
- Create `crates/core/src/lib.rs`: public module exports and schema constant.
- Create `crates/core/src/model.rs`: core serializable domain and bundle container types.
- Create `crates/core/tests/model_serialization.rs`: test-first coverage for schema shape and serde names.
- Create `crates/storage/Cargo.toml`: `antennabench-storage` crate metadata and dependencies.
- Create `crates/storage/src/lib.rs`: bundle store API, JSON/JSONL read/write helpers, and errors.
- Create `crates/storage/tests/bundle_roundtrip.rs`: generated-tempdir round-trip tests.
- Create: `Cargo.lock`: generated Cargo dependency lockfile after the storage crate can resolve.
- Create `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/`: golden bundle fixture.
- Create `crates/storage/tests/golden_bundle.rs`: import fixture, export copy, and re-import tests.

## Task 1: Repo Foundation

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `README.md`
- Create: `crates/.gitkeep`

- [ ] **Step 1: Create the workspace manifest**

Write `Cargo.toml` with this complete content:

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.78"
license = "MIT OR Apache-2.0"
repository = "https://github.com/rwjblue/antennabench"

[workspace.dependencies]
chrono = { version = "0.4.38", default-features = false, features = ["clock", "serde", "std"] }
serde = { version = "1.0.204", features = ["derive"] }
serde_json = "1.0.120"
tempfile = "3.10.1"
thiserror = "1.0.63"
```

- [ ] **Step 2: Pin the Rust toolchain**

Write `rust-toolchain.toml` with this complete content:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Add ignore rules**

Write `.gitignore` with this complete content:

```gitignore
/target/
/.DS_Store
/.idea/
/.vscode/
*.swp
*.swo
```

- [ ] **Step 4: Add the initial README**

Write `README.md` with this complete content:

```markdown
# AntennaBench

AntennaBench is a local-first antenna comparison and profiling app for WSPR experiments.

The first implementation slice focuses on the durable session bundle: JSON and JSONL files that preserve station details, antennas, schedules, operator events, observations, adapter inputs, propagation snapshots, and analysis metadata. SQLite, UI state, reports, and hosted publishing are derived from the bundle rather than being the source of truth.

First build slice scope:

- Rust workspace foundation.
- Core bundle schema crate.
- Filesystem bundle import/export crate.
- Golden fixture and round-trip tests for a minimal whole-station A/B session.

Planned later slices include the desktop app, WSJT-X companion adapter, rig-control adapters, public spot imports, analysis/report generation, and hosted report viewing.
```

- [ ] **Step 5: Keep the empty crate directory present**

Write `crates/.gitkeep` with this complete content:

```text
Keep the crates directory present so Cargo workspace globs can be empty before crate manifests exist.
```

- [ ] **Step 6: Verify workspace metadata can load before member crates exist**

Run:

```bash
cargo metadata --no-deps
```

Expected: PASS with an empty `workspace_members` list. The `crates/*` member glob lets the first slice add crates incrementally while still keeping all crates under the workspace once their manifests exist.

- [ ] **Step 7: Commit the foundation files**

Run:

```bash
jj status
jj commit -m "chore: add Rust workspace foundation"
```

Expected: `jj commit` creates a described change and leaves a new empty working-copy commit.

## Task 2: Core Session, Station, Antenna, and Schedule Types

**Files:**
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/core/src/model.rs`
- Create: `crates/core/tests/model_serialization.rs`

- [ ] **Step 1: Create the core crate manifest**

Write `crates/core/Cargo.toml` with this complete content:

```toml
[package]
name = "antennabench-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Write the failing model serialization tests**

Write `crates/core/tests/model_serialization.rs` with this complete content:

```rust
use antennabench_core::{
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents, BundleFiles,
    BundleManifest, ExperimentMode, PlannedSlot, Schedule, SessionGoal, Station, SCHEMA_VERSION,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn serializes_minimum_station_and_schedule_shape() {
    let session_id = "session-2026-07-09-n1rwj-20m".to_string();
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 0).unwrap();

    let station = Station {
        schema_version: SCHEMA_VERSION,
        session_id: session_id.clone(),
        callsign: "N1RWJ".to_string(),
        grid: "FN42".to_string(),
        power_watts: Some(5.0),
        operator_notes: Some("Backyard comparison".to_string()),
    };

    let schedule = Schedule {
        schema_version: SCHEMA_VERSION,
        session_id,
        mode: ExperimentMode::WholeStationAb,
        goal: SessionGoal::GeneralCoverage,
        slots: vec![PlannedSlot {
            slot_id: "slot-001".to_string(),
            sequence_number: 1,
            starts_at,
            duration_seconds: 120,
            guard_seconds: 15,
            band: Band::M20,
            antenna_label: "A".to_string(),
        }],
    };

    assert_eq!(
        serde_json::to_value(station).unwrap(),
        json!({
            "schema_version": 1,
            "session_id": "session-2026-07-09-n1rwj-20m",
            "callsign": "N1RWJ",
            "grid": "FN42",
            "power_watts": 5.0,
            "operator_notes": "Backyard comparison"
        })
    );

    assert_eq!(
        serde_json::to_value(schedule).unwrap(),
        json!({
            "schema_version": 1,
            "session_id": "session-2026-07-09-n1rwj-20m",
            "mode": "whole_station_ab",
            "goal": "general_coverage",
            "slots": [{
                "slot_id": "slot-001",
                "sequence_number": 1,
                "starts_at": "2026-07-09T20:00:00Z",
                "duration_seconds": 120,
                "guard_seconds": 15,
                "band": "20m",
                "antenna_label": "A"
            }]
        })
    );
}

#[test]
fn bundle_contents_groups_required_bundle_files() {
    let session_id = "session-2026-07-09-n1rwj-20m".to_string();
    let created_at = Utc.with_ymd_and_hms(2026, 7, 9, 19, 58, 0).unwrap();

    let bundle = BundleContents {
        manifest: BundleManifest {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            created_at,
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            antennas: vec![Antenna {
                label: "A".to_string(),
                facets: vec!["vertical".to_string()],
                height_m: Some(7.0),
                radial_count: Some(16),
                radial_length_m: Some(5.0),
                orientation_degrees: None,
                tuner: Some("manual".to_string()),
                feedline: Some("RG-8X".to_string()),
                notes: Some("Temporary ground-mounted vertical".to_string()),
            }],
        },
        schedule: Schedule {
            schema_version: SCHEMA_VERSION,
            session_id,
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: Vec::new(),
        },
        events: Vec::new(),
        observations: Vec::new(),
        wsjtx: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION,
            session_id: "session-2026-07-09-n1rwj-20m".to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: vec!["analysis engine not part of the first slice".to_string()],
        },
    };

    assert_eq!(bundle.manifest.files.manifest, "manifest.json");
    assert_eq!(bundle.manifest.files.station, "station.json");
    assert_eq!(bundle.manifest.files.antennas, "antennas.json");
    assert_eq!(bundle.manifest.files.schedule, "schedule.json");
    assert_eq!(bundle.manifest.files.events, "events.jsonl");
    assert_eq!(bundle.manifest.files.observations, "observations.jsonl");
    assert_eq!(bundle.manifest.files.wsjtx, "wsjtx.jsonl");
    assert_eq!(bundle.manifest.files.rig, "rig.jsonl");
    assert_eq!(bundle.manifest.files.propagation, "propagation.jsonl");
    assert_eq!(bundle.manifest.files.analysis, "analysis.json");
}
```

- [ ] **Step 3: Run the tests and confirm the crate is missing implementation**

Run:

```bash
cargo test -p antennabench-core --test model_serialization
```

Expected: FAIL with unresolved imports from `antennabench_core`.

- [ ] **Step 4: Add public exports**

Write `crates/core/src/lib.rs` with this complete content:

```rust
mod model;

pub use model::*;

pub const SCHEMA_VERSION: u16 = 1;
```

- [ ] **Step 5: Add the initial core model**

Write `crates/core/src/model.rs` with this complete content:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: u16,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub files: BundleFiles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleFiles {
    pub manifest: String,
    pub station: String,
    pub antennas: String,
    pub schedule: String,
    pub events: String,
    pub observations: String,
    pub wsjtx: String,
    pub rig: String,
    pub propagation: String,
    pub analysis: String,
    pub attachments_dir: String,
}

impl Default for BundleFiles {
    fn default() -> Self {
        Self {
            manifest: "manifest.json".to_string(),
            station: "station.json".to_string(),
            antennas: "antennas.json".to_string(),
            schedule: "schedule.json".to_string(),
            events: "events.jsonl".to_string(),
            observations: "observations.jsonl".to_string(),
            wsjtx: "wsjtx.jsonl".to_string(),
            rig: "rig.jsonl".to_string(),
            propagation: "propagation.jsonl".to_string(),
            analysis: "analysis.json".to_string(),
            attachments_dir: "attachments".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleContents {
    pub manifest: BundleManifest,
    pub station: Station,
    pub antennas: AntennasFile,
    pub schedule: Schedule,
    pub events: Vec<OperatorEvent>,
    pub observations: Vec<ObservationRecord>,
    pub wsjtx: Vec<WsjtXRecord>,
    pub rig: Vec<RigRecord>,
    pub propagation: Vec<PropagationRecord>,
    pub analysis: AnalysisFile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub schema_version: u16,
    pub session_id: String,
    pub callsign: String,
    pub grid: String,
    pub power_watts: Option<f32>,
    pub operator_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntennasFile {
    pub schema_version: u16,
    pub session_id: String,
    pub antennas: Vec<Antenna>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Antenna {
    pub label: String,
    pub facets: Vec<String>,
    pub height_m: Option<f32>,
    pub radial_count: Option<u16>,
    pub radial_length_m: Option<f32>,
    pub orientation_degrees: Option<u16>,
    pub tuner: Option<String>,
    pub feedline: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub schema_version: u16,
    pub session_id: String,
    pub mode: ExperimentMode,
    pub goal: SessionGoal,
    pub slots: Vec<PlannedSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentMode {
    WholeStationAb,
    TxFocused,
    RxFocused,
    SingleAntennaProfiling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionGoal {
    Dx,
    Regional,
    NvisLocal,
    GeneralCoverage,
    WeakSignalReliability,
    SingleAntennaProfiling,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedSlot {
    pub slot_id: String,
    pub sequence_number: u32,
    pub starts_at: DateTime<Utc>,
    pub duration_seconds: u32,
    pub guard_seconds: u32,
    pub band: Band,
    pub antenna_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Band {
    #[serde(rename = "160m")]
    M160,
    #[serde(rename = "80m")]
    M80,
    #[serde(rename = "60m")]
    M60,
    #[serde(rename = "40m")]
    M40,
    #[serde(rename = "30m")]
    M30,
    #[serde(rename = "20m")]
    M20,
    #[serde(rename = "17m")]
    M17,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "12m")]
    M12,
    #[serde(rename = "10m")]
    M10,
    #[serde(rename = "6m")]
    M6,
    #[serde(rename = "2m")]
    M2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordMeta {
    pub schema_version: u16,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: RecordSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordSource {
    Operator,
    WsjtxUdp,
    WsjtxLog,
    Wsprnet,
    WsprLive,
    ImportedFile,
    RigAdapter,
    NoaaSwpc,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorEvent {
    pub meta: RecordMeta,
    pub event_id: String,
    pub slot_id: Option<String>,
    pub event_type: OperatorEventType,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorEventType {
    SessionStarted,
    Switched,
    MissedSlot,
    BadSlot,
    NoteAdded,
    SessionEnded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub meta: RecordMeta,
    pub observation_id: String,
    pub observation_kind: ObservationKind,
    pub band: Band,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub reporter_call: Option<String>,
    pub heard_call: Option<String>,
    pub reporter_grid: Option<String>,
    pub heard_grid: Option<String>,
    pub distance_km: Option<f64>,
    pub azimuth_degrees: Option<f64>,
    pub snr_db: Option<f32>,
    pub drift_hz_per_minute: Option<f32>,
    pub power_watts: Option<f32>,
    pub slot_id: Option<String>,
    pub slot_label: Option<String>,
    pub slot_confidence: Option<f32>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    LocalDecode,
    PublicReport,
    ImportedSpot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsjtXRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub message_type: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub status: String,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropagationRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub observed_at: DateTime<Utc>,
    pub solar_flux_f107: Option<f32>,
    pub sunspot_number: Option<u16>,
    pub kp_index: Option<f32>,
    pub a_index: Option<u16>,
    pub solar_wind_speed_kms: Option<f32>,
    pub bz_nt: Option<f32>,
    pub alerts: Vec<String>,
    pub daylight_state: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFile {
    pub schema_version: u16,
    pub session_id: String,
    pub generated_at: Option<DateTime<Utc>>,
    pub status: AnalysisStatus,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    NotRun,
    Generated,
}
```

- [ ] **Step 6: Run core tests**

Run:

```bash
cargo test -p antennabench-core --test model_serialization
```

Expected: PASS.

- [ ] **Step 7: Format and commit**

Run:

```bash
cargo fmt
jj status
jj commit -m "feat: add core bundle model"
```

Expected: formatted Rust files and a described jj change.

## Task 3: Storage Bundle Round Trip

**Files:**
- Create: `crates/storage/Cargo.toml`
- Create: `crates/storage/src/lib.rs`
- Create: `crates/storage/tests/bundle_roundtrip.rs`
- Create: `Cargo.lock`

- [ ] **Step 1: Create the storage crate manifest**

Write `crates/storage/Cargo.toml` with this complete content:

```toml
[package]
name = "antennabench-storage"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
antennabench-core = { path = "../core" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
chrono = { workspace = true }
tempfile = { workspace = true }
```

- [ ] **Step 2: Write the failing storage round-trip test**

Write `crates/storage/tests/bundle_roundtrip.rs` with this complete content:

```rust
use antennabench_core::{
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents, BundleFiles,
    BundleManifest, ExperimentMode, ObservationKind, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, RecordMeta, RecordSource, Schedule, SessionGoal, Station,
    SCHEMA_VERSION,
};
use antennabench_storage::BundleStore;
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn writes_and_reads_bundle_directory_without_losing_records() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("example.session.wsprabundle");
    let original = sample_bundle();

    BundleStore::new(&bundle_path).write(&original).unwrap();
    let imported = BundleStore::new(&bundle_path).read().unwrap();

    assert_eq!(imported, original);
    assert!(bundle_path.join("manifest.json").is_file());
    assert!(bundle_path.join("station.json").is_file());
    assert!(bundle_path.join("antennas.json").is_file());
    assert!(bundle_path.join("schedule.json").is_file());
    assert!(bundle_path.join("events.jsonl").is_file());
    assert!(bundle_path.join("observations.jsonl").is_file());
    assert!(bundle_path.join("attachments").is_dir());
}

fn sample_bundle() -> BundleContents {
    let session_id = "session-2026-07-09-n1rwj-20m".to_string();
    let created_at = Utc.with_ymd_and_hms(2026, 7, 9, 19, 58, 0).unwrap();
    let slot_start = Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 0).unwrap();
    let event_time = Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 3).unwrap();
    let observation_time = Utc.with_ymd_and_hms(2026, 7, 9, 20, 1, 11).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            created_at,
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: Some("Round-trip test".to_string()),
        },
        antennas: AntennasFile {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            antennas: vec![
                Antenna {
                    label: "A".to_string(),
                    facets: vec!["vertical".to_string()],
                    height_m: Some(7.0),
                    radial_count: Some(16),
                    radial_length_m: Some(5.0),
                    orientation_degrees: None,
                    tuner: Some("manual".to_string()),
                    feedline: Some("RG-8X".to_string()),
                    notes: Some("Temporary vertical".to_string()),
                },
                Antenna {
                    label: "B".to_string(),
                    facets: vec!["dipole".to_string()],
                    height_m: Some(9.0),
                    radial_count: None,
                    radial_length_m: None,
                    orientation_degrees: Some(70),
                    tuner: None,
                    feedline: Some("RG-58".to_string()),
                    notes: Some("Inverted vee".to_string()),
                },
            ],
        },
        schedule: Schedule {
            schema_version: SCHEMA_VERSION,
            session_id: session_id.clone(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: vec![
                PlannedSlot {
                    slot_id: "slot-001".to_string(),
                    sequence_number: 1,
                    starts_at: slot_start,
                    duration_seconds: 120,
                    guard_seconds: 15,
                    band: Band::M20,
                    antenna_label: "A".to_string(),
                },
                PlannedSlot {
                    slot_id: "slot-002".to_string(),
                    sequence_number: 2,
                    starts_at: Utc.with_ymd_and_hms(2026, 7, 9, 20, 2, 0).unwrap(),
                    duration_seconds: 120,
                    guard_seconds: 15,
                    band: Band::M20,
                    antenna_label: "B".to_string(),
                },
            ],
        },
        events: vec![OperatorEvent {
            meta: RecordMeta {
                schema_version: SCHEMA_VERSION,
                session_id: session_id.clone(),
                timestamp: event_time,
                source: RecordSource::Operator,
            },
            event_id: "event-001".to_string(),
            slot_id: Some("slot-001".to_string()),
            event_type: OperatorEventType::Switched,
            note: Some("A connected".to_string()),
        }],
        observations: vec![ObservationRecord {
            meta: RecordMeta {
                schema_version: SCHEMA_VERSION,
                session_id: session_id.clone(),
                timestamp: observation_time,
                source: RecordSource::WsjtxLog,
            },
            observation_id: "obs-001".to_string(),
            observation_kind: ObservationKind::LocalDecode,
            band: Band::M20,
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".to_string()),
            reporter_call: Some("N1RWJ".to_string()),
            heard_call: Some("K1ABC".to_string()),
            reporter_grid: Some("FN42".to_string()),
            heard_grid: Some("EM12".to_string()),
            distance_km: Some(2500.0),
            azimuth_degrees: Some(250.0),
            snr_db: Some(-18.0),
            drift_hz_per_minute: Some(0.0),
            power_watts: Some(5.0),
            slot_id: Some("slot-001".to_string()),
            slot_label: Some("A".to_string()),
            slot_confidence: Some(0.95),
            raw: json!({"line": "example wsjtx decode"}),
        }],
        wsjtx: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION,
            session_id,
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: vec!["analysis engine not part of the first slice".to_string()],
        },
    }
}
```

- [ ] **Step 3: Run the test and confirm storage API is missing**

Run:

```bash
cargo test -p antennabench-storage --test bundle_roundtrip
```

Expected: FAIL with unresolved import or missing `BundleStore`.

- [ ] **Step 4: Implement bundle read/write**

Write `crates/storage/src/lib.rs` with this complete content:

```rust
use antennabench_core::{
    AnalysisFile, AntennasFile, BundleContents, BundleFiles, BundleManifest, ObservationRecord,
    OperatorEvent, PropagationRecord, RigRecord, Schedule, Station, WsjtXRecord,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct BundleStore {
    root: PathBuf,
}

impl BundleStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, bundle: &BundleContents) -> Result<(), BundleStoreError> {
        fs::create_dir_all(&self.root).map_err(|source| BundleStoreError::CreateDirectory {
            path: self.root.clone(),
            source,
        })?;

        let files = &bundle.manifest.files;
        write_json(&self.root.join(files.manifest.as_str()), &bundle.manifest)?;
        write_json(&self.root.join(files.station.as_str()), &bundle.station)?;
        write_json(&self.root.join(files.antennas.as_str()), &bundle.antennas)?;
        write_json(&self.root.join(files.schedule.as_str()), &bundle.schedule)?;
        write_jsonl(&self.root.join(files.events.as_str()), &bundle.events)?;
        write_jsonl(&self.root.join(files.observations.as_str()), &bundle.observations)?;
        write_jsonl(&self.root.join(files.wsjtx.as_str()), &bundle.wsjtx)?;
        write_jsonl(&self.root.join(files.rig.as_str()), &bundle.rig)?;
        write_jsonl(&self.root.join(files.propagation.as_str()), &bundle.propagation)?;
        write_json(&self.root.join(files.analysis.as_str()), &bundle.analysis)?;

        fs::create_dir_all(self.root.join(files.attachments_dir.as_str())).map_err(|source| {
            BundleStoreError::CreateDirectory {
                path: self.root.join(files.attachments_dir.as_str()),
                source,
            }
        })?;

        Ok(())
    }

    pub fn read(&self) -> Result<BundleContents, BundleStoreError> {
        let default_files = BundleFiles::default();
        let manifest: BundleManifest = read_json(&self.root.join(default_files.manifest.as_str()))?;
        let files = &manifest.files;

        Ok(BundleContents {
            station: read_json(&self.root.join(files.station.as_str()))?,
            antennas: read_json(&self.root.join(files.antennas.as_str()))?,
            schedule: read_json(&self.root.join(files.schedule.as_str()))?,
            events: read_jsonl(&self.root.join(files.events.as_str()))?,
            observations: read_jsonl(&self.root.join(files.observations.as_str()))?,
            wsjtx: read_jsonl(&self.root.join(files.wsjtx.as_str()))?,
            rig: read_jsonl(&self.root.join(files.rig.as_str()))?,
            propagation: read_jsonl(&self.root.join(files.propagation.as_str()))?,
            analysis: read_json(&self.root.join(files.analysis.as_str()))?,
            manifest,
        })
    }
}

#[derive(Debug, Error)]
pub enum BundleStoreError {
    #[error("failed to create directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}")]
    ParseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize JSON for {path}")]
    SerializeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, BundleStoreError> {
    let text = fs::read_to_string(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&text).map_err(|source| BundleStoreError::ParseJson {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), BundleStoreError> {
    let text =
        serde_json::to_string_pretty(value).map_err(|source| BundleStoreError::SerializeJson {
            path: path.to_path_buf(),
            source,
        })?;
    fs::write(path, format!("{text}\n")).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, BundleStoreError> {
    let text = fs::read_to_string(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|source| BundleStoreError::ParseJson {
                path: path.to_path_buf(),
                source,
            })
        })
        .collect()
}

fn write_jsonl<T: Serialize>(path: &Path, values: &[T]) -> Result<(), BundleStoreError> {
    let mut text = String::new();
    for value in values {
        let line =
            serde_json::to_string(value).map_err(|source| BundleStoreError::SerializeJson {
                path: path.to_path_buf(),
                source,
            })?;
        text.push_str(&line);
        text.push('\n');
    }

    fs::write(path, text).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[allow(dead_code)]
fn _type_check_bundle_files(
    _station: Station,
    _antennas: AntennasFile,
    _schedule: Schedule,
    _events: Vec<OperatorEvent>,
    _observations: Vec<ObservationRecord>,
    _wsjtx: Vec<WsjtXRecord>,
    _rig: Vec<RigRecord>,
    _propagation: Vec<PropagationRecord>,
    _analysis: AnalysisFile,
) {
}
```

- [ ] **Step 5: Run storage round-trip test**

Run:

```bash
cargo test -p antennabench-storage --test bundle_roundtrip
```

Expected: PASS.

- [ ] **Step 6: Run all tests and commit**

Run:

```bash
cargo test
cargo fmt
jj status
jj commit -m "feat: add bundle storage round trip"
```

Expected: all tests pass, `Cargo.lock` is present from dependency resolution, and a described jj change is created.

## Task 4: Golden Bundle Fixture

**Files:**
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/manifest.json`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/station.json`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/antennas.json`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/schedule.json`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/events.jsonl`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/observations.jsonl`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/wsjtx.jsonl`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/rig.jsonl`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/propagation.jsonl`
- Create: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/analysis.json`
- Create directory: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/attachments`
- Create: `crates/storage/tests/golden_bundle.rs`

- [ ] **Step 1: Add `manifest.json`**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/manifest.json` with this complete content:

```json
{
  "schema_version": 1,
  "session_id": "session-2026-07-09-n1rwj-20m",
  "created_at": "2026-07-09T19:58:00Z",
  "app_version": "0.1.0",
  "files": {
    "manifest": "manifest.json",
    "station": "station.json",
    "antennas": "antennas.json",
    "schedule": "schedule.json",
    "events": "events.jsonl",
    "observations": "observations.jsonl",
    "wsjtx": "wsjtx.jsonl",
    "rig": "rig.jsonl",
    "propagation": "propagation.jsonl",
    "analysis": "analysis.json",
    "attachments_dir": "attachments"
  }
}
```

- [ ] **Step 2: Add `station.json`**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/station.json` with this complete content:

```json
{
  "schema_version": 1,
  "session_id": "session-2026-07-09-n1rwj-20m",
  "callsign": "N1RWJ",
  "grid": "FN42",
  "power_watts": 5.0,
  "operator_notes": "Minimal whole-station A/B fixture"
}
```

- [ ] **Step 3: Add `antennas.json`**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/antennas.json` with this complete content:

```json
{
  "schema_version": 1,
  "session_id": "session-2026-07-09-n1rwj-20m",
  "antennas": [
    {
      "label": "A",
      "facets": ["vertical"],
      "height_m": 7.0,
      "radial_count": 16,
      "radial_length_m": 5.0,
      "orientation_degrees": null,
      "tuner": "manual",
      "feedline": "RG-8X",
      "notes": "Temporary ground-mounted vertical"
    },
    {
      "label": "B",
      "facets": ["dipole"],
      "height_m": 9.0,
      "radial_count": null,
      "radial_length_m": null,
      "orientation_degrees": 70,
      "tuner": null,
      "feedline": "RG-58",
      "notes": "Inverted vee"
    }
  ]
}
```

- [ ] **Step 4: Add `schedule.json`**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/schedule.json` with this complete content:

```json
{
  "schema_version": 1,
  "session_id": "session-2026-07-09-n1rwj-20m",
  "mode": "whole_station_ab",
  "goal": "general_coverage",
  "slots": [
    {
      "slot_id": "slot-001",
      "sequence_number": 1,
      "starts_at": "2026-07-09T20:00:00Z",
      "duration_seconds": 120,
      "guard_seconds": 15,
      "band": "20m",
      "antenna_label": "A"
    },
    {
      "slot_id": "slot-002",
      "sequence_number": 2,
      "starts_at": "2026-07-09T20:02:00Z",
      "duration_seconds": 120,
      "guard_seconds": 15,
      "band": "20m",
      "antenna_label": "B"
    }
  ]
}
```

- [ ] **Step 5: Add append-style stream files**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/events.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:00:03Z","source":"operator"},"event_id":"event-001","slot_id":"slot-001","event_type":"switched","note":"A connected"}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:02:04Z","source":"operator"},"event_id":"event-002","slot_id":"slot-002","event_type":"switched","note":"B connected"}
```

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/observations.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:01:11Z","source":"wsjtx_log"},"observation_id":"obs-001","observation_kind":"local_decode","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"N1RWJ","heard_call":"K1ABC","reporter_grid":"FN42","heard_grid":"EM12","distance_km":2500.0,"azimuth_degrees":250.0,"snr_db":-18.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-001","slot_label":"A","slot_confidence":0.95,"raw":{"line":"example local decode"}}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:03:10Z","source":"wsprnet"},"observation_id":"obs-002","observation_kind":"public_report","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"K9XYZ","heard_call":"N1RWJ","reporter_grid":"EN52","heard_grid":"FN42","distance_km":1350.0,"azimuth_degrees":276.0,"snr_db":-21.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-002","slot_label":"B","slot_confidence":0.9,"raw":{"source":"fixture public report"}}
```

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/wsjtx.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T19:59:55Z","source":"wsjtx_log"},"record_id":"wsjtx-001","message_type":"status_snapshot","raw":{"mode":"WSPR","dial_frequency_hz":14095600}}
```

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/rig.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T19:59:56Z","source":"rig_adapter"},"record_id":"rig-001","status":"manual_confirmation","frequency_hz":14095600,"mode":"WSPR","raw":{"operator_confirmed":true}}
```

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/propagation.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:00:00Z","source":"imported_file"},"record_id":"prop-001","observed_at":"2026-07-09T20:00:00Z","solar_flux_f107":145.0,"sunspot_number":88,"kp_index":2.0,"a_index":8,"solar_wind_speed_kms":410.0,"bz_nt":-1.2,"alerts":[],"daylight_state":"mixed_path","raw":{"fixture":true}}
```

- [ ] **Step 6: Add `analysis.json` and attachments directory**

Write `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/analysis.json` with this complete content:

```json
{
  "schema_version": 1,
  "session_id": "session-2026-07-09-n1rwj-20m",
  "generated_at": null,
  "status": "not_run",
  "notes": ["analysis engine not part of the first slice"]
}
```

Create the attachments directory:

```bash
mkdir -p fixtures/session-bundles/minimal-whole-station.session.wsprabundle/attachments
```

- [ ] **Step 7: Write the failing golden fixture test**

Write `crates/storage/tests/golden_bundle.rs` with this complete content:

```rust
use antennabench_core::{AnalysisStatus, ExperimentMode, ObservationKind, RecordSource};
use antennabench_storage::BundleStore;

#[test]
fn imports_minimal_whole_station_fixture() {
    let fixture = fixture_path();
    let bundle = BundleStore::new(&fixture).read().unwrap();

    assert_eq!(bundle.manifest.schema_version, 1);
    assert_eq!(bundle.manifest.session_id, "session-2026-07-09-n1rwj-20m");
    assert_eq!(bundle.station.callsign, "N1RWJ");
    assert_eq!(bundle.station.grid, "FN42");
    assert_eq!(bundle.antennas.antennas.len(), 2);
    assert_eq!(bundle.schedule.mode, ExperimentMode::WholeStationAb);
    assert_eq!(bundle.schedule.slots.len(), 2);
    assert_eq!(bundle.events.len(), 2);
    assert_eq!(bundle.observations.len(), 2);
    assert_eq!(bundle.observations[0].observation_kind, ObservationKind::LocalDecode);
    assert_eq!(bundle.observations[1].meta.source, RecordSource::Wsprnet);
    assert_eq!(bundle.wsjtx.len(), 1);
    assert_eq!(bundle.rig.len(), 1);
    assert_eq!(bundle.propagation.len(), 1);
    assert_eq!(bundle.analysis.status, AnalysisStatus::NotRun);
}

#[test]
fn exports_imported_fixture_without_changing_data_model() {
    let fixture = fixture_path();
    let bundle = BundleStore::new(&fixture).read().unwrap();
    let tempdir = tempfile::tempdir().unwrap();
    let exported = tempdir.path().join("exported.session.wsprabundle");

    BundleStore::new(&exported).write(&bundle).unwrap();
    let reimported = BundleStore::new(&exported).read().unwrap();

    assert_eq!(reimported, bundle);
}

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle")
}
```

- [ ] **Step 8: Run golden fixture tests**

Run:

```bash
cargo test -p antennabench-storage --test golden_bundle
```

Expected: PASS after the fixture files and test are present.

- [ ] **Step 9: Commit the fixture**

Run:

```bash
cargo test
cargo fmt
jj status
jj commit -m "test: add minimal whole-station bundle fixture"
```

Expected: all tests pass and a described jj change is created.

## Task 5: Schema Guardrails and Error Coverage

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/tests/bundle_errors.rs`

- [ ] **Step 1: Write failing tests for missing files and invalid JSONL**

Write `crates/storage/tests/bundle_errors.rs` with this complete content:

```rust
use antennabench_storage::{BundleStore, BundleStoreError};

#[test]
fn missing_manifest_returns_read_error_with_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("missing.session.wsprabundle");
    std::fs::create_dir_all(&bundle_path).unwrap();

    let error = BundleStore::new(&bundle_path).read().unwrap_err();

    match error {
        BundleStoreError::Read { path, .. } => {
            assert!(path.ends_with("manifest.json"));
        }
        other => panic!("expected read error, got {other:?}"),
    }
}

#[test]
fn invalid_jsonl_returns_parse_error_with_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("invalid-jsonl.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    std::fs::write(
        bundle_path.join("manifest.json"),
        r#"{
          "schema_version": 1,
          "session_id": "session-invalid-jsonl",
          "created_at": "2026-07-09T19:58:00Z",
          "app_version": "0.1.0",
          "files": {
            "manifest": "manifest.json",
            "station": "station.json",
            "antennas": "antennas.json",
            "schedule": "schedule.json",
            "events": "events.jsonl",
            "observations": "observations.jsonl",
            "wsjtx": "wsjtx.jsonl",
            "rig": "rig.jsonl",
            "propagation": "propagation.jsonl",
            "analysis": "analysis.json",
            "attachments_dir": "attachments"
          }
        }"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("station.json"),
        r#"{"schema_version":1,"session_id":"session-invalid-jsonl","callsign":"N1RWJ","grid":"FN42","power_watts":5.0,"operator_notes":null}"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("antennas.json"),
        r#"{"schema_version":1,"session_id":"session-invalid-jsonl","antennas":[]}"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("schedule.json"),
        r#"{"schema_version":1,"session_id":"session-invalid-jsonl","mode":"whole_station_ab","goal":"general_coverage","slots":[]}"#,
    )
    .unwrap();
    std::fs::write(bundle_path.join("events.jsonl"), "{not valid json}\n").unwrap();
    std::fs::write(bundle_path.join("observations.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("wsjtx.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("rig.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("propagation.jsonl"), "").unwrap();
    std::fs::write(
        bundle_path.join("analysis.json"),
        r#"{"schema_version":1,"session_id":"session-invalid-jsonl","generated_at":null,"status":"not_run","notes":[]}"#,
    )
    .unwrap();

    let error = BundleStore::new(&bundle_path).read().unwrap_err();

    match error {
        BundleStoreError::ParseJson { path, .. } => {
            assert!(path.ends_with("events.jsonl"));
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run the error tests**

Run:

```bash
cargo test -p antennabench-storage --test bundle_errors
```

Expected: PASS with the public `BundleStoreError` enum from Task 3.

- [ ] **Step 3: Remove the private type-check helper if no longer needed**

In `crates/storage/src/lib.rs`, replace the opening import block with this complete import block:

```rust
use antennabench_core::{BundleContents, BundleFiles, BundleManifest};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
```

Then delete this complete helper function from the bottom of `crates/storage/src/lib.rs`:

```rust
#[allow(dead_code)]
fn _type_check_bundle_files(
    _station: Station,
    _antennas: AntennasFile,
    _schedule: Schedule,
    _events: Vec<OperatorEvent>,
    _observations: Vec<ObservationRecord>,
    _wsjtx: Vec<WsjtXRecord>,
    _rig: Vec<RigRecord>,
    _propagation: Vec<PropagationRecord>,
    _analysis: AnalysisFile,
) {
}
```

The file should still compile because all storage behavior is covered by public tests.

- [ ] **Step 4: Run full verification and commit**

Run:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
jj status
jj commit -m "test: cover bundle storage errors"
```

Expected: tests and clippy pass, formatting is stable, and a described jj change is created.

## Task 6: Final Verification

**Files:**
- Modify only if verification exposes a concrete issue in files created by Tasks 1-5.

- [ ] **Step 1: Confirm the plan’s first slice commands pass**

Run:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
jj status
```

Expected:

- `cargo test` passes all tests.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo fmt --check` passes.
- `jj status` shows no uncommitted file changes in the current working-copy commit after the last task commit.

- [ ] **Step 2: Inspect the jj history**

Run:

```bash
jj log --limit 8
```

Expected: the recent history shows the task commits from this plan above the original design-spec commit.

## Self-Review

**Spec coverage checked**

- Local-first durable bundle source of truth: covered by `BundleContents`, JSON/JSONL files, and storage round-trip tests.
- Proposed bundle shape: covered by `BundleFiles::default()` and the golden `.session.wsprabundle` fixture.
- Minimum data for A/B comparison: covered by station, antennas, planned slots, operator events, and observations.
- Observation record fields: covered by `ObservationRecord` and fixture lines containing band, frequency, mode, reporter/heard callsigns, grids, distance, azimuth, SNR, drift, power, slot label/confidence, source, and raw data.
- Propagation time-scoped snapshots: covered by `PropagationRecord` and `propagation.jsonl`.
- Append-style event records: covered by JSONL event stream tests and storage helpers.
- Rebuildable derived layers: SQLite, reports, and charts are not created in this slice; the README and architecture keep them derived from bundle data.
- Explicitly out of this slice: Tauri UI, Cloudflare hosting, WSJT-X UDP parsing, Hamlib, public spot fetching, analysis statistics, and report charts.

**Vague-step scan completed**

- Each file creation step includes complete file content.
- Each verification step includes exact commands and expected outcomes.
- Function, type, and file names are consistent across `crates/core`, `crates/storage`, and tests.

**Type and path consistency checked**

- Workspace members are discovered with `crates/*`, allowing `crates/core` and `crates/storage` to be added incrementally.
- Package names are `antennabench-core` and `antennabench-storage`; Rust import names are `antennabench_core` and `antennabench_storage`.
- `BundleFiles` names match the fixture files and storage read/write paths.
- Record enum serde names match fixture JSON values: `whole_station_ab`, `general_coverage`, `operator`, `wsjtx_log`, `wsprnet`, `rig_adapter`, `imported_file`, `local_decode`, `public_report`, and `not_run`.
- Golden fixture path in `golden_bundle.rs` resolves from `crates/storage` to `fixtures/session-bundles/minimal-whole-station.session.wsprabundle`.
