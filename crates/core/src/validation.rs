use std::collections::HashSet;

use chrono::Duration;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{BundleContents, PlannedSlot, SCHEMA_VERSION};

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
            (
                record.event_id.as_str(),
                record.meta.schema_version,
                record.meta.session_id.as_str(),
            )
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
            (
                record.record_id.as_str(),
                record.meta.schema_version,
                record.meta.session_id.as_str(),
            )
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::Rig,
        expected_session_id,
        bundle.rig.iter().map(|record| {
            (
                record.record_id.as_str(),
                record.meta.schema_version,
                record.meta.session_id.as_str(),
            )
        }),
    );
    validate_record_meta(
        &mut issues,
        BundleFileRole::Propagation,
        expected_session_id,
        bundle.propagation.iter().map(|record| {
            (
                record.record_id.as_str(),
                record.meta.schema_version,
                record.meta.session_id.as_str(),
            )
        }),
    );

    validate_duplicates(
        &mut issues,
        BundleIdKind::Slot,
        bundle
            .schedule
            .slots
            .iter()
            .map(|slot| slot.slot_id.as_str()),
    );
    validate_duplicates(
        &mut issues,
        BundleIdKind::OperatorEvent,
        bundle.events.iter().map(|event| event.event_id.as_str()),
    );
    validate_duplicates(
        &mut issues,
        BundleIdKind::Observation,
        bundle
            .observations
            .iter()
            .map(|observation| observation.observation_id.as_str()),
    );
    validate_duplicates(
        &mut issues,
        BundleIdKind::WsjtXRecord,
        bundle.wsjtx.iter().map(|record| record.record_id.as_str()),
    );
    validate_duplicates(
        &mut issues,
        BundleIdKind::RigRecord,
        bundle.rig.iter().map(|record| record.record_id.as_str()),
    );
    validate_duplicates(
        &mut issues,
        BundleIdKind::PropagationRecord,
        bundle
            .propagation
            .iter()
            .map(|record| record.record_id.as_str()),
    );

    validate_schedule_references_and_windows(&mut issues, bundle);
    validate_event_and_observation_references(&mut issues, bundle);
    validate_slot_confidence_ranges(&mut issues, bundle);

    issues
}

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
                previous.starts_at + Duration::seconds(i64::from(previous.duration_seconds));
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
