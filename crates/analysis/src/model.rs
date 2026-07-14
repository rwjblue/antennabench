use antennabench_core::{
    AlignedSlotStatus, Band, BundleValidationError, ObservationKind, RecordSource,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::AnalysisResourceError;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AnalysisError {
    #[error("bundle is not valid for analysis")]
    InvalidBundle(#[from] BundleValidationError),
    #[error("observation {observation_id} has a non-finite SNR")]
    NonFiniteSnr { observation_id: String },
    #[error(transparent)]
    Resource(#[from] AnalysisResourceError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub session_id: String,
    pub evidence_quality: EvidenceQuality,
    pub overall: EvidenceSummary,
    pub antennas: Vec<AntennaEvidenceSummary>,
    pub bands: Vec<BandEvidenceSummary>,
    pub slots: Vec<SlotEvidenceSummary>,
    pub comparison: PairedComparisonAnalysis,
    #[serde(default, skip_serializing_if = "EvidenceEligibility::is_empty")]
    pub eligibility: EvidenceEligibility,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceEligibility {
    pub exclusions: Vec<EligibilityExclusionCount>,
}

impl EvidenceEligibility {
    pub fn is_empty(&self) -> bool {
        self.exclusions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EligibilityExclusionCount {
    pub code: String,
    pub category: EligibilityExclusionCategory,
    pub scope: EligibilityScope,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityExclusionCategory {
    Missing,
    Malformed,
    Contradictory,
    Unsupported,
    Duplicate,
    DeliberatelyExcluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityScope {
    Field,
    Observation,
    Slot,
    ComparisonStratum,
    ComparisonBlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSummary {
    pub observation_counts: ObservationCounts,
    pub exclusions: Vec<ExclusionCount>,
    pub usable_observation_kinds: Vec<ObservationKindCount>,
    pub snr: Option<SnrStatistics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationCounts {
    pub total: usize,
    pub usable: usize,
    pub excluded: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExclusionCount {
    pub reason: ObservationExclusionReason,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationKindCount {
    pub kind: ObservationKind,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntennaEvidenceSummary {
    pub antenna_label: String,
    pub contributing_slot_count: usize,
    pub evidence_quality: EvidenceQuality,
    pub evidence: EvidenceSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandEvidenceSummary {
    pub band: Band,
    pub evidence: EvidenceSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotEvidenceSummary {
    pub slot_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub planned_label: String,
    pub actual_label: Option<String>,
    pub status: AlignedSlotStatus,
    pub evidence: EvidenceSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnrStatistics {
    pub sample_count: usize,
    pub min_db: f64,
    pub median_db: f64,
    pub mean_db: f64,
    pub max_db: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceQuality {
    Insufficient,
    Weak,
    Moderate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationExclusionReason {
    GuardTime,
    NearBoundary,
    BeforeObservedSwitch,
    MissedSlot,
    BadSlot,
    BandMismatch,
    OutsideSchedule,
    MissingEvidence,
    MalformedEvidence,
    ContradictoryEvidence,
    UnsupportedEvidence,
    DuplicateEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedComparisonAnalysis {
    pub availability: ComparisonAvailability,
    pub left_label: Option<String>,
    pub right_label: Option<String>,
    pub delta_orientation: Option<DeltaOrientation>,
    pub diagnostics: ComparisonDiagnostics,
    pub blocks: Vec<ComparisonBlock>,
    pub overlap_rows: Vec<PathOverlapRow>,
    pub timeline_rows: Vec<ComparisonTimelineRow>,
    pub paired_rows: Vec<PairedObservationRow>,
    pub path_summaries: Vec<PairedPathSummary>,
    pub strata: Vec<PairedStratumSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonAvailability {
    NotApplicable,
    UnsupportedComparisonShape,
    NoEligibleBlocks,
    NoMatchedPaths,
    DescriptivePairsAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaOrientation {
    pub minuend_label: String,
    pub subtrahend_label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonDiagnostics {
    pub block_count: usize,
    pub eligible_block_count: usize,
    pub invalid_block_count: usize,
    pub left_then_right_block_count: usize,
    pub right_then_left_block_count: usize,
    pub paired_row_count: usize,
    pub unique_path_count: usize,
    pub unmatched_left_count: usize,
    pub unmatched_right_count: usize,
    pub missing_snr_left_count: usize,
    pub missing_snr_right_count: usize,
    pub missing_or_invalid_mode_count: usize,
    #[serde(default, skip_serializing_if = "usize_is_zero")]
    pub missing_mode_count: usize,
    #[serde(default, skip_serializing_if = "usize_is_zero")]
    pub malformed_mode_count: usize,
    pub ambiguous_path_count: usize,
    pub exact_duplicate_count: usize,
    pub conflicting_duplicate_group_count: usize,
    pub excluded_observation_count: usize,
}

fn usize_is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonBlock {
    pub block_index: usize,
    pub band: Band,
    pub first_slot_id: String,
    pub first_sequence_number: u32,
    pub first_starts_at: DateTime<Utc>,
    pub first_label: Option<String>,
    pub first_status: AlignedSlotStatus,
    pub second_slot_id: Option<String>,
    pub second_sequence_number: Option<u32>,
    pub second_starts_at: Option<DateTime<Utc>>,
    pub second_label: Option<String>,
    pub second_status: Option<AlignedSlotStatus>,
    pub order: Option<ComparisonOrder>,
    pub eligibility: ComparisonBlockEligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOrder {
    LeftThenRight,
    RightThenLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonBlockEligibility {
    Eligible,
    AmbiguousSequenceOrder,
    IncompleteSameBandRun,
    MissingActualLabel,
    RepeatedLabel,
    UnsupportedLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathDirection {
    Transmit,
    Receive,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignalMode(String);

impl SignalMode {
    pub fn normalize(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            return None;
        }
        Some(Self(value.to_ascii_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonStratum {
    pub direction: PathDirection,
    pub band: Band,
    pub mode: SignalMode,
    pub observation_kind: ObservationKind,
    pub source: RecordSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathOverlapRow {
    pub stratum: ComparisonStratum,
    pub remote_path: String,
    pub left_finite_count: usize,
    pub right_finite_count: usize,
    pub paired_count: usize,
    pub unmatched_left_count: usize,
    pub unmatched_right_count: usize,
    pub missing_snr_left_count: usize,
    pub missing_snr_right_count: usize,
    pub exact_duplicate_count: usize,
    pub conflicting_duplicate_group_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonTimelineRow {
    pub block_index: usize,
    pub block_eligible: bool,
    pub sequence_number: u32,
    pub slot_id: String,
    pub starts_at: DateTime<Utc>,
    pub band: Band,
    pub actual_label: Option<String>,
    pub side: Option<ComparisonSide>,
    pub status: AlignedSlotStatus,
    pub total_observation_count: usize,
    pub usable_observation_count: usize,
    pub excluded_observation_count: usize,
    pub missing_snr_count: usize,
    pub missing_or_invalid_mode_count: usize,
    pub ambiguous_path_count: usize,
    pub exact_duplicate_count: usize,
    pub conflicting_duplicate_group_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedObservationRow {
    pub stratum: ComparisonStratum,
    pub block_index: usize,
    pub order: ComparisonOrder,
    pub remote_path: String,
    pub left_observation_id: String,
    pub right_observation_id: String,
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub left_timestamp: DateTime<Utc>,
    pub right_timestamp: DateTime<Utc>,
    pub elapsed_seconds: i64,
    pub left_snr_db: f64,
    pub right_snr_db: f64,
    pub delta_right_minus_left_db: f64,
    pub left_remote_grid: Option<String>,
    pub right_remote_grid: Option<String>,
    pub left_distance_km: Option<f64>,
    pub right_distance_km: Option<f64>,
    pub left_azimuth_degrees: Option<f64>,
    pub right_azimuth_degrees: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedPathSummary {
    pub stratum: ComparisonStratum,
    pub remote_path: String,
    pub paired_row_count: usize,
    pub median_delta_right_minus_left_db: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedStratumSummary {
    pub stratum: ComparisonStratum,
    pub paired_row_count: usize,
    pub unique_path_count: usize,
    pub contributing_block_count: usize,
    pub left_then_right_block_count: usize,
    pub right_then_left_block_count: usize,
    pub unmatched_left_count: usize,
    pub unmatched_right_count: usize,
    pub missing_snr_left_count: usize,
    pub missing_snr_right_count: usize,
    pub exact_duplicate_count: usize,
    pub conflicting_duplicate_group_count: usize,
    pub minimum_delta_right_minus_left_db: Option<f64>,
    pub median_path_delta_right_minus_left_db: Option<f64>,
    pub maximum_delta_right_minus_left_db: Option<f64>,
}
