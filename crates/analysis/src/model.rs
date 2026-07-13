use antennabench_core::{AlignedSlotStatus, Band, BundleValidationError, ObservationKind};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AnalysisError {
    #[error("bundle is not valid for analysis")]
    InvalidBundle(#[from] BundleValidationError),
    #[error("observation {observation_id} has a non-finite SNR")]
    NonFiniteSnr { observation_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub session_id: String,
    pub evidence_quality: EvidenceQuality,
    pub overall: EvidenceSummary,
    pub antennas: Vec<AntennaEvidenceSummary>,
    pub bands: Vec<BandEvidenceSummary>,
    pub slots: Vec<SlotEvidenceSummary>,
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
}
