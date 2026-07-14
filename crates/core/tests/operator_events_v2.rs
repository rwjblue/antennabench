use antennabench_core::{
    reduce_operator_events_v2, validate_lifecycle_transition_v2, validate_operator_event_append_v2,
    CorrectableOperatorEventPayloadV2, EventCorrectionActionV2, EventTimeBasisV2,
    LifecycleTransitionErrorV2, MutationMember, OperatorEventAppendErrorV2,
    OperatorEventDiagnosticCodeV2, OperatorEventPayloadV2, OperatorEventV2, Provenance,
    RecordMetaV2, RecordSource, ReplacementOperatorEventV2, SessionLifecycleV2, SCHEMA_VERSION_V2,
};
use chrono::{TimeZone, Utc};

const SESSION_ID: &str = "session-operator-events-v2";

#[test]
fn typed_event_timing_and_payload_round_trip_deterministically() {
    let event = event(
        "event-confirm",
        0,
        Some("slot-001"),
        OperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label: "B".into(),
            note: Some("actual state differs from plan".into()),
        },
    );
    let mut event = event;
    event.time_basis = EventTimeBasisV2::OperatorReported;
    event.uncertainty_seconds = Some(12);
    event.occurred_at -= chrono::Duration::seconds(30);

    let json = serde_json::to_string(&event).unwrap();
    let decoded: OperatorEventV2 = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, event);
    assert!(json.contains("\"recorded_at\""));
    assert!(json.contains("\"occurred_at\""));
    assert!(json.contains("\"operator_reported\""));
    assert!(json.contains("\"antenna_state_confirmed\""));
}

#[test]
fn lifecycle_reduction_accepts_every_valid_path_and_rejects_stale_or_terminal_changes() {
    let events = vec![
        event(
            "start",
            0,
            None,
            OperatorEventPayloadV2::SessionStarted { note: None },
        ),
        event(
            "interrupt",
            1,
            None,
            OperatorEventPayloadV2::SessionInterrupted {
                reason: Some("operator requested".into()),
            },
        ),
        event(
            "resume",
            2,
            None,
            OperatorEventPayloadV2::SessionResumed { note: None },
        ),
        event(
            "detected",
            3,
            None,
            OperatorEventPayloadV2::InterruptionDetected {
                reason: Some("writer lease was lost".into()),
            },
        ),
        event(
            "end",
            4,
            None,
            OperatorEventPayloadV2::SessionEnded { reason: None },
        ),
    ];

    let reduction = reduce_operator_events_v2(SessionLifecycleV2::Ready, &events);
    assert_eq!(reduction.lifecycle, SessionLifecycleV2::Ended);
    assert!(reduction.diagnostics.is_empty());

    assert_eq!(
        validate_lifecycle_transition_v2(
            SessionLifecycleV2::Ready,
            8,
            7,
            &OperatorEventPayloadV2::SessionStarted { note: None },
        ),
        Err(LifecycleTransitionErrorV2::StaleRevision {
            expected: 7,
            actual: 8,
        })
    );

    let mut invalid = events;
    invalid.push(event(
        "resume-after-end",
        5,
        None,
        OperatorEventPayloadV2::SessionResumed { note: None },
    ));
    let reduction = reduce_operator_events_v2(SessionLifecycleV2::Ready, &invalid);
    assert_eq!(reduction.lifecycle, SessionLifecycleV2::Ended);
    assert_eq!(
        reduction.diagnostics.last().unwrap().code,
        OperatorEventDiagnosticCodeV2::InvalidLifecycleTransition
    );
}

#[test]
fn corrections_use_append_order_and_invalid_targets_leave_the_effective_view_unchanged() {
    let original = event(
        "confirm-a",
        0,
        Some("slot-001"),
        OperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label: "A".into(),
            note: None,
        },
    );
    let correction = event(
        "correct-to-b",
        1,
        None,
        OperatorEventPayloadV2::EventCorrected {
            target_event_id: "confirm-a".into(),
            correction: EventCorrectionActionV2::Replace {
                replacement: ReplacementOperatorEventV2 {
                    occurred_at: original.occurred_at - chrono::Duration::hours(1),
                    time_basis: EventTimeBasisV2::OperatorReported,
                    uncertainty_seconds: Some(30),
                    slot_id: Some("slot-001".into()),
                    payload: CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                        antenna_label: "B".into(),
                        note: Some("wrong switch recorded initially".into()),
                    },
                },
            },
            reason: "operator corrected the actual antenna".into(),
        },
    );
    let invalid = event(
        "bad-target",
        2,
        None,
        OperatorEventPayloadV2::EventCorrected {
            target_event_id: "future-event".into(),
            correction: EventCorrectionActionV2::Retract,
            reason: "invalid future target".into(),
        },
    );

    let reduction = reduce_operator_events_v2(
        SessionLifecycleV2::Ready,
        &[original.clone(), correction.clone(), invalid],
    );
    assert_eq!(reduction.effective_events.len(), 1);
    assert_eq!(
        reduction.effective_events[0].payload,
        CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label: "B".into(),
            note: Some("wrong switch recorded initially".into()),
        }
    );
    assert_eq!(
        reduction.diagnostics[0].code,
        OperatorEventDiagnosticCodeV2::InvalidCorrectionTarget
    );

    let retraction = event(
        "retract",
        3,
        None,
        OperatorEventPayloadV2::EventCorrected {
            target_event_id: "confirm-a".into(),
            correction: EventCorrectionActionV2::Retract,
            reason: "confirmation was not trustworthy".into(),
        },
    );
    let reduction = reduce_operator_events_v2(
        SessionLifecycleV2::Ready,
        &[original, correction, retraction],
    );
    assert!(reduction.effective_events.is_empty());
}

#[test]
fn append_validation_is_idempotent_with_respect_to_invalid_or_stale_requests() {
    let existing = vec![event(
        "start",
        0,
        None,
        OperatorEventPayloadV2::SessionStarted { note: None },
    )];
    let duplicate = existing[0].clone();
    assert!(matches!(
        validate_operator_event_append_v2(SessionLifecycleV2::Ready, 1, 1, &existing, &duplicate,),
        Err(OperatorEventAppendErrorV2::InvalidEvent { .. })
    ));
    let interruption = event(
        "interrupt",
        1,
        None,
        OperatorEventPayloadV2::SessionInterrupted { reason: None },
    );
    assert_eq!(
        validate_operator_event_append_v2(
            SessionLifecycleV2::Ready,
            2,
            1,
            &existing,
            &interruption,
        ),
        Err(OperatorEventAppendErrorV2::StaleRevision {
            expected: 1,
            actual: 2,
        })
    );

    let accepted = validate_operator_event_append_v2(
        SessionLifecycleV2::Ready,
        1,
        1,
        &existing,
        &interruption,
    )
    .unwrap();
    assert_eq!(accepted.lifecycle, SessionLifecycleV2::Interrupted);
}

fn event(
    event_id: &str,
    index: u32,
    slot_id: Option<&str>,
    payload: OperatorEventPayloadV2,
) -> OperatorEventV2 {
    let recorded_at = Utc.with_ymd_and_hms(2026, 7, 14, 20, 0, index).unwrap();
    OperatorEventV2 {
        meta: RecordMetaV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: SESSION_ID.into(),
            recorded_at,
            provenance: Provenance::from_legacy(RecordSource::Operator, "test"),
            mutation: MutationMember {
                mutation_id: format!("mutation-{event_id}"),
                member_index: 0,
                member_count: 1,
            },
        },
        event_id: event_id.into(),
        occurred_at: recorded_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: slot_id.map(str::to_string),
        payload,
    }
}
