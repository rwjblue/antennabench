use antennabench_analysis::{
    AnalysisError, ComparisonAvailability, ComparisonBlock, ComparisonBlockEligibility,
    ComparisonDiagnostics, ComparisonSide, ComparisonStratum, ComparisonTimelineRow,
    DeltaOrientation, EligibilityExclusionCount, EvidenceQuality, ExclusionCount,
    ObservationCounts, ObservationExclusionRecord, PairedObservationRow, PairedPathSummary,
    PairedStratumSummary, PathDirection, PathOverlapRow, ReporterActivityAnalysis, SnrStatistics,
    SolarContextAnalysis,
};
use antennabench_core::{
    v2::SessionLifecycleV2,
    v3::WsprCycleDirection,
    v5::{AntennaControlDispositionV5, AntennaControlOutputV5, AntennaControlRoleV5},
    AlignedSlotStatus, Antenna, Band, ExperimentMode, SessionGoal,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ReportResourceError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionReport {
    #[serde(default, skip_serializing_if = "ReportCompleteness::is_full_detail")]
    pub completeness: ReportCompleteness,
    #[serde(default)]
    pub overview: ReportOverview,
    pub context: SessionContext,
    pub evidence: EvidenceSections,
    pub comparison: ReportComparisonData,
    #[serde(default, skip_serializing_if = "ReporterActivityAnalysis::is_empty")]
    pub reporter_activity: ReporterActivityAnalysis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_maps: Vec<ReportCoverageMapGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common_opportunity_maps: Vec<ReportCommonOpportunityMapGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_overlap: Vec<ReportCoverageOverlapGroup>,
    pub solar_context: SolarContextAnalysis,
    pub chart_data: ReportChartData,
    pub notices: Vec<ReportNotice>,
    #[serde(default, skip_serializing_if = "ReportSnapshotContext::is_empty")]
    pub snapshot: ReportSnapshotContext,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eligibility_exclusions: Vec<EligibilityExclusionCount>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusion_records: Vec<ObservationExclusionRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCoverageMapGroup {
    pub stratum: ComparisonStratum,
    pub panels: Vec<ReportCoveragePanel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCoveragePanel {
    pub side: antennabench_analysis::ComparisonSide,
    pub antenna_label: String,
    pub coverage: antennabench_analysis::ReporterActivityCoverage,
    pub active_reporter_count: usize,
    pub heard_reporter_count: usize,
    pub mapped_reporter_count: usize,
    pub unmapped_reporter_count: usize,
    pub cells: Vec<ReportCoverageCell>,
    pub polar_cells: Vec<ReportCoveragePolarCell>,
    pub reporters: Vec<ReportCoverageReporter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportCoverageCell {
    pub maidenhead_4: String,
    pub state: ReportCoverageState,
    pub active_reporter_count: usize,
    pub heard_reporter_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportCoveragePolarCell {
    pub bearing_sector: u8,
    pub distance_ring: u8,
    pub state: ReportCoverageState,
    pub active_reporter_count: usize,
    pub heard_reporter_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportCoverageReporter {
    pub reporter: String,
    pub reporter_grid: String,
    pub state: ReportCoverageState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportCoverageState {
    Heard,
    ActiveNotHeard,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCommonOpportunityMapGroup {
    pub stratum: ComparisonStratum,
    pub coverage: antennabench_analysis::ReporterActivityCoverage,
    pub eligible_block_count: usize,
    pub known_coverage_block_count: usize,
    pub unique_common_active_receiver_count: usize,
    pub receiver_block_opportunity_count: usize,
    pub located_unique_receiver_count: usize,
    pub located_receiver_block_opportunity_count: usize,
    pub location_unavailable_unique_receiver_count: usize,
    pub location_unavailable_receiver_block_opportunity_count: usize,
    pub distance_cells: Vec<ReportCommonOpportunityCell<ReportDistanceBin>>,
    pub azimuth_cells: Vec<ReportCommonOpportunityCell<ReportAzimuthSector>>,
    pub polar_cells: Vec<ReportCommonOpportunityPolarCell>,
    pub blocks: Vec<ReportCommonOpportunityBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCommonOpportunityCell<T> {
    pub category: T,
    pub coverage: antennabench_analysis::ReporterActivityCoverage,
    pub unique_common_active_receiver_count: usize,
    pub receiver_block_opportunity_count: usize,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
    pub left_heard_count: usize,
    pub right_heard_count: usize,
    pub left_detection_rate: Option<f64>,
    pub right_detection_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCommonOpportunityPolarCell {
    pub bearing_sector: ReportAzimuthSector,
    pub distance_bin: ReportDistanceBin,
    pub facts: ReportCommonOpportunityCell<ReportDistanceBin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCommonOpportunityBlock {
    pub block_index: usize,
    pub order: antennabench_analysis::ComparisonOrder,
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub coverage: antennabench_analysis::ReporterActivityCoverage,
    pub common_active_receiver_count: usize,
    pub located_receiver_count: usize,
    pub location_unavailable_receiver_count: usize,
    pub polar_cells: Vec<ReportCommonOpportunityPolarCell>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportCoverageOverlapGroup {
    pub stratum: ComparisonStratum,
    pub observed: Option<ReportObservedComplementarity>,
    pub common_opportunity: Option<ReportOpportunityComplementarity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportObservedComplementarity {
    pub eligible_block_count: usize,
    pub left_only_unique_path_count: usize,
    pub shared_unique_path_count: usize,
    pub right_only_unique_path_count: usize,
    pub total_system_unique_path_count: usize,
    pub incremental_left_path_count: usize,
    pub incremental_right_path_count: usize,
    pub left: Option<ReportAntennaRepeatability>,
    pub right: Option<ReportAntennaRepeatability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportAntennaRepeatability {
    pub side: ComparisonSide,
    pub antenna_label: String,
    pub unique_endpoint_count: usize,
    pub path_block_observation_count: usize,
    pub observed_once_path_count: usize,
    pub repeated_path_count: usize,
    pub block_count_distribution: Vec<ReportBlockCountFrequency>,
    pub paths: Vec<ReportPathRepeatability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportBlockCountFrequency {
    pub observed_block_count: usize,
    pub unique_path_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPathRepeatability {
    pub remote_path: String,
    pub observed_block_count: usize,
    pub observation_count: usize,
    pub left_then_right_block_count: usize,
    pub right_then_left_block_count: usize,
    pub block_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOpportunityComplementarity {
    pub coverage: antennabench_analysis::ReporterActivityCoverage,
    pub eligible_block_count: usize,
    pub known_coverage_block_count: usize,
    pub unique_common_active_receiver_count: usize,
    pub receiver_block_opportunity_count: usize,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
    pub order_summaries: Vec<ReportOpportunityOrderSummary>,
    pub receiver_frequencies: Vec<ReportReceiverDetectionFrequency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportOpportunityOrderSummary {
    pub order: antennabench_analysis::ComparisonOrder,
    pub block_count: usize,
    pub receiver_block_opportunity_count: usize,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportReceiverDetectionFrequency {
    pub receiver: String,
    pub opportunity_count: usize,
    pub left_detection_count: usize,
    pub right_detection_count: usize,
    pub heard_both_count: usize,
    pub left_only_count: usize,
    pub right_only_count: usize,
    pub heard_neither_count: usize,
    pub left_then_right_opportunity_count: usize,
    pub right_then_left_opportunity_count: usize,
}

/// A concise, renderer-neutral projection of the session questions and the
/// descriptive paired evidence available to answer them.
///
/// This deliberately contains no conclusion, score, threshold, or winner.
/// Detailed report data remains available elsewhere on [`SessionReport`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverview {
    pub scope: ReportOverviewScope,
    pub lifecycle: ReportOverviewLifecycle,
    /// Availability is projected separately for each operator question. The
    /// legacy comparison availability below remains the compatibility view for
    /// the original finite-SNR paired comparison only.
    #[serde(default)]
    pub answerability: ReportQuestionAnswerability,
    pub comparison_availability: ComparisonAvailability,
    pub strata: Vec<ReportOverviewStratum>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timeline: Vec<ReportRunTimelineRow>,
    pub limitations: Vec<ReportOverviewLimitation>,
}

impl Default for ReportOverview {
    fn default() -> Self {
        Self {
            scope: ReportOverviewScope::default(),
            lifecycle: ReportOverviewLifecycle::default(),
            answerability: ReportQuestionAnswerability::default(),
            comparison_availability: ComparisonAvailability::NotApplicable,
            strata: Vec::new(),
            timeline: Vec::new(),
            limitations: Vec::new(),
        }
    }
}

/// Renderer-neutral availability for the distinct questions a report may
/// answer. These values are availability facts, never grades or conclusions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportQuestionAnswerability {
    pub same_path_signal: SamePathSignalAnswerability,
    pub paired_detectability: PairedDetectabilityAnswerability,
    pub observed_reach: ObservedReachAnswerability,
    pub geographic_profile: GeographicProfileAnswerability,
    pub repeatability: RepeatabilityAnswerability,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamePathSignalAnswerability {
    Available,
    NoMatchedPaths,
    NoFiniteSnr,
    NoEligibleBlocks,
    #[default]
    NotApplicable,
    UnsupportedShape,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairedDetectabilityAnswerability {
    Available,
    NoCommonActiveReporters,
    ActivityCoverageUnknown,
    UnsupportedDirection,
    NoEligibleBlocks,
    #[default]
    NotApplicable,
    UnsupportedShape,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedReachAnswerability {
    Available,
    #[default]
    NoUsablePaths,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeographicProfileAnswerability {
    Available,
    #[default]
    NoLocatedPaths,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatabilityAnswerability {
    Available,
    #[default]
    InsufficientRepetition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewScope {
    pub session_id: String,
    pub station: StationContext,
    pub goal: Option<SessionGoal>,
    pub experiment_mode: Option<ExperimentMode>,
    pub bands: Vec<Band>,
    pub antenna_labels: Vec<String>,
    pub observed_directions: Vec<PathDirection>,
    pub delta_orientation: Option<DeltaOrientation>,
}

impl Default for ReportOverviewScope {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            station: StationContext {
                callsign: String::new(),
                grid: String::new(),
                power_watts: None,
            },
            goal: None,
            experiment_mode: None,
            bands: Vec::new(),
            antenna_labels: Vec::new(),
            observed_directions: Vec::new(),
            delta_orientation: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportOverviewLifecycle {
    pub checkpoint_revision: Option<u64>,
    pub state: ReportOverviewLifecycleState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOverviewLifecycleState {
    #[default]
    NotRecorded,
    Recorded(SessionLifecycleV2),
}

/// A single existing comparison stratum, projected without pooling across any
/// stratum key. The delta range is derived from existing path summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewStratum {
    pub stratum: ComparisonStratum,
    pub availability: ReportStratumAvailability,
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
    pub path_delta: ReportOverviewPathDelta,
    /// One finite path-median delta per remote path. This is a bounded,
    /// renderer-ready projection: prolific reporters therefore cannot occupy
    /// more than one headline dot.
    pub path_median_deltas: Vec<ReportOverviewPathMedianDelta>,
    /// Unique finite paths classified by the antennas on which they were
    /// observed. Missing SNR is deliberately accounted for separately.
    pub reach: ReportOverviewReach,
    /// All usable observed paths, kept separate by antenna and exact stratum.
    /// This describes collected footprint rather than a matched-path effect or
    /// controlled detection comparison.
    #[serde(default)]
    pub observed_profile: ReportOverviewObservedProfile,
    /// One deterministic location context record per paired remote path. It
    /// never pools strata and never lets repeated paired rows dominate a
    /// distance bin or azimuth sector.
    #[serde(default)]
    pub location_context: ReportOverviewLocationContext,
}

/// Typed answerability for one already-separated comparison stratum. These
/// states describe availability only; they are not evidence-strength grades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStratumAvailability {
    DescriptivePairsAvailable,
    NoFinitePairedPaths,
}

/// Compact planned-versus-actual row retained even in bounded-overview mode.
/// The renderer may summarize it visually, while `event_history` supplies the
/// exact accessible note and correction trail for the row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRunTimelineRow {
    pub item_id: String,
    pub sequence_number: u32,
    pub block_index: Option<usize>,
    pub block_eligibility: Option<ComparisonBlockEligibility>,
    pub band: Band,
    pub direction: Option<WsprCycleDirection>,
    pub planned_antenna: String,
    pub actual_antenna: Option<String>,
    pub planned_starts_at: DateTime<Utc>,
    pub planned_ends_at: DateTime<Utc>,
    pub actual_starts_at: Option<DateTime<Utc>>,
    pub actual_ends_at: Option<DateTime<Utc>>,
    pub readiness_basis: Option<ReportWsprReadinessBasis>,
    pub attribution: Option<ReportWsprAttribution>,
    pub status: AlignedSlotStatus,
    pub total_observation_count: usize,
    pub usable_observation_count: usize,
    pub excluded_observation_count: usize,
    pub event_history: Vec<ReportOperatorEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewPathMedianDelta {
    pub remote_path: String,
    pub paired_row_count: usize,
    pub median_delta_right_minus_left_db: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportOverviewReach {
    pub left_only_unique_path_count: usize,
    pub both_unique_path_count: usize,
    pub right_only_unique_path_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewObservedProfile {
    pub left: Option<ReportObservedAntennaProfile>,
    pub right: Option<ReportObservedAntennaProfile>,
    pub distance_composition: Vec<ReportObservedDistanceComposition>,
    pub composition_location_unavailable_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportObservedAntennaProfile {
    pub side: ComparisonSide,
    pub antenna_label: String,
    pub unique_path_count: usize,
    pub located_path_count: usize,
    pub missing_location_path_count: usize,
    pub inconsistent_location_path_count: usize,
    pub distance_bins: Vec<ReportObservedProfileCell<ReportDistanceBin>>,
    pub azimuth_sectors: Vec<ReportObservedProfileCell<ReportAzimuthSector>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportObservedProfileCell<T> {
    pub category: T,
    pub unique_path_count: usize,
    pub observation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportObservedDistanceComposition {
    pub category: ReportDistanceBin,
    pub left_only_unique_path_count: usize,
    pub shared_unique_path_count: usize,
    pub right_only_unique_path_count: usize,
}

/// Fixed, documented distance bins for observed-session path context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDistanceBin {
    Under500Km,
    Km500To1499,
    Km1500To2999,
    Km3000AndAbove,
}

/// Fixed 45° compass sectors for observed-session path context. North wraps
/// across 360°: [337.5°, 360°) and [0°, 22.5°).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportAzimuthSector {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl ReportAzimuthSector {
    pub const ALL: [Self; 8] = [
        Self::North,
        Self::NorthEast,
        Self::East,
        Self::SouthEast,
        Self::South,
        Self::SouthWest,
        Self::West,
        Self::NorthWest,
    ];
}

/// Why a paired path cannot participate in a geographic aggregate. The raw
/// paired rows retain the exact left/right values in the audit appendix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportPathLocationAvailability {
    Available,
    Missing,
    Inconsistent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewLocationPath {
    pub remote_path: String,
    pub paired_row_count: usize,
    pub median_delta_right_minus_left_db: f64,
    pub availability: ReportPathLocationAvailability,
    pub distance_km: Option<f64>,
    pub azimuth_degrees: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewLocationCell<T> {
    pub category: T,
    pub unique_located_path_count: usize,
    pub paired_row_count: usize,
    pub median_path_delta_right_minus_left_db: Option<f64>,
}

/// Bounded, renderer-ready geographic context for one comparison stratum.
/// Every path has one status record; available paths contribute once to one
/// fixed distance bin and one fixed azimuth sector.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportOverviewLocationContext {
    pub paths: Vec<ReportOverviewLocationPath>,
    pub distance_bins: Vec<ReportOverviewLocationCell<ReportDistanceBin>>,
    pub azimuth_sectors: Vec<ReportOverviewLocationCell<ReportAzimuthSector>>,
    pub missing_location_path_count: usize,
    pub inconsistent_location_path_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "availability")]
pub enum ReportOverviewPathDelta {
    Unavailable,
    Available {
        minimum_delta_right_minus_left_db: f64,
        median_path_delta_right_minus_left_db: f64,
        maximum_delta_right_minus_left_db: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ReportOverviewLimitation {
    ComparisonNotApplicable,
    UnsupportedComparisonShape,
    NoEligibleBlocks,
    NoMatchedPaths,
    UnmatchedPaths {
        left_count: usize,
        right_count: usize,
    },
    MissingSnr {
        left_count: usize,
        right_count: usize,
    },
    DuplicateEvidence {
        exact_count: usize,
        conflicting_group_count: usize,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSnapshotContext {
    pub checkpoint_revision: Option<u64>,
    pub lifecycle: Option<SessionLifecycleV2>,
    pub lifecycle_events: Vec<ReportLifecycleEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operator_events: Vec<ReportOperatorEvent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wspr_cycles: Vec<ReportWsprCycle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub antenna_control_attempts: Vec<ReportAntennaControlAttempt>,
    pub adapter_evidence: ReportAdapterEvidence,
}

impl ReportSnapshotContext {
    fn is_empty(&self) -> bool {
        self.checkpoint_revision.is_none()
            && self.lifecycle.is_none()
            && self.lifecycle_events.is_empty()
            && self.operator_events.is_empty()
            && self.wspr_cycles.is_empty()
            && self.antenna_control_attempts.is_empty()
            && self.adapter_evidence.record_count == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportOperatorEvent {
    pub event_id: String,
    pub occurred_at: DateTime<Utc>,
    pub slot_id: Option<String>,
    pub affected_slot_id: Option<String>,
    pub kind: ReportOperatorEventKind,
    pub detail: Option<String>,
    pub correction: Option<ReportEventCorrection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOperatorEventKind {
    SessionStarted,
    SessionInterrupted,
    InterruptionDetected,
    SessionResumed,
    SessionEnded,
    SessionAbandoned,
    AntennaSwitchStarted,
    WsprCycleArmed,
    AntennaStateConfirmed,
    SignalStateConfirmed,
    SlotMissed,
    SlotBad,
    NoteAdded,
    EventCorrected,
    Switched,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportEventCorrection {
    pub target_event_id: String,
    pub action: ReportEventCorrectionAction,
    pub reason: String,
    pub applied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportEventCorrectionAction {
    Retracted,
    Replaced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportWsprCycle {
    pub intent_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub direction: Option<WsprCycleDirection>,
    pub planned_antenna: String,
    pub actual_antenna: Option<String>,
    pub ready_at: Option<DateTime<Utc>>,
    pub starts_at: Option<DateTime<Utc>>,
    pub transmission_ends_at: Option<DateTime<Utc>>,
    pub attribution: ReportWsprAttribution,
    pub readiness_basis: Option<ReportWsprReadinessBasis>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportWsprReadinessBasis {
    OperatorConfirmed,
    CommandVerified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportAntennaControlAttempt {
    pub record_id: String,
    pub role: AntennaControlRoleV5,
    pub controller_profile_name: String,
    pub controller_profile_revision: String,
    pub resolved_program: String,
    pub resolved_arguments: Vec<String>,
    pub intent_id: String,
    pub antenna: String,
    pub target: String,
    pub mode: ExperimentMode,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub elapsed_milliseconds: u64,
    pub disposition: AntennaControlDispositionV5,
    pub stdout: AntennaControlOutputV5,
    pub stderr: AntennaControlOutputV5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportWsprAttribution {
    Pending,
    Skipped,
    Attributable,
    UnknownAntennaOccupancy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportLifecycleEvent {
    pub kind: ReportLifecycleEventKind,
    pub occurred_at: DateTime<Utc>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportLifecycleEventKind {
    Started,
    Interrupted,
    InterruptionDetected,
    Resumed,
    Ended,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportAdapterEvidence {
    pub record_count: usize,
    pub accepted_count: usize,
    pub malformed_count: usize,
    pub unsupported_count: usize,
    pub filtered_count: usize,
    pub duplicate_count: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub conflict_count: usize,
    pub partially_normalized_count: usize,
    pub gap_count: usize,
    #[serde(default)]
    pub workflow_status: ReportAcquisitionWorkflowStatus,
    #[serde(default)]
    pub provider_completeness: ReportProviderCompleteness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<ReportImportedEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportImportedEvidence {
    pub provider_id: String,
    pub source_id: String,
    pub captured_at: DateTime<Utc>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub selected_bands: Vec<Band>,
    pub total_count: usize,
    pub accepted_count: usize,
    pub malformed_count: usize,
    pub filtered_count: usize,
    pub unsupported_count: usize,
    pub duplicate_count: usize,
    pub conflict_count: usize,
    pub observations_created: usize,
    #[serde(default)]
    pub provider_completeness: ReportProviderCompleteness,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportAcquisitionWorkflowStatus {
    #[default]
    NotConfigured,
    Completed,
    Incomplete,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportProviderCompleteness {
    Known,
    Unknown,
    #[default]
    Unsupported,
}

impl Default for ReportAdapterEvidence {
    fn default() -> Self {
        Self {
            record_count: 0,
            accepted_count: 0,
            malformed_count: 0,
            unsupported_count: 0,
            filtered_count: 0,
            duplicate_count: 0,
            conflict_count: 0,
            partially_normalized_count: 0,
            gap_count: 0,
            workflow_status: ReportAcquisitionWorkflowStatus::NotConfigured,
            provider_completeness: ReportProviderCompleteness::Unsupported,
            imports: Vec::new(),
        }
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportCompleteness {
    #[default]
    FullDetail,
    BoundedOverview,
}

impl ReportCompleteness {
    fn is_full_detail(&self) -> bool {
        *self == Self::FullDetail
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportComparisonData {
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
    #[serde(default)]
    pub observed_path_profiles: Vec<antennabench_analysis::ObservedAntennaPathProfile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub station: StationContext,
    pub experiment_mode: ExperimentMode,
    pub goal: SessionGoal,
    pub scheduled_time_range: Option<ScheduledTimeRange>,
    pub antennas: Vec<Antenna>,
    pub bands: Vec<Band>,
    pub schedule: ScheduleOverview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StationContext {
    pub callsign: String,
    pub grid: String,
    pub power_watts: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTimeRange {
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleOverview {
    pub slot_count: usize,
    pub slots: Vec<ScheduledSlotContext>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledSlotContext {
    pub slot_id: String,
    pub sequence_number: u32,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub guard_seconds: u32,
    pub band: Band,
    pub planned_label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceSections {
    pub evidence_quality: EvidenceQuality,
    pub overall: ReportEvidenceSummary,
    pub antennas: Vec<AntennaEvidenceSection>,
    pub bands: Vec<BandEvidenceSection>,
    pub slots: Vec<SlotEvidenceSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportEvidenceSummary {
    pub observation_counts: ObservationCounts,
    pub exclusions: Vec<ExclusionCount>,
    pub usable_observation_kinds: UsableObservationKindCounts,
    pub snr: Option<SnrStatistics>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsableObservationKindCounts {
    pub local_decode: usize,
    pub public_report: usize,
    pub imported_spot: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntennaEvidenceSection {
    pub antenna_label: String,
    pub contributing_slot_count: usize,
    pub evidence_quality: EvidenceQuality,
    pub evidence: ReportEvidenceSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandEvidenceSection {
    pub band: Band,
    pub evidence: ReportEvidenceSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotEvidenceSection {
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
    pub evidence: ReportEvidenceSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportChartData {
    pub antenna_snr: Vec<AntennaSnrRow>,
    pub band_evidence_counts: Vec<BandEvidenceCountRow>,
    pub slot_evidence_counts: Vec<SlotEvidenceCountRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntennaSnrRow {
    pub antenna_label: String,
    pub usable_observation_count: usize,
    pub snr: Option<SnrStatistics>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandEvidenceCountRow {
    pub band: Band,
    pub observation_counts: ObservationCounts,
    pub usable_observation_kinds: UsableObservationKindCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotEvidenceCountRow {
    pub slot_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub planned_label: String,
    pub actual_label: Option<String>,
    pub status: AlignedSlotStatus,
    pub observation_counts: ObservationCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportNotice {
    NoScheduledSlots,
    NoUsableObservations,
    NoUsableSnrSamples,
    DetailOmitted {
        family: ReportDetailFamily,
        row_count: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDetailFamily {
    LifecycleHistory,
    Schedule,
    AntennaContext,
    AntennaEvidence,
    BandEvidence,
    SlotEvidence,
    ExclusionRecords,
    OperatorEvents,
    ComparisonBlocks,
    PathOverlap,
    ComparisonTimeline,
    PairedObservations,
    SolarContext,
    PathSummaries,
    Strata,
    ObservedPathProfileRows,
    ReporterActivityAudit,
    CoverageMapReporters,
    CommonOpportunityGeographyAudit,
    CoverageOverlapAudit,
    Charts,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ReportError {
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
    #[error(transparent)]
    Resource(#[from] ReportResourceError),
    #[error("report model serialization failed: {message}")]
    Serialization { message: String },
}
