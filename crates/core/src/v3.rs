use std::collections::{BTreeMap, BTreeSet};

use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AdapterRecordV2, AnalysisFile, AntennasFile, Band, BundleFilesV2, BundleManifestV2,
    BundleV2Contents, CorrectableOperatorEventPayloadV2, CurrentBundleContents, CurrentRecordKind,
    CurrentRecordProvenance, EventCorrectionActionV2, EventTimeBasisV2, ExperimentMode,
    MutationMember, ObservationRecordV2, OperatorEventPayloadV2, OperatorEventV2,
    PropagationRecordV2, Provenance, RecordMetaV2, ReplacementOperatorEventV2, RigRecordV2,
    Schedule, SessionGoal, SessionStateV2, Station, IDENTITY_MAX_BYTES, SCHEMA_VERSION_V2,
    SCHEMA_VERSION_V3, SCHEMA_VERSION_V5,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AntennaControlPolicyV5 {
    #[default]
    Manual,
    CommandControlled {
        invocation: AntennaControlInvocationPolicyV5,
        manual_review_required: bool,
    },
}

impl AntennaControlPolicyV5 {
    pub fn is_manual(&self) -> bool {
        matches!(self, Self::Manual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntennaControlInvocationPolicyV5 {
    OperatorTriggered,
    Automatic,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WsprReadinessBasisV5 {
    #[default]
    OperatorConfirmed,
    CommandVerified {
        switch_record_id: String,
        verification_record_id: String,
    },
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

pub const COMMAND_OUTPUT_MAX_BYTES: usize = 64 * 1024;
pub const COMMAND_TEMPLATE_MAX_BYTES: usize = 16 * 1024;
pub const COMMAND_PROGRAM_MAX_BYTES: usize = 4 * 1024;
pub const COMMAND_ARGUMENT_MAX_BYTES: usize = 16 * 1024;
pub const COMMAND_ARGUMENT_COUNT_MAX: usize = 128;
pub const COMMAND_INVOCATION_MAX_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntennaControlRoleV5 {
    Switch,
    Verification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntennaControlCommandV5 {
    pub program_template: String,
    pub argument_templates: Vec<String>,
    pub resolved_program: String,
    pub resolved_arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntennaControlContextV5 {
    pub antenna: String,
    pub target: String,
    pub mode: ExperimentMode,
    pub direction: WsprCycleDirection,
    pub band: Band,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_hz: Option<u64>,
    pub sequence: u32,
    pub intent_id: String,
    pub session_id: String,
    pub callsign: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AntennaControlDispositionV5 {
    Exit { code: i32 },
    SpawnError { message: String },
    Signaled { signal: Option<i32> },
    Timeout,
}

impl AntennaControlDispositionV5 {
    pub fn is_exit_zero(&self) -> bool {
        matches!(self, Self::Exit { code: 0 })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntennaControlOutputEncodingV5 {
    Utf8,
    Base64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntennaControlOutputV5 {
    pub encoding: AntennaControlOutputEncodingV5,
    pub data: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AntennaControlInvocationV5 {
    pub role: AntennaControlRoleV5,
    pub controller_profile_name: String,
    pub controller_profile_revision: String,
    pub command: AntennaControlCommandV5,
    pub context: AntennaControlContextV5,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub elapsed_milliseconds: u64,
    pub disposition: AntennaControlDispositionV5,
    pub stdout: AntennaControlOutputV5,
    pub stderr: AntennaControlOutputV5,
}

pub type BundleV5Contents = BundleV3Contents;
pub type ScheduleV5 = ScheduleV3;
pub type OperatorEventV5 = OperatorEventV3;
pub type OperatorEventPayloadV5 = OperatorEventPayloadV3;
pub type RigRecordV5 = RigRecordV3;

/// Deterministically upgrades a schema-v3 or schema-v4 model to schema v5.
///
/// Historical ready actions were operator actions. The upgrade records that
/// fact explicitly and never invents command invocation evidence.
pub fn upgrade_v3_bundle_model_to_v5(mut bundle: BundleV3Contents) -> BundleV5Contents {
    bundle.manifest.schema_version = SCHEMA_VERSION_V5;
    bundle.session_state.schema_version = SCHEMA_VERSION_V5;
    bundle.station.schema_version = SCHEMA_VERSION_V5;
    bundle.antennas.schema_version = SCHEMA_VERSION_V5;
    bundle.schedule.schema_version = SCHEMA_VERSION_V5;
    bundle.schedule.antenna_control = Some(AntennaControlPolicyV5::Manual);
    bundle.analysis.schema_version = SCHEMA_VERSION_V5;
    for event in &mut bundle.events {
        event.meta.schema_version = SCHEMA_VERSION_V5;
        if let OperatorEventPayloadV3::WsprCycleArmed { readiness, .. } = &mut event.payload {
            *readiness = Some(WsprReadinessBasisV5::OperatorConfirmed);
        }
    }
    for record in &mut bundle.observations {
        record.meta.schema_version = SCHEMA_VERSION_V5;
    }
    for record in &mut bundle.adapter_records {
        record.meta.schema_version = SCHEMA_VERSION_V5;
    }
    for record in &mut bundle.rig {
        record.meta.schema_version = SCHEMA_VERSION_V5;
        record.antenna_control = None;
    }
    for record in &mut bundle.propagation {
        record.meta.schema_version = SCHEMA_VERSION_V5;
    }
    bundle
}

pub fn validate_antenna_control_v5(bundle: &BundleV5Contents) -> Result<(), String> {
    if bundle.manifest.schema_version < SCHEMA_VERSION_V5 {
        if bundle.schedule.antenna_control.is_some()
            || bundle
                .rig
                .iter()
                .any(|record| record.antenna_control.is_some())
            || bundle.events.iter().any(|event| {
                matches!(
                    event.payload,
                    OperatorEventPayloadV3::WsprCycleArmed {
                        readiness: Some(_),
                        ..
                    }
                )
            })
        {
            return Err(
                "schema-v3/v4 bundles cannot contain schema-v5 antenna-control fields".into(),
            );
        }
        return Ok(());
    }

    if bundle.schedule.antenna_control.is_none() {
        return Err("schema-v5 schedules require an explicit antenna-control policy".into());
    }
    if bundle
        .rig
        .iter()
        .any(|record| record.antenna_control.is_some())
        && !matches!(
            bundle.schedule.antenna_control,
            Some(AntennaControlPolicyV5::CommandControlled { .. })
        )
    {
        return Err(
            "antenna-control invocation evidence requires a command-controlled session policy"
                .into(),
        );
    }

    for record in &bundle.rig {
        if let Some(invocation) = &record.antenna_control {
            validate_invocation_v5(record, invocation, bundle)?;
        }
    }

    let mut referenced = BTreeSet::new();
    for event in &bundle.events {
        let OperatorEventPayloadV3::WsprCycleArmed {
            antenna_label,
            readiness,
            ..
        } = &event.payload
        else {
            continue;
        };
        let Some(readiness) = readiness else {
            return Err("schema-v5 armed WSPR cycles require an explicit readiness basis".into());
        };
        let WsprReadinessBasisV5::CommandVerified {
            switch_record_id,
            verification_record_id,
        } = readiness
        else {
            continue;
        };
        if switch_record_id == verification_record_id
            || !referenced.insert(switch_record_id.as_str())
            || !referenced.insert(verification_record_id.as_str())
        {
            return Err(
                "command-verified readiness references must be distinct and used once".into(),
            );
        }
        if !matches!(
            bundle.schedule.antenna_control.as_ref(),
            Some(AntennaControlPolicyV5::CommandControlled {
                manual_review_required: false,
                ..
            })
        ) {
            return Err("command-verified readiness is incompatible with the session antenna-control policy".into());
        }
        let intent_id = event.slot_id.as_deref().ok_or_else(|| {
            "command-verified readiness requires an intention reference".to_string()
        })?;
        let intent = bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .find(|intent| intent.intent_id == intent_id)
            .ok_or_else(|| {
                "command-verified readiness references an unknown intention".to_string()
            })?;
        let switch = referenced_invocation(bundle, switch_record_id, AntennaControlRoleV5::Switch)?;
        let verification = referenced_invocation(
            bundle,
            verification_record_id,
            AntennaControlRoleV5::Verification,
        )?;
        for (record, invocation) in [switch, verification] {
            let context = &invocation.context;
            if !invocation.disposition.is_exit_zero()
                || context.session_id != bundle.manifest.session_id
                || context.intent_id != intent.intent_id
                || context.antenna != *antenna_label
                || context.antenna != intent.antenna_label
                || context.mode != bundle.schedule.mode
                || context.direction
                    != intent.direction.ok_or_else(|| {
                        "command-verified intention requires a direction".to_string()
                    })?
                || context.band != intent.band
                || context.frequency_hz != intent.signal.as_ref().map(|signal| signal.frequency_hz)
                || context.sequence != intent.sequence_number
                || context.callsign != bundle.station.callsign
                || record.meta.recorded_at > event.meta.recorded_at
                || invocation.completed_at > event.meta.recorded_at
            {
                return Err("command-verified readiness has a failed, future, or mismatched invocation reference".into());
            }
            if record.meta.mutation.mutation_id != event.meta.mutation.mutation_id
                || record.meta.mutation.member_count != 3
                || event.meta.mutation.member_count != 3
            {
                return Err("command-verified rig records and armed event must share one three-member mutation".into());
            }
        }
        if switch.1.context.target != verification.1.context.target
            || switch.1.controller_profile_name != verification.1.controller_profile_name
            || switch.1.controller_profile_revision != verification.1.controller_profile_revision
            || switch.0.meta.mutation.member_index != 0
            || verification.0.meta.mutation.member_index != 1
            || event.meta.mutation.member_index != 2
            || switch.1.completed_at > verification.1.started_at
        {
            return Err("command-verified mutation order or target context is invalid".into());
        }
    }
    Ok(())
}

fn referenced_invocation<'a>(
    bundle: &'a BundleV5Contents,
    record_id: &str,
    role: AntennaControlRoleV5,
) -> Result<(&'a RigRecordV5, &'a AntennaControlInvocationV5), String> {
    let record = bundle
        .rig
        .iter()
        .find(|record| record.record_id == record_id)
        .ok_or_else(|| {
            format!("command-verified readiness references missing rig record {record_id:?}")
        })?;
    let invocation = record.antenna_control.as_ref().ok_or_else(|| {
        format!("referenced rig record {record_id:?} is not an antenna-control invocation")
    })?;
    if invocation.role != role {
        return Err(format!(
            "referenced rig record {record_id:?} has the wrong role"
        ));
    }
    Ok((record, invocation))
}

fn validate_invocation_v5(
    record: &RigRecordV5,
    invocation: &AntennaControlInvocationV5,
    bundle: &BundleV5Contents,
) -> Result<(), String> {
    let command = &invocation.command;
    if !record.adapter_record_ids.is_empty()
        || record.status != "antenna_control_attempt"
        || record.frequency_hz.is_some()
        || record.mode.is_some()
        || record.power_watts.is_some()
        || !record.raw.is_null()
        || invocation.context.session_id != bundle.manifest.session_id
        || invocation.completed_at < invocation.started_at
        || invocation.completed_at > record.meta.recorded_at
        || invocation.controller_profile_name.is_empty()
        || invocation.controller_profile_name.len() > 256
        || invocation.controller_profile_revision.is_empty()
        || invocation.controller_profile_revision.len() > 256
        || command.program_template.is_empty()
        || command.program_template.len() > COMMAND_TEMPLATE_MAX_BYTES
        || command.resolved_program.is_empty()
        || command.resolved_program.len() > COMMAND_PROGRAM_MAX_BYTES
        || command.argument_templates.len() > COMMAND_ARGUMENT_COUNT_MAX
        || command.resolved_arguments.len() != command.argument_templates.len()
        || command
            .argument_templates
            .iter()
            .any(|argument| argument.len() > COMMAND_ARGUMENT_MAX_BYTES)
        || command
            .resolved_arguments
            .iter()
            .any(|argument| argument.len() > COMMAND_ARGUMENT_MAX_BYTES)
        || invocation.context.target.is_empty()
        || invocation.context.target.len() > COMMAND_ARGUMENT_MAX_BYTES
        || matches!(
            &invocation.disposition,
            AntennaControlDispositionV5::SpawnError { message }
                if message.len() > COMMAND_ARGUMENT_MAX_BYTES
        )
    {
        return Err(format!(
            "rig record {:?} has an invalid bounded command invocation",
            record.record_id
        ));
    }
    let intent = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .find(|intent| intent.intent_id == invocation.context.intent_id)
        .ok_or_else(|| {
            format!(
                "rig record {:?} references an unknown command intention",
                record.record_id
            )
        })?;
    if invocation.context.antenna != intent.antenna_label
        || invocation.context.mode != bundle.schedule.mode
        || Some(invocation.context.direction) != intent.direction
        || invocation.context.band != intent.band
        || invocation.context.frequency_hz
            != intent.signal.as_ref().map(|signal| signal.frequency_hz)
        || invocation.context.sequence != intent.sequence_number
        || invocation.context.callsign != bundle.station.callsign
    {
        return Err(format!(
            "rig record {:?} has command context that does not match its intention",
            record.record_id
        ));
    }
    let invocation_bytes = command.program_template.len()
        + command
            .argument_templates
            .iter()
            .map(String::len)
            .sum::<usize>()
        + command.resolved_program.len()
        + command
            .resolved_arguments
            .iter()
            .map(String::len)
            .sum::<usize>();
    if invocation_bytes > COMMAND_INVOCATION_MAX_BYTES {
        return Err(format!(
            "rig record {:?} exceeds the expanded invocation limit",
            record.record_id
        ));
    }
    let elapsed = (invocation.completed_at - invocation.started_at).num_milliseconds();
    if elapsed < 0 || u64::try_from(elapsed).ok() != Some(invocation.elapsed_milliseconds) {
        return Err(format!(
            "rig record {:?} has inconsistent command timing",
            record.record_id
        ));
    }
    validate_output_v5(&invocation.stdout)?;
    validate_output_v5(&invocation.stderr)?;
    Ok(())
}

fn validate_output_v5(output: &AntennaControlOutputV5) -> Result<(), String> {
    let byte_len = match output.encoding {
        AntennaControlOutputEncodingV5::Utf8 => output.data.len(),
        AntennaControlOutputEncodingV5::Base64 => base64::engine::general_purpose::STANDARD
            .decode(&output.data)
            .map_err(|_| "antenna-control output contains invalid base64".to_string())?
            .len(),
    };
    if byte_len > COMMAND_OUTPUT_MAX_BYTES {
        return Err("antenna-control output exceeds the 64 KiB capture limit".into());
    }
    Ok(())
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
