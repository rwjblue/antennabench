use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::v5::{AntennaControlInvocationV5, AntennaControlPolicyV5, WsprReadinessBasisV5};
use crate::{
    AdapterRecordV2, AnalysisFile, AntennasFile, Band, BundleFilesV2, BundleManifestV2,
    BundleV2Contents, CorrectableOperatorEventPayloadV2, CurrentBundleContents, CurrentRecordKind,
    CurrentRecordProvenance, EventCorrectionActionV2, EventTimeBasisV2, ExperimentMode,
    MutationMember, ObservationRecordV2, OperatorEventPayloadV2, OperatorEventV2,
    PropagationRecordV2, Provenance, RecordMetaV2, ReplacementOperatorEventV2, RigRecordV2,
    Schedule, SessionGoal, SessionStateV2, Station, IDENTITY_MAX_BYTES, SCHEMA_VERSION_V2,
    SCHEMA_VERSION_V3,
};

pub use crate::{
    operator_events::{
        reduce_operator_events_v3, EffectiveOperatorEventV3, OperatorEventReductionV3,
    },
    wspr::{
        project_wspr_run_v3, AntennaOccupancyIntervalV3, ArmedWsprCycleV3, WsprRunDiagnosticV3,
        WsprRunProjectionV3,
    },
};

pub type BundleFilesV3 = BundleFilesV2;
pub type BundleManifestV3 = BundleManifestV2;
pub type SessionStateV3 = SessionStateV2;
pub type AdapterRecordV3 = AdapterRecordV2;
pub type ObservationRecordV3 = ObservationRecordV2;
pub type PropagationRecordV3 = PropagationRecordV2;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignalPlanIdV3(String);

impl SignalPlanIdV3 {
    pub fn new(value: impl Into<String>) -> Result<Self, SignalPlanIdentityErrorV3> {
        let value = value.into();
        validate_signal_identity(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SignalVariantIdV3(String);

impl SignalVariantIdV3 {
    pub fn new(value: impl Into<String>) -> Result<Self, SignalPlanIdentityErrorV3> {
        let value = value.into();
        validate_signal_identity(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CounterbalanceBlockIdV3(String);

impl CounterbalanceBlockIdV3 {
    pub fn new(value: impl Into<String>) -> Result<Self, SignalPlanIdentityErrorV3> {
        let value = value.into();
        validate_signal_identity(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalPlanIdentityErrorV3 {
    Empty,
    TooLong,
    InvalidSyntax,
}

fn validate_signal_identity(value: &str) -> Result<(), SignalPlanIdentityErrorV3> {
    if value.is_empty() {
        return Err(SignalPlanIdentityErrorV3::Empty);
    }
    if value.len() > IDENTITY_MAX_BYTES {
        return Err(SignalPlanIdentityErrorV3::TooLong);
    }
    let mut previous_was_separator = true;
    for byte in value.bytes() {
        let separator = matches!(byte, b'.' | b'_' | b'-');
        if separator {
            if previous_was_separator {
                return Err(SignalPlanIdentityErrorV3::InvalidSyntax);
            }
        } else if !(byte.is_ascii_lowercase() || byte.is_ascii_digit()) {
            return Err(SignalPlanIdentityErrorV3::InvalidSyntax);
        }
        previous_was_separator = separator;
    }
    if previous_was_separator {
        return Err(SignalPlanIdentityErrorV3::InvalidSyntax);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalModeV3 {
    Cw,
    Rtty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalCollectionProfileV3 {
    ManualObservation,
    RbnCwV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalCadenceV3 {
    pub message: String,
    pub repetition_count: u16,
    pub key_speed_wpm: Option<u16>,
    pub transmit_seconds: u32,
    pub interval_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalPlanV3 {
    pub signal_plan_id: SignalPlanIdV3,
    pub mode: SignalModeV3,
    pub planned_power_watts: Option<f32>,
    pub transmitted_callsign: String,
    pub differing_identity_validated: bool,
    pub cadence: SignalCadenceV3,
    pub collection_profile: SignalCollectionProfileV3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalAllocationV3 {
    pub signal_plan_id: SignalPlanIdV3,
    pub frequency_hz: u64,
    pub frequency_variant_id: SignalVariantIdV3,
    pub counterbalance_block_id: CounterbalanceBlockIdV3,
    pub counterbalance_position: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedSlotV3 {
    pub slot_id: String,
    pub sequence_number: u32,
    pub starts_at: DateTime<Utc>,
    pub duration_seconds: u32,
    pub guard_seconds: u32,
    pub band: Band,
    pub antenna_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalAllocationV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WsprCycleIntentV3 {
    pub intent_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub antenna_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<WsprCycleDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalAllocationV3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsprCycleDirection {
    Receive,
    Transmit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleV3 {
    pub schema_version: u16,
    pub session_id: String,
    pub mode: ExperimentMode,
    pub goal: SessionGoal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antenna_control: Option<AntennaControlPolicyV5>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signal_plans: Vec<SignalPlanV3>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wspr_cycle_intents: Vec<WsprCycleIntentV3>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<PlannedSlotV3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordMetaV3 {
    pub schema_version: u16,
    pub session_id: String,
    pub recorded_at: DateTime<Utc>,
    pub provenance: Provenance,
    pub mutation: MutationMember,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalStateConfirmationV3 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_hz: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SignalModeV3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_watts: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transmitted_callsign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence_followed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CorrectableOperatorEventPayloadV3 {
    AntennaStateConfirmed {
        antenna_label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    SignalStateConfirmed {
        confirmation: SignalStateConfirmationV3,
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
pub struct ReplacementOperatorEventV3 {
    pub occurred_at: DateTime<Utc>,
    pub time_basis: EventTimeBasisV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_seconds: Option<u32>,
    pub slot_id: Option<String>,
    pub payload: CorrectableOperatorEventPayloadV3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EventCorrectionActionV3 {
    Retract,
    Replace {
        replacement: ReplacementOperatorEventV3,
    },
}

impl From<EventCorrectionActionV2> for EventCorrectionActionV3 {
    fn from(value: EventCorrectionActionV2) -> Self {
        match value {
            EventCorrectionActionV2::Retract => Self::Retract,
            EventCorrectionActionV2::Replace { replacement } => Self::Replace {
                replacement: ReplacementOperatorEventV3 {
                    occurred_at: replacement.occurred_at,
                    time_basis: replacement.time_basis,
                    uncertainty_seconds: replacement.uncertainty_seconds,
                    slot_id: replacement.slot_id,
                    payload: match replacement.payload {
                        crate::CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                            antenna_label,
                            note,
                        } => CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
                            antenna_label,
                            note,
                        },
                        crate::CorrectableOperatorEventPayloadV2::SlotMissed { reason } => {
                            CorrectableOperatorEventPayloadV3::SlotMissed { reason }
                        }
                        crate::CorrectableOperatorEventPayloadV2::SlotBad { reason } => {
                            CorrectableOperatorEventPayloadV3::SlotBad { reason }
                        }
                        crate::CorrectableOperatorEventPayloadV2::NoteAdded { note } => {
                            CorrectableOperatorEventPayloadV3::NoteAdded { note }
                        }
                    },
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperatorEventPayloadV3 {
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
    AntennaSwitchStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    WsprCycleArmed {
        antenna_label: String,
        cycle_starts_at: DateTime<Utc>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        readiness: Option<WsprReadinessBasisV5>,
    },
    AntennaStateConfirmed {
        antenna_label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    SignalStateConfirmed {
        confirmation: SignalStateConfirmationV3,
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
        correction: EventCorrectionActionV3,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorEventV3 {
    pub meta: RecordMetaV3,
    pub event_id: String,
    pub occurred_at: DateTime<Utc>,
    pub time_basis: EventTimeBasisV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty_seconds: Option<u32>,
    pub slot_id: Option<String>,
    pub payload: OperatorEventPayloadV3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RigRecordV3 {
    pub meta: RecordMetaV3,
    pub record_id: String,
    pub adapter_record_ids: Vec<String>,
    pub status: String,
    pub frequency_hz: Option<u64>,
    pub mode: Option<String>,
    pub power_watts: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub antenna_control: Option<AntennaControlInvocationV5>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleV3Contents {
    pub manifest: BundleManifestV3,
    pub session_state: SessionStateV3,
    pub station: Station,
    pub antennas: AntennasFile,
    pub schedule: ScheduleV3,
    pub events: Vec<OperatorEventV3>,
    pub observations: Vec<ObservationRecordV3>,
    pub adapter_records: Vec<AdapterRecordV3>,
    pub rig: Vec<RigRecordV3>,
    pub propagation: Vec<PropagationRecordV3>,
    pub analysis: AnalysisFile,
}

impl BundleV3Contents {
    /// Projects the v3 bundle into the established analysis/report model.
    ///
    /// Signal-specific planned and actual facts have no representation in that
    /// model and are intentionally omitted. Their provenance remains visible.
    pub fn into_current(mut self) -> CurrentBundleContents {
        crate::wspr_live_projection::repair_confirmed_wspr_live_observations(
            self.manifest.schema_version,
            &self.schedule,
            &self.events,
            &mut self.observations,
            &self.adapter_records,
        );
        let all_event_provenance = self
            .events
            .iter()
            .map(|event| CurrentRecordProvenance {
                record_kind: CurrentRecordKind::Event,
                record_id: event.event_id.clone(),
                provenance: event.meta.provenance.clone(),
            })
            .collect::<Vec<_>>();
        self.manifest.schema_version = SCHEMA_VERSION_V2;
        self.session_state.schema_version = SCHEMA_VERSION_V2;
        self.station.schema_version = SCHEMA_VERSION_V2;
        self.antennas.schema_version = SCHEMA_VERSION_V2;
        self.analysis.schema_version = SCHEMA_VERSION_V2;
        let mut projected_slots = self
            .schedule
            .slots
            .into_iter()
            .map(|slot| crate::PlannedSlot {
                slot_id: slot.slot_id,
                sequence_number: slot.sequence_number,
                starts_at: slot.starts_at,
                duration_seconds: slot.duration_seconds,
                guard_seconds: slot.guard_seconds,
                band: slot.band,
                antenna_label: slot.antenna_label,
            })
            .collect::<Vec<_>>();
        for event in &self.events {
            let OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label,
                cycle_starts_at,
                ..
            } = &event.payload
            else {
                continue;
            };
            let Some(intent_id) = event.slot_id.as_deref() else {
                continue;
            };
            let Some(intent) = self
                .schedule
                .wspr_cycle_intents
                .iter()
                .find(|intent| intent.intent_id == intent_id)
            else {
                continue;
            };
            if projected_slots
                .iter()
                .any(|slot| slot.slot_id == intent.intent_id)
            {
                continue;
            }
            projected_slots.push(crate::PlannedSlot {
                slot_id: intent.intent_id.clone(),
                sequence_number: intent.sequence_number,
                starts_at: *cycle_starts_at,
                duration_seconds: 120,
                guard_seconds: 0,
                band: intent.band,
                antenna_label: antenna_label.clone(),
            });
        }
        projected_slots.sort_by_key(|slot| (slot.sequence_number, slot.starts_at));
        let schedule = Schedule {
            schema_version: SCHEMA_VERSION_V2,
            session_id: self.schedule.session_id,
            mode: self.schedule.mode,
            goal: self.schedule.goal,
            slots: projected_slots,
        };
        let events = self
            .events
            .into_iter()
            .filter_map(project_v3_event_to_v2)
            .collect();
        for record in &mut self.observations {
            record.meta.schema_version = SCHEMA_VERSION_V2;
        }
        for record in &mut self.adapter_records {
            record.meta.schema_version = SCHEMA_VERSION_V2;
        }
        for record in &mut self.propagation {
            record.meta.schema_version = SCHEMA_VERSION_V2;
        }
        let rig = self
            .rig
            .into_iter()
            .map(|record| RigRecordV2 {
                meta: project_v3_meta_to_v2(record.meta),
                record_id: record.record_id,
                adapter_record_ids: record.adapter_record_ids,
                status: record.status,
                frequency_hz: record.frequency_hz,
                mode: record.mode,
                raw: record.raw,
            })
            .collect();
        let mut current = BundleV2Contents {
            manifest: self.manifest,
            session_state: self.session_state,
            station: self.station,
            antennas: self.antennas,
            schedule,
            events,
            observations: self.observations,
            adapter_records: self.adapter_records,
            rig,
            propagation: self.propagation,
            analysis: self.analysis,
        }
        .into_current();
        let projected_event_ids = current
            .record_provenance
            .iter()
            .filter(|record| record.record_kind == CurrentRecordKind::Event)
            .map(|record| record.record_id.clone())
            .collect::<BTreeSet<_>>();
        current.record_provenance.extend(
            all_event_provenance
                .into_iter()
                .filter(|record| !projected_event_ids.contains(&record.record_id)),
        );
        current
    }
}

fn project_v3_meta_to_v2(meta: RecordMetaV3) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V2,
        session_id: meta.session_id,
        recorded_at: meta.recorded_at,
        provenance: meta.provenance,
        mutation: meta.mutation,
    }
}

fn project_v3_event_to_v2(event: OperatorEventV3) -> Option<OperatorEventV2> {
    let payload = match event.payload {
        OperatorEventPayloadV3::SessionStarted { note } => {
            OperatorEventPayloadV2::SessionStarted { note }
        }
        OperatorEventPayloadV3::SessionInterrupted { reason } => {
            OperatorEventPayloadV2::SessionInterrupted { reason }
        }
        OperatorEventPayloadV3::InterruptionDetected { reason } => {
            OperatorEventPayloadV2::InterruptionDetected { reason }
        }
        OperatorEventPayloadV3::SessionResumed { note } => {
            OperatorEventPayloadV2::SessionResumed { note }
        }
        OperatorEventPayloadV3::SessionEnded { reason } => {
            OperatorEventPayloadV2::SessionEnded { reason }
        }
        OperatorEventPayloadV3::SessionAbandoned { reason } => {
            OperatorEventPayloadV2::SessionAbandoned { reason }
        }
        OperatorEventPayloadV3::AntennaSwitchStarted { .. } => return None,
        OperatorEventPayloadV3::WsprCycleArmed { antenna_label, .. } => {
            OperatorEventPayloadV2::AntennaStateConfirmed {
                antenna_label,
                note: Some("Antenna ready for the armed WSPR cycle.".into()),
            }
        }
        OperatorEventPayloadV3::AntennaStateConfirmed {
            antenna_label,
            note,
        } => OperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label,
            note,
        },
        OperatorEventPayloadV3::SignalStateConfirmed { .. } => return None,
        OperatorEventPayloadV3::SlotMissed { reason } => {
            OperatorEventPayloadV2::SlotMissed { reason }
        }
        OperatorEventPayloadV3::SlotBad { reason } => OperatorEventPayloadV2::SlotBad { reason },
        OperatorEventPayloadV3::NoteAdded { note } => OperatorEventPayloadV2::NoteAdded { note },
        OperatorEventPayloadV3::EventCorrected {
            target_event_id,
            correction,
            reason,
        } => OperatorEventPayloadV2::EventCorrected {
            target_event_id,
            correction: project_v3_correction_to_v2(correction)?,
            reason,
        },
    };
    Some(OperatorEventV2 {
        meta: project_v3_meta_to_v2(event.meta),
        event_id: event.event_id,
        occurred_at: event.occurred_at,
        time_basis: event.time_basis,
        uncertainty_seconds: event.uncertainty_seconds,
        slot_id: event.slot_id,
        payload,
    })
}

fn project_v3_correction_to_v2(
    correction: EventCorrectionActionV3,
) -> Option<EventCorrectionActionV2> {
    match correction {
        EventCorrectionActionV3::Retract => Some(EventCorrectionActionV2::Retract),
        EventCorrectionActionV3::Replace { replacement } => {
            Some(EventCorrectionActionV2::Replace {
                replacement: ReplacementOperatorEventV2 {
                    occurred_at: replacement.occurred_at,
                    time_basis: replacement.time_basis,
                    uncertainty_seconds: replacement.uncertainty_seconds,
                    slot_id: replacement.slot_id,
                    payload: match replacement.payload {
                        CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
                            antenna_label,
                            note,
                        } => CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                            antenna_label,
                            note,
                        },
                        CorrectableOperatorEventPayloadV3::SignalStateConfirmed { .. } => {
                            return None;
                        }
                        CorrectableOperatorEventPayloadV3::SlotMissed { reason } => {
                            CorrectableOperatorEventPayloadV2::SlotMissed { reason }
                        }
                        CorrectableOperatorEventPayloadV3::SlotBad { reason } => {
                            CorrectableOperatorEventPayloadV2::SlotBad { reason }
                        }
                        CorrectableOperatorEventPayloadV3::NoteAdded { note } => {
                            CorrectableOperatorEventPayloadV2::NoteAdded { note }
                        }
                    },
                },
            })
        }
    }
}

/// Deterministically upgrades the modeled contents of a schema-v2 bundle.
///
/// Schema v2 contains no planned signal allocations, confirmed signal state, or
/// rig power evidence. Those v3 fields therefore remain empty rather than being
/// inferred from unrelated records.
pub fn upgrade_v2_bundle_model(mut bundle: BundleV2Contents) -> BundleV3Contents {
    bundle.manifest.schema_version = SCHEMA_VERSION_V3;
    bundle.session_state.schema_version = SCHEMA_VERSION_V3;
    bundle.station.schema_version = SCHEMA_VERSION_V3;
    bundle.antennas.schema_version = SCHEMA_VERSION_V3;
    bundle.analysis.schema_version = SCHEMA_VERSION_V3;

    let schedule = ScheduleV3 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: bundle.schedule.session_id,
        mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        antenna_control: None,
        signal_plans: Vec::new(),
        wspr_cycle_intents: Vec::new(),
        slots: bundle
            .schedule
            .slots
            .into_iter()
            .map(|slot| PlannedSlotV3 {
                slot_id: slot.slot_id,
                sequence_number: slot.sequence_number,
                starts_at: slot.starts_at,
                duration_seconds: slot.duration_seconds,
                guard_seconds: slot.guard_seconds,
                band: slot.band,
                antenna_label: slot.antenna_label,
                signal: None,
            })
            .collect(),
    };

    let events = bundle.events.into_iter().map(upgrade_v2_event).collect();
    let mut observations = bundle.observations;
    for record in &mut observations {
        record.meta.schema_version = SCHEMA_VERSION_V3;
    }
    let mut adapter_records = bundle.adapter_records;
    for record in &mut adapter_records {
        record.meta.schema_version = SCHEMA_VERSION_V3;
    }
    let rig = bundle.rig.into_iter().map(upgrade_v2_rig).collect();
    let mut propagation = bundle.propagation;
    for record in &mut propagation {
        record.meta.schema_version = SCHEMA_VERSION_V3;
    }

    BundleV3Contents {
        manifest: bundle.manifest,
        session_state: bundle.session_state,
        station: bundle.station,
        antennas: bundle.antennas,
        schedule,
        events,
        observations,
        adapter_records,
        rig,
        propagation,
        analysis: bundle.analysis,
    }
}

fn upgrade_v2_meta(meta: RecordMetaV2) -> RecordMetaV3 {
    RecordMetaV3 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: meta.session_id,
        recorded_at: meta.recorded_at,
        provenance: meta.provenance,
        mutation: meta.mutation,
    }
}

fn upgrade_v2_event(event: OperatorEventV2) -> OperatorEventV3 {
    let payload = match event.payload {
        OperatorEventPayloadV2::SessionStarted { note } => {
            OperatorEventPayloadV3::SessionStarted { note }
        }
        OperatorEventPayloadV2::SessionInterrupted { reason } => {
            OperatorEventPayloadV3::SessionInterrupted { reason }
        }
        OperatorEventPayloadV2::InterruptionDetected { reason } => {
            OperatorEventPayloadV3::InterruptionDetected { reason }
        }
        OperatorEventPayloadV2::SessionResumed { note } => {
            OperatorEventPayloadV3::SessionResumed { note }
        }
        OperatorEventPayloadV2::SessionEnded { reason } => {
            OperatorEventPayloadV3::SessionEnded { reason }
        }
        OperatorEventPayloadV2::SessionAbandoned { reason } => {
            OperatorEventPayloadV3::SessionAbandoned { reason }
        }
        OperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label,
            note,
        } => OperatorEventPayloadV3::AntennaStateConfirmed {
            antenna_label,
            note,
        },
        OperatorEventPayloadV2::SlotMissed { reason } => {
            OperatorEventPayloadV3::SlotMissed { reason }
        }
        OperatorEventPayloadV2::SlotBad { reason } => OperatorEventPayloadV3::SlotBad { reason },
        OperatorEventPayloadV2::NoteAdded { note } => OperatorEventPayloadV3::NoteAdded { note },
        OperatorEventPayloadV2::EventCorrected {
            target_event_id,
            correction,
            reason,
        } => OperatorEventPayloadV3::EventCorrected {
            target_event_id,
            correction: correction.into(),
            reason,
        },
    };
    OperatorEventV3 {
        meta: upgrade_v2_meta(event.meta),
        event_id: event.event_id,
        occurred_at: event.occurred_at,
        time_basis: event.time_basis,
        uncertainty_seconds: event.uncertainty_seconds,
        slot_id: event.slot_id,
        payload,
    }
}

fn upgrade_v2_rig(record: RigRecordV2) -> RigRecordV3 {
    RigRecordV3 {
        meta: upgrade_v2_meta(record.meta),
        record_id: record.record_id,
        adapter_record_ids: record.adapter_record_ids,
        status: record.status,
        frequency_hz: record.frequency_hz,
        mode: record.mode,
        power_watts: None,
        antenna_control: None,
        raw: record.raw,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPlanDiagnosticV3 {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

pub fn validate_signal_plan_schedule_v3(
    station_callsign: &str,
    schedule: &ScheduleV3,
) -> Vec<SignalPlanDiagnosticV3> {
    let mut diagnostics = Vec::new();
    let mut plans = BTreeMap::new();

    for (index, plan) in schedule.signal_plans.iter().enumerate() {
        if plans.insert(plan.signal_plan_id.clone(), plan).is_some() {
            diagnostics.push(diagnostic(
                "signal_plan.duplicate_id",
                format!("/signal_plans/{index}/signal_plan_id"),
                "signal plan identity must be unique",
            ));
        }
        if plan
            .planned_power_watts
            .is_some_and(|power| !power.is_finite() || power <= 0.0)
        {
            diagnostics.push(diagnostic(
                "signal_plan.invalid_power",
                format!("/signal_plans/{index}/planned_power_watts"),
                "planned power must be finite and greater than zero",
            ));
        }
        if plan.cadence.message.trim().is_empty()
            || plan.cadence.repetition_count == 0
            || plan.cadence.transmit_seconds == 0
            || plan.cadence.interval_seconds < plan.cadence.transmit_seconds
            || (plan.mode == SignalModeV3::Cw && plan.cadence.key_speed_wpm.is_none())
        {
            diagnostics.push(diagnostic(
                "signal_plan.invalid_cadence",
                format!("/signal_plans/{index}/cadence"),
                "cadence must contain a message, repetitions, coherent timing, and CW key speed",
            ));
        }
        if !plan
            .transmitted_callsign
            .eq_ignore_ascii_case(station_callsign)
            && !plan.differing_identity_validated
        {
            diagnostics.push(diagnostic(
                "signal_plan.unvalidated_identity",
                format!("/signal_plans/{index}/transmitted_callsign"),
                "a transmitted identity differing from the station callsign requires explicit validation",
            ));
        }
        if plan.mode == SignalModeV3::Rtty
            && plan.collection_profile == SignalCollectionProfileV3::RbnCwV1
        {
            diagnostics.push(diagnostic(
                "signal_plan.profile_mode_mismatch",
                format!("/signal_plans/{index}/collection_profile"),
                "the RBN CW collection profile cannot be applied to RTTY",
            ));
        }
    }

    let mut allocations =
        BTreeMap::<SignalPlanIdV3, Vec<(usize, &PlannedSlotV3, &SignalAllocationV3)>>::new();
    for (index, slot) in schedule.slots.iter().enumerate() {
        let Some(allocation) = &slot.signal else {
            continue;
        };
        if allocation.frequency_hz == 0 {
            diagnostics.push(diagnostic(
                "signal_plan.invalid_frequency",
                format!("/slots/{index}/signal/frequency_hz"),
                "planned frequency must be greater than zero",
            ));
        }
        if !plans.contains_key(&allocation.signal_plan_id) {
            diagnostics.push(diagnostic(
                "signal_plan.unknown_reference",
                format!("/slots/{index}/signal/signal_plan_id"),
                "slot references an unknown signal plan",
            ));
            continue;
        }
        allocations
            .entry(allocation.signal_plan_id.clone())
            .or_default()
            .push((index, slot, allocation));
    }
    for (index, intent) in schedule.wspr_cycle_intents.iter().enumerate() {
        let Some(allocation) = &intent.signal else {
            continue;
        };
        if allocation.frequency_hz == 0 {
            diagnostics.push(diagnostic(
                "signal_plan.invalid_frequency",
                format!("/wspr_cycle_intents/{index}/signal/frequency_hz"),
                "planned frequency must be greater than zero",
            ));
        }
        if !plans.contains_key(&allocation.signal_plan_id) {
            diagnostics.push(diagnostic(
                "signal_plan.unknown_reference",
                format!("/wspr_cycle_intents/{index}/signal/signal_plan_id"),
                "cycle intent references an unknown signal plan",
            ));
        }
    }

    for (plan_id, mut slots) in allocations {
        let plan = plans[&plan_id];
        slots.sort_by_key(|(_, slot, _)| slot.starts_at);
        if plan.collection_profile == SignalCollectionProfileV3::RbnCwV1 {
            validate_rbn_suppression(&slots, &mut diagnostics);
        }
        validate_counterbalance(&slots, &mut diagnostics);
    }

    diagnostics
}

pub fn validate_signal_state_event_v3(
    schedule: &ScheduleV3,
    event: &OperatorEventV3,
) -> Vec<SignalPlanDiagnosticV3> {
    let OperatorEventPayloadV3::SignalStateConfirmed { confirmation } = &event.payload else {
        return Vec::new();
    };
    validate_signal_state_confirmation_v3(schedule, event.slot_id.as_deref(), confirmation)
}

pub fn validate_signal_state_confirmation_v3(
    schedule: &ScheduleV3,
    slot_id: Option<&str>,
    confirmation: &SignalStateConfirmationV3,
) -> Vec<SignalPlanDiagnosticV3> {
    let mut diagnostics = Vec::new();
    let Some(slot_id) = slot_id else {
        diagnostics.push(diagnostic(
            "signal_state.missing_slot",
            "/slot_id".into(),
            "signal-state confirmation must identify one planned slot",
        ));
        return diagnostics;
    };
    let allocation = schedule
        .slots
        .iter()
        .find(|slot| slot.slot_id == slot_id)
        .and_then(|slot| slot.signal.as_ref())
        .or_else(|| {
            schedule
                .wspr_cycle_intents
                .iter()
                .find(|intent| intent.intent_id == slot_id)
                .and_then(|intent| intent.signal.as_ref())
        });
    let known_reference = schedule.slots.iter().any(|slot| slot.slot_id == slot_id)
        || schedule
            .wspr_cycle_intents
            .iter()
            .any(|intent| intent.intent_id == slot_id);
    if !known_reference {
        diagnostics.push(diagnostic(
            "signal_state.unknown_slot",
            "/slot_id".into(),
            "signal-state confirmation references an unknown cycle intent",
        ));
        return diagnostics;
    }
    let Some(allocation) = allocation else {
        diagnostics.push(diagnostic(
            "signal_state.slot_without_plan",
            "/slot_id".into(),
            "signal-state confirmation references a cycle intent without a signal allocation",
        ));
        return diagnostics;
    };
    let Some(plan) = schedule
        .signal_plans
        .iter()
        .find(|plan| plan.signal_plan_id == allocation.signal_plan_id)
    else {
        diagnostics.push(diagnostic(
            "signal_state.unknown_plan",
            "/slot_id".into(),
            "signal-state confirmation cannot resolve the slot's signal plan",
        ));
        return diagnostics;
    };

    for (present, field) in [
        (confirmation.frequency_hz.is_some(), "frequency_hz"),
        (confirmation.mode.is_some(), "mode"),
        (
            confirmation.transmitted_callsign.is_some(),
            "transmitted_callsign",
        ),
        (confirmation.cadence_followed.is_some(), "cadence_followed"),
    ] {
        if !present {
            diagnostics.push(diagnostic(
                "signal_state.missing_actual_fact",
                format!("/payload/confirmation/{field}"),
                format!("actual {field} was not confirmed"),
            ));
        }
    }
    if confirmation.frequency_hz == Some(0) {
        diagnostics.push(diagnostic(
            "signal_state.invalid_frequency",
            "/payload/confirmation/frequency_hz".into(),
            "confirmed frequency must be greater than zero",
        ));
    } else if confirmation
        .frequency_hz
        .is_some_and(|value| value != allocation.frequency_hz)
    {
        diagnostics.push(diagnostic(
            "signal_state.frequency_mismatch",
            "/payload/confirmation/frequency_hz".into(),
            "confirmed frequency differs from the planned slot frequency",
        ));
    }
    if confirmation.mode.is_some_and(|value| value != plan.mode) {
        diagnostics.push(diagnostic(
            "signal_state.mode_mismatch",
            "/payload/confirmation/mode".into(),
            "confirmed mode differs from the signal plan",
        ));
    }
    if confirmation
        .power_watts
        .is_some_and(|value| !value.is_finite() || value <= 0.0)
    {
        diagnostics.push(diagnostic(
            "signal_state.invalid_power",
            "/payload/confirmation/power_watts".into(),
            "confirmed power must be finite and greater than zero",
        ));
    } else if matches!(
        (confirmation.power_watts, plan.planned_power_watts),
        (Some(actual), Some(planned)) if actual != planned
    ) {
        diagnostics.push(diagnostic(
            "signal_state.power_mismatch",
            "/payload/confirmation/power_watts".into(),
            "confirmed power differs from the signal plan",
        ));
    }
    if confirmation
        .transmitted_callsign
        .as_ref()
        .is_some_and(|value| !value.eq_ignore_ascii_case(&plan.transmitted_callsign))
    {
        diagnostics.push(diagnostic(
            "signal_state.identity_mismatch",
            "/payload/confirmation/transmitted_callsign".into(),
            "confirmed transmitted identity differs from the signal plan",
        ));
    }
    if confirmation.cadence_followed == Some(false) {
        diagnostics.push(diagnostic(
            "signal_state.cadence_mismatch",
            "/payload/confirmation/cadence_followed".into(),
            "operator reported that the planned cadence was not followed",
        ));
    }

    diagnostics
}

fn validate_rbn_suppression(
    slots: &[(usize, &PlannedSlotV3, &SignalAllocationV3)],
    diagnostics: &mut Vec<SignalPlanDiagnosticV3>,
) {
    for (left_offset, (left_index, left_slot, left)) in slots.iter().enumerate() {
        for (right_index, right_slot, right) in slots.iter().skip(left_offset + 1) {
            let separation_seconds = (right_slot.starts_at - left_slot.starts_at).num_seconds();
            if separation_seconds >= 600 {
                break;
            }
            let frequency_delta = left.frequency_hz.abs_diff(right.frequency_hz);
            if separation_seconds >= 0 && frequency_delta < 300 {
                diagnostics.push(diagnostic(
                    "signal_plan.rbn_suppression_risk",
                    format!("/slots/{right_index}/signal/frequency_hz"),
                    format!(
                        "RBN CW slots {left_index} and {right_index} are {separation_seconds}s and {frequency_delta}Hz apart; require at least 600s or 300Hz"
                    ),
                ));
            }
        }
    }
}

fn validate_counterbalance(
    slots: &[(usize, &PlannedSlotV3, &SignalAllocationV3)],
    diagnostics: &mut Vec<SignalPlanDiagnosticV3>,
) {
    let antennas = slots
        .iter()
        .map(|(_, slot, _)| slot.antenna_label.as_str())
        .collect::<BTreeSet<_>>();
    let variants = slots
        .iter()
        .map(|(_, _, allocation)| allocation.frequency_variant_id.clone())
        .collect::<BTreeSet<_>>();
    let expected_pairs = antennas.len().saturating_mul(variants.len());
    let mut variant_frequencies = BTreeMap::<SignalVariantIdV3, u64>::new();
    let mut antenna_positions = BTreeMap::<&str, (u64, u64)>::new();
    let mut variant_positions = BTreeMap::<SignalVariantIdV3, (u64, u64)>::new();
    let mut blocks = BTreeMap::<CounterbalanceBlockIdV3, Vec<_>>::new();
    for slot in slots {
        if variant_frequencies
            .insert(slot.2.frequency_variant_id.clone(), slot.2.frequency_hz)
            .is_some_and(|frequency| frequency != slot.2.frequency_hz)
        {
            diagnostics.push(diagnostic(
                "signal_plan.variant_frequency_mismatch",
                format!("/slots/{}/signal/frequency_hz", slot.0),
                "one frequency variant must map to one exact frequency",
            ));
        }
        let antenna_position = antenna_positions
            .entry(slot.1.antenna_label.as_str())
            .or_default();
        antenna_position.0 += u64::from(slot.2.counterbalance_position);
        antenna_position.1 += 1;
        let variant_position = variant_positions
            .entry(slot.2.frequency_variant_id.clone())
            .or_default();
        variant_position.0 += u64::from(slot.2.counterbalance_position);
        variant_position.1 += 1;
        blocks
            .entry(slot.2.counterbalance_block_id.clone())
            .or_default()
            .push(*slot);
    }

    for (block_id, block) in blocks {
        let pairs = block
            .iter()
            .map(|(_, slot, allocation)| {
                (
                    slot.antenna_label.as_str(),
                    allocation.frequency_variant_id.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        let positions = block
            .iter()
            .map(|(_, _, allocation)| allocation.counterbalance_position)
            .collect::<BTreeSet<_>>();
        if block.len() != expected_pairs
            || pairs.len() != expected_pairs
            || positions.len() != block.len()
        {
            diagnostics.push(diagnostic(
                "signal_plan.unbalanced_block",
                "/slots".into(),
                format!(
                    "counterbalance block {} must contain each antenna/frequency pair once with unique positions",
                    block_id.as_str()
                ),
            ));
        }
    }

    if !balanced_average_position(antenna_positions.values().copied())
        || !balanced_average_position(variant_positions.values().copied())
    {
        diagnostics.push(diagnostic(
            "signal_plan.unbalanced_order",
            "/slots".into(),
            "antenna and frequency variants must have equal average positions across complete blocks",
        ));
    }
}

fn balanced_average_position(values: impl Iterator<Item = (u64, u64)>) -> bool {
    let values = values.collect::<Vec<_>>();
    let Some(&(first_sum, first_count)) = values.first() else {
        return true;
    };
    values.iter().all(|(sum, count)| {
        first_count != 0 && *count != 0 && sum * first_count == first_sum * count
    })
}

fn diagnostic(
    code: &'static str,
    path: String,
    message: impl Into<String>,
) -> SignalPlanDiagnosticV3 {
    SignalPlanDiagnosticV3 {
        code,
        path,
        message: message.into(),
    }
}
