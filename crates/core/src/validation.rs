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
