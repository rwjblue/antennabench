use antennabench_analysis::{
    AnalysisError, ComparisonAvailability, ComparisonBlock, ComparisonDiagnostics,
    ComparisonTimelineRow, DeltaOrientation, EvidenceQuality, ExclusionCount, ObservationCounts,
    PairedObservationRow, PairedPathSummary, PairedStratumSummary, PathOverlapRow, SnrStatistics,
};
use antennabench_core::{AlignedSlotStatus, Antenna, Band, ExperimentMode, SessionGoal};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionReport {
    pub context: SessionContext,
    pub evidence: EvidenceSections,
    pub comparison: ReportComparisonData,
    pub chart_data: ReportChartData,
    pub notices: Vec<ReportNotice>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportNotice {
    NoScheduledSlots,
    NoUsableObservations,
    NoUsableSnrSamples,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ReportError {
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
}
