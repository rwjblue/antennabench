use crate::BundleFileRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BundleValidationProfile {
    CompatibilityRead,
    Analysis,
    StrictCreation,
    Upgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BundleDiagnosticCategory {
    Wire,
    Structural,
    Semantic,
    Eligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BundleDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BundleRecordKind {
    Antenna,
    Slot,
    OperatorEvent,
    Observation,
    AdapterRecord,
    WsjtXRecord,
    RigRecord,
    PropagationRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BundleDiagnosticLocation {
    pub file: BundleFileRole,
    pub record_kind: Option<BundleRecordKind>,
    pub record_id: Option<String>,
    pub record_index: Option<usize>,
    pub physical_line: Option<usize>,
    pub field_path: Option<String>,
}

impl BundleDiagnosticLocation {
    pub fn file(file: BundleFileRole) -> Self {
        Self {
            file,
            record_kind: None,
            record_id: None,
            record_index: None,
            physical_line: None,
            field_path: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleDiagnostic {
    pub code: String,
    pub category: BundleDiagnosticCategory,
    pub severity: BundleDiagnosticSeverity,
    pub blocked_operations: Vec<BundleValidationProfile>,
    pub location: BundleDiagnosticLocation,
    pub message: String,
    pub related_locations: Vec<BundleDiagnosticLocation>,
}

impl BundleDiagnostic {
    pub fn blocks(&self, profile: BundleValidationProfile) -> bool {
        self.blocked_operations.contains(&profile)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleValidationReport {
    diagnostics: Vec<BundleDiagnostic>,
}

impl BundleValidationReport {
    pub fn new(diagnostics: Vec<BundleDiagnostic>) -> Self {
        let mut report = Self { diagnostics };
        report.sort_deterministically();
        report
    }

    pub fn diagnostics(&self) -> &[BundleDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<BundleDiagnostic> {
        self.diagnostics
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn allows(&self, profile: BundleValidationProfile) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocks(profile))
    }

    pub fn blocking_diagnostics(
        &self,
        profile: BundleValidationProfile,
    ) -> impl Iterator<Item = &BundleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(move |diagnostic| diagnostic.blocks(profile))
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = BundleDiagnostic>) {
        self.diagnostics.extend(diagnostics);
        self.sort_deterministically();
    }

    fn sort_deterministically(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            (
                left.location.file,
                left.location.record_index,
                &left.location.field_path,
                &left.code,
                &left.location.record_id,
            )
                .cmp(&(
                    right.location.file,
                    right.location.record_index,
                    &right.location.field_path,
                    &right.code,
                    &right.location.record_id,
                ))
        });
        for diagnostic in &mut self.diagnostics {
            diagnostic.blocked_operations.sort_unstable();
            diagnostic.blocked_operations.dedup();
            diagnostic.related_locations.sort();
            diagnostic.related_locations.dedup();
        }
    }
}

pub mod codes {
    pub const INVALID_JSON: &str = "bundle.wire.invalid_json";
    pub const DUPLICATE_MEMBER: &str = "bundle.wire.duplicate_member";
    pub const DUPLICATE_RAW_MEMBER: &str = "bundle.wire.duplicate_raw_member";
    pub const UNKNOWN_FIELD: &str = "bundle.wire.unknown_field";
    pub const UNSUPPORTED_SCHEMA_VERSION: &str = "bundle.wire.unsupported_schema_version";
    pub const SESSION_ID_MISMATCH: &str = "bundle.structure.session_id_mismatch";
    pub const DUPLICATE_ID: &str = "bundle.structure.duplicate_id";
    pub const UNKNOWN_ANTENNA_LABEL: &str = "bundle.structure.unknown_antenna_label";
    pub const SLOT_WINDOW_OUT_OF_ORDER: &str = "bundle.structure.slot_window_out_of_order";
    pub const SLOT_WINDOW_OVERLAP: &str = "bundle.structure.slot_window_overlap";
    pub const UNKNOWN_EVENT_SLOT: &str = "bundle.structure.unknown_event_slot";
    pub const UNKNOWN_OBSERVATION_SLOT: &str = "bundle.structure.unknown_observation_slot";
    pub const INVALID_SLOT_CONFIDENCE: &str = "bundle.semantic.invalid_slot_confidence";
    pub const ALIGNMENT_ANNOTATION_MISMATCH: &str = "bundle.semantic.alignment_annotation_mismatch";
    pub const EMPTY_IDENTITY: &str = "bundle.semantic.empty_identity";
    pub const INVALID_IDENTITY: &str = "bundle.semantic.invalid_identity";
    pub const INVALID_REQUIRED_TEXT: &str = "bundle.semantic.invalid_required_text";
    pub const INVALID_ANTENNA_LABEL: &str = "bundle.semantic.invalid_antenna_label";
    pub const DUPLICATE_ANTENNA_LABEL: &str = "bundle.semantic.duplicate_antenna_label";
    pub const EMPTY_SCHEDULE: &str = "bundle.semantic.empty_schedule";
    pub const DUPLICATE_SEQUENCE_NUMBER: &str = "bundle.semantic.duplicate_sequence_number";
    pub const SLOT_SEQUENCE_OUT_OF_ORDER: &str = "bundle.semantic.slot_sequence_out_of_order";
    pub const INVALID_SLOT_DURATION: &str = "bundle.semantic.invalid_slot_duration";
    pub const INVALID_SLOT_GUARD: &str = "bundle.semantic.invalid_slot_guard";
    pub const EXPERIMENT_SHAPE_MISMATCH: &str = "bundle.semantic.experiment_shape_mismatch";
    pub const NON_FINITE_NUMBER: &str = "bundle.semantic.non_finite_number";
    pub const INVALID_RANGE: &str = "bundle.semantic.invalid_range";
    pub const ANALYSIS_METADATA_MISMATCH: &str = "bundle.semantic.analysis_metadata_mismatch";
    pub const V2_CHECKPOINT_MISMATCH: &str = "bundle.structure.v2_checkpoint_mismatch";
    pub const V2_ADAPTER_LINK: &str = "bundle.structure.v2_adapter_link";
    pub const V2_ATTACHMENT: &str = "bundle.structure.v2_attachment";
    pub const V2_MUTATION: &str = "bundle.structure.v2_mutation";
    pub const V2_EVENT_SEMANTICS: &str = "bundle.semantic.v2_operator_event";
    pub const V2_EVENT_CONFLICT: &str = "bundle.eligibility.v2_operator_event_conflict";
    pub const V2_LIFECYCLE_MISMATCH: &str = "bundle.structure.v2_lifecycle_mismatch";
}

pub const ALL_TYPED_OPERATIONS: [BundleValidationProfile; 4] = [
    BundleValidationProfile::CompatibilityRead,
    BundleValidationProfile::Analysis,
    BundleValidationProfile::StrictCreation,
    BundleValidationProfile::Upgrade,
];

pub const ANALYSIS_AND_WRITE_OPERATIONS: [BundleValidationProfile; 3] = [
    BundleValidationProfile::Analysis,
    BundleValidationProfile::StrictCreation,
    BundleValidationProfile::Upgrade,
];

pub const WRITE_OPERATIONS: [BundleValidationProfile; 2] = [
    BundleValidationProfile::StrictCreation,
    BundleValidationProfile::Upgrade,
];
