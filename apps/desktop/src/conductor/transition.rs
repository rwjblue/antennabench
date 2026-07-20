//! Trusted cycle-to-cycle transition evaluation and Rust-owned continuation.

use std::{path::Path, sync::Arc, thread, time::Duration as StdDuration};

use antennabench_core::{
    next_wspr_cycle_after_ready,
    v2::{EventTimeBasisV2, MutationMember, Provenance, SessionLifecycleV2},
    v3::{
        project_wspr_run_v3, BundleV3Contents, OperatorEventPayloadV3, OperatorEventV3,
        RecordMetaV3, WsprCycleIntentV3,
    },
    v5::{
        AntennaControlInvocationPolicyV5, AntennaControlPolicyV5, AntennaControlRoleV5,
        WsprReadinessBasisV5,
    },
    RecordSource, SCHEMA_VERSION_V6,
};
use antennabench_storage::{
    BundleStore, LiveEventMutationV3, LivePersistenceHooks, SystemLivePersistenceHooks,
};
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use super::{
    live_error_payload, ConductorSessionState, ConductorTransitionView, TransitionDisposition,
};
use crate::open_session::{
    active_session_source, update_active_session_live_projection,
    with_waiting_foreground_operation, ActiveSessionState, SessionErrorKind, SessionErrorPayload,
};

const WAIT_POLL: StdDuration = StdDuration::from_millis(100);

#[derive(Debug, Clone)]
pub(crate) struct TransitionPlan {
    pub(crate) view: ConductorTransitionView,
    pub(crate) operator_action_required: bool,
    source_ready_event_id: Option<String>,
    next_intent_id: Option<String>,
    prior_transmission_ends_at: Option<DateTime<Utc>>,
}

impl TransitionPlan {
    pub(crate) fn can_continue(&self) -> bool {
        [
            self.view.antenna,
            self.view.direction,
            self.view.band,
            self.view.signal,
        ]
        .into_iter()
        .all(|value| value == TransitionDisposition::NoChangeNeeded)
            && self.source_ready_event_id.is_some()
            && self.next_intent_id.is_some()
            && self.prior_transmission_ends_at.is_some()
    }

    pub(crate) fn prior_transmission_ends_at(&self) -> Option<DateTime<Utc>> {
        self.prior_transmission_ends_at
    }

    pub(crate) fn automatic_verification_can_arm(&self) -> bool {
        self.view.antenna == TransitionDisposition::AutomaticActionPending
            && [self.view.direction, self.view.band, self.view.signal]
                .into_iter()
                .all(|value| value == TransitionDisposition::NoChangeNeeded)
    }
}

pub(crate) fn transition_plan(
    bundle: &BundleV3Contents,
    next: Option<&WsprCycleIntentV3>,
) -> TransitionPlan {
    let unknown = ConductorTransitionView {
        antenna: TransitionDisposition::UnknownBlocked,
        direction: TransitionDisposition::UnknownBlocked,
        band: TransitionDisposition::UnknownBlocked,
        signal: TransitionDisposition::UnknownBlocked,
    };
    let Some(next) = next else {
        return TransitionPlan {
            view: unknown,
            operator_action_required: true,
            source_ready_event_id: None,
            next_intent_id: None,
            prior_transmission_ends_at: None,
        };
    };
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let Some(occupancy) = projection
        .occupancies
        .last()
        .filter(|value| value.ends_at.is_none())
    else {
        return TransitionPlan {
            view: unknown,
            operator_action_required: true,
            source_ready_event_id: None,
            next_intent_id: Some(next.intent_id.clone()),
            prior_transmission_ends_at: projection
                .cycles
                .last()
                .map(|cycle| cycle.window.transmission_ends_at),
        };
    };
    let Some(previous_cycle) = projection.cycles.last().filter(|cycle| {
        cycle.ready_event_id == occupancy.ready_event_id
            && cycle.antenna_label == occupancy.antenna_label
    }) else {
        return TransitionPlan {
            view: unknown,
            operator_action_required: true,
            source_ready_event_id: None,
            next_intent_id: Some(next.intent_id.clone()),
            prior_transmission_ends_at: None,
        };
    };
    let Some(previous) = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .find(|intent| intent.intent_id == previous_cycle.intent_id)
    else {
        return TransitionPlan {
            view: unknown,
            operator_action_required: true,
            source_ready_event_id: None,
            next_intent_id: Some(next.intent_id.clone()),
            prior_transmission_ends_at: Some(previous_cycle.window.transmission_ends_at),
        };
    };

    let verified_automatically = has_verified_automatic_attempt(bundle, &next.intent_id);
    let successful_switch_attempt = has_successful_switch_attempt(bundle, &next.intent_id);
    let antenna = if previous.antenna_label == next.antenna_label {
        TransitionDisposition::NoChangeNeeded
    } else if verified_automatically {
        TransitionDisposition::AutomaticallyCompletedAndVerified
    } else if successful_switch_attempt {
        TransitionDisposition::OperatorActionRequired
    } else if matches!(
        bundle.schedule.antenna_control,
        Some(AntennaControlPolicyV5::CommandControlled {
            invocation: AntennaControlInvocationPolicyV5::Automatic,
            ..
        })
    ) {
        TransitionDisposition::AutomaticActionPending
    } else {
        TransitionDisposition::OperatorActionRequired
    };
    let direction = unchanged(previous.direction, next.direction);
    let band = unchanged(previous.band, next.band);
    let signal = unchanged(previous.signal.as_ref(), next.signal.as_ref());
    let manual_review_required = matches!(
        bundle.schedule.antenna_control,
        Some(AntennaControlPolicyV5::CommandControlled {
            manual_review_required: true,
            ..
        })
    ) && antenna
        == TransitionDisposition::AutomaticallyCompletedAndVerified;
    let operator_action_required = manual_review_required
        || [antenna, direction, band, signal].into_iter().any(|value| {
            !matches!(
                value,
                TransitionDisposition::NoChangeNeeded
                    | TransitionDisposition::AutomaticallyCompletedAndVerified
            )
        });
    TransitionPlan {
        view: ConductorTransitionView {
            antenna,
            direction,
            band,
            signal,
        },
        operator_action_required,
        source_ready_event_id: Some(occupancy.ready_event_id.clone()),
        next_intent_id: Some(next.intent_id.clone()),
        prior_transmission_ends_at: Some(previous_cycle.window.transmission_ends_at),
    }
}

fn has_verified_automatic_attempt(bundle: &BundleV3Contents, intent_id: &str) -> bool {
    bundle.rig.iter().any(|switch| {
        let Some(switch_invocation) = switch.antenna_control.as_ref().filter(|invocation| {
            invocation.role == AntennaControlRoleV5::Switch
                && invocation.context.intent_id == intent_id
                && invocation.disposition.is_exit_zero()
        }) else {
            return false;
        };
        bundle.rig.iter().any(|verification| {
            verification.meta.mutation.mutation_id == switch.meta.mutation.mutation_id
                && verification
                    .antenna_control
                    .as_ref()
                    .is_some_and(|invocation| {
                        invocation.role == AntennaControlRoleV5::Verification
                            && invocation.context.intent_id == intent_id
                            && invocation.context.target == switch_invocation.context.target
                            && invocation.disposition.is_exit_zero()
                    })
        })
    })
}

fn has_successful_switch_attempt(bundle: &BundleV3Contents, intent_id: &str) -> bool {
    bundle.rig.iter().any(|record| {
        record.antenna_control.as_ref().is_some_and(|invocation| {
            invocation.role == AntennaControlRoleV5::Switch
                && invocation.context.intent_id == intent_id
                && invocation.disposition.is_exit_zero()
        })
    })
}

fn unchanged<T: PartialEq>(previous: T, next: T) -> TransitionDisposition {
    if previous == next {
        TransitionDisposition::NoChangeNeeded
    } else {
        TransitionDisposition::OperatorActionRequired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContinuationOutcome {
    NotApplicable,
    Waiting(DateTime<Utc>),
    Continued,
}

pub(crate) fn persist_continued_readiness(
    active_state: &ActiveSessionState,
    source: &Path,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<ContinuationOutcome, SessionErrorPayload> {
    with_waiting_foreground_operation(active_state, || {
        let (active_source, _) = active_session_source(active_state)?;
        if active_source != source {
            return Ok(ContinuationOutcome::NotApplicable);
        }
        let store = BundleStore::new(source);
        let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
        if bundle.manifest.schema_version < SCHEMA_VERSION_V6
            || bundle.session_state.lifecycle != SessionLifecycleV2::Running
        {
            return Ok(ContinuationOutcome::NotApplicable);
        }
        let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
        let next = bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
            !projection
                .cycles
                .iter()
                .any(|cycle| cycle.intent_id == intent.intent_id)
                && !projection
                    .skipped_intent_ids
                    .iter()
                    .any(|id| id == &intent.intent_id)
        });
        let plan = transition_plan(&bundle, next);
        if !plan.can_continue() {
            return Ok(ContinuationOutcome::NotApplicable);
        }
        let now = hooks.now();
        let deadline = plan
            .prior_transmission_ends_at()
            .expect("continuation has a deadline");
        if now < deadline {
            return Ok(ContinuationOutcome::Waiting(deadline));
        }
        let next = next.expect("continuation has a pending intention");
        let cycle = next_wspr_cycle_after_ready(now, Duration::seconds(1)).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The next WSPR cycle could not be calculated.",
                error.to_string(),
            )
        })?;
        let source_ready_event_id = plan
            .source_ready_event_id
            .expect("continuation has source evidence");
        let identity = continuation_identity(
            &bundle.manifest.session_id,
            &source_ready_event_id,
            &next.intent_id,
        );
        let event = OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: bundle.manifest.schema_version,
                session_id: bundle.manifest.session_id.clone(),
                recorded_at: now,
                provenance: Provenance::from_legacy(
                    RecordSource::Derived,
                    "conductor-continuation-v1",
                ),
                mutation: MutationMember {
                    mutation_id: "pending".into(),
                    member_index: 0,
                    member_count: 1,
                },
                runtime_context_id: None,
            },
            event_id: format!("event-{identity}"),
            occurred_at: now,
            time_basis: EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id: Some(next.intent_id.clone()),
            payload: OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: next.antenna_label.clone(),
                cycle_starts_at: cycle.starts_at,
                readiness: Some(WsprReadinessBasisV5::Continued {
                    source_ready_event_id,
                }),
            },
        };
        let mutation_id = format!("mutation-{identity}");
        let append = {
            let mut writer = crate::build_context::open_v3_writer_with_hooks(&store, hooks)
                .map_err(live_error_payload)?;
            writer.append_event(LiveEventMutationV3 {
                expected_revision: bundle.session_state.revision,
                mutation_id: mutation_id.clone(),
                event,
            })
        };
        if let Err(error) = append {
            let committed = store.read_v3_checkpointed().ok().is_some_and(|current| {
                current.session_state.last_committed_mutation_id.as_deref() == Some(&mutation_id)
            });
            if !committed {
                return Err(live_error_payload(error));
            }
        }
        let committed = store.read_v3_checkpointed().map_err(live_error_payload)?;
        update_active_session_live_projection(active_state, source, &committed)?;
        Ok(ContinuationOutcome::Continued)
    })
}

fn continuation_identity(session_id: &str, source_event_id: &str, intent_id: &str) -> String {
    let digest = Sha256::digest(format!("{session_id}\0{source_event_id}\0{intent_id}").as_bytes());
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("continued-{}", &hex[..32])
}

pub(crate) fn schedule_transition_coordinator(app: AppHandle) {
    let active_state = app.state::<ActiveSessionState>();
    let Ok((source, _)) = active_session_source(active_state.inner()) else {
        return;
    };
    let Ok(bundle) = BundleStore::new(&source).read_v3_checkpointed() else {
        return;
    };
    if bundle.session_state.lifecycle != SessionLifecycleV2::Running {
        return;
    }
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let next = bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
        !projection
            .cycles
            .iter()
            .any(|cycle| cycle.intent_id == intent.intent_id)
            && !projection
                .skipped_intent_ids
                .iter()
                .any(|id| id == &intent.intent_id)
    });
    if !transition_plan(&bundle, next).can_continue() {
        return;
    }
    let state = app.state::<ConductorSessionState>();
    let Ok(Some(generation)) = state.begin_continuation_worker(&source) else {
        return;
    };
    thread::spawn(move || run_transition_worker(app, source, generation));
}

fn run_transition_worker(app: AppHandle, source: std::path::PathBuf, generation: u64) {
    loop {
        let active = app.state::<ActiveSessionState>();
        match persist_continued_readiness(
            active.inner(),
            &source,
            Arc::new(SystemLivePersistenceHooks),
        ) {
            Ok(ContinuationOutcome::Waiting(deadline)) => {
                while Utc::now() < deadline {
                    thread::sleep(WAIT_POLL);
                }
            }
            Ok(ContinuationOutcome::Continued) => {
                app.state::<ConductorSessionState>()
                    .finish_continuation_worker(generation);
                schedule_transition_coordinator(app);
                return;
            }
            Ok(ContinuationOutcome::NotApplicable) => {
                app.state::<ConductorSessionState>()
                    .finish_continuation_worker(generation);
                return;
            }
            Err(_) => {
                // This mutation has a deterministic identity and no external
                // side effect, so retrying persistence is safe. Re-evaluation
                // on every pass stops the loop after interruption, replacement,
                // or any newly unresolved station requirement.
                thread::sleep(StdDuration::from_secs(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use antennabench_core::{
        v2::V2_BUNDLE_SUFFIX,
        v3::{
            CounterbalanceBlockIdV3, SignalAllocationV3, SignalPlanIdV3, SignalVariantIdV3,
            WsprCycleDirection,
        },
        v5::{validate_antenna_control_v5, WsprReadinessBasisV5},
        Band, ExperimentMode,
    };
    use antennabench_storage::LivePersistencePoint;
    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        conductor::{
            actions::{ConductorAction, ConductorMutationRequest},
            live_session::{mutate_conductor_with_hooks, read_conductor_with_hooks},
        },
        open_session::{activate_created_bundle, ActiveSessionState},
        setup::create_e2e_session,
    };

    #[derive(Debug)]
    struct FixedHooks {
        now: Mutex<DateTime<Utc>>,
        next_id: Mutex<u64>,
    }

    impl FixedHooks {
        fn new(now: DateTime<Utc>) -> Self {
            Self {
                now: Mutex::new(now),
                next_id: Mutex::new(1),
            }
        }

        fn set_now(&self, now: DateTime<Utc>) {
            *self.now.lock().unwrap() = now;
        }
    }

    impl LivePersistenceHooks for FixedHooks {
        fn now(&self) -> DateTime<Utc> {
            *self.now.lock().unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.next_id.lock().unwrap();
            let value = format!("fixed-{kind}-{next}");
            *next += 1;
            value
        }

        fn check(&self, _point: LivePersistencePoint) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn request(
        view: &super::super::ConductorView,
        action: ConductorAction,
    ) -> ConductorMutationRequest {
        ConductorMutationRequest {
            action_token: view.action_token.clone(),
            expected_revision: view.revision,
            action,
        }
    }

    #[test]
    fn repeated_tx_state_persists_continued_readiness_without_an_operator_prompt() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let mut bundle = BundleStore::new(&created.path)
            .read_v3_checkpointed()
            .unwrap();
        bundle.schedule.mode = ExperimentMode::TxFocused;
        bundle.schedule.wspr_cycle_intents.truncate(4);
        let labels = [
            created.antenna_labels[0].clone(),
            created.antenna_labels[1].clone(),
            created.antenna_labels[1].clone(),
            created.antenna_labels[0].clone(),
        ];
        for (index, (intent, label)) in bundle
            .schedule
            .wspr_cycle_intents
            .iter_mut()
            .zip(labels)
            .enumerate()
        {
            intent.sequence_number = u32::try_from(index + 1).unwrap();
            intent.antenna_label = label;
            intent.direction = Some(WsprCycleDirection::Transmit);
            intent.signal = None;
        }
        BundleStore::refresh_v3_checkpoint(&mut bundle).unwrap();
        let source = temp.path().join(format!("continued{V2_BUNDLE_SUFFIX}"));
        BundleStore::new(&source).write_v3(&bundle).unwrap();
        activate_created_bundle(&active, source.clone()).unwrap();

        let hooks = Arc::new(FixedHooks::new(
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap() - Duration::seconds(10),
        ));
        let conductor = ConductorSessionState::default();
        let ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let started = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();
        let first_id = started.next_intent.as_ref().unwrap().intent_id.clone();
        let first = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &started,
                ConductorAction::ArmWsprCycle {
                    intent_id: first_id,
                    antenna_label: created.antenna_labels[0].clone(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert!(first.next_intent.is_some());

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 1, 52).unwrap());
        let second_ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let second_id = second_ready.next_intent.as_ref().unwrap().intent_id.clone();
        let second = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &second_ready,
                ConductorAction::ArmWsprCycle {
                    intent_id: second_id,
                    antenna_label: created.antenna_labels[1].clone(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(second.phase, super::super::ConductorPhase::AwaitingSlot);

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 3, 52).unwrap());
        let repeated = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let next = repeated.next_intent.as_ref().unwrap();
        assert!(!next.operator_action_required);
        assert!(repeated.guidance.contains("continue automatically"));
        assert_eq!(
            next.transition.antenna,
            TransitionDisposition::NoChangeNeeded
        );
        assert_eq!(
            next.transition.direction,
            TransitionDisposition::NoChangeNeeded
        );
        assert_eq!(next.transition.band, TransitionDisposition::NoChangeNeeded);
        assert_eq!(
            next.transition.signal,
            TransitionDisposition::NoChangeNeeded
        );

        assert_eq!(
            persist_continued_readiness(&active, &source, hooks.clone()).unwrap(),
            ContinuationOutcome::Continued
        );
        let current = BundleStore::new(&source).read_v3_checkpointed().unwrap();
        validate_antenna_control_v5(&current).unwrap();
        let continued = current.events.last().unwrap();
        assert!(matches!(
            continued.payload,
            OperatorEventPayloadV3::WsprCycleArmed {
                readiness: Some(WsprReadinessBasisV5::Continued { .. }),
                ..
            }
        ));
        let projection = project_wspr_run_v3(&current.schedule, &current.events);
        assert_eq!(projection.cycles.len(), 3);
        assert_eq!(
            projection.cycles[1].ready_event_id,
            projection.cycles[2].ready_event_id
        );
        assert!(projection.cycles[2].occupancy_fully_covers_transmission);

        let mut changed_direction = current.clone();
        changed_direction.schedule.wspr_cycle_intents[2].direction =
            Some(WsprCycleDirection::Receive);
        assert!(validate_antenna_control_v5(&changed_direction)
            .unwrap_err()
            .contains("cannot change antenna, direction, band, or signal"));
        let mut changed_band = current.clone();
        changed_band.schedule.wspr_cycle_intents[2].band = Band::M40;
        assert!(validate_antenna_control_v5(&changed_band).is_err());
        let mut changed_signal = current.clone();
        changed_signal.schedule.wspr_cycle_intents[2].signal = Some(SignalAllocationV3 {
            signal_plan_id: SignalPlanIdV3::new("plan-1").unwrap(),
            frequency_hz: 14_050_000,
            frequency_variant_id: SignalVariantIdV3::new("variant-1").unwrap(),
            counterbalance_block_id: CounterbalanceBlockIdV3::new("block-1").unwrap(),
            counterbalance_position: 1,
        });
        assert!(validate_antenna_control_v5(&changed_signal).is_err());

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 3, 55).unwrap());
        let awaiting = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &awaiting,
                ConductorAction::Interrupt {
                    reason: Some("test interruption".into()),
                },
            ),
            hooks,
        )
        .unwrap();
        let interrupted = BundleStore::new(&source).read_v3_checkpointed().unwrap();
        let projection = project_wspr_run_v3(&interrupted.schedule, &interrupted.events);
        assert!(!projection.cycles[2].occupancy_fully_covers_transmission);
    }
}
