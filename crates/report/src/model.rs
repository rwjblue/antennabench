use antennabench_analysis::{
    AnalysisError, ComparisonAvailability, ComparisonBlock, ComparisonDiagnostics,
    ComparisonTimelineRow, DeltaOrientation, EligibilityExclusionCount, EvidenceQuality,
    ExclusionCount, ObservationCounts, PairedObservationRow, PairedPathSummary,
    PairedStratumSummary, PathOverlapRow, SnrStatistics, SolarContextAnalysis,
};
use antennabench_core::{
    AlignedSlotStatus, Antenna, Band, ExperimentMode, SessionGoal, SessionLifecycleV2,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ReportResourceError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionReport {
    #[serde(default, skip_serializing_if = "ReportCompleteness::is_full_detail")]
    pub completeness: ReportCompleteness,
    pub context: SessionContext,
    pub evidence: EvidenceSections,
    pub comparison: ReportComparisonData,
    pub solar_context: SolarContextAnalysis,
    pub chart_data: ReportChartData,
    pub notices: Vec<ReportNotice>,
    #[serde(default, skip_serializing_if = "ReportSnapshotContext::is_empty")]
    pub snapshot: ReportSnapshotContext,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eligibility_exclusions: Vec<EligibilityExclusionCount>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSnapshotContext {
    pub checkpoint_revision: Option<u64>,
    pub lifecycle: Option<SessionLifecycleV2>,
    pub lifecycle_events: Vec<ReportLifecycleEvent>,
    pub adapter_evidence: ReportAdapterEvidence,
}

impl ReportSnapshotContext {
    fn is_empty(&self) -> bool {
        self.checkpoint_revision.is_none()
            && self.lifecycle.is_none()
            && self.lifecycle_events.is_empty()
            && self.adapter_evidence.record_count == 0
    }
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
    pub evidence_complete: bool,
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
    pub completeness_known: bool,
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
            evidence_complete: true,
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
    ComparisonBlocks,
    PathOverlap,
    ComparisonTimeline,
    PairedObservations,
    SolarContext,
    PathSummaries,
    Strata,
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
