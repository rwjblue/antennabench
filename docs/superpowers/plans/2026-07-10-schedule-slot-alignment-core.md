# Schedule Slot Alignment Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build deterministic Rust core logic that maps planned WSPR slots and operator events into actual slot state, then assigns observations to slots with labels and confidence.

**Architecture:** Keep alignment as pure `antennabench-core` code that consumes the existing bundle model: `Schedule`, `OperatorEvent`, and `ObservationRecord`. Storage remains bundle I/O only; the golden fixture is expanded so storage tests prove persisted observation slot annotations can be regenerated from the bundle inputs.

**Tech Stack:** Rust workspace, `chrono`, `serde`, `thiserror` for library errors, `anyhow` only for future app/binary code, `insta` inline snapshots for result-shaped tests, Cargo tests, fixture JSON/JSONL bundles, jj commits.

---

## Scope

This slice creates working, testable software for:

- Actual slot state derived from planned slots plus `Switched`, `MissedSlot`, and `BadSlot` operator events.
- Observation-to-slot assignment using planned slot windows, per-slot guard time, band matching, bad/missed slot handling, late manual switch handling, and near-boundary confidence reduction.
- A helper that returns cloned `ObservationRecord` values with `slot_id`, `slot_label`, and `slot_confidence` filled from deterministic assignments.
- Fixture-driven tests that update the existing `minimal-whole-station.session.wsprabundle` with good, bad, missed, and late-switch slot examples.

This slice does not create Tauri UI, WSJT-X UDP live integration, Hamlib, public spot fetching, SQLite indexing, report charts, hosted publishing, or statistical report conclusions.

## Rust Preferences

- Library crates should expose typed errors with `thiserror` when new fallible public APIs are added.
- App or binary code may use `anyhow`, but this slice does not add app code and should not add `anyhow`.
- Use `insta` inline snapshots when asserting structured alignment output makes tests easier to iterate.
- Keep targeted `assert_eq!` checks for focused invariants where snapshots would be brittle or obscure intent.

## Alignment Semantics

- Slot windows are half-open: `starts_at <= observation timestamp < starts_at + duration_seconds`.
- `guard_seconds` is the low-confidence switching interval at the start of each planned slot.
- `BadSlot` takes precedence over `MissedSlot`; both take precedence over `Switched`.
- A slot with no operator switch, missed, or bad event remains usable as `PlannedNoSwitchEvent`, but observation confidence is lower than an observed on-time switch.
- A switch event at or before `starts_at + guard_seconds` is `Switched`; a switch event after guard is `LateSwitch`.
- For a late switch, observations before the switch are assigned to the planned slot for auditability but get no label and confidence `0.10`.
- Observations inside guard time get confidence `0.25`; they get a label only when an observed switch has already occurred.
- Observations within `boundary_seconds` of a slot end get confidence `0.60`.
- Good interior observations get confidence `0.95` for observed on-time switches, `0.80` for no switch event, and `0.70` after a late switch.
- Observations in bad or missed slots keep the slot id, get no label, and get confidence `0.0`.
- Observations with a timestamp inside a slot but on a different band are not assigned and get reason `BandMismatch`.
- Observations outside every planned slot are not assigned and get reason `OutsideSchedule`.

## Existing Model Changes

No durable bundle schema change is required. Use these existing fields:

- `Schedule.slots` and `PlannedSlot.guard_seconds` describe planned slot windows.
- `OperatorEvent.slot_id` plus `OperatorEventType::{Switched, MissedSlot, BadSlot}` describe actual operator actions.
- `ObservationRecord.{slot_id, slot_label, slot_confidence}` store the computed assignment.

Add new exported core alignment types in `crates/core/src/alignment.rs`. Do not add fields to `BundleContents`, `Schedule`, `OperatorEvent`, or `ObservationRecord` in this slice.

## File Structure

- Modify `Cargo.toml`: add `insta` as a workspace test dependency.
- Modify `Cargo.lock`: update generated dependency lock entries after running Cargo.
- Modify `crates/core/Cargo.toml`: add `insta` as a dev-dependency for alignment tests.
- Modify `crates/core/src/lib.rs`: export the new alignment module.
- Create `crates/core/src/alignment.rs`: pure alignment policy, result types, slot derivation, observation assignment, and annotation helper.
- Create `crates/core/tests/slot_alignment.rs`: deterministic unit tests for actual slot state, assignment edge cases, and observation annotation.
- Modify `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/schedule.json`: expand the planned schedule to four slots.
- Modify `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/events.jsonl`: add switch, bad, missed, and late-switch operator events.
- Modify `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/observations.jsonl`: add fixture observations whose slot annotations match the alignment output.
- Modify `crates/storage/tests/golden_bundle.rs`: assert the expanded fixture imports, exports, and regenerates the persisted slot annotations.

## Task 1: Core Actual Slot State

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/core/Cargo.toml`
- Create: `crates/core/src/alignment.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `crates/core/tests/slot_alignment.rs`

- [ ] **Step 1: Add the inline snapshot test dependency**

Replace `Cargo.toml` with this complete content:

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
insta = "1.48.0"
serde = { version = "1.0.204", features = ["derive"] }
serde_json = "1.0.120"
tempfile = "3.10.1"
thiserror = "1.0.63"
```

Replace `crates/core/Cargo.toml` with this complete content:

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

[dev-dependencies]
insta.workspace = true
```

- [ ] **Step 2: Write the failing actual-slot test**

Write `crates/core/tests/slot_alignment.rs` with this complete content:

```rust
use antennabench_core::{
    align_schedule_slots, Band, ExperimentMode, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, RecordMeta, RecordSource, Schedule, SessionGoal,
    SlotAlignmentPolicy,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-alignment-test";

#[test]
fn derives_actual_slot_state_from_operator_events() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
        operator_event("event-001", "slot-001", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(3)),
        operator_event("event-002", "slot-002", OperatorEventType::BadSlot, starts_at + chrono::Duration::seconds(140)),
        operator_event("event-003", "slot-003", OperatorEventType::MissedSlot, starts_at + chrono::Duration::seconds(240)),
        operator_event("event-004", "slot-004", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(385)),
    ];

    let result = align_schedule_slots(&schedule, &events, &[], SlotAlignmentPolicy::default());

    insta::assert_json_snapshot!(
        result.slots.iter().map(|slot| {
            json!({
                "slot_id": &slot.slot_id,
                "status": &slot.status,
                "planned_label": &slot.planned_label,
                "actual_label": &slot.actual_label,
                "switch_event_id": &slot.switch_event_id,
                "switch_delay_seconds": slot.switch_delay_seconds,
                "usable_start": slot.usable_start,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "actual_label": "A",
            "planned_label": "A",
            "slot_id": "slot-001",
            "status": "switched",
            "switch_delay_seconds": 3,
            "switch_event_id": "event-001",
            "usable_start": "2026-07-10T20:00:15Z"
          },
          {
            "actual_label": null,
            "planned_label": "B",
            "slot_id": "slot-002",
            "status": "bad",
            "switch_delay_seconds": null,
            "switch_event_id": null,
            "usable_start": "2026-07-10T20:02:15Z"
          },
          {
            "actual_label": null,
            "planned_label": "A",
            "slot_id": "slot-003",
            "status": "missed",
            "switch_delay_seconds": null,
            "switch_event_id": null,
            "usable_start": "2026-07-10T20:04:15Z"
          },
          {
            "actual_label": "B",
            "planned_label": "B",
            "slot_id": "slot-004",
            "status": "late_switch",
            "switch_delay_seconds": 25,
            "switch_event_id": "event-004",
            "usable_start": "2026-07-10T20:06:25Z"
          }
        ]
        "###
    );
}

fn schedule_with_slots(starts_at: chrono::DateTime<Utc>) -> Schedule {
    Schedule {
        schema_version: 1,
        session_id: SESSION_ID.to_string(),
        mode: ExperimentMode::WholeStationAb,
        goal: SessionGoal::GeneralCoverage,
        slots: vec![
            planned_slot("slot-001", 1, starts_at, "A"),
            planned_slot("slot-002", 2, starts_at + chrono::Duration::seconds(120), "B"),
            planned_slot("slot-003", 3, starts_at + chrono::Duration::seconds(240), "A"),
            planned_slot("slot-004", 4, starts_at + chrono::Duration::seconds(360), "B"),
        ],
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
        meta: RecordMeta {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            timestamp,
            source: RecordSource::Operator,
        },
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type,
        note: None,
    }
}
```

- [ ] **Step 3: Run the failing actual-slot test**

Run:

```bash
cargo test -p antennabench-core --test slot_alignment derives_actual_slot_state_from_operator_events
```

Expected: FAIL with unresolved imports for `align_schedule_slots` and `SlotAlignmentPolicy`.

- [ ] **Step 4: Export the alignment module**

Replace `crates/core/src/lib.rs` with this complete content:

```rust
mod alignment;
mod model;

pub use alignment::*;
pub use model::*;

pub const SCHEMA_VERSION: u16 = 1;
```

- [ ] **Step 5: Add the initial alignment implementation**

Write `crates/core/src/alignment.rs` with this complete content:

```rust
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{Band, ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot, Schedule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotAlignmentPolicy {
    pub boundary_seconds: i64,
}

impl Default for SlotAlignmentPolicy {
    fn default() -> Self {
        Self {
            boundary_seconds: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleSlotAlignment {
    pub slots: Vec<AlignedSlot>,
    pub observation_assignments: Vec<ObservationSlotAssignment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlignedSlot {
    pub slot_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub planned_label: String,
    pub actual_label: Option<String>,
    pub status: AlignedSlotStatus,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub usable_start: DateTime<Utc>,
    pub switch_event_id: Option<String>,
    pub switch_timestamp: Option<DateTime<Utc>>,
    pub switch_delay_seconds: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignedSlotStatus {
    PlannedNoSwitchEvent,
    Switched,
    LateSwitch,
    Missed,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationSlotAssignment {
    pub observation_id: String,
    pub slot_id: Option<String>,
    pub slot_label: Option<String>,
    pub confidence: f32,
    pub reason: SlotAssignmentReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotAssignmentReason {
    Interior,
    GuardTime,
    NearBoundary,
    LateSwitch,
    BeforeObservedSwitch,
    MissedSlot,
    BadSlot,
    BandMismatch,
    OutsideSchedule,
}

pub fn align_schedule_slots(
    schedule: &Schedule,
    events: &[OperatorEvent],
    observations: &[ObservationRecord],
    policy: SlotAlignmentPolicy,
) -> ScheduleSlotAlignment {
    let slots = schedule
        .slots
        .iter()
        .map(|slot| align_slot(slot, events))
        .collect::<Vec<_>>();
    let observation_assignments = observations
        .iter()
        .map(|observation| assign_observation_to_slot(observation, &slots, policy))
        .collect();

    ScheduleSlotAlignment {
        slots,
        observation_assignments,
    }
}

pub fn apply_slot_assignments(
    observations: &[ObservationRecord],
    assignments: &[ObservationSlotAssignment],
) -> Vec<ObservationRecord> {
    observations
        .iter()
        .map(|observation| {
            let mut annotated = observation.clone();
            if let Some(assignment) = assignments
                .iter()
                .find(|assignment| assignment.observation_id == observation.observation_id)
            {
                annotated.slot_id = assignment.slot_id.clone();
                annotated.slot_label = assignment.slot_label.clone();
                annotated.slot_confidence = Some(assignment.confidence);
            }
            annotated
        })
        .collect()
}

fn align_slot(slot: &PlannedSlot, events: &[OperatorEvent]) -> AlignedSlot {
    let slot_events = events
        .iter()
        .filter(|event| event.slot_id.as_deref() == Some(slot.slot_id.as_str()))
        .collect::<Vec<_>>();
    let switch_event = slot_events
        .iter()
        .copied()
        .filter(|event| event.event_type == OperatorEventType::Switched)
        .min_by_key(|event| event.meta.timestamp);
    let has_bad_event = slot_events
        .iter()
        .any(|event| event.event_type == OperatorEventType::BadSlot);
    let has_missed_event = slot_events
        .iter()
        .any(|event| event.event_type == OperatorEventType::MissedSlot);
    let guard_end = slot.starts_at + Duration::seconds(slot.guard_seconds.into());
    let ends_at = slot.starts_at + Duration::seconds(slot.duration_seconds.into());

    if has_bad_event {
        return AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::Bad,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: switch_event.map(|event| event.event_id.clone()),
            switch_timestamp: switch_event.map(|event| event.meta.timestamp),
            switch_delay_seconds: switch_event.map(|event| switch_delay_seconds(slot, event)),
        };
    }

    if has_missed_event {
        return AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: None,
            status: AlignedSlotStatus::Missed,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: switch_event.map(|event| event.event_id.clone()),
            switch_timestamp: switch_event.map(|event| event.meta.timestamp),
            switch_delay_seconds: switch_event.map(|event| switch_delay_seconds(slot, event)),
        };
    }

    match switch_event {
        Some(event) => {
            let status = if event.meta.timestamp <= guard_end {
                AlignedSlotStatus::Switched
            } else {
                AlignedSlotStatus::LateSwitch
            };
            AlignedSlot {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                band: slot.band,
                planned_label: slot.antenna_label.clone(),
                actual_label: Some(slot.antenna_label.clone()),
                status,
                starts_at: slot.starts_at,
                ends_at,
                usable_start: guard_end.max(event.meta.timestamp),
                switch_event_id: Some(event.event_id.clone()),
                switch_timestamp: Some(event.meta.timestamp),
                switch_delay_seconds: Some(switch_delay_seconds(slot, event)),
            }
        }
        None => AlignedSlot {
            slot_id: slot.slot_id.clone(),
            sequence_number: slot.sequence_number,
            band: slot.band,
            planned_label: slot.antenna_label.clone(),
            actual_label: Some(slot.antenna_label.clone()),
            status: AlignedSlotStatus::PlannedNoSwitchEvent,
            starts_at: slot.starts_at,
            ends_at,
            usable_start: guard_end,
            switch_event_id: None,
            switch_timestamp: None,
            switch_delay_seconds: None,
        },
    }
}

fn switch_delay_seconds(slot: &PlannedSlot, event: &OperatorEvent) -> i64 {
    (event.meta.timestamp - slot.starts_at).num_seconds()
}

fn assign_observation_to_slot(
    observation: &ObservationRecord,
    slots: &[AlignedSlot],
    policy: SlotAlignmentPolicy,
) -> ObservationSlotAssignment {
    let timestamp = observation.meta.timestamp;
    let Some(slot) = slots
        .iter()
        .find(|slot| timestamp >= slot.starts_at && timestamp < slot.ends_at)
    else {
        return assignment(observation, None, None, 0.0, SlotAssignmentReason::OutsideSchedule);
    };

    if observation.band != slot.band {
        return assignment(observation, None, None, 0.0, SlotAssignmentReason::BandMismatch);
    }

    match slot.status {
        AlignedSlotStatus::Bad => {
            return assignment(observation, Some(slot), None, 0.0, SlotAssignmentReason::BadSlot);
        }
        AlignedSlotStatus::Missed => {
            return assignment(observation, Some(slot), None, 0.0, SlotAssignmentReason::MissedSlot);
        }
        AlignedSlotStatus::LateSwitch if timestamp < slot.usable_start => {
            return assignment(
                observation,
                Some(slot),
                None,
                0.10,
                SlotAssignmentReason::BeforeObservedSwitch,
            );
        }
        _ => {}
    }

    if timestamp < slot.usable_start {
        let label = if slot
            .switch_timestamp
            .is_some_and(|switch_timestamp| switch_timestamp <= timestamp)
        {
            slot.actual_label.as_deref()
        } else {
            None
        };
        return assignment(observation, Some(slot), label, 0.25, SlotAssignmentReason::GuardTime);
    }

    if (slot.ends_at - timestamp).num_seconds() <= policy.boundary_seconds {
        return assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.60,
            SlotAssignmentReason::NearBoundary,
        );
    }

    match slot.status {
        AlignedSlotStatus::LateSwitch => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.70,
            SlotAssignmentReason::LateSwitch,
        ),
        AlignedSlotStatus::PlannedNoSwitchEvent => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.80,
            SlotAssignmentReason::Interior,
        ),
        AlignedSlotStatus::Switched => assignment(
            observation,
            Some(slot),
            slot.actual_label.as_deref(),
            0.95,
            SlotAssignmentReason::Interior,
        ),
        AlignedSlotStatus::Bad | AlignedSlotStatus::Missed => unreachable!("handled above"),
    }
}

fn assignment(
    observation: &ObservationRecord,
    slot: Option<&AlignedSlot>,
    slot_label: Option<&str>,
    confidence: f32,
    reason: SlotAssignmentReason,
) -> ObservationSlotAssignment {
    ObservationSlotAssignment {
        observation_id: observation.observation_id.clone(),
        slot_id: slot.map(|slot| slot.slot_id.clone()),
        slot_label: slot_label.map(str::to_string),
        confidence,
        reason,
    }
}
```

- [ ] **Step 6: Run the actual-slot test**

Run:

```bash
cargo test -p antennabench-core --test slot_alignment derives_actual_slot_state_from_operator_events
```

Expected: PASS.

- [ ] **Step 7: Format and commit**

Run:

```bash
cargo fmt
jj status
jj commit -m "feat: derive actual WSPR slot state"
```

Expected: the new alignment module and test are committed in jj.

## Task 2: Observation Assignment Edge Cases

**Files:**
- Modify: `crates/core/tests/slot_alignment.rs`
- Modify: `crates/core/src/alignment.rs`

- [ ] **Step 1: Add failing observation assignment tests**

Append this content to `crates/core/tests/slot_alignment.rs`:

```rust
use antennabench_core::ObservationKind;

#[test]
fn assigns_observations_with_guard_boundary_band_and_outside_reasons() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
        operator_event("event-001", "slot-001", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(3)),
        operator_event("event-002", "slot-002", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(123)),
    ];
    let observations = vec![
        observation("obs-good-a", starts_at + chrono::Duration::seconds(60), Band::M20),
        observation("obs-boundary-a", starts_at + chrono::Duration::seconds(118), Band::M20),
        observation("obs-guard-b", starts_at + chrono::Duration::seconds(125), Band::M20),
        observation("obs-wrong-band", starts_at + chrono::Duration::seconds(70), Band::M40),
        observation("obs-outside", starts_at - chrono::Duration::seconds(5), Band::M20),
    ];

    let result = align_schedule_slots(&schedule, &events, &observations, SlotAlignmentPolicy::default());

    insta::assert_json_snapshot!(
        result.observation_assignments.iter().map(|assignment| {
            json!({
                "observation_id": &assignment.observation_id,
                "slot_id": &assignment.slot_id,
                "slot_label": &assignment.slot_label,
                "confidence": assignment.confidence,
                "reason": &assignment.reason,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "confidence": 0.95,
            "observation_id": "obs-good-a",
            "reason": "interior",
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "confidence": 0.6,
            "observation_id": "obs-boundary-a",
            "reason": "near_boundary",
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "confidence": 0.25,
            "observation_id": "obs-guard-b",
            "reason": "guard_time",
            "slot_id": "slot-002",
            "slot_label": "B"
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-wrong-band",
            "reason": "band_mismatch",
            "slot_id": null,
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-outside",
            "reason": "outside_schedule",
            "slot_id": null,
            "slot_label": null
          }
        ]
        "###
    );
}

#[test]
fn assigns_bad_missed_and_late_switch_observations_conservatively() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![
        operator_event("event-001", "slot-001", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(3)),
        operator_event("event-002", "slot-002", OperatorEventType::BadSlot, starts_at + chrono::Duration::seconds(140)),
        operator_event("event-003", "slot-003", OperatorEventType::MissedSlot, starts_at + chrono::Duration::seconds(240)),
        operator_event("event-004", "slot-004", OperatorEventType::Switched, starts_at + chrono::Duration::seconds(385)),
    ];
    let observations = vec![
        observation("obs-bad", starts_at + chrono::Duration::seconds(170), Band::M20),
        observation("obs-missed", starts_at + chrono::Duration::seconds(280), Band::M20),
        observation("obs-before-late-switch", starts_at + chrono::Duration::seconds(370), Band::M20),
        observation("obs-after-late-switch", starts_at + chrono::Duration::seconds(420), Band::M20),
    ];

    let result = align_schedule_slots(&schedule, &events, &observations, SlotAlignmentPolicy::default());

    insta::assert_json_snapshot!(
        result.observation_assignments.iter().map(|assignment| {
            json!({
                "observation_id": &assignment.observation_id,
                "slot_id": &assignment.slot_id,
                "slot_label": &assignment.slot_label,
                "confidence": assignment.confidence,
                "reason": &assignment.reason,
            })
        }).collect::<Vec<_>>(),
        @r###"
        [
          {
            "confidence": 0.0,
            "observation_id": "obs-bad",
            "reason": "bad_slot",
            "slot_id": "slot-002",
            "slot_label": null
          },
          {
            "confidence": 0.0,
            "observation_id": "obs-missed",
            "reason": "missed_slot",
            "slot_id": "slot-003",
            "slot_label": null
          },
          {
            "confidence": 0.1,
            "observation_id": "obs-before-late-switch",
            "reason": "before_observed_switch",
            "slot_id": "slot-004",
            "slot_label": null
          },
          {
            "confidence": 0.7,
            "observation_id": "obs-after-late-switch",
            "reason": "late_switch",
            "slot_id": "slot-004",
            "slot_label": "B"
          }
        ]
        "###
    );
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    band: Band,
) -> ObservationRecord {
    ObservationRecord {
        meta: RecordMeta {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            timestamp,
            source: RecordSource::WsjtxLog,
        },
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::LocalDecode,
        band,
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
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: json!({ "fixture": observation_id }),
    }
}
```

- [ ] **Step 2: Run the assignment tests**

Run:

```bash
cargo test -p antennabench-core --test slot_alignment assigns_
```

Expected: PASS if Task 1 implementation already includes the complete assignment rules. If any assertion fails, change only `crates/core/src/alignment.rs` to match the semantics in this plan.

- [ ] **Step 3: Run all core tests**

Run:

```bash
cargo test -p antennabench-core
```

Expected: PASS.

- [ ] **Step 4: Format and commit**

Run:

```bash
cargo fmt
jj status
jj commit -m "feat: assign observations to WSPR slots"
```

Expected: the observation assignment behavior is committed in jj.

## Task 3: Observation Annotation Helper

**Files:**
- Modify: `crates/core/tests/slot_alignment.rs`
- Modify: `crates/core/src/alignment.rs`

- [ ] **Step 1: Add the failing annotation test**

Append this content to `crates/core/tests/slot_alignment.rs`:

```rust
use antennabench_core::apply_slot_assignments;

#[test]
fn applies_slot_assignments_to_observation_records_without_changing_raw_observations() {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();
    let schedule = schedule_with_slots(starts_at);
    let events = vec![operator_event(
        "event-001",
        "slot-001",
        OperatorEventType::Switched,
        starts_at + chrono::Duration::seconds(3),
    )];
    let observations = vec![observation(
        "obs-good-a",
        starts_at + chrono::Duration::seconds(60),
        Band::M20,
    )];

    let result = align_schedule_slots(&schedule, &events, &observations, SlotAlignmentPolicy::default());
    let annotated = apply_slot_assignments(&observations, &result.observation_assignments);

    assert_eq!(observations[0].slot_id, None);
    assert_eq!(observations[0].slot_label, None);
    assert_eq!(observations[0].slot_confidence, None);
    assert_eq!(annotated[0].slot_id.as_deref(), Some("slot-001"));
    assert_eq!(annotated[0].slot_label.as_deref(), Some("A"));
    assert_eq!(annotated[0].slot_confidence, Some(0.95));
    assert_eq!(annotated[0].raw, observations[0].raw);
    assert_eq!(annotated[0].snr_db, observations[0].snr_db);
}
```

- [ ] **Step 2: Run the annotation test**

Run:

```bash
cargo test -p antennabench-core --test slot_alignment applies_slot_assignments_to_observation_records_without_changing_raw_observations
```

Expected: PASS if Task 1 implementation already includes `apply_slot_assignments`. If it fails, add the exact `apply_slot_assignments` function shown in Task 1 Step 5 to `crates/core/src/alignment.rs`.

- [ ] **Step 3: Run all core tests**

Run:

```bash
cargo test -p antennabench-core
```

Expected: PASS.

- [ ] **Step 4: Format and commit**

Run:

```bash
cargo fmt
jj status
jj commit -m "feat: annotate observations with slot assignments"
```

Expected: the annotation helper is committed in jj.

## Task 4: Golden Fixture Alignment Coverage

**Files:**
- Modify: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/schedule.json`
- Modify: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/events.jsonl`
- Modify: `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/observations.jsonl`
- Modify: `crates/storage/tests/golden_bundle.rs`

- [ ] **Step 1: Expand the fixture schedule**

Replace `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/schedule.json` with this complete content:

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
    },
    {
      "slot_id": "slot-003",
      "sequence_number": 3,
      "starts_at": "2026-07-09T20:04:00Z",
      "duration_seconds": 120,
      "guard_seconds": 15,
      "band": "20m",
      "antenna_label": "A"
    },
    {
      "slot_id": "slot-004",
      "sequence_number": 4,
      "starts_at": "2026-07-09T20:06:00Z",
      "duration_seconds": 120,
      "guard_seconds": 15,
      "band": "20m",
      "antenna_label": "B"
    }
  ]
}
```

- [ ] **Step 2: Expand the fixture operator events**

Replace `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/events.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:00:03Z","source":"operator"},"event_id":"event-001","slot_id":"slot-001","event_type":"switched","note":"A connected"}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:02:20Z","source":"operator"},"event_id":"event-002","slot_id":"slot-002","event_type":"bad_slot","note":"High SWR during B slot"}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:04:00Z","source":"operator"},"event_id":"event-003","slot_id":"slot-003","event_type":"missed_slot","note":"Operator missed the A switch prompt"}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:06:25Z","source":"operator"},"event_id":"event-004","slot_id":"slot-004","event_type":"switched","note":"B connected late"}
```

- [ ] **Step 3: Expand the fixture observations**

Replace `fixtures/session-bundles/minimal-whole-station.session.wsprabundle/observations.jsonl` with this complete content:

```jsonl
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:01:11Z","source":"wsjtx_log"},"observation_id":"obs-001","observation_kind":"local_decode","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"N1RWJ","heard_call":"K1ABC","reporter_grid":"FN42","heard_grid":"EM12","distance_km":2500.0,"azimuth_degrees":250.0,"snr_db":-18.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-001","slot_label":"A","slot_confidence":0.95,"raw":{"line":"example local decode"}}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:02:50Z","source":"wsprnet"},"observation_id":"obs-002","observation_kind":"public_report","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"K9XYZ","heard_call":"N1RWJ","reporter_grid":"EN52","heard_grid":"FN42","distance_km":1350.0,"azimuth_degrees":276.0,"snr_db":-21.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-002","slot_label":null,"slot_confidence":0.0,"raw":{"source":"fixture public report in bad slot"}}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:04:40Z","source":"wsjtx_log"},"observation_id":"obs-003","observation_kind":"local_decode","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"N1RWJ","heard_call":"W3AAA","reporter_grid":"FN42","heard_grid":"FM19","distance_km":650.0,"azimuth_degrees":231.0,"snr_db":-24.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-003","slot_label":null,"slot_confidence":0.0,"raw":{"line":"decode during missed slot"}}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:06:10Z","source":"wsprnet"},"observation_id":"obs-004","observation_kind":"public_report","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"VE3ZZZ","heard_call":"N1RWJ","reporter_grid":"FN03","heard_grid":"FN42","distance_km":640.0,"azimuth_degrees":294.0,"snr_db":-27.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-004","slot_label":null,"slot_confidence":0.1,"raw":{"source":"before late switch"}}
{"meta":{"schema_version":1,"session_id":"session-2026-07-09-n1rwj-20m","timestamp":"2026-07-09T20:07:00Z","source":"wsprnet"},"observation_id":"obs-005","observation_kind":"public_report","band":"20m","frequency_hz":14095600,"mode":"WSPR","reporter_call":"K4BBB","heard_call":"N1RWJ","reporter_grid":"EM74","heard_grid":"FN42","distance_km":1550.0,"azimuth_degrees":238.0,"snr_db":-19.0,"drift_hz_per_minute":0.0,"power_watts":5.0,"slot_id":"slot-004","slot_label":"B","slot_confidence":0.7,"raw":{"source":"after late switch"}}
```

- [ ] **Step 4: Update the storage golden test**

Replace `crates/storage/tests/golden_bundle.rs` with this complete content:

```rust
use std::path::PathBuf;

use antennabench_core::{
    align_schedule_slots, apply_slot_assignments, AlignedSlotStatus, AnalysisStatus,
    ExperimentMode, ObservationKind, RecordSource, SessionGoal, SlotAlignmentPolicy,
};
use antennabench_storage::BundleStore;

const SESSION_ID: &str = "session-2026-07-09-n1rwj-20m";

#[test]
fn imports_exports_and_regenerates_minimal_whole_station_alignment() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");

    let imported = BundleStore::new(&fixture).read().unwrap();

    assert_eq!(imported.manifest.schema_version, 1);
    assert_eq!(imported.manifest.session_id, SESSION_ID);
    assert_eq!(imported.station.callsign, "N1RWJ");
    assert_eq!(imported.station.grid, "FN42");
    assert_eq!(imported.antennas.antennas.len(), 2);
    assert_eq!(imported.schedule.mode, ExperimentMode::WholeStationAb);
    assert_eq!(imported.schedule.goal, SessionGoal::GeneralCoverage);
    assert_eq!(imported.schedule.slots.len(), 4);
    assert_eq!(imported.events.len(), 4);
    assert_eq!(imported.observations.len(), 5);
    assert_eq!(
        imported.observations[0].observation_kind,
        ObservationKind::LocalDecode
    );
    assert_eq!(imported.observations[1].meta.source, RecordSource::Wsprnet);
    assert_eq!(imported.wsjtx.len(), 1);
    assert_eq!(imported.rig.len(), 1);
    assert_eq!(imported.propagation.len(), 1);
    assert_eq!(imported.analysis.status, AnalysisStatus::NotRun);

    let alignment = align_schedule_slots(
        &imported.schedule,
        &imported.events,
        &imported.observations,
        SlotAlignmentPolicy::default(),
    );
    assert_eq!(alignment.slots[0].status, AlignedSlotStatus::Switched);
    assert_eq!(alignment.slots[1].status, AlignedSlotStatus::Bad);
    assert_eq!(alignment.slots[2].status, AlignedSlotStatus::Missed);
    assert_eq!(alignment.slots[3].status, AlignedSlotStatus::LateSwitch);

    let regenerated_observations =
        apply_slot_assignments(&imported.observations, &alignment.observation_assignments);
    assert_eq!(regenerated_observations, imported.observations);

    let tempdir = tempfile::tempdir().unwrap();
    let exported = tempdir.path().join("exported.session.wsprabundle");
    BundleStore::new(&exported).write(&imported).unwrap();

    let reimported = BundleStore::new(&exported).read().unwrap();

    assert_eq!(reimported, imported);
}
```

- [ ] **Step 5: Run the storage golden test**

Run:

```bash
cargo test -p antennabench-storage --test golden_bundle
```

Expected: PASS and prove that persisted fixture annotations match regenerated core alignment output.

- [ ] **Step 6: Format and commit**

Run:

```bash
cargo fmt
jj status
jj commit -m "test: cover golden bundle slot alignment"
```

Expected: fixture and storage golden test updates are committed in jj.

## Task 5: Full Verification

**Files:**
- No file changes.

- [ ] **Step 1: Run the full Rust test suite**

Run:

```bash
cargo test
```

Expected: PASS for all `antennabench-core` and `antennabench-storage` tests.

- [ ] **Step 2: Inspect the jj diff**

Run:

```bash
jj status
jj log -r 'trunk()..@'
```

Expected: only the planned alignment module, core tests, fixture updates, and storage golden test changed.

- [ ] **Step 3: Leave implementation ready for review**

Run:

```bash
jj status
```

Expected: working copy has no uncommitted file changes after the task commits.

## Self-Review Notes

- Spec coverage: planned slots, actual operator events, observation assignment, slot label, slot confidence, guard time, missed slots, bad slots, manual switch events, and near-boundary observations are all covered by core tests and the expanded fixture.
- Deferred scope check: UI, WSJT-X live UDP, Hamlib, public spot fetching, SQLite, report charts, and hosted publishing are not touched.
- Type consistency: `align_schedule_slots`, `apply_slot_assignments`, `SlotAlignmentPolicy`, `AlignedSlotStatus`, and `SlotAssignmentReason` are used consistently across core and storage tests.
- Rust preference consistency: library error guidance is explicit, this slice avoids `anyhow`, and `insta` inline snapshots are used only for structured alignment results.
- Bundle consistency: no durable schema fields are added; existing observation assignment fields are regenerated from existing schedule, event, and observation inputs.
