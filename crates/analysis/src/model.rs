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
    #[serde(default, skip_serializing_if = "ReporterActivityAnalysis::is_empty")]
    pub reporter_activity: ReporterActivityAnalysis,
    pub solar_context: SolarContextAnalysis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusion_records: Vec<ObservationExclusionRecord>,
    #[serde(default, skip_serializing_if = "EvidenceEligibility::is_empty")]
    pub eligibility: EvidenceEligibility,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReporterActivityAnalysis {
    pub census_cycles: Vec<ReporterActivityCensusCycle>,
    pub cycle_rates: Vec<ReporterActivityCycleRate>,
    pub paired_rates: Vec<ReporterActivityPairedRate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joint_summaries: Vec<ReporterActivityJointSummary>,
}

impl ReporterActivityAnalysis {
    pub fn is_empty(&self) -> bool {
        self.census_cycles.is_empty()
            && self.cycle_rates.is_empty()
            && self.paired_rates.is_empty()
            && self.joint_summaries.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReporterActivityCensusCycle {
    pub cycle_time: DateTime<Utc>,
    pub band: Band,
    pub coverage: ReporterActivityCoverage,
    pub active_reporters: Vec<ReporterActivityReporter>,
    pub summary_record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReporterActivityReporter {
    pub reporter: String,
    pub reporter_grid: Option<String>,
    pub census_record_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "reason")]
pub enum ReporterActivityCoverage {
    Unknown(ReporterActivityUnknownReason),
    Complete,
    Partial,
    Truncated,
}

impl ReporterActivityCoverage {
    pub fn is_known(self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReporterActivityUnknownReason {
    NoCensusCoverage,
    UnsupportedReceiveDirection,
    UnsupportedSignalMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReporterActivityCycleRate {
    pub stratum: ComparisonStratum,
    pub block_index: Option<usize>,
    pub side: Option<ComparisonSide>,
    pub slot_id: String,
    pub antenna_label: String,
    pub cycle_starts_at: DateTime<Utc>,
    pub census_cycle_index: Option<usize>,
    pub coverage: ReporterActivityCoverage,
    pub active_reporter_count: usize,
    pub heard_reporter_count: usize,
    pub hearing_rate: Option<f64>,
    pub heard_reporters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReporterActivityPairedRate {
    pub stratum: ComparisonStratum,
    pub block_index: usize,
    pub order: ComparisonOrder,
    pub coverage: ReporterActivityCoverage,
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub active_in_both_count: usize,
    pub left_heard_count: usize,
    pub right_heard_count: usize,
    pub left_hearing_rate: Option<f64>,
    pub right_hearing_rate: Option<f64>,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
    pub receivers: Vec<ReporterActivityJointReceiver>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReporterActivityJointOutcome {
    HeardBoth,
    LeftOnly,
    RightOnly,
    HeardNeither,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReporterActivityJointReceiver {
    pub receiver: String,
    pub receiver_grid: Option<String>,
    pub outcome: ReporterActivityJointOutcome,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReporterActivityJointSummary {
    pub stratum: ComparisonStratum,
    pub coverage: ReporterActivityCoverage,
    pub eligible_block_count: usize,
    pub known_coverage_block_count: usize,
    pub left_then_right_block_count: usize,
    pub right_then_left_block_count: usize,
    pub unique_active_receiver_count: usize,
    pub receiver_block_opportunity_count: usize,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
    pub left_detection_rate: Option<f64>,
    pub right_detection_rate: Option<f64>,
}

/// Record-level accounting for an observation excluded by the existing
/// eligibility and alignment rules. The durable observation remains the source
/// of truth; this projection makes the exact disposition auditable in reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationExclusionRecord {
    pub observation_id: String,
    pub reason: ObservationExclusionReason,
    pub timestamp: DateTime<Utc>,
    pub band: Band,
    pub observation_kind: ObservationKind,
    pub source: RecordSource,
    pub mode: Option<String>,
    pub slot_id: Option<String>,
    pub assigned_label: Option<String>,
    pub assignment_confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolarContextAnalysis {
    pub algorithm: SolarContextAlgorithm,
    pub rows: Vec<SolarContextRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolarContextAlgorithm {
    pub algorithm_id: String,
    pub algorithm_version: u16,
    pub coordinate_method: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolarContextRow {
    pub stratum: ComparisonStratum,
    pub block_index: usize,
    pub order: ComparisonOrder,
    pub remote_path: String,
    pub left: SolarObservationContext,
    pub right: SolarObservationContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolarObservationContext {
    pub observation_id: String,
    pub timestamp: DateTime<Utc>,
    pub station: SolarEndpointContext,
    pub remote: SolarEndpointContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarEndpointRole {
    Station,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolarEndpointContext {
    pub role: SolarEndpointRole,
    pub endpoint_id: String,
    pub grid: Option<String>,
    pub result: SolarPositionResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SolarPositionResult {
    Available {
        coordinates: SolarCoordinates,
        elevation_degrees: f64,
        light_state: SolarLightState,
        gray_line: bool,
    },
    Missing {
        reason: SolarContextMissingReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolarCoordinates {
    pub latitude_degrees: f64,
    pub longitude_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarContextMissingReason {
    MissingGrid,
    InvalidGrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarLightState {
    Daylight,
    CivilTwilight,
    NauticalTwilight,
    AstronomicalTwilight,
    Night,
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
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub usable_start: DateTime<Utc>,
    pub switch_event_id: Option<String>,
    pub switch_timestamp: Option<DateTime<Utc>>,
    pub switch_delay_seconds: Option<i64>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_path_profiles: Vec<ObservedAntennaPathProfile>,
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
pub struct ObservedAntennaPathProfile {
    pub stratum: ComparisonStratum,
    pub side: ComparisonSide,
    pub antenna_label: String,
    pub unique_path_count: usize,
    pub located_path_count: usize,
    pub missing_location_path_count: usize,
    pub inconsistent_location_path_count: usize,
    pub paths: Vec<ObservedAntennaPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedAntennaPath {
    pub remote_path: String,
    pub location: ObservedPathLocation,
    pub block_support_count: usize,
    pub slot_support_count: usize,
    pub observation_count: usize,
    pub block_indices: Vec<usize>,
    pub slot_ids: Vec<String>,
    pub observation_ids: Vec<String>,
    pub snr: Option<SnrStatistics>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "availability")]
pub enum ObservedPathLocation {
    Available {
        remote_grid: String,
        distance_km: f64,
        initial_bearing_degrees: f64,
    },
    Missing,
    Inconsistent,
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
    pub excluded_observation_count: usize,
    pub exact_duplicate_count: usize,
    pub conflicting_duplicate_group_count: usize,
    pub minimum_delta_right_minus_left_db: Option<f64>,
    pub median_path_delta_right_minus_left_db: Option<f64>,
    pub maximum_delta_right_minus_left_db: Option<f64>,
}
