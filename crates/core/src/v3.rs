use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Band, ExperimentMode, SessionGoal, IDENTITY_MAX_BYTES};

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
pub struct ScheduleV3 {
    pub schema_version: u16,
    pub session_id: String,
    pub mode: ExperimentMode,
    pub goal: SessionGoal,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signal_plans: Vec<SignalPlanV3>,
    pub slots: Vec<PlannedSlotV3>,
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
