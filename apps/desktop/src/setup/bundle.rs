use std::collections::BTreeSet;

use antennabench_core::{
    v2::BundleV2Contents,
    v3::{
        upgrade_v2_bundle_model, BundleV3Contents, CounterbalanceBlockIdV3, SignalAllocationV3,
        SignalCadenceV3, SignalPlanIdV3, SignalPlanV3, SignalVariantIdV3, WsprCycleDirection,
        WsprCycleIntentV3,
    },
    Antenna, Band, ExperimentMode, SCHEMA_VERSION_V5,
};
use antennabench_storage::LivePersistenceHooks;

use super::{
    ParsedSignalPlan, SessionErrorPayload, SetupAntennaReview, SetupCapabilityReview,
    SetupControllerReview, SetupPlanReview, SetupScheduleReview, SetupSignalPlanReview,
    SetupSlotReview, SetupSlotSignalReview, SetupStationReview, SetupTransitionReview,
};

/// Builds the durable current-schema schedule and its review projection.
///
/// This module deliberately contains all deterministic WSPR and signal-plan
/// expansion so draft parsing and committed creation cannot independently alter
/// the reviewed bundle shape.
pub(super) fn planned_wspr_cycles(
    antenna_labels: &[&String],
    mode: ExperimentMode,
    repetitions: u32,
) -> Vec<(String, WsprCycleDirection)> {
    let forward = antenna_labels
        .iter()
        .map(|label| (*label).clone())
        .collect::<Vec<_>>();
    let reverse = forward.iter().rev().cloned().collect::<Vec<_>>();
    let mut cycles = Vec::new();
    for repetition in 0..repetitions {
        let (first, second) = if repetition.is_multiple_of(2) {
            (&forward, &reverse)
        } else {
            (&reverse, &forward)
        };
        match mode {
            ExperimentMode::WholeStationAb | ExperimentMode::SingleAntennaProfiling => {
                cycles.extend(
                    first
                        .iter()
                        .cloned()
                        .map(|label| (label, WsprCycleDirection::Receive)),
                );
                cycles.extend(
                    second
                        .iter()
                        .cloned()
                        .map(|label| (label, WsprCycleDirection::Transmit)),
                );
            }
            ExperimentMode::TxFocused => cycles.extend(
                first
                    .iter()
                    .cloned()
                    .map(|label| (label, WsprCycleDirection::Transmit)),
            ),
            ExperimentMode::RxFocused => cycles.extend(
                first
                    .iter()
                    .cloned()
                    .map(|label| (label, WsprCycleDirection::Receive)),
            ),
        }
    }
    cycles
}

pub(super) fn use_latest_schema(bundle: &mut BundleV3Contents) {
    bundle.manifest.schema_version = SCHEMA_VERSION_V5;
    bundle.session_state.schema_version = SCHEMA_VERSION_V5;
    bundle.station.schema_version = SCHEMA_VERSION_V5;
    bundle.antennas.schema_version = SCHEMA_VERSION_V5;
    bundle.schedule.schema_version = SCHEMA_VERSION_V5;
    bundle.schedule.antenna_control = Some(antennabench_core::v5::AntennaControlPolicyV5::Manual);
    bundle.analysis.schema_version = SCHEMA_VERSION_V5;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_v3_setup_bundle(
    bundle: BundleV2Contents,
    plan: ParsedSignalPlan,
    antenna_labels: &[String],
    blocks: u32,
    band: Band,
    hooks: &dyn LivePersistenceHooks,
) -> Result<BundleV3Contents, SessionErrorPayload> {
    let mut bundle = upgrade_v2_bundle_model(bundle);
    let signal_plan_id = SignalPlanIdV3::new("primary").expect("fixed plan identity is valid");
    let variants = plan
        .frequencies_hz
        .iter()
        .enumerate()
        .map(|(index, frequency_hz)| {
            Ok((
                SignalVariantIdV3::new(format!("f-{}", index + 1)).map_err(|_| {
                    SessionErrorPayload::report_pipeline("generated frequency identity is invalid")
                })?,
                *frequency_hz,
            ))
        })
        .collect::<Result<Vec<_>, SessionErrorPayload>>()?;
    let block_size = antenna_labels
        .len()
        .checked_mul(variants.len())
        .ok_or_else(|| SessionErrorPayload::report_pipeline("signal schedule size overflowed"))?;
    let mut intents = Vec::with_capacity(
        block_size
            .checked_mul(usize::try_from(blocks).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                SessionErrorPayload::report_pipeline("signal schedule size overflowed")
            })?,
    );
    for block_index in 0..blocks {
        let block_id =
            CounterbalanceBlockIdV3::new(format!("block-{}", block_index + 1)).map_err(|_| {
                SessionErrorPayload::report_pipeline("generated counterbalance identity is invalid")
            })?;
        let mut pairs = Vec::with_capacity(block_size);
        for (antenna_index, antenna_label) in antenna_labels.iter().enumerate() {
            for (variant_index, (variant_id, frequency_hz)) in variants.iter().enumerate() {
                let forward = antenna_index * variants.len() + variant_index;
                let position = if block_index % 2 == 0 {
                    forward
                } else {
                    block_size - 1 - forward
                };
                pairs.push((
                    position,
                    antenna_label.clone(),
                    variant_id.clone(),
                    *frequency_hz,
                ));
            }
        }
        pairs.sort_by_key(|(position, _, _, _)| *position);
        for (position, antenna_label, frequency_variant_id, frequency_hz) in pairs {
            let sequence_number = u32::try_from(intents.len() + 1).map_err(|_| {
                SessionErrorPayload::report_pipeline("signal slot count overflowed")
            })?;
            intents.push(WsprCycleIntentV3 {
                intent_id: hooks.new_id("intent"),
                sequence_number,
                band,
                antenna_label,
                direction: Some(WsprCycleDirection::Transmit),
                signal: Some(SignalAllocationV3 {
                    signal_plan_id: signal_plan_id.clone(),
                    frequency_hz,
                    frequency_variant_id,
                    counterbalance_block_id: block_id.clone(),
                    counterbalance_position: u16::try_from(position).map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "counterbalance position exceeds the supported range",
                        )
                    })?,
                }),
            });
        }
    }
    bundle.schedule.signal_plans = vec![SignalPlanV3 {
        signal_plan_id,
        mode: plan.mode,
        planned_power_watts: plan.planned_power_watts,
        transmitted_callsign: plan.transmitted_callsign,
        differing_identity_validated: plan.differing_identity_validated,
        cadence: SignalCadenceV3 {
            message: plan.message,
            repetition_count: plan.repetition_count,
            key_speed_wpm: plan.key_speed_wpm,
            transmit_seconds: plan.transmit_seconds,
            interval_seconds: plan.interval_seconds,
        },
        collection_profile: plan.collection_profile,
    }];
    bundle.schedule.slots.clear();
    bundle.schedule.wspr_cycle_intents = intents;
    Ok(bundle)
}

pub(super) fn setup_plan_review_v3(
    bundle: &BundleV3Contents,
    antenna_controller: Option<SetupControllerReview>,
) -> SetupPlanReview {
    let signal_plan = bundle.schedule.signal_plans.first();
    let schedule_review = schedule_review(bundle);
    let capabilities = capability_review(bundle);
    SetupPlanReview {
        schema_version: bundle.manifest.schema_version,
        session_id: bundle.manifest.session_id.clone(),
        created_at: bundle.manifest.created_at,
        station: SetupStationReview {
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            power_watts: bundle.station.power_watts,
            operator_notes: bundle.station.operator_notes.clone(),
        },
        antennas: bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| SetupAntennaReview {
                label: antenna.label.clone(),
                context: antenna_context(antenna),
            })
            .collect(),
        mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        wspr_live_acquisition_enabled: bundle.session_state.wspr_live_acquisition_enabled,
        signal_plan: signal_plan.map(|plan| SetupSignalPlanReview {
            mode: plan.mode,
            collection_profile: plan.collection_profile,
            planned_power_watts: plan.planned_power_watts,
            transmitted_callsign: plan.transmitted_callsign.clone(),
            message: plan.cadence.message.clone(),
            repetition_count: plan.cadence.repetition_count,
            key_speed_wpm: plan.cadence.key_speed_wpm,
            transmit_seconds: plan.cadence.transmit_seconds,
            interval_seconds: plan.cadence.interval_seconds,
            frequencies_hz: bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .filter_map(|intent| intent.signal.as_ref().map(|signal| signal.frequency_hz))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        }),
        schedule_review,
        capabilities,
        slots: bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .map(|intent| SetupSlotReview {
                slot_id: intent.intent_id.clone(),
                sequence_number: intent.sequence_number,
                band: intent.band,
                antenna_label: intent.antenna_label.clone(),
                direction: intent.direction,
                signal: intent.signal.as_ref().map(|signal| SetupSlotSignalReview {
                    frequency_hz: signal.frequency_hz,
                    frequency_variant_id: signal.frequency_variant_id.as_str().into(),
                    counterbalance_block_id: signal.counterbalance_block_id.as_str().into(),
                    counterbalance_position: signal.counterbalance_position,
                }),
            })
            .collect(),
        antenna_controller,
    }
}

fn schedule_review(bundle: &BundleV3Contents) -> SetupScheduleReview {
    let slots = &bundle.schedule.wspr_cycle_intents;
    let transitions = slots
        .windows(2)
        .map(|pair| {
            let from = &pair[0];
            let to = &pair[1];
            let antenna_change = from.antenna_label != to.antenna_label;
            let direction_change = from.direction != to.direction;
            let summary = match (antenna_change, direction_change) {
                (true, true) => "Change antenna and TX/RX direction",
                (true, false) => "Change antenna; keep TX/RX direction",
                (false, true) => "Keep antenna; change TX/RX direction",
                (false, false) => "Keep antenna and TX/RX direction",
            };
            SetupTransitionReview {
                from_sequence_number: from.sequence_number,
                to_sequence_number: to.sequence_number,
                antenna_change,
                direction_change,
                summary,
            }
        })
        .collect::<Vec<_>>();
    let antenna_changes = transitions
        .iter()
        .filter(|transition| transition.antenna_change)
        .count();
    let direction_changes = transitions
        .iter()
        .filter(|transition| transition.direction_change)
        .count();
    let combined_changes = transitions
        .iter()
        .filter(|transition| transition.antenna_change && transition.direction_change)
        .count();
    let transition_summary = format!(
        "{} transitions: {antenna_changes} antenna changes, {direction_changes} direction changes, {combined_changes} requiring both.",
        transitions.len()
    );
    let scheduled_antenna_count = slots
        .iter()
        .map(|slot| slot.antenna_label.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let has_signal_plan = !bundle.schedule.signal_plans.is_empty();
    let counterbalance_explanation = if has_signal_plan {
        "Successive controlled-signal blocks reverse the antenna-by-frequency order so each combination is represented in early and late positions."
            .into()
    } else if scheduled_antenna_count < 2 {
        "A single-antenna plan has no A/B early/late order to alternate; transmit and receive direction changes remain explicit."
            .into()
    } else if bundle.schedule.mode == ExperimentMode::WholeStationAb {
        "Successive repetitions reverse the starting antenna while retaining directed receive and transmit blocks, balancing early/late positions without forcing an antenna change at every cycle."
            .into()
    } else {
        "Successive repetitions reverse the antenna order so each antenna is represented equally in early and late positions."
            .into()
    };
    if has_signal_plan {
        SetupScheduleReview {
            period_kind: "controlled_signal_slot",
            period_count: slots.len(),
            wspr_cycle_count: None,
            required_cycle_minutes: None,
            summary: format!(
                "{} controlled signal slots; timing follows the reviewed operator cadence.",
                slots.len()
            ),
            counterbalance_explanation,
            transition_summary,
            transitions,
        }
    } else {
        let required_cycle_minutes = u64::try_from(slots.len())
            .expect("setup slot count is bounded")
            .saturating_mul(2);
        SetupScheduleReview {
            period_kind: "wspr_cycle",
            period_count: slots.len(),
            wspr_cycle_count: Some(slots.len()),
            required_cycle_minutes: Some(required_cycle_minutes),
            summary: format!(
                "{} directed WSPR cycles; about {required_cycle_minutes} minutes of required cycle time.",
                slots.len()
            ),
            counterbalance_explanation,
            transition_summary,
            transitions,
        }
    }
}

fn capability_review(bundle: &BundleV3Contents) -> SetupCapabilityReview {
    let has_transmit = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .any(|slot| slot.direction == Some(WsprCycleDirection::Transmit));
    let has_receive = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .any(|slot| slot.direction == Some(WsprCycleDirection::Receive));
    let scheduled_antenna_count = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .map(|slot| slot.antenna_label.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let single_antenna = scheduled_antenna_count < 2;
    let mut can_describe = Vec::new();
    if single_antenna {
        if has_transmit {
            can_describe.push(
                "Transmit reach reported for the profiled antenna and its observed remote receiving paths; there is no A/B comparison."
                    .into(),
            );
        }
        if has_receive {
            can_describe.push(
                "Signals decoded by the profiled antenna and their observed remote transmitting paths; there is no A/B comparison."
                    .into(),
            );
        }
    } else {
        if has_transmit {
            can_describe.push(
                "Transmit-path same-path signal differences between scheduled antennas when the same remote receiving path reports both."
                    .into(),
            );
            can_describe.push(
                "Transmit-path overlap and unmatched receiving paths, kept separate from same-path differences."
                    .into(),
            );
        }
        if has_receive {
            can_describe.push(
                "Receive-path same-path signal differences between scheduled antennas when the same remote transmitting path is decoded on both."
                    .into(),
            );
            can_describe.push(
                "Receive-path overlap and unmatched transmitting paths, kept separate from same-path differences."
                    .into(),
            );
        }
    }
    can_describe.push(
        "Band and planned direction remain separate; distance and azimuth context is available only where valid locator evidence supports it."
            .into(),
    );
    can_describe.push(
        "Run-quality limits remain visible through missed, bad, or unknown cycles, acquisition gaps, duplicates, conflicts, and missing SNR."
            .into(),
    );

    let mut cannot_establish = Vec::new();
    if !has_receive {
        cannot_establish.push(
            "Receive-path antenna performance; this plan contains no receive periods.".into(),
        );
    }
    if !has_transmit {
        cannot_establish.push(
            "Transmit reach or transmit-path antenna performance; this plan contains no transmit periods."
                .into(),
        );
    }
    cannot_establish.push(
        "Universal antenna gain or performance beyond the recorded band, directions, times, station, and paths."
            .into(),
    );
    cannot_establish.push(
        "Causal superiority: counterbalancing reduces but does not eliminate time and propagation confounding."
            .into(),
    );
    cannot_establish.push(
        "A missing decode as a zero-strength measurement; unmatched paths remain separate.".into(),
    );
    cannot_establish.push(if bundle.session_state.wspr_live_acquisition_enabled {
        "Guaranteed public completeness: WSPR.live collection is best effort, and the upstream mirror does not provide an independent completeness guarantee."
            .into()
    } else {
        "Guaranteed public completeness: automatic WSPR.live collection is off, and direct/local receiver evidence remains separately attributed."
            .into()
    });
    cannot_establish.push(if single_antenna {
        "A relative antenna winner, superiority, equivalence, or same-path A/B difference; only one antenna is scheduled."
            .into()
    } else {
        "A winner, antenna superiority, or practical equivalence; this plan supports descriptive evidence only."
            .into()
    });
    SetupCapabilityReview {
        can_describe,
        cannot_establish,
    }
}

fn antenna_context(antenna: &Antenna) -> String {
    let mut context = Vec::new();
    if !antenna.facets.is_empty() {
        context.push(antenna.facets.join(", "));
    }
    if let Some(height) = antenna.height_m {
        context.push(format!("{height} m high"));
    }
    if let Some(orientation) = antenna.orientation_degrees {
        context.push(format!("{orientation}° orientation"));
    }
    if let Some(feedline) = &antenna.feedline {
        context.push(format!("feedline: {feedline}"));
    }
    if let Some(notes) = &antenna.notes {
        context.push(notes.clone());
    }
    context.join(" · ")
}
