use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: u16,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub files: BundleFiles,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleFiles {
    pub manifest: String,
    pub station: String,
    pub antennas: String,
    pub schedule: String,
    pub events: String,
    pub observations: String,
    pub wsjtx: String,
    pub rig: String,
    pub propagation: String,
    pub analysis: String,
    pub attachments_dir: String,
}

impl Default for BundleFiles {
    fn default() -> Self {
        Self {
            manifest: "manifest.json".to_string(),
            station: "station.json".to_string(),
            antennas: "antennas.json".to_string(),
            schedule: "schedule.json".to_string(),
            events: "events.jsonl".to_string(),
            observations: "observations.jsonl".to_string(),
            wsjtx: "wsjtx.jsonl".to_string(),
            rig: "rig.jsonl".to_string(),
            propagation: "propagation.jsonl".to_string(),
            analysis: "analysis.json".to_string(),
            attachments_dir: "attachments".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleContents {
    pub manifest: BundleManifest,
    pub station: Station,
    pub antennas: AntennasFile,
    pub schedule: Schedule,
    pub events: Vec<OperatorEvent>,
    pub observations: Vec<ObservationRecord>,
    pub wsjtx: Vec<WsjtXRecord>,
    pub rig: Vec<RigRecord>,
    pub propagation: Vec<PropagationRecord>,
    pub analysis: AnalysisFile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub schema_version: u16,
    pub session_id: String,
    pub callsign: String,
    pub grid: String,
    pub power_watts: Option<f32>,
    pub operator_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntennasFile {
    pub schema_version: u16,
    pub session_id: String,
    pub antennas: Vec<Antenna>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Antenna {
    pub label: String,
    pub facets: Vec<String>,
    pub height_m: Option<f32>,
    pub radial_count: Option<u32>,
    pub radial_length_m: Option<f32>,
    pub orientation_degrees: Option<f32>,
    pub tuner: Option<String>,
    pub feedline: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub schema_version: u16,
    pub session_id: String,
    pub mode: ExperimentMode,
    pub goal: SessionGoal,
    pub slots: Vec<PlannedSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentMode {
    WholeStationAb,
    TxFocused,
    RxFocused,
    SingleAntennaProfiling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionGoal {
    Dx,
    Regional,
    NvisLocal,
    GeneralCoverage,
    WeakSignalReliability,
    SingleAntennaProfiling,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedSlot {
    pub slot_id: String,
    pub sequence_number: u32,
    pub starts_at: DateTime<Utc>,
    pub duration_seconds: u32,
    pub guard_seconds: u32,
    pub band: Band,
    pub antenna_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Band {
    #[serde(rename = "160m")]
    M160,
    #[serde(rename = "80m")]
    M80,
    #[serde(rename = "60m")]
    M60,
    #[serde(rename = "40m")]
    M40,
    #[serde(rename = "30m")]
    M30,
    #[serde(rename = "20m")]
    M20,
    #[serde(rename = "17m")]
    M17,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "12m")]
    M12,
    #[serde(rename = "10m")]
    M10,
    #[serde(rename = "6m")]
    M6,
    #[serde(rename = "2m")]
    M2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordMeta {
    pub schema_version: u16,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: RecordSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordSource {
    Operator,
    WsjtxUdp,
    WsjtxLog,
    Wsprnet,
    WsprLive,
    ImportedFile,
    RigAdapter,
    NoaaSwpc,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorEvent {
    pub meta: RecordMeta,
    pub event_id: String,
    pub slot_id: Option<String>,
    pub event_type: OperatorEventType,
    pub note: Option<String>,
    /// Schema-v2 projection of explicitly confirmed actual antenna state.
    ///
    /// This is deliberately absent from the schema-v1 wire format. V1 keeps
    /// its historical planned-label inference, while v2 readers populate this
    /// field only from typed operator evidence.
    #[serde(skip)]
    pub actual_antenna_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorEventType {
    SessionStarted,
    Switched,
    MissedSlot,
    BadSlot,
    NoteAdded,
    SessionEnded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub meta: RecordMeta,
    pub observation_id: String,
    pub observation_kind: ObservationKind,
    pub band: Band,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub reporter_call: Option<String>,
    pub heard_call: Option<String>,
    pub reporter_grid: Option<String>,
    pub heard_grid: Option<String>,
    pub distance_km: Option<f64>,
    pub azimuth_degrees: Option<f64>,
    pub snr_db: Option<f32>,
    pub drift_hz_per_minute: Option<f32>,
    pub power_watts: Option<f32>,
    pub slot_id: Option<String>,
    pub slot_label: Option<String>,
    pub slot_confidence: Option<f32>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    LocalDecode,
    PublicReport,
    ImportedSpot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsjtXRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub message_type: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub status: String,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropagationRecord {
    pub meta: RecordMeta,
    pub record_id: String,
    pub observed_at: DateTime<Utc>,
    pub solar_flux_f107: Option<f32>,
    pub sunspot_number: Option<u16>,
    pub kp_index: Option<f32>,
    pub a_index: Option<u16>,
    pub solar_wind_speed_kms: Option<f32>,
    pub bz_nt: Option<f32>,
    pub alerts: Vec<String>,
    pub daylight_state: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFile {
    pub schema_version: u16,
    pub session_id: String,
    pub generated_at: Option<DateTime<Utc>>,
    pub status: AnalysisStatus,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    NotRun,
    Generated,
}
