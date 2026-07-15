use std::{collections::BTreeMap, fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{
    AnalysisFile, AntennasFile, Band, BundleContents, BundleFiles, BundleManifest, ObservationKind,
    ObservationRecord, PropagationRecord, RecordMeta, RecordSource, RigRecord, Schedule, Station,
    WsjtXRecord, SCHEMA_VERSION_V2,
};

pub const V1_BUNDLE_SUFFIX: &str = ".session.wsprabundle";
pub const V2_BUNDLE_SUFFIX: &str = ".session.antennabundle";
pub const IDENTITY_MAX_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdentityError {
    #[error("identity must not be empty")]
    Empty,
    #[error("identity exceeds {IDENTITY_MAX_BYTES} ASCII bytes")]
    TooLong,
    #[error(
        "identity must contain lowercase ASCII alphanumeric segments separated by '.', '_', or '-'"
    )]
    InvalidSyntax,
}

fn validate_identity(value: &str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::Empty);
    }
    if value.len() > IDENTITY_MAX_BYTES {
        return Err(IdentityError::TooLong);
    }

    let mut previous_was_separator = true;
    for byte in value.bytes() {
        let separator = matches!(byte, b'.' | b'_' | b'-');
        if separator {
            if previous_was_separator {
                return Err(IdentityError::InvalidSyntax);
            }
        } else if !(byte.is_ascii_lowercase() || byte.is_ascii_digit()) {
            return Err(IdentityError::InvalidSyntax);
        }
        previous_was_separator = separator;
    }
    if previous_was_separator {
        return Err(IdentityError::InvalidSyntax);
    }
    Ok(())
}

macro_rules! identity_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
                let value = value.into();
                validate_identity(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdentityError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentityError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identity_type!(ProviderId);
identity_type!(SourceId);
identity_type!(AcquisitionChannelId);
identity_type!(AdapterId);
identity_type!(AdapterReasonId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub provider_id: ProviderId,
    pub source_id: SourceId,
    pub acquisition_channel: AcquisitionChannelId,
    pub adapter_id: AdapterId,
    pub adapter_version: String,
}

impl Provenance {
    pub fn from_legacy(source: RecordSource, adapter_version: impl Into<String>) -> Self {
        let (provider, source_id, channel, adapter) = match source {
            RecordSource::Operator => (
                "antennabench",
                "operator-evidence",
                "operator-entry",
                "antennabench.operator",
            ),
            RecordSource::WsjtxUdp => ("wsjt-x", "wspr", "udp", "antennabench.wsjt-x-udp"),
            RecordSource::WsjtxLog => (
                "wsjt-x",
                "all-wspr",
                "log-import",
                "antennabench.wsjt-x-log",
            ),
            RecordSource::Wsprnet => (
                "wsprnet",
                "spots",
                "legacy-unspecified",
                "antennabench.legacy",
            ),
            RecordSource::WsprLive => (
                "wspr-live",
                "spots",
                "legacy-unspecified",
                "antennabench.legacy",
            ),
            RecordSource::ImportedFile => (
                "legacy",
                "imported-file",
                "file-import",
                "antennabench.legacy",
            ),
            RecordSource::RigAdapter => (
                "local-rig",
                "rig-state",
                "local-adapter",
                "antennabench.rig",
            ),
            RecordSource::NoaaSwpc => (
                "noaa-swpc",
                "space-weather",
                "https",
                "antennabench.noaa-swpc",
            ),
            RecordSource::Derived => ("antennabench", "derived", "internal", "antennabench.core"),
        };
        Self {
            provider_id: ProviderId::new(provider).expect("static provider identity"),
            source_id: SourceId::new(source_id).expect("static source identity"),
            acquisition_channel: AcquisitionChannelId::new(channel)
                .expect("static acquisition identity"),
            adapter_id: AdapterId::new(adapter).expect("static adapter identity"),
            adapter_version: adapter_version.into(),
        }
    }

    pub fn legacy_source(&self) -> RecordSource {
        match (
            self.provider_id.as_str(),
            self.source_id.as_str(),
            self.acquisition_channel.as_str(),
        ) {
            ("antennabench", "operator-evidence", _) => RecordSource::Operator,
            ("wsjt-x", _, "udp") => RecordSource::WsjtxUdp,
            ("wsjt-x", _, "log-import") => RecordSource::WsjtxLog,
            ("wsprnet", _, _) => RecordSource::Wsprnet,
            ("wspr-live", _, _) => RecordSource::WsprLive,
            ("local-rig", _, _) => RecordSource::RigAdapter,
            ("noaa-swpc", _, _) => RecordSource::NoaaSwpc,
            ("antennabench", "derived", _) => RecordSource::Derived,
            _ => RecordSource::ImportedFile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationMember {
    pub mutation_id: String,
    pub member_index: u32,
    pub member_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordMetaV2 {
    pub schema_version: u16,
    pub session_id: String,
    pub recorded_at: DateTime<Utc>,
    pub provenance: Provenance,
    pub mutation: MutationMember,
}

impl RecordMetaV2 {
    fn project(&self) -> RecordMeta {
        RecordMeta {
            schema_version: SCHEMA_VERSION_V2,
            session_id: self.session_id.clone(),
            timestamp: self.recorded_at,
            source: self.provenance.legacy_source(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterDisposition {
    Accepted,
    Malformed,
    Filtered,
    Duplicate,
    Conflict,
    Unsupported,
    PartiallyNormalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizedRecordKind {
    Observation,
    Rig,
    Propagation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedRecordLink {
    pub record_kind: NormalizedRecordKind,
    pub record_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentReference {
    /// Lowercase SHA-256 hexadecimal digest.
    pub sha256: String,
    pub byte_size: u64,
    pub media_type: String,
    pub encoding: Option<String>,
    pub container: Option<String>,
    pub source_locator: Option<String>,
}

impl AttachmentReference {
    pub fn has_valid_digest(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    pub fn relative_path(&self) -> Option<String> {
        self.has_valid_digest()
            .then(|| format!("sha256/{}", self.sha256))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdapterInput {
    Inline {
        data: String,
        media_type: String,
        encoding: Option<String>,
        source_locator: Option<String>,
    },
    Attachment {
        attachment: AttachmentReference,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterRecordV2 {
    pub meta: RecordMetaV2,
    pub record_id: String,
    pub source_time: Option<DateTime<Utc>>,
    pub record_type: String,
    pub disposition: AdapterDisposition,
    pub reason: AdapterReasonId,
    pub normalized_records: Vec<NormalizedRecordLink>,
    pub input: AdapterInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorEventV2 {
    pub meta: RecordMetaV2,
    pub event_id: String,
    pub occurred_at: DateTime<Utc>,
    pub time_basis: EventTimeBasisV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_seconds: Option<u32>,
    pub slot_id: Option<String>,
    pub payload: OperatorEventPayloadV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTimeBasisV2 {
    ObservedNow,
    OperatorReported,
    RecoverySystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CorrectableOperatorEventPayloadV2 {
    AntennaStateConfirmed {
        antenna_label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    SlotMissed {
        reason: Option<String>,
    },
    SlotBad {
        reason: String,
    },
    NoteAdded {
        note: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplacementOperatorEventV2 {
    pub occurred_at: DateTime<Utc>,
    pub time_basis: EventTimeBasisV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_seconds: Option<u32>,
    pub slot_id: Option<String>,
    pub payload: CorrectableOperatorEventPayloadV2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EventCorrectionActionV2 {
    Retract,
    Replace {
        replacement: ReplacementOperatorEventV2,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperatorEventPayloadV2 {
    SessionStarted {
        note: Option<String>,
    },
    SessionInterrupted {
        reason: Option<String>,
    },
    InterruptionDetected {
        reason: Option<String>,
    },
    SessionResumed {
        note: Option<String>,
    },
    SessionEnded {
        reason: Option<String>,
    },
    SessionAbandoned {
        reason: Option<String>,
    },
    AntennaStateConfirmed {
        antenna_label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    SlotMissed {
        reason: Option<String>,
    },
    SlotBad {
        reason: String,
    },
    NoteAdded {
        note: String,
    },
    EventCorrected {
        target_event_id: String,
        correction: EventCorrectionActionV2,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecordV2 {
    pub meta: RecordMetaV2,
    pub observation_id: String,
    pub adapter_record_ids: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigRecordV2 {
    pub meta: RecordMetaV2,
    pub record_id: String,
    pub adapter_record_ids: Vec<String>,
    pub status: String,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropagationRecordV2 {
    pub meta: RecordMetaV2,
    pub record_id: String,
    pub adapter_record_ids: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFilesV2 {
    pub manifest: String,
    pub session_state: String,
    pub station: String,
    pub antennas: String,
    pub schedule: String,
    pub events: String,
    pub observations: String,
    pub adapter_records: String,
    pub rig: String,
    pub propagation: String,
    pub analysis: String,
    pub attachments_dir: String,
}

impl Default for BundleFilesV2 {
    fn default() -> Self {
        Self {
            manifest: "manifest.json".into(),
            session_state: "session-state.json".into(),
            station: "station.json".into(),
            antennas: "antennas.json".into(),
            schedule: "schedule.json".into(),
            events: "events.jsonl".into(),
            observations: "observations.jsonl".into(),
            adapter_records: "adapter-records.jsonl".into(),
            rig: "rig.jsonl".into(),
            propagation: "propagation.jsonl".into(),
            analysis: "analysis.json".into(),
            attachments_dir: "attachments".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleManifestV2 {
    pub schema_version: u16,
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub app_version: String,
    pub files: BundleFilesV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleV2 {
    Draft,
    Ready,
    Running,
    Interrupted,
    Ended,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanGenerationV2 {
    pub generation_id: String,
    pub station_sha256: String,
    pub antennas_sha256: String,
    pub schedule_sha256: String,
    pub root_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamCheckpointV2 {
    pub committed_bytes: u64,
    pub record_count: u64,
    pub last_record_id: Option<String>,
    pub committed_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStateV2 {
    pub schema_version: u16,
    pub session_id: String,
    pub revision: u64,
    pub lifecycle: SessionLifecycleV2,
    pub active_plan: PlanGenerationV2,
    pub streams: BTreeMap<String, StreamCheckpointV2>,
    pub last_committed_mutation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleV2Contents {
    pub manifest: BundleManifestV2,
    pub session_state: SessionStateV2,
    pub station: Station,
    pub antennas: AntennasFile,
    pub schedule: Schedule,
    pub events: Vec<OperatorEventV2>,
    pub observations: Vec<ObservationRecordV2>,
    pub adapter_records: Vec<AdapterRecordV2>,
    pub rig: Vec<RigRecordV2>,
    pub propagation: Vec<PropagationRecordV2>,
    pub analysis: AnalysisFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentRecordKind {
    Event,
    Observation,
    Rig,
    Propagation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRecordProvenance {
    pub record_kind: CurrentRecordKind,
    pub record_id: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrentBundleContents {
    pub bundle: BundleContents,
    pub record_provenance: Vec<CurrentRecordProvenance>,
    pub adapter_records: Vec<AdapterRecordV2>,
    pub session_state: Option<SessionStateV2>,
}

impl CurrentBundleContents {
    pub fn from_v1(bundle: BundleContents) -> Self {
        let mut record_provenance = Vec::new();
        for event in &bundle.events {
            record_provenance.push(CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Event,
                record_id: event.event_id.clone(),
                provenance: Provenance::from_legacy(event.meta.source, "legacy-v1"),
            });
        }
        for observation in &bundle.observations {
            record_provenance.push(CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Observation,
                record_id: observation.observation_id.clone(),
                provenance: Provenance::from_legacy(observation.meta.source, "legacy-v1"),
            });
        }
        for record in &bundle.rig {
            record_provenance.push(CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Rig,
                record_id: record.record_id.clone(),
                provenance: Provenance::from_legacy(record.meta.source, "legacy-v1"),
            });
        }
        for record in &bundle.propagation {
            record_provenance.push(CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Propagation,
                record_id: record.record_id.clone(),
                provenance: Provenance::from_legacy(record.meta.source, "legacy-v1"),
            });
        }
        Self {
            bundle,
            record_provenance,
            adapter_records: Vec::new(),
            session_state: None,
        }
    }
}

impl BundleV2Contents {
    pub fn into_current(self) -> CurrentBundleContents {
        let mut record_provenance = Vec::new();
        for event in &self.events {
            record_provenance.push(CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Event,
                record_id: event.event_id.clone(),
                provenance: event.meta.provenance.clone(),
            });
        }
        let reduction = crate::reduce_operator_events_v2(SessionLifecycleV2::Ready, &self.events);
        let mut effective = reduction
            .effective_events
            .into_iter()
            .filter_map(|event| event.project_legacy())
            .map(|event| (event.event_id.clone(), event))
            .collect::<BTreeMap<_, _>>();
        let events = self
            .events
            .iter()
            .filter_map(|event| {
                event
                    .project_legacy_lifecycle()
                    .or_else(|| effective.remove(&event.event_id))
            })
            .collect();
        let observations = self
            .observations
            .iter()
            .map(|observation| {
                record_provenance.push(CurrentRecordProvenance {
                    record_kind: CurrentRecordKind::Observation,
                    record_id: observation.observation_id.clone(),
                    provenance: observation.meta.provenance.clone(),
                });
                ObservationRecord {
                    meta: observation.meta.project(),
                    observation_id: observation.observation_id.clone(),
                    observation_kind: observation.observation_kind,
                    band: observation.band,
                    frequency_hz: observation.frequency_hz,
                    mode: observation.mode.clone(),
                    reporter_call: observation.reporter_call.clone(),
                    heard_call: observation.heard_call.clone(),
                    reporter_grid: observation.reporter_grid.clone(),
                    heard_grid: observation.heard_grid.clone(),
                    distance_km: observation.distance_km,
                    azimuth_degrees: observation.azimuth_degrees,
                    snr_db: observation.snr_db,
                    drift_hz_per_minute: observation.drift_hz_per_minute,
                    power_watts: observation.power_watts,
                    slot_id: observation.slot_id.clone(),
                    slot_label: observation.slot_label.clone(),
                    slot_confidence: observation.slot_confidence,
                    raw: observation.raw.clone(),
                }
            })
            .collect();
        let rig = self
            .rig
            .iter()
            .map(|record| {
                record_provenance.push(CurrentRecordProvenance {
                    record_kind: CurrentRecordKind::Rig,
                    record_id: record.record_id.clone(),
                    provenance: record.meta.provenance.clone(),
                });
                RigRecord {
                    meta: record.meta.project(),
                    record_id: record.record_id.clone(),
                    status: record.status.clone(),
                    frequency_hz: record.frequency_hz,
                    mode: record.mode.clone(),
                    raw: record.raw.clone(),
                }
            })
            .collect();
        let propagation = self
            .propagation
            .iter()
            .map(|record| {
                record_provenance.push(CurrentRecordProvenance {
                    record_kind: CurrentRecordKind::Propagation,
                    record_id: record.record_id.clone(),
                    provenance: record.meta.provenance.clone(),
                });
                PropagationRecord {
                    meta: record.meta.project(),
                    record_id: record.record_id.clone(),
                    observed_at: record.observed_at,
                    solar_flux_f107: record.solar_flux_f107,
                    sunspot_number: record.sunspot_number,
                    kp_index: record.kp_index,
                    a_index: record.a_index,
                    solar_wind_speed_kms: record.solar_wind_speed_kms,
                    bz_nt: record.bz_nt,
                    alerts: record.alerts.clone(),
                    daylight_state: record.daylight_state.clone(),
                    raw: record.raw.clone(),
                }
            })
            .collect();

        let bundle = BundleContents {
            manifest: BundleManifest {
                schema_version: SCHEMA_VERSION_V2,
                session_id: self.manifest.session_id.clone(),
                created_at: self.manifest.created_at,
                app_version: self.manifest.app_version.clone(),
                files: BundleFiles::default(),
            },
            station: self.station,
            antennas: self.antennas,
            schedule: self.schedule,
            events,
            observations,
            wsjtx: Vec::<WsjtXRecord>::new(),
            rig,
            propagation,
            analysis: self.analysis,
        };
        CurrentBundleContents {
            bundle,
            record_provenance,
            adapter_records: self.adapter_records,
            session_state: Some(self.session_state),
        }
    }
}
