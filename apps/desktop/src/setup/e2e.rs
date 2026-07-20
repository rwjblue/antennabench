use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::v2::V2_BUNDLE_SUFFIX;

use super::*;

#[derive(Debug)]
pub(crate) struct E2eCreatedSession {
    pub(crate) path: PathBuf,
    pub(crate) session_id: String,
    pub(crate) slot_ids: Vec<String>,
    pub(crate) antenna_labels: Vec<String>,
}

pub(crate) fn create_e2e_session(
    root: &Path,
    active_state: &ActiveSessionState,
) -> E2eCreatedSession {
    create_e2e_session_with_signal(root, active_state, false, false)
}

pub(crate) fn create_e2e_signal_session(
    root: &Path,
    active_state: &ActiveSessionState,
) -> E2eCreatedSession {
    create_e2e_session_with_signal(root, active_state, true, false)
}

pub(crate) fn create_e2e_controller_session(
    root: &Path,
    active_state: &ActiveSessionState,
) -> E2eCreatedSession {
    create_e2e_session_with_signal(root, active_state, false, true)
}

fn create_e2e_session_with_signal(
    root: &Path,
    active_state: &ActiveSessionState,
    with_signal: bool,
    with_controller: bool,
) -> E2eCreatedSession {
    #[derive(Debug)]
    struct DeterministicHooks(Mutex<u64>);

    impl LivePersistenceHooks for DeterministicHooks {
        fn now(&self) -> DateTime<Utc> {
            "2026-07-15T19:59:30Z".parse().unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.0.lock().unwrap();
            let id = format!("e2e-{kind}-{next:04}");
            *next += 1;
            id
        }
    }

    let setup_state = SetupSessionState::default();
    let mut draft = SetupDraft {
        station: SetupStationDraft {
            callsign: " n1rwj ".into(),
            grid: " FN42 ".into(),
            power_watts: "5".into(),
            operator_notes: "deterministic complete workflow".into(),
        },
        antennas: vec![
            SetupAntennaDraft {
                label: "Vertical".into(),
                facets: "omnidirectional, ground mounted".into(),
                height_m: "2.5".into(),
                radial_count: "16".into(),
                radial_length_m: "5".into(),
                orientation_degrees: "".into(),
                tuner: "none".into(),
                feedline: "RG-8X".into(),
                notes: "north lawn".into(),
            },
            SetupAntennaDraft {
                label: "Dipole".into(),
                facets: "broadside east-west".into(),
                height_m: "8".into(),
                radial_count: "".into(),
                radial_length_m: "".into(),
                orientation_degrees: "90".into(),
                tuner: "internal".into(),
                feedline: "RG-213".into(),
                notes: "".into(),
            },
        ],
        schedule: SetupScheduleDraft {
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            band: Band::M20,
            rounds: "2".into(),
        },
        wspr_live_acquisition_enabled: false,
        signal_plan: with_signal.then(|| SetupSignalPlanDraft {
            mode: SignalModeV3::Cw,
            collection_profile: SignalCollectionProfileV3::ManualObservation,
            planned_power_watts: "5".into(),
            transmitted_callsign: "n1rwj".into(),
            differing_identity_validated: false,
            message: "CQ CQ N1RWJ N1RWJ TEST".into(),
            repetition_count: "2".into(),
            key_speed_wpm: "20".into(),
            transmit_seconds: "20".into(),
            interval_seconds: "30".into(),
            frequencies_hz: "14050000".into(),
        }),
        antenna_controller: None,
    };
    if with_controller {
        draft.antenna_controller = Some(SetupControllerDraft {
            enabled: true,
            arm_for_session: false,
            invocation: antennabench_core::v5::AntennaControlInvocationPolicyV5::Automatic,
            manual_review_required: false,
            profile: crate::antenna_control::ControllerProfileDraft {
                profile_id: None,
                name: "E2E controller".into(),
                switch_command: crate::antenna_control::ControllerCommandDraft {
                    one_line: "switch {target}".into(),
                    program: String::new(),
                    arguments: Vec::new(),
                },
                verification_command: Some(crate::antenna_control::ControllerCommandDraft {
                    one_line: "verify {target}".into(),
                    program: String::new(),
                    arguments: Vec::new(),
                }),
                timeout_seconds: 10,
            },
            targets: vec![
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Vertical".into(),
                    target: "relay-a".into(),
                },
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Dipole".into(),
                    target: "relay-b".into(),
                },
            ],
        });
    }
    let review = build_review(&setup_state, draft, &DeterministicHooks(Mutex::new(1)))
        .expect("deterministic setup review");
    assert!(review.valid, "setup diagnostics: {:?}", review.diagnostics);
    let review_id = review.review_id.expect("valid review ID");
    let plan = review.plan.expect("valid reviewed plan");
    if with_signal {
        assert_eq!(plan.schedule_review.period_kind, "controlled_signal_slot");
        assert!(plan.schedule_review.wspr_cycle_count.is_none());
        assert!(plan.schedule_review.required_cycle_minutes.is_none());
        assert!(plan.schedule_review.finalization_grace_minutes.is_none());
    } else {
        assert_eq!(plan.schedule_review.period_kind, "wspr_cycle");
        assert_eq!(plan.schedule_review.wspr_cycle_count, Some(8));
        assert_eq!(plan.schedule_review.required_cycle_minutes, Some(16));
        assert!(plan.schedule_review.finalization_grace_minutes.is_none());
        assert_eq!(plan.schedule_review.transitions.len(), 7);
        assert!(plan
            .capabilities
            .can_describe
            .iter()
            .any(|statement| statement.contains("Transmit-path same-path signal differences")));
        assert!(plan
            .capabilities
            .can_describe
            .iter()
            .any(|statement| statement.contains("Receive-path same-path signal differences")));
        assert!(plan.capabilities.cannot_establish.iter().any(|statement| {
            statement.contains("reduces but does not eliminate time and propagation confounding")
        }));
    }
    let stem = if with_signal {
        "signal-workflow"
    } else {
        "complete-workflow"
    };
    let path = root.join(format!("{stem}{V2_BUNDLE_SUFFIX}"));
    let outcome = create_with_selection(&setup_state, active_state, &review_id, |_| {
        Ok(Some(path.clone()))
    })
    .expect("atomic setup creation");
    let CreateSessionOutcome::Created { session, .. } = outcome else {
        panic!("deterministic selection must create the session")
    };
    assert_eq!(session.session_id, plan.session_id);
    E2eCreatedSession {
        path,
        session_id: plan.session_id,
        slot_ids: plan.slots.into_iter().map(|slot| slot.slot_id).collect(),
        antenna_labels: plan
            .antennas
            .into_iter()
            .map(|antenna| antenna.label)
            .collect(),
    }
}
