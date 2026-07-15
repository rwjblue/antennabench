use antennabench_core::{
    validate_signal_plan_schedule_v3, validate_signal_state_event_v3, AcquisitionChannelId,
    AdapterId, Band, CounterbalanceBlockIdV3, EventTimeBasisV2, ExperimentMode, MutationMember,
    OperatorEventPayloadV3, OperatorEventV3, PlannedSlotV3, Provenance, ProviderId, RecordMetaV3,
    ScheduleV3, SessionGoal, SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3,
    SignalModeV3, SignalPlanIdV3, SignalPlanV3, SignalStateConfirmationV3, SignalVariantIdV3,
    SourceId, SCHEMA_VERSION_V3,
};
use chrono::{Duration, TimeZone, Utc};

fn plan(mode: SignalModeV3, profile: SignalCollectionProfileV3) -> SignalPlanV3 {
    SignalPlanV3 {
        signal_plan_id: SignalPlanIdV3::new("comparison").unwrap(),
        mode,
        planned_power_watts: Some(10.0),
        transmitted_callsign: "N1RWJ".into(),
        differing_identity_validated: false,
        cadence: SignalCadenceV3 {
            message: "CQ CQ N1RWJ N1RWJ TEST".into(),
            repetition_count: 2,
            key_speed_wpm: (mode == SignalModeV3::Cw).then_some(20),
            transmit_seconds: 20,
            interval_seconds: 30,
        },
        collection_profile: profile,
    }
}

fn slot(
    sequence: u32,
    seconds: i64,
    antenna: &str,
    frequency_hz: u64,
    variant: &str,
    block: &str,
    position: u16,
) -> PlannedSlotV3 {
    PlannedSlotV3 {
        slot_id: format!("slot-{sequence}"),
        sequence_number: sequence,
        starts_at: Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap()
            + Duration::seconds(seconds),
        duration_seconds: 20,
        guard_seconds: 5,
        band: Band::M20,
        antenna_label: antenna.into(),
        signal: Some(SignalAllocationV3 {
            signal_plan_id: SignalPlanIdV3::new("comparison").unwrap(),
            frequency_hz,
            frequency_variant_id: SignalVariantIdV3::new(variant).unwrap(),
            counterbalance_block_id: CounterbalanceBlockIdV3::new(block).unwrap(),
            counterbalance_position: position,
        }),
    }
}

fn schedule(plan: SignalPlanV3, slots: Vec<PlannedSlotV3>) -> ScheduleV3 {
    ScheduleV3 {
        schema_version: SCHEMA_VERSION_V3,
        session_id: "session-v3".into(),
        mode: ExperimentMode::TxFocused,
        goal: SessionGoal::GeneralCoverage,
        signal_plans: vec![plan],
        slots,
    }
}

fn codes(schedule: &ScheduleV3) -> Vec<&'static str> {
    validate_signal_plan_schedule_v3("N1RWJ", schedule)
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn rbn_cw_frequency_and_time_boundaries_are_exact() {
    for delta in [299_u64, 300, 301] {
        let schedule = schedule(
            plan(SignalModeV3::Cw, SignalCollectionProfileV3::RbnCwV1),
            vec![
                slot(1, 0, "A", 14_050_000, "low", "block-1", 0),
                slot(2, 599, "A", 14_050_000 + delta, "high", "block-1", 1),
            ],
        );
        assert_eq!(
            codes(&schedule).contains(&"signal_plan.rbn_suppression_risk"),
            delta == 299,
            "delta={delta}"
        );
    }

    for seconds in [599_i64, 600, 601] {
        let schedule = schedule(
            plan(SignalModeV3::Cw, SignalCollectionProfileV3::RbnCwV1),
            vec![
                slot(1, 0, "A", 14_050_000, "fixed", "block-1", 0),
                slot(2, seconds, "A", 14_050_000, "fixed", "block-2", 0),
            ],
        );
        assert_eq!(
            codes(&schedule).contains(&"signal_plan.rbn_suppression_risk"),
            seconds == 599,
            "seconds={seconds}"
        );
    }
}

#[test]
fn complete_antenna_frequency_blocks_are_required() {
    let balanced = schedule(
        plan(
            SignalModeV3::Cw,
            SignalCollectionProfileV3::ManualObservation,
        ),
        vec![
            slot(1, 0, "A", 14_050_000, "low", "block-1", 0),
            slot(2, 30, "B", 14_050_300, "high", "block-1", 1),
            slot(3, 60, "B", 14_050_000, "low", "block-1", 2),
            slot(4, 90, "A", 14_050_300, "high", "block-1", 3),
            slot(5, 120, "A", 14_050_300, "high", "block-2", 0),
            slot(6, 150, "B", 14_050_000, "low", "block-2", 1),
            slot(7, 180, "B", 14_050_300, "high", "block-2", 2),
            slot(8, 210, "A", 14_050_000, "low", "block-2", 3),
        ],
    );
    assert!(!codes(&balanced).contains(&"signal_plan.unbalanced_block"));
    assert!(!codes(&balanced).contains(&"signal_plan.unbalanced_order"));

    let mut incomplete = balanced;
    incomplete.slots.pop();
    assert!(codes(&incomplete).contains(&"signal_plan.unbalanced_block"));
}

#[test]
fn callsign_differences_and_rtty_profile_mismatches_are_explicit() {
    let mut invalid_plan = plan(SignalModeV3::Rtty, SignalCollectionProfileV3::RbnCwV1);
    invalid_plan.transmitted_callsign = "N1RWJ/P".into();
    let schedule = schedule(invalid_plan, Vec::new());
    let codes = codes(&schedule);
    assert!(codes.contains(&"signal_plan.unvalidated_identity"));
    assert!(codes.contains(&"signal_plan.profile_mode_mismatch"));
}

#[test]
fn signal_plan_machine_identities_are_bounded_and_lowercase() {
    assert!(SignalPlanIdV3::new("rbn-cw.primary").is_ok());
    for invalid in ["", "RBN", "-leading", "two..dots", "has space"] {
        assert!(SignalPlanIdV3::new(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn manual_signal_confirmation_is_typed_and_reports_missing_or_changed_actual_facts() {
    let schedule = schedule(
        plan(
            SignalModeV3::Cw,
            SignalCollectionProfileV3::ManualObservation,
        ),
        vec![slot(1, 0, "A", 14_050_000, "fixed", "block-1", 0)],
    );
    let event = OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V3,
            session_id: "session-v3".into(),
            recorded_at: schedule.slots[0].starts_at,
            provenance: Provenance {
                provider_id: ProviderId::new("antennabench").unwrap(),
                source_id: SourceId::new("operator-evidence").unwrap(),
                acquisition_channel: AcquisitionChannelId::new("operator-entry").unwrap(),
                adapter_id: AdapterId::new("antennabench.operator").unwrap(),
                adapter_version: "3".into(),
            },
            mutation: MutationMember {
                mutation_id: "mutation-1".into(),
                member_index: 0,
                member_count: 1,
            },
        },
        event_id: "signal-confirmation-1".into(),
        occurred_at: schedule.slots[0].starts_at,
        time_basis: EventTimeBasisV2::OperatorReported,
        uncertainty_seconds: None,
        slot_id: Some("slot-1".into()),
        payload: OperatorEventPayloadV3::SignalStateConfirmed {
            confirmation: SignalStateConfirmationV3 {
                frequency_hz: Some(14_050_100),
                mode: Some(SignalModeV3::Rtty),
                power_watts: None,
                transmitted_callsign: None,
                cadence_followed: Some(false),
                note: Some("operator noticed the mismatch".into()),
            },
        },
    };

    let diagnostics = validate_signal_state_event_v3(&schedule, &event);
    let codes = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"signal_state.frequency_mismatch"));
    assert!(codes.contains(&"signal_state.mode_mismatch"));
    assert!(codes.contains(&"signal_state.missing_actual_fact"));
    assert!(codes.contains(&"signal_state.cadence_mismatch"));

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["payload"]["kind"], "signal_state_confirmed");
    assert_eq!(json["payload"]["confirmation"]["frequency_hz"], 14_050_100);
    assert!(json["payload"]["confirmation"].get("power_watts").is_none());
    assert_eq!(
        serde_json::from_value::<OperatorEventV3>(json).unwrap(),
        event
    );
}
