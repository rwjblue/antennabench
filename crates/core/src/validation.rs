use std::collections::HashSet;

use chrono::Duration;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    align_schedule_slots, codes, semantic_diagnostics, BundleContents, BundleDiagnostic,
    BundleDiagnosticCategory, BundleDiagnosticLocation, BundleDiagnosticSeverity, BundleRecordKind,
    BundleValidationProfile, BundleValidationReport, PlannedSlot, SlotAlignmentPolicy,
    ALL_TYPED_OPERATIONS, ANALYSIS_AND_WRITE_OPERATIONS, LATEST_SCHEMA_VERSION, SCHEMA_VERSION_V1,
    SCHEMA_VERSION_V2,
};

#[derive(Debug, Error, Clone, PartialEq)]
#[error("bundle validation failed with one or more issues")]
pub struct BundleValidationError {
    issues: Vec<BundleValidationIssue>,
    report: BundleValidationReport,
}

impl BundleValidationError {
    pub fn new(issues: Vec<BundleValidationIssue>) -> Self {
        assert!(
            !issues.is_empty(),
            "BundleValidationError requires at least one issue"
        );
        let diagnostics = issues
            .iter()
            .map(|issue| diagnostic_from_issue(issue, None))
            .collect();
        Self {
            issues,
            report: BundleValidationReport::new(diagnostics),
        }
    }

    pub fn from_report(report: BundleValidationReport) -> Self {
        assert!(
            !report.is_empty(),
            "BundleValidationError requires at least one diagnostic"
        );
        Self {
            issues: Vec::new(),
            report,
        }
    }

    fn from_issues_and_report(
        issues: Vec<BundleValidationIssue>,
        report: BundleValidationReport,
    ) -> Self {
        assert!(!issues.is_empty() || !report.is_empty());
        Self { issues, report }
    }

    pub fn issues(&self) -> &[BundleValidationIssue] {
        &self.issues
    }

    pub fn into_issues(self) -> Vec<BundleValidationIssue> {
        self.issues
    }

    pub fn report(&self) -> &BundleValidationReport {
        &self.report
    }

    pub fn diagnostic_count(&self) -> usize {
        self.report.diagnostics().len()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BundleFileRole {
    Manifest,
    SessionState,
    Station,
    Antennas,
    Schedule,
    Events,
    Observations,
    AdapterRecords,
    WsjtX,
    Rig,
    Propagation,
    Analysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    let report = report_from_issues(bundle, &issues);

    if report.is_empty() {
        Ok(())
    } else {
        Err(BundleValidationError::from_issues_and_report(
            issues, report,
        ))
    }
}

pub fn validate_bundle_report(bundle: &BundleContents) -> BundleValidationReport {
    report_from_issues(bundle, &validate_bundle_issues(bundle))
}

fn validate_bundle_issues(bundle: &BundleContents) -> Vec<BundleValidationIssue> {
    let mut issues = Vec::new();
    let expected_session_id = bundle.manifest.session_id.as_str();
    let expected_schema_version = bundle.manifest.schema_version;

    if !matches!(
        expected_schema_version,
        SCHEMA_VERSION_V1 | SCHEMA_VERSION_V2
    ) {
        issues.push(BundleValidationIssue::UnexpectedSchemaVersion {
            file: BundleFileRole::Manifest,
            record_id: None,
            expected: LATEST_SCHEMA_VERSION,
            actual: expected_schema_version,
        });
    }

    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Manifest,
        bundle.manifest.schema_version,
        expected_schema_version,
        bundle.manifest.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Station,
        bundle.station.schema_version,
        expected_schema_version,
        bundle.station.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Antennas,
        bundle.antennas.schema_version,
        expected_schema_version,
        bundle.antennas.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Schedule,
        bundle.schedule.schema_version,
        expected_schema_version,
        bundle.schedule.session_id.as_str(),
        expected_session_id,
    );
    validate_root_schema_and_session(
        &mut issues,
        BundleFileRole::Analysis,
        bundle.analysis.schema_version,
        expected_schema_version,
        bundle.analysis.session_id.as_str(),
        expected_session_id,
    );

    validate_record_meta(
        &mut issues,
        BundleFileRole::Events,
        expected_session_id,
        expected_schema_version,
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
        expected_schema_version,
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
        expected_schema_version,
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
        expected_schema_version,
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
        expected_schema_version,
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
    validate_alignment_annotations(&mut issues, bundle);

    issues
}

fn validate_root_schema_and_session(
    issues: &mut Vec<BundleValidationIssue>,
    file: BundleFileRole,
    actual_schema_version: u16,
    expected_schema_version: u16,
    actual_session_id: &str,
    expected_session_id: &str,
) {
    if actual_schema_version != expected_schema_version {
        issues.push(BundleValidationIssue::UnexpectedSchemaVersion {
            file,
            record_id: None,
            expected: expected_schema_version,
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
    expected_schema_version: u16,
    records: impl IntoIterator<Item = (&'a str, u16, &'a str)>,
) {
    for (record_id, actual_schema_version, actual_session_id) in records {
        if actual_schema_version != expected_schema_version {
            issues.push(BundleValidationIssue::UnexpectedSchemaVersion {
                file,
                record_id: Some(record_id.to_string()),
                expected: expected_schema_version,
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
            if slot.starts_at <= previous.starts_at {
                issues.push(BundleValidationIssue::SlotWindowOutOfOrder {
                    previous_slot_id: previous.slot_id.clone(),
                    slot_id: slot.slot_id.clone(),
                });
            }

            let previous_ends_at =
                previous.starts_at + Duration::seconds(i64::from(previous.duration_seconds));
            let slot_ends_at = slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds));
            if slot.starts_at < previous_ends_at && previous.starts_at < slot_ends_at {
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
    for (observation, assignment) in bundle
        .observations
        .iter()
        .zip(alignment.observation_assignments.iter())
    {
        if observation.slot_id != assignment.slot_id {
            push_alignment_annotation_mismatch(
                issues,
                observation.observation_id.as_str(),
                AlignmentAnnotationField::SlotId,
                format!("{:?}", assignment.slot_id),
                format!("{:?}", observation.slot_id),
            );
        }

        if observation.slot_label != assignment.slot_label {
            push_alignment_annotation_mismatch(
                issues,
                observation.observation_id.as_str(),
                AlignmentAnnotationField::SlotLabel,
                format!("{:?}", assignment.slot_label),
                format!("{:?}", observation.slot_label),
            );
        }

        let expected_slot_confidence = Some(assignment.confidence);
        if !slot_confidence_matches(observation.slot_confidence, expected_slot_confidence) {
            push_alignment_annotation_mismatch(
                issues,
                observation.observation_id.as_str(),
                AlignmentAnnotationField::SlotConfidence,
                format!("{:?}", expected_slot_confidence),
                format!("{:?}", observation.slot_confidence),
            );
        }
    }
}

fn slot_confidence_matches(actual: Option<f32>, expected: Option<f32>) -> bool {
    const SLOT_CONFIDENCE_TOLERANCE: f32 = 0.000_001;

    match (actual, expected) {
        (Some(actual), Some(expected)) => (actual - expected).abs() <= SLOT_CONFIDENCE_TOLERANCE,
        (None, None) => true,
        _ => false,
    }
}

fn push_alignment_annotation_mismatch(
    issues: &mut Vec<BundleValidationIssue>,
    observation_id: &str,
    field: AlignmentAnnotationField,
    expected: String,
    actual: String,
) {
    issues.push(BundleValidationIssue::AlignmentAnnotationMismatch {
        observation_id: observation_id.to_string(),
        field,
        expected,
        actual,
    });
}

fn report_from_issues(
    bundle: &BundleContents,
    issues: &[BundleValidationIssue],
) -> BundleValidationReport {
    let mut report = BundleValidationReport::new(
        issues
            .iter()
            .map(|issue| diagnostic_from_issue(issue, Some(bundle)))
            .collect(),
    );
    report.extend(semantic_diagnostics(bundle));
    report
}

fn diagnostic_from_issue(
    issue: &BundleValidationIssue,
    bundle: Option<&BundleContents>,
) -> BundleDiagnostic {
    match issue {
        BundleValidationIssue::UnexpectedSchemaVersion {
            file,
            record_id,
            expected,
            actual,
        } => BundleDiagnostic {
            code: codes::UNSUPPORTED_SCHEMA_VERSION.to_string(),
            category: BundleDiagnosticCategory::Wire,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: record_location(
                bundle,
                *file,
                record_id.as_deref(),
                record_id
                    .as_ref()
                    .map_or("/schema_version", |_| "/meta/schema_version"),
            ),
            message: format!(
                "schema version {actual} is not supported; expected {expected}"
            ),
            related_locations: Vec::new(),
        },
        BundleValidationIssue::SessionIdMismatch {
            file,
            record_id,
            expected,
            actual,
        } => BundleDiagnostic {
            code: codes::SESSION_ID_MISMATCH.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: record_location(
                bundle,
                *file,
                record_id.as_deref(),
                record_id
                    .as_ref()
                    .map_or("/session_id", |_| "/meta/session_id"),
            ),
            message: format!("session ID {actual:?} does not match manifest ID {expected:?}"),
            related_locations: vec![BundleDiagnosticLocation {
                field_path: Some("/session_id".to_string()),
                ..BundleDiagnosticLocation::file(BundleFileRole::Manifest)
            }],
        },
        BundleValidationIssue::DuplicateId { kind, id } => {
            let (location, related_locations) = duplicate_id_locations(bundle, *kind, id);
            BundleDiagnostic {
                code: codes::DUPLICATE_ID.to_string(),
                category: BundleDiagnosticCategory::Structural,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
                location,
                message: format!("durable {:?} ID {id:?} is duplicated", kind),
                related_locations,
            }
        }
        BundleValidationIssue::UnknownAntennaLabel {
            slot_id,
            antenna_label,
        } => BundleDiagnostic {
            code: codes::UNKNOWN_ANTENNA_LABEL.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: slot_location(bundle, slot_id, "/antenna_label"),
            message: format!(
                "slot {slot_id:?} references unknown antenna label {antenna_label:?}"
            ),
            related_locations: Vec::new(),
        },
        BundleValidationIssue::SlotWindowOutOfOrder {
            previous_slot_id,
            slot_id,
        } => BundleDiagnostic {
            code: codes::SLOT_WINDOW_OUT_OF_ORDER.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: slot_location(bundle, slot_id, "/starts_at"),
            message: format!(
                "slot {slot_id:?} does not start strictly after preceding slot {previous_slot_id:?}"
            ),
            related_locations: vec![slot_location(bundle, previous_slot_id, "/starts_at")],
        },
        BundleValidationIssue::SlotWindowOverlap {
            previous_slot_id,
            previous_ends_at,
            slot_id,
            starts_at,
        } => BundleDiagnostic {
            code: codes::SLOT_WINDOW_OVERLAP.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: slot_location(bundle, slot_id, "/starts_at"),
            message: format!(
                "slot {slot_id:?} starts at {starts_at} before slot {previous_slot_id:?} ends at {previous_ends_at}"
            ),
            related_locations: vec![slot_location(bundle, previous_slot_id, "/duration_seconds")],
        },
        BundleValidationIssue::UnknownEventSlot { event_id, slot_id } => BundleDiagnostic {
            code: codes::UNKNOWN_EVENT_SLOT.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: record_location(
                bundle,
                BundleFileRole::Events,
                Some(event_id),
                "/slot_id",
            ),
            message: format!("event {event_id:?} references unknown slot {slot_id:?}"),
            related_locations: Vec::new(),
        },
        BundleValidationIssue::UnknownObservationSlot {
            observation_id,
            slot_id,
        } => BundleDiagnostic {
            code: codes::UNKNOWN_OBSERVATION_SLOT.to_string(),
            category: BundleDiagnosticCategory::Structural,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
            location: record_location(
                bundle,
                BundleFileRole::Observations,
                Some(observation_id),
                "/slot_id",
            ),
            message: format!(
                "observation {observation_id:?} references unknown slot {slot_id:?}"
            ),
            related_locations: Vec::new(),
        },
        BundleValidationIssue::InvalidSlotConfidence {
            observation_id,
            slot_confidence,
        } => BundleDiagnostic {
            code: codes::INVALID_SLOT_CONFIDENCE.to_string(),
            category: BundleDiagnosticCategory::Semantic,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
            location: record_location(
                bundle,
                BundleFileRole::Observations,
                Some(observation_id),
                "/slot_confidence",
            ),
            message: format!(
                "observation {observation_id:?} has slot confidence {slot_confidence}, outside [0, 1]"
            ),
            related_locations: Vec::new(),
        },
        BundleValidationIssue::AlignmentAnnotationMismatch {
            observation_id,
            field,
            expected,
            actual,
        } => {
            let field_path = match field {
                AlignmentAnnotationField::SlotId => "/slot_id",
                AlignmentAnnotationField::SlotLabel => "/slot_label",
                AlignmentAnnotationField::SlotConfidence => "/slot_confidence",
            };
            BundleDiagnostic {
                code: codes::ALIGNMENT_ANNOTATION_MISMATCH.to_string(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Warning,
                blocked_operations: vec![BundleValidationProfile::StrictCreation],
                location: record_location(
                    bundle,
                    BundleFileRole::Observations,
                    Some(observation_id),
                    field_path,
                ),
                message: format!(
                    "persisted alignment annotation for observation {observation_id:?} is {actual}; regenerated value is {expected}"
                ),
                related_locations: Vec::new(),
            }
        }
    }
}

fn record_location(
    bundle: Option<&BundleContents>,
    file: BundleFileRole,
    record_id: Option<&str>,
    field_path: &str,
) -> BundleDiagnosticLocation {
    let (record_kind, record_index) = record_id.map_or((None, None), |record_id| {
        let (kind, index) = match (bundle, file) {
            (Some(bundle), BundleFileRole::Events) => (
                Some(BundleRecordKind::OperatorEvent),
                bundle
                    .events
                    .iter()
                    .position(|record| record.event_id == record_id),
            ),
            (Some(bundle), BundleFileRole::Observations) => (
                Some(BundleRecordKind::Observation),
                bundle
                    .observations
                    .iter()
                    .position(|record| record.observation_id == record_id),
            ),
            (Some(bundle), BundleFileRole::WsjtX) => (
                Some(BundleRecordKind::WsjtXRecord),
                bundle
                    .wsjtx
                    .iter()
                    .position(|record| record.record_id == record_id),
            ),
            (Some(bundle), BundleFileRole::Rig) => (
                Some(BundleRecordKind::RigRecord),
                bundle
                    .rig
                    .iter()
                    .position(|record| record.record_id == record_id),
            ),
            (Some(bundle), BundleFileRole::Propagation) => (
                Some(BundleRecordKind::PropagationRecord),
                bundle
                    .propagation
                    .iter()
                    .position(|record| record.record_id == record_id),
            ),
            _ => (None, None),
        };
        (kind, index)
    });

    BundleDiagnosticLocation {
        file,
        record_kind,
        record_id: record_id.map(str::to_string),
        record_index,
        physical_line: None,
        field_path: Some(field_path.to_string()),
    }
}

fn slot_location(
    bundle: Option<&BundleContents>,
    slot_id: &str,
    field_path: &str,
) -> BundleDiagnosticLocation {
    let record_index = bundle.and_then(|bundle| {
        bundle
            .schedule
            .slots
            .iter()
            .position(|slot| slot.slot_id == slot_id)
    });
    BundleDiagnosticLocation {
        file: BundleFileRole::Schedule,
        record_kind: Some(BundleRecordKind::Slot),
        record_id: Some(slot_id.to_string()),
        record_index,
        physical_line: None,
        field_path: Some(record_index.map_or_else(
            || field_path.to_string(),
            |index| format!("/slots/{index}{field_path}"),
        )),
    }
}

fn duplicate_id_locations(
    bundle: Option<&BundleContents>,
    kind: BundleIdKind,
    id: &str,
) -> (BundleDiagnosticLocation, Vec<BundleDiagnosticLocation>) {
    let (file, record_kind, indices): (BundleFileRole, BundleRecordKind, Vec<usize>) =
        match (bundle, kind) {
            (Some(bundle), BundleIdKind::Slot) => (
                BundleFileRole::Schedule,
                BundleRecordKind::Slot,
                bundle
                    .schedule
                    .slots
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.slot_id == id).then_some(index))
                    .collect(),
            ),
            (Some(bundle), BundleIdKind::OperatorEvent) => (
                BundleFileRole::Events,
                BundleRecordKind::OperatorEvent,
                bundle
                    .events
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.event_id == id).then_some(index))
                    .collect(),
            ),
            (Some(bundle), BundleIdKind::Observation) => (
                BundleFileRole::Observations,
                BundleRecordKind::Observation,
                bundle
                    .observations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.observation_id == id).then_some(index))
                    .collect(),
            ),
            (Some(bundle), BundleIdKind::WsjtXRecord) => (
                BundleFileRole::WsjtX,
                BundleRecordKind::WsjtXRecord,
                bundle
                    .wsjtx
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.record_id == id).then_some(index))
                    .collect(),
            ),
            (Some(bundle), BundleIdKind::RigRecord) => (
                BundleFileRole::Rig,
                BundleRecordKind::RigRecord,
                bundle
                    .rig
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.record_id == id).then_some(index))
                    .collect(),
            ),
            (Some(bundle), BundleIdKind::PropagationRecord) => (
                BundleFileRole::Propagation,
                BundleRecordKind::PropagationRecord,
                bundle
                    .propagation
                    .iter()
                    .enumerate()
                    .filter_map(|(index, record)| (record.record_id == id).then_some(index))
                    .collect(),
            ),
            (None, BundleIdKind::Slot) => {
                (BundleFileRole::Schedule, BundleRecordKind::Slot, Vec::new())
            }
            (None, BundleIdKind::OperatorEvent) => (
                BundleFileRole::Events,
                BundleRecordKind::OperatorEvent,
                Vec::new(),
            ),
            (None, BundleIdKind::Observation) => (
                BundleFileRole::Observations,
                BundleRecordKind::Observation,
                Vec::new(),
            ),
            (None, BundleIdKind::WsjtXRecord) => (
                BundleFileRole::WsjtX,
                BundleRecordKind::WsjtXRecord,
                Vec::new(),
            ),
            (None, BundleIdKind::RigRecord) => {
                (BundleFileRole::Rig, BundleRecordKind::RigRecord, Vec::new())
            }
            (None, BundleIdKind::PropagationRecord) => (
                BundleFileRole::Propagation,
                BundleRecordKind::PropagationRecord,
                Vec::new(),
            ),
        };
    let field_name = if kind == BundleIdKind::Slot {
        "slot_id"
    } else if kind == BundleIdKind::OperatorEvent {
        "event_id"
    } else if kind == BundleIdKind::Observation {
        "observation_id"
    } else {
        "record_id"
    };
    let make_location = |index: Option<usize>| BundleDiagnosticLocation {
        file,
        record_kind: Some(record_kind),
        record_id: Some(id.to_string()),
        record_index: index,
        physical_line: matches!(
            file,
            BundleFileRole::Events
                | BundleFileRole::Observations
                | BundleFileRole::WsjtX
                | BundleFileRole::Rig
                | BundleFileRole::Propagation
        )
        .then(|| index.map(|index| index + 1))
        .flatten(),
        field_path: Some(match (kind, index) {
            (BundleIdKind::Slot, Some(index)) => format!("/slots/{index}/{field_name}"),
            _ => format!("/{field_name}"),
        }),
    };
    let location = make_location(indices.get(1).copied());
    let related = indices
        .first()
        .copied()
        .map(|index| vec![make_location(Some(index))])
        .unwrap_or_default();
    (location, related)
}
