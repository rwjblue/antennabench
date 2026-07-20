use std::collections::BTreeSet;

use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    Band, BundleV3Contents, ExperimentMode, OperatorEventPayloadV3, RigRecordV3, ScheduleV3,
    WsprCycleDirection, WsprCycleWindow, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};

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
    Continued {
        source_ready_event_id: String,
    },
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
pub type OperatorEventV5 = crate::OperatorEventV3;
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
        if let WsprReadinessBasisV5::Continued {
            source_ready_event_id,
        } = readiness
        {
            validate_continued_readiness(bundle, event, source_ready_event_id)?;
            continue;
        }
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
            return Err(
                "command-verified readiness is incompatible with the session antenna-control policy"
                    .into(),
            );
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
                return Err(
                    "command-verified readiness has a failed, future, or mismatched invocation reference"
                        .into(),
                );
            }
            if record.meta.mutation.mutation_id != event.meta.mutation.mutation_id
                || record.meta.mutation.member_count != 3
                || event.meta.mutation.member_count != 3
            {
                return Err(
                    "command-verified rig records and armed event must share one three-member mutation"
                        .into(),
                );
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

fn validate_continued_readiness(
    bundle: &BundleV5Contents,
    event: &OperatorEventV5,
    source_ready_event_id: &str,
) -> Result<(), String> {
    if bundle.manifest.schema_version < SCHEMA_VERSION_V6 {
        return Err("continued readiness requires schema v6".into());
    }
    let event_index = bundle
        .events
        .iter()
        .position(|candidate| candidate.event_id == event.event_id)
        .ok_or_else(|| "continued readiness event is missing from its bundle".to_string())?;
    let source_index = bundle
        .events
        .iter()
        .position(|candidate| candidate.event_id == source_ready_event_id)
        .ok_or_else(|| "continued readiness references an unknown source event".to_string())?;
    if source_index >= event_index {
        return Err("continued readiness must reference earlier readiness".into());
    }
    let source = &bundle.events[source_index];
    let OperatorEventPayloadV3::WsprCycleArmed {
        antenna_label: source_antenna,
        cycle_starts_at: source_starts_at,
        readiness: source_readiness,
    } = &source.payload
    else {
        return Err("continued readiness source is not an armed WSPR cycle".into());
    };
    if matches!(
        source_readiness,
        Some(WsprReadinessBasisV5::Continued { .. })
    ) {
        return Err("continued readiness must reference original readiness evidence".into());
    }
    let OperatorEventPayloadV3::WsprCycleArmed { antenna_label, .. } = &event.payload else {
        unreachable!("caller filters armed WSPR events")
    };
    if source_antenna != antenna_label {
        return Err("continued readiness cannot change the antenna".into());
    }
    let source_intent_id = source
        .slot_id
        .as_deref()
        .ok_or_else(|| "continued readiness source has no intention".to_string())?;
    let intent_id = event
        .slot_id
        .as_deref()
        .ok_or_else(|| "continued readiness has no intention".to_string())?;
    let source_intent = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .find(|intent| intent.intent_id == source_intent_id)
        .ok_or_else(|| "continued readiness source intention is unknown".to_string())?;
    let intent = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .find(|intent| intent.intent_id == intent_id)
        .ok_or_else(|| "continued readiness intention is unknown".to_string())?;
    if source_intent.antenna_label != intent.antenna_label
        || source_intent.direction != intent.direction
        || source_intent.band != intent.band
        || source_intent.signal != intent.signal
    {
        return Err(
            "continued readiness cannot change antenna, direction, band, or signal context".into(),
        );
    }
    let source_window = WsprCycleWindow::from_start(*source_starts_at)
        .map_err(|_| "continued readiness source has invalid WSPR timing".to_string())?;
    if source_window.transmission_ends_at > event.occurred_at {
        return Err("continued readiness cannot be recorded during an active transmission".into());
    }
    let latest_prior_window = bundle.events[..event_index]
        .iter()
        .rev()
        .find_map(|candidate| match &candidate.payload {
            OperatorEventPayloadV3::WsprCycleArmed {
                cycle_starts_at, ..
            } => WsprCycleWindow::from_start(*cycle_starts_at).ok(),
            _ => None,
        })
        .ok_or_else(|| "continued readiness has no prior armed cycle".to_string())?;
    if latest_prior_window.transmission_ends_at > event.occurred_at {
        return Err("continued readiness cannot be recorded during an active transmission".into());
    }
    if bundle.events[source_index + 1..event_index]
        .iter()
        .any(|candidate| {
            matches!(
                candidate.payload,
                OperatorEventPayloadV3::AntennaSwitchStarted { .. }
                    | OperatorEventPayloadV3::SessionInterrupted { .. }
                    | OperatorEventPayloadV3::InterruptionDetected { .. }
                    | OperatorEventPayloadV3::SessionEnded { .. }
                    | OperatorEventPayloadV3::SessionAbandoned { .. }
            ) || matches!(
                candidate.payload,
                OperatorEventPayloadV3::WsprCycleArmed {
                    readiness: Some(WsprReadinessBasisV5::OperatorConfirmed)
                        | Some(WsprReadinessBasisV5::CommandVerified { .. })
                        | None,
                    ..
                }
            )
        })
    {
        return Err("continued readiness source no longer owns an open antenna occupancy".into());
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
