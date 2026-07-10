# Bundle Validation + Alignment Consistency Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic bundle validation that catches schema/session drift, duplicate IDs, invalid references, invalid slot windows, invalid slot confidence values, and stale persisted alignment annotations.

**Architecture:** Put validation policy and typed validation errors in `crates/core` so every future caller can validate the canonical bundle model without filesystem concerns. Keep `crates/storage` responsible for JSON/JSONL bundle I/O, and add a small `BundleStore::read_validated()` convenience method that reads a bundle and delegates to core validation.

**Tech Stack:** Rust workspace, `chrono`, `serde`, `thiserror` typed library errors, existing core alignment module, `insta` inline snapshots, fixture-driven Cargo tests, jj workflows.

---

## Scope

This slice creates working, testable software for:

- Validating `schema_version == SCHEMA_VERSION` across bundle root files and JSONL record metadata.
- Validating every root file and JSONL record uses `manifest.session_id`.
- Validating unique planned slot IDs, operator event IDs, observation IDs, WSJT-X record IDs, rig record IDs, and propagation record IDs.
- Validating planned slot `antenna_label` values exist in `antennas.json`.
- Validating planned slot windows are sorted by `starts_at` and non-overlapping using half-open windows.
- Validating operator events with `slot_id` reference existing planned slots.
- Validating observations with `slot_id` reference existing planned slots.
- Validating persisted `slot_confidence` values are in `0.0..=1.0`.
- Regenerating alignment from `schedule + events + observations` and validating persisted observation `slot_id`, `slot_label`, and `slot_confidence` match regenerated annotations.
- Adding storage-level convenience API `BundleStore::read_validated()` that preserves `BundleStore::read()` as parse-only I/O.
- Keeping the existing golden fixture as a passing validated bundle.

This slice does not add Tauri UI, WSJT-X UDP/log import, Hamlib, public spot fetching, SQLite indexing, report charts, hosted publishing, or statistical analysis.

## Existing Model Changes

No durable bundle schema changes are required.

Use existing model fields:

- `BundleManifest.schema_version`, `BundleManifest.session_id`
- `Station.schema_version`, `Station.session_id`
- `AntennasFile.schema_version`, `AntennasFile.session_id`, `Antenna.label`
- `Schedule.schema_version`, `Schedule.session_id`, `Schedule.slots`
- `AnalysisFile.schema_version`, `AnalysisFile.session_id`
- `RecordMeta.schema_version`, `RecordMeta.session_id`
- `OperatorEvent.event_id`, `OperatorEvent.slot_id`
- `ObservationRecord.observation_id`, `ObservationRecord.slot_id`, `ObservationRecord.slot_label`, `ObservationRecord.slot_confidence`
- `WsjtXRecord.record_id`, `RigRecord.record_id`, `PropagationRecord.record_id`

Add non-durable exported core types only:

- `BundleValidationError`
- `BundleValidationIssue`
- `BundleFileRole`
- `BundleIdKind`
- `AlignmentAnnotationField`
- `validate_bundle`

## File Structure

- Modify `crates/core/Cargo.toml`: add `thiserror.workspace = true`.
- Modify `crates/core/src/lib.rs`: export the new validation module.
- Create `crates/core/src/validation.rs`: validation API, typed errors, invariant checks, and alignment annotation comparison.
- Create `crates/core/tests/bundle_validation.rs`: deterministic validation tests with inline snapshots.
- Modify `crates/storage/src/lib.rs`: import `validate_bundle`, add `BundleStore::read_validated()`, and add `BundleStoreError::Validation`.
- Modify `crates/storage/tests/golden_bundle.rs`: assert the golden fixture passes `read_validated()`.
- Modify `crates/storage/tests/bundle_errors.rs`: assert invalid-but-parseable bundles surface validation errors through storage.

## Validation Semantics

- Validation should collect all issues in deterministic order and return one `BundleValidationError` containing a non-empty `Vec<BundleValidationIssue>`.
- `validate_bundle(&bundle)` should return `Ok(())` only when no issues are found.
- Root file session checks should use `bundle.manifest.session_id` as the expected session id.
- Schema checks should use `SCHEMA_VERSION` as the expected schema version.
- Slot window ordering should inspect `schedule.slots` in declared order. A slot is out of order when its `starts_at` is earlier than the previous slot's `starts_at`.
- Slot overlap should use half-open windows: `[starts_at, starts_at + duration_seconds)`.
- Duplicate ID validation should report the second and subsequent occurrences.
- Alignment consistency should call `align_schedule_slots(..., SlotAlignmentPolicy::default())`, then `apply_slot_assignments(...)`, and compare each observation by list order. This matches the current alignment fixture behavior.
- `slot_confidence` comparison should use a tiny tolerance, `0.000_001`, to avoid false failures from decimal JSON parsing.

## Task 1: Core Validation Error Types and Public API

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/src/validation.rs`
- Create: `crates/core/tests/bundle_validation.rs`

- [ ] **Step 1: Add the core error dependency**

Change `crates/core/Cargo.toml` so `[dependencies]` contains:

```toml
[dependencies]
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

- [ ] **Step 2: Write the failing validation API test**

Create `crates/core/tests/bundle_validation.rs` with this initial test and helper imports:

```rust
use antennabench_core::{
    validate_bundle, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents,
    BundleFiles, BundleManifest, BundleValidationIssue, ExperimentMode, ObservationKind,
    ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot, PropagationRecord,
    RecordMeta, RecordSource, RigRecord, Schedule, SessionGoal, Station, WsjtXRecord,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-validation-test";

#[test]
fn accepts_a_valid_bundle() {
    let bundle = valid_bundle();

    validate_bundle(&bundle).unwrap();
}
```

Add the `valid_bundle()` helper from Step 5 before running the test.

- [ ] **Step 3: Run the failing validation API test**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation accepts_a_valid_bundle
```

Expected: FAIL with unresolved import `validate_bundle`.

- [ ] **Step 4: Export the validation module**

Change `crates/core/src/lib.rs` to:

```rust
pub mod alignment;
mod model;
mod validation;

pub use alignment::*;
pub use model::*;
pub use validation::*;

pub const SCHEMA_VERSION: u16 = 1;
```

- [ ] **Step 5: Create validation types and a stub implementation**

Create `crates/core/src/validation.rs`:

```rust
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::BundleContents;

#[derive(Debug, Error, Clone, PartialEq)]
#[error("bundle validation failed with one or more issues")]
pub struct BundleValidationError {
    issues: Vec<BundleValidationIssue>,
}

impl BundleValidationError {
    pub fn new(issues: Vec<BundleValidationIssue>) -> Self {
        assert!(
            !issues.is_empty(),
            "BundleValidationError requires at least one issue"
        );
        Self { issues }
    }

    pub fn issues(&self) -> &[BundleValidationIssue] {
        &self.issues
    }

    pub fn into_issues(self) -> Vec<BundleValidationIssue> {
        self.issues
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BundleValidationIssue {
    UnexpectedSchemaVersion {
        file: BundleFileRole,
        record_id: Option<String>,
        expected: u16,
        actual: u16,
    },
    SessionIdMismatch {
        file: BundleFileRole,
        record_id: Option<String>,
        expected: String,
        actual: String,
    },
    DuplicateId {
        kind: BundleIdKind,
        id: String,
    },
    UnknownAntennaLabel {
        slot_id: String,
        antenna_label: String,
    },
    SlotWindowOutOfOrder {
        previous_slot_id: String,
        slot_id: String,
    },
    SlotWindowOverlap {
        previous_slot_id: String,
        previous_ends_at: DateTime<Utc>,
        slot_id: String,
        starts_at: DateTime<Utc>,
    },
    UnknownEventSlot {
        event_id: String,
        slot_id: String,
    },
    UnknownObservationSlot {
        observation_id: String,
        slot_id: String,
    },
    InvalidSlotConfidence {
        observation_id: String,
        slot_confidence: f32,
    },
    AlignmentAnnotationMismatch {
        observation_id: String,
        field: AlignmentAnnotationField,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleFileRole {
    Manifest,
    Station,
    Antennas,
    Schedule,
    Events,
    Observations,
    WsjtX,
    Rig,
    Propagation,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleIdKind {
    Slot,
    OperatorEvent,
    Observation,
    WsjtXRecord,
    RigRecord,
    PropagationRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentAnnotationField {
    SlotId,
    SlotLabel,
    SlotConfidence,
}

pub fn validate_bundle(bundle: &BundleContents) -> Result<(), BundleValidationError> {
    let issues = validate_bundle_issues(bundle);

    if issues.is_empty() {
        Ok(())
    } else {
        Err(BundleValidationError::new(issues))
    }
}

fn validate_bundle_issues(_bundle: &BundleContents) -> Vec<BundleValidationIssue> {
    Vec::new()
}
```

Append this helper to `crates/core/tests/bundle_validation.rs`:

```rust
fn valid_bundle() -> BundleContents {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 7, 10, 19, 58, 0).unwrap(),
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            antennas: vec![
                antenna("A", "vertical"),
                antenna("B", "dipole"),
            ],
        },
        schedule: Schedule {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: vec![
                planned_slot("slot-001", 1, starts_at, "A"),
                planned_slot("slot-002", 2, starts_at + chrono::Duration::seconds(120), "B"),
            ],
        },
        events: vec![
            operator_event(
                "event-001",
                "slot-001",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(3),
            ),
            operator_event(
                "event-002",
                "slot-002",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(123),
            ),
        ],
        observations: vec![
            observation(
                "obs-001",
                starts_at + chrono::Duration::seconds(60),
                Some("slot-001"),
                Some("A"),
                Some(0.95),
            ),
            observation(
                "obs-002",
                starts_at + chrono::Duration::seconds(180),
                Some("slot-002"),
                Some("B"),
                Some(0.95),
            ),
        ],
        wsjtx: vec![WsjtXRecord {
            meta: record_meta(starts_at - chrono::Duration::seconds(5), RecordSource::WsjtxLog),
            record_id: "wsjtx-001".to_string(),
            message_type: "status_snapshot".to_string(),
            raw: json!({"mode": "WSPR"}),
        }],
        rig: vec![RigRecord {
            meta: record_meta(starts_at - chrono::Duration::seconds(4), RecordSource::RigAdapter),
            record_id: "rig-001".to_string(),
            status: "manual_confirmation".to_string(),
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".to_string()),
            raw: json!({"operator_confirmed": true}),
        }],
        propagation: vec![PropagationRecord {
            meta: record_meta(starts_at, RecordSource::ImportedFile),
            record_id: "prop-001".to_string(),
            observed_at: starts_at,
            solar_flux_f107: Some(145.0),
            sunspot_number: Some(88),
            kp_index: Some(2.0),
            a_index: Some(8),
            solar_wind_speed_kms: None,
            bz_nt: None,
            alerts: Vec::new(),
            daylight_state: Some("mixed_path".to_string()),
            raw: json!({"fixture": true}),
        }],
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    }
}
```

Add the helper constructors below `valid_bundle()`:

```rust
fn antenna(label: &str, facet: &str) -> Antenna {
    Antenna {
        label: label.to_string(),
        facets: vec![facet.to_string()],
        height_m: None,
        radial_count: None,
        radial_length_m: None,
        orientation_degrees: None,
        tuner: None,
        feedline: None,
        notes: None,
    }
}

fn planned_slot(
    slot_id: &str,
    sequence_number: u32,
    starts_at: chrono::DateTime<Utc>,
    antenna_label: &str,
) -> PlannedSlot {
    PlannedSlot {
        slot_id: slot_id.to_string(),
        sequence_number,
        starts_at,
        duration_seconds: 120,
        guard_seconds: 15,
        band: Band::M20,
        antenna_label: antenna_label.to_string(),
    }
}

fn operator_event(
    event_id: &str,
    slot_id: &str,
    event_type: OperatorEventType,
    timestamp: chrono::DateTime<Utc>,
) -> OperatorEvent {
    OperatorEvent {
        meta: record_meta(timestamp, RecordSource::Operator),
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type,
        note: None,
    }
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    slot_id: Option<&str>,
    slot_label: Option<&str>,
    slot_confidence: Option<f32>,
) -> ObservationRecord {
    ObservationRecord {
        meta: record_meta(timestamp, RecordSource::Wsprnet),
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::PublicReport,
        band: Band::M20,
        frequency_hz: Some(14_095_600),
        mode: Some("WSPR".to_string()),
        reporter_call: Some("K1ABC".to_string()),
        heard_call: Some("N1RWJ".to_string()),
        reporter_grid: Some("FN31".to_string()),
        heard_grid: Some("FN42".to_string()),
        distance_km: Some(150.0),
        azimuth_degrees: Some(240.0),
        snr_db: Some(-18.0),
        drift_hz_per_minute: Some(0.0),
        power_watts: Some(5.0),
        slot_id: slot_id.map(str::to_string),
        slot_label: slot_label.map(str::to_string),
        slot_confidence,
        raw: json!({}),
    }
}

fn record_meta(timestamp: chrono::DateTime<Utc>, source: RecordSource) -> RecordMeta {
    RecordMeta {
        schema_version: 1,
        session_id: SESSION_ID.to_string(),
        timestamp,
        source,
    }
}
```

- [ ] **Step 6: Run the validation API test**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation accepts_a_valid_bundle
```

Expected: PASS.

- [ ] **Step 7: Commit the API skeleton**

Run:

```bash
jj status
jj commit -m "test: add core bundle validation API skeleton"
```

Expected: a described jj change containing only the core manifest, core lib export, validation module, and initial validation test.

## Task 2: Schema, Session, ID, Reference, Window, and Confidence Validation

**Files:**
- Modify: `crates/core/src/validation.rs`
- Modify: `crates/core/tests/bundle_validation.rs`

- [ ] **Step 1: Write failing tests for structural validation issues**

Append these tests to `crates/core/tests/bundle_validation.rs`:

```rust
#[test]
fn reports_schema_version_and_session_id_mismatches() {
    let mut bundle = valid_bundle();
    bundle.station.schema_version = 2;
    bundle.schedule.session_id = "other-session".to_string();
    bundle.events[0].meta.schema_version = 2;
    bundle.observations[0].meta.session_id = "other-session".to_string();
    bundle.wsjtx[0].meta.session_id = "other-session".to_string();
    bundle.analysis.schema_version = 2;

    let error = validate_bundle(&bundle).unwrap_err();
    let non_alignment_issues = error
        .issues()
        .iter()
        .filter(|issue| !matches!(issue, BundleValidationIssue::AlignmentAnnotationMismatch { .. }))
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(
        non_alignment_issues,
        @r###"
        [
            UnexpectedSchemaVersion {
                file: Station,
                record_id: None,
                expected: 1,
                actual: 2,
            },
            SessionIdMismatch {
                file: Schedule,
                record_id: None,
                expected: "session-validation-test",
                actual: "other-session",
            },
            UnexpectedSchemaVersion {
                file: Events,
                record_id: Some(
                    "event-001",
                ),
                expected: 1,
                actual: 2,
            },
            SessionIdMismatch {
                file: Observations,
                record_id: Some(
                    "obs-001",
                ),
                expected: "session-validation-test",
                actual: "other-session",
            },
            SessionIdMismatch {
                file: WsjtX,
                record_id: Some(
                    "wsjtx-001",
                ),
                expected: "session-validation-test",
                actual: "other-session",
            },
            UnexpectedSchemaVersion {
                file: Analysis,
                record_id: None,
                expected: 1,
                actual: 2,
            },
        ]
        "###
    );
}

#[test]
fn reports_duplicate_ids_unknown_references_bad_windows_and_invalid_confidence() {
    let mut bundle = valid_bundle();
    let starts_at = bundle.schedule.slots[0].starts_at;
    bundle.schedule.slots.push(planned_slot(
        "slot-002",
        3,
        starts_at + chrono::Duration::seconds(60),
        "missing-antenna",
    ));
    bundle.events.push(operator_event(
        "event-001",
        "missing-slot",
        OperatorEventType::Switched,
        starts_at + chrono::Duration::seconds(10),
    ));
    bundle.observations.push(observation(
        "obs-001",
        starts_at + chrono::Duration::seconds(90),
        Some("missing-slot"),
        Some("A"),
        Some(1.5),
    ));
    bundle.wsjtx.push(WsjtXRecord {
        meta: record_meta(starts_at, RecordSource::WsjtxLog),
        record_id: "wsjtx-001".to_string(),
        message_type: "status_snapshot".to_string(),
        raw: json!({}),
    });
    bundle.rig.push(RigRecord {
        meta: record_meta(starts_at, RecordSource::RigAdapter),
        record_id: "rig-001".to_string(),
        status: "duplicate".to_string(),
        frequency_hz: None,
        mode: None,
        raw: json!({}),
    });
    bundle.propagation.push(PropagationRecord {
        meta: record_meta(starts_at, RecordSource::ImportedFile),
        record_id: "prop-001".to_string(),
        observed_at: starts_at,
        solar_flux_f107: None,
        sunspot_number: None,
        kp_index: None,
        a_index: None,
        solar_wind_speed_kms: None,
        bz_nt: None,
        alerts: Vec::new(),
        daylight_state: None,
        raw: json!({}),
    });

    let error = validate_bundle(&bundle).unwrap_err();
    let non_alignment_issues = error
        .issues()
        .iter()
        .filter(|issue| !matches!(issue, BundleValidationIssue::AlignmentAnnotationMismatch { .. }))
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(
        non_alignment_issues,
        @r###"
        [
            DuplicateId {
                kind: Slot,
                id: "slot-002",
            },
            DuplicateId {
                kind: OperatorEvent,
                id: "event-001",
            },
            DuplicateId {
                kind: Observation,
                id: "obs-001",
            },
            DuplicateId {
                kind: WsjtXRecord,
                id: "wsjtx-001",
            },
            DuplicateId {
                kind: RigRecord,
                id: "rig-001",
            },
            DuplicateId {
                kind: PropagationRecord,
                id: "prop-001",
            },
            UnknownAntennaLabel {
                slot_id: "slot-002",
                antenna_label: "missing-antenna",
            },
            SlotWindowOutOfOrder {
                previous_slot_id: "slot-002",
                slot_id: "slot-002",
            },
            SlotWindowOverlap {
                previous_slot_id: "slot-002",
                previous_ends_at: 2026-07-10T20:04:00Z,
                slot_id: "slot-002",
                starts_at: 2026-07-10T20:01:00Z,
            },
            UnknownEventSlot {
                event_id: "event-001",
                slot_id: "missing-slot",
            },
            UnknownObservationSlot {
                observation_id: "obs-001",
                slot_id: "missing-slot",
            },
            InvalidSlotConfidence {
                observation_id: "obs-001",
                slot_confidence: 1.5,
            },
        ]
        "###
    );
}
```

- [ ] **Step 2: Run the failing structural validation tests**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation
```

Expected: FAIL because `validate_bundle_issues` still returns no issues.

- [ ] **Step 3: Implement deterministic issue collection**

Replace `validate_bundle_issues` in `crates/core/src/validation.rs` with this implementation:

```rust
fn validate_bundle_issues(bundle: &BundleContents) -> Vec<BundleValidationIssue> {
    let mut issues = Vec::new();
    let expected_session_id = bundle.manifest.session_id.as_str();

    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Manifest,
        bundle.manifest.schema_version,
        bundle.manifest.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Station,
        bundle.station.schema_version,
        bundle.station.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Antennas,
        bundle.antennas.schema_version,
        bundle.antennas.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Schedule,
        bundle.schedule.schema_version,
        bundle.schedule.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Analysis,
        bundle.analysis.schema_version,
        bundle.analysis.session_id.as_str(),
        expected_session_id,
    );

    validate_record_meta(
        &mut issues,
        BundleFileRole::Events,
        expected_session_id,
        bundle.events.iter().map(|record| {
            (record.event_id.as_str(), record.meta.schema_version, record.meta.session_id.as_str())
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::Observations,
        expected_session_id,
        bundle.observations.iter().map(|record| {
            (
                record.observation_id.as_str(),
                record.meta.schema_version,
                record.meta.session_id.as_str(),
            )
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::WsjtX,
        expected_session_id,
        bundle.wsjtx.iter().map(|record| {
            (record.record_id.as_str(), record.meta.schema_version, record.meta.session_id.as_str())
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::Rig,
        expected_session_id,
        bundle.rig.iter().map(|record| {
            (record.record_id.as_str(), record.meta.schema_version, record.meta.session_id.as_str())
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::Propagation,
        expected_session_id,
        bundle.propagation.iter().map(|record| {
            (record.record_id.as_str(), record.meta.schema_version, record.meta.session_id.as_str())
        }),
    );

    validate_duplicates(&mut issues, BundleIdKind::Slot, bundle.schedule.slots.iter().map(|slot| slot.slot_id.as_str()));
    validate_duplicates(&mut issues, BundleIdKind::OperatorEvent, bundle.events.iter().map(|event| event.event_id.as_str()));
    validate_duplicates(&mut issues, BundleIdKind::Observation, bundle.observations.iter().map(|observation| observation.observation_id.as_str()));
    validate_duplicates(&mut issues, BundleIdKind::WsjtXRecord, bundle.wsjtx.iter().map(|record| record.record_id.as_str()));
    validate_duplicates(&mut issues, BundleIdKind::RigRecord, bundle.rig.iter().map(|record| record.record_id.as_str()));
    validate_duplicates(&mut issues, BundleIdKind::PropagationRecord, bundle.propagation.iter().map(|record| record.record_id.as_str()));

    validate_schedule_references_and_windows(&mut issues, bundle);
    validate_event_and_observation_references(&mut issues, bundle);
    validate_slot_confidence_ranges(&mut issues, bundle);

    issues
}
```

Add these private helpers using `HashSet` and the existing model fields:

```rust
use std::collections::HashSet;

use chrono::Duration;

use crate::{BundleContents, PlannedSlot, SCHEMA_VERSION};
```

```rust
fn validate_root_schema_and_session(
    issues: &mut Vec<BundleValidationIssue>,
    file: BundleFileRole,
    actual_schema_version: u16,
    actual_session_id: &str,
    expected_session_id: &str,
) {
    if actual_schema_version != SCHEMA_VERSION {
        issues.push(BundleValidationIssue::UnexpectedSchemaVersion {
            file,
            record_id: None,
            expected: SCHEMA_VERSION,
            actual: actual_schema_version,
        });
    }

    if actual_session_id != expected_session_id {
        issues.push(BundleValidationIssue::SessionIdMismatch {
            file,
            record_id: None,
            expected: expected_session_id.to_string(),
            actual: actual_session_id.to_string(),
        });
    }
}

fn validate_record_meta<'a>(
    issues: &mut Vec<BundleValidationIssue>,
    file: BundleFileRole,
    expected_session_id: &str,
    records: impl IntoIterator<Item = (&'a str, u16, &'a str)>,
) {
    for (record_id, actual_schema_version, actual_session_id) in records {
        if actual_schema_version != SCHEMA_VERSION {
            issues.push(BundleValidationIssue::UnexpectedSchemaVersion {
                file,
                record_id: Some(record_id.to_string()),
                expected: SCHEMA_VERSION,
                actual: actual_schema_version,
            });
        }

        if actual_session_id != expected_session_id {
            issues.push(BundleValidationIssue::SessionIdMismatch {
                file,
                record_id: Some(record_id.to_string()),
                expected: expected_session_id.to_string(),
                actual: actual_session_id.to_string(),
            });
        }
    }
}

fn validate_duplicates<'a>(
    issues: &mut Vec<BundleValidationIssue>,
    kind: BundleIdKind,
    ids: impl IntoIterator<Item = &'a str>,
) {
    let mut seen = HashSet::new();

    for id in ids {
        if !seen.insert(id) {
            issues.push(BundleValidationIssue::DuplicateId {
                kind,
                id: id.to_string(),
            });
        }
    }
}

fn validate_schedule_references_and_windows(
    issues: &mut Vec<BundleValidationIssue>,
    bundle: &BundleContents,
) {
    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.as_str())
        .collect::<HashSet<_>>();

    let mut previous_slot: Option<&PlannedSlot> = None;

    for slot in &bundle.schedule.slots {
        if !antenna_labels.contains(slot.antenna_label.as_str()) {
            issues.push(BundleValidationIssue::UnknownAntennaLabel {
                slot_id: slot.slot_id.clone(),
                antenna_label: slot.antenna_label.clone(),
            });
        }

        if let Some(previous) = previous_slot {
            if slot.starts_at < previous.starts_at {
                issues.push(BundleValidationIssue::SlotWindowOutOfOrder {
                    previous_slot_id: previous.slot_id.clone(),
                    slot_id: slot.slot_id.clone(),
                });
            }

            let previous_ends_at =
                previous.starts_at + Duration::seconds(previous.duration_seconds.into());
            if slot.starts_at < previous_ends_at {
                issues.push(BundleValidationIssue::SlotWindowOverlap {
                    previous_slot_id: previous.slot_id.clone(),
                    previous_ends_at,
                    slot_id: slot.slot_id.clone(),
                    starts_at: slot.starts_at,
                });
            }
        }

        previous_slot = Some(slot);
    }
}

fn validate_event_and_observation_references(
    issues: &mut Vec<BundleValidationIssue>,
    bundle: &BundleContents,
) {
    let slot_ids = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| slot.slot_id.as_str())
        .collect::<HashSet<_>>();

    for event in &bundle.events {
        if let Some(slot_id) = &event.slot_id {
            if !slot_ids.contains(slot_id.as_str()) {
                issues.push(BundleValidationIssue::UnknownEventSlot {
                    event_id: event.event_id.clone(),
                    slot_id: slot_id.clone(),
                });
            }
        }
    }

    for observation in &bundle.observations {
        if let Some(slot_id) = &observation.slot_id {
            if !slot_ids.contains(slot_id.as_str()) {
                issues.push(BundleValidationIssue::UnknownObservationSlot {
                    observation_id: observation.observation_id.clone(),
                    slot_id: slot_id.clone(),
                });
            }
        }
    }
}

fn validate_slot_confidence_ranges(
    issues: &mut Vec<BundleValidationIssue>,
    bundle: &BundleContents,
) {
    for observation in &bundle.observations {
        if let Some(slot_confidence) = observation.slot_confidence {
            if !(0.0..=1.0).contains(&slot_confidence) {
                issues.push(BundleValidationIssue::InvalidSlotConfidence {
                    observation_id: observation.observation_id.clone(),
                    slot_confidence,
                });
            }
        }
    }
}
```

- [ ] **Step 4: Run the structural validation tests**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation
```

Expected: PASS.

- [ ] **Step 5: Run all core validation tests**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation
```

Expected: PASS.

- [ ] **Step 6: Commit structural validation**

Run:

```bash
jj status
jj commit -m "feat: validate bundle structure invariants"
```

Expected: a described jj change containing structural validation implementation and tests.

## Task 3: Persisted Alignment Annotation Validation

**Files:**
- Modify: `crates/core/src/validation.rs`
- Modify: `crates/core/tests/bundle_validation.rs`

- [ ] **Step 1: Write failing tests for stale persisted alignment annotations**

Append this test to `crates/core/tests/bundle_validation.rs`:

```rust
#[test]
fn reports_persisted_alignment_annotation_mismatches() {
    let mut bundle = valid_bundle();
    bundle.observations[0].slot_id = Some("slot-002".to_string());
    bundle.observations[0].slot_label = Some("B".to_string());
    bundle.observations[0].slot_confidence = Some(0.25);

    let error = validate_bundle(&bundle).unwrap_err();

    insta::assert_debug_snapshot!(
        error.issues(),
        @r###"
        [
            AlignmentAnnotationMismatch {
                observation_id: "obs-001",
                field: SlotId,
                expected: "Some(\"slot-001\")",
                actual: "Some(\"slot-002\")",
            },
            AlignmentAnnotationMismatch {
                observation_id: "obs-001",
                field: SlotLabel,
                expected: "Some(\"A\")",
                actual: "Some(\"B\")",
            },
            AlignmentAnnotationMismatch {
                observation_id: "obs-001",
                field: SlotConfidence,
                expected: "Some(0.95)",
                actual: "Some(0.25)",
            },
        ]
        "###
    );
}
```

- [ ] **Step 2: Run the failing alignment validation test**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation reports_persisted_alignment_annotation_mismatches
```

Expected: FAIL because validation does not compare regenerated annotations yet.

- [ ] **Step 3: Implement alignment annotation comparison**

In `crates/core/src/validation.rs`, import the existing alignment API:

```rust
use crate::{align_schedule_slots, apply_slot_assignments, SlotAlignmentPolicy};
```

Call a new helper at the end of `validate_bundle_issues`:

```rust
validate_alignment_annotations(&mut issues, bundle);
```

Implement the helper:

```rust
fn validate_alignment_annotations(
    issues: &mut Vec<BundleValidationIssue>,
    bundle: &BundleContents,
) {
    let alignment = align_schedule_slots(
        &bundle.schedule,
        &bundle.events,
        &bundle.observations,
        SlotAlignmentPolicy::default(),
    );
    let regenerated =
        apply_slot_assignments(&bundle.observations, &alignment.observation_assignments);

    for (actual, expected) in bundle.observations.iter().zip(regenerated.iter()) {
        push_annotation_mismatch(
            issues,
            actual.observation_id.as_str(),
            AlignmentAnnotationField::SlotId,
            format!("{:?}", expected.slot_id),
            format!("{:?}", actual.slot_id),
        );
        push_annotation_mismatch(
            issues,
            actual.observation_id.as_str(),
            AlignmentAnnotationField::SlotLabel,
            format!("{:?}", expected.slot_label),
            format!("{:?}", actual.slot_label),
        );

        if !slot_confidence_matches(expected.slot_confidence, actual.slot_confidence) {
            issues.push(BundleValidationIssue::AlignmentAnnotationMismatch {
                observation_id: actual.observation_id.clone(),
                field: AlignmentAnnotationField::SlotConfidence,
                expected: format!("{:?}", expected.slot_confidence),
                actual: format!("{:?}", actual.slot_confidence),
            });
        }
    }
}

fn push_annotation_mismatch(
    issues: &mut Vec<BundleValidationIssue>,
    observation_id: &str,
    field: AlignmentAnnotationField,
    expected: String,
    actual: String,
) {
    if expected != actual {
        issues.push(BundleValidationIssue::AlignmentAnnotationMismatch {
            observation_id: observation_id.to_string(),
            field,
            expected,
            actual,
        });
    }
}

fn slot_confidence_matches(expected: Option<f32>, actual: Option<f32>) -> bool {
    match (expected, actual) {
        (Some(expected), Some(actual)) => (expected - actual).abs() <= 0.000_001,
        (None, None) => true,
        _ => false,
    }
}
```

- [ ] **Step 4: Run the alignment validation test**

Run:

```bash
cargo test -p antennabench-core --test bundle_validation reports_persisted_alignment_annotation_mismatches
```

Expected: PASS.

- [ ] **Step 5: Run the full core test suite**

Run:

```bash
cargo test -p antennabench-core
```

Expected: PASS.

- [ ] **Step 6: Commit alignment validation**

Run:

```bash
jj status
jj commit -m "feat: validate persisted slot alignment annotations"
```

Expected: a described jj change containing alignment consistency validation and tests.

## Task 4: Golden Fixture Validation Coverage

**Files:**
- Modify: `crates/storage/tests/golden_bundle.rs`

- [ ] **Step 1: Update the existing storage golden test expectation**

In `crates/storage/tests/golden_bundle.rs`, import `validate_bundle`:

```rust
use antennabench_core::{
    align_schedule_slots, apply_slot_assignments, validate_bundle, AlignedSlotStatus,
    AnalysisStatus, ExperimentMode, ObservationKind, RecordSource, SessionGoal,
    SlotAlignmentPolicy,
};
```

After reading the fixture, add:

```rust
validate_bundle(&imported).unwrap();
```

- [ ] **Step 2: Run the storage golden test**

Run:

```bash
cargo test -p antennabench-storage --test golden_bundle
```

Expected: PASS.

- [ ] **Step 3: Commit fixture validation coverage**

Run:

```bash
jj status
jj commit -m "test: validate golden bundle fixture"
```

Expected: a described jj change containing only storage golden test updates.

## Task 5: Storage Read-and-Validate Convenience API

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Modify: `crates/storage/tests/bundle_errors.rs`
- Modify: `crates/storage/tests/golden_bundle.rs`

- [ ] **Step 1: Write failing storage API tests**

In `crates/storage/tests/golden_bundle.rs`, change the fixture import line to:

```rust
let imported = BundleStore::new(&fixture).read_validated().unwrap();
```

Append this test to `crates/storage/tests/bundle_errors.rs`:

```rust
#[test]
fn read_validated_returns_validation_error_for_parseable_invalid_bundle() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("invalid-reference.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    std::fs::write(
        bundle_path.join("events.jsonl"),
        r#"{"meta":{"schema_version":1,"session_id":"session-invalid-jsonl","timestamp":"2026-07-09T20:00:00Z","source":"operator"},"event_id":"event-001","slot_id":"missing-slot","event_type":"switched","note":null}
"#,
    )
    .unwrap();

    let error = BundleStore::new(&bundle_path).read_validated().unwrap_err();

    match error {
        BundleStoreError::Validation { source } => {
            assert_eq!(source.issues().len(), 1);
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run the failing storage tests**

Run:

```bash
cargo test -p antennabench-storage --test golden_bundle --test bundle_errors
```

Expected: FAIL because `BundleStore::read_validated` and `BundleStoreError::Validation` do not exist.

- [ ] **Step 3: Add the convenience API and error variant**

In `crates/storage/src/lib.rs`, change the core import:

```rust
use antennabench_core::{
    validate_bundle, BundleContents, BundleFiles, BundleManifest, BundleValidationError,
};
```

Add this method to `impl BundleStore` after `read`:

```rust
pub fn read_validated(&self) -> Result<BundleContents, BundleStoreError> {
    let bundle = self.read()?;
    validate_bundle(&bundle)?;
    Ok(bundle)
}
```

Add this variant to `BundleStoreError`:

```rust
#[error(transparent)]
Validation {
    #[from]
    source: BundleValidationError,
},
```

- [ ] **Step 4: Run storage tests**

Run:

```bash
cargo test -p antennabench-storage --test golden_bundle --test bundle_errors
```

Expected: PASS.

- [ ] **Step 5: Commit storage convenience validation**

Run:

```bash
jj status
jj commit -m "feat: add validated bundle read API"
```

Expected: a described jj change containing storage API and storage tests.

## Task 6: Final Verification

**Files:**
- Inspect all files changed by this plan.

- [ ] **Step 1: Format the workspace**

Run:

```bash
cargo fmt
```

Expected: PASS and either no diff or only formatting changes in files touched by this plan.

- [ ] **Step 2: Check formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run all tests**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Inspect the jj diff**

Run:

```bash
jj diff
```

Expected: the final diff contains only validation-related Rust changes and test updates described in this plan.

- [ ] **Step 6: Commit final verification fixes if needed**

If formatting or clippy required edits, run:

```bash
jj status
jj commit -m "chore: finish bundle validation slice"
```

Expected: either no extra commit is needed, or a small described jj change contains only mechanical fixes from final verification.
