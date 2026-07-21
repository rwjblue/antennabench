use std::sync::Arc;

use antennabench_core::{
    v2::SessionLifecycleV2,
    v3::{SignalModeV3, SignalStateConfirmationV3, WsprCycleDirection},
    v6::{DiagnosticOperationV6, DiagnosticPhaseV6, EvidenceEffectV6},
    SCHEMA_VERSION_V4,
};
use antennabench_storage::{BundleStore, SystemLivePersistenceHooks};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::antenna_control::{schedule_automatic_coordinator, AntennaControllerState};
use crate::open_session::{
    active_session_source, storage_error_payload, ActiveSessionState, SessionErrorKind,
    SessionErrorPayload,
};
use crate::wsjtx_session::WsjtxSessionState;

mod actions;
mod live_session;
mod timing;
pub(crate) mod transition;
mod view;

#[cfg(test)]
use actions::{event_for_action, CorrectableAction};
use actions::{ConductorAction, ConductorMutationRequest};
#[cfg(test)]
use live_session::PendingAction;
use live_session::{mutate_conductor_with_hooks, read_conductor_with_hooks};
#[cfg(test)]
use timing::slot_evidence;
pub(crate) use transition::schedule_transition_coordinator;
#[cfg(test)]
use view::build_view_v3;
use view::requires_wsjtx_receiver;

const CONDUCTOR_VIEW_IPC_BYTES: u64 = 512 * 1024;
const MAX_PENDING_ACTION_TOKENS: usize = 32;

pub(crate) use live_session::{live_error_payload, ConductorSessionState, ControllerActionPort};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConductorView {
    bundle_name: String,
    session_id: String,
    revision: u64,
    lifecycle: SessionLifecycleV2,
    now: DateTime<Utc>,
    action_token: String,
    phase: ConductorPhase,
    guidance: String,
    wsjtx_required: bool,
    wsjtx_readiness: Option<WsjtxReadinessView>,
    seconds_to_transition: Option<i64>,
    antennas: Vec<String>,
    current_slot: Option<ConductorSlotView>,
    next_slot: Option<ConductorSlotView>,
    next_intent: Option<ConductorIntentView>,
    antenna_in_use: Option<String>,
    slots: Vec<ConductorSlotView>,
    effective_events: Vec<ConductorEventView>,
    diagnostics: Vec<ConductorDiagnostic>,
    recovery: Option<ConductorRecoveryView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WsjtxReadinessView {
    band: String,
    power_watts: Option<f32>,
    wspr_live_acquisition_enabled: bool,
    has_receive_periods: bool,
    next_direction: WsprCycleDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConductorPhase {
    Ready,
    AwaitingSlot,
    Guard,
    Active,
    BetweenSlots,
    Switching,
    Finalizing,
    Complete,
    Interrupted,
    Ended,
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorIntentView {
    intent_id: String,
    sequence_number: u32,
    band: String,
    antenna_label: String,
    direction: Option<WsprCycleDirection>,
    transition: ConductorTransitionView,
    operator_action_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConductorTransitionView {
    pub(crate) antenna: TransitionDisposition,
    pub(crate) direction: TransitionDisposition,
    pub(crate) band: TransitionDisposition,
    pub(crate) signal: TransitionDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransitionDisposition {
    NoChangeNeeded,
    AutomaticActionPending,
    AutomaticallyCompletedAndVerified,
    OperatorActionRequired,
    UnknownBlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorSlotView {
    slot_id: String,
    sequence_number: u32,
    starts_at: DateTime<Utc>,
    usable_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    band: String,
    planned_antenna: String,
    direction: Option<WsprCycleDirection>,
    actual_antenna: Option<String>,
    evidence_status: SlotEvidenceStatus,
    planned_signal: Option<ConductorPlannedSignalView>,
    actual_signal: Option<SignalStateConfirmationV3>,
    signal_status: SignalEvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorPlannedSignalView {
    mode: SignalModeV3,
    frequency_hz: u64,
    planned_power_watts: Option<f32>,
    transmitted_callsign: String,
    message: String,
    repetition_count: u16,
    key_speed_wpm: Option<u16>,
    transmit_seconds: u32,
    interval_seconds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SignalEvidenceStatus {
    NotPlanned,
    Missing,
    Confirmed,
    Deviated,
    Conflicting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SlotEvidenceStatus {
    Unknown,
    Confirmed,
    Missed,
    Bad,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorEventView {
    source_event_id: String,
    effective_through_event_id: String,
    occurred_at: DateTime<Utc>,
    slot_id: Option<String>,
    kind: &'static str,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorDiagnostic {
    code: String,
    message: String,
    slot_id: Option<String>,
    event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConductorRecoveryView {
    disposition: &'static str,
    starting_revision: u64,
    final_revision: u64,
    artifact_count: usize,
    interruption_recorded: bool,
}

#[tauri::command]
pub(crate) fn active_session_conductor(
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
) -> Result<ConductorView, SessionErrorPayload> {
    read_conductor_with_hooks(
        active_state.inner(),
        conductor_state.inner(),
        Arc::new(SystemLivePersistenceHooks),
    )
}

#[tauri::command]
pub(crate) fn mutate_active_session_conductor(
    app: AppHandle,
    request: ConductorMutationRequest,
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<ConductorView, SessionErrorPayload> {
    let diagnostic_source = active_session_source(active_state.inner())
        .ok()
        .map(|(source, _)| source);
    if matches!(&request.action, ConductorAction::Start { .. }) {
        let (source, _) = active_session_source(active_state.inner())?;
        let store = BundleStore::new(&source);
        let schema_version = store.schema_version().map_err(storage_error_payload)?;
        if schema_version >= SCHEMA_VERSION_V4 {
            let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
            if requires_wsjtx_receiver(&bundle)
                && !wsjtx_state.is_running_for_source(&source, Utc::now())
            {
                return Err(SessionErrorPayload::new(
                    SessionErrorKind::Conflict,
                    "Start the required WSJT-X UDP receiver before starting this session.",
                    "receive-capable schema-v4 WSPR sessions require active local WSJT-X intake when automatic WSPR.live acquisition is disabled",
                ));
            }
        }
    }
    let mutation = mutate_conductor_with_hooks(
        active_state.inner(),
        conductor_state.inner(),
        request,
        Arc::new(SystemLivePersistenceHooks),
    );
    let view = match mutation {
        Ok(view) => view,
        Err(payload) => {
            return Err(diagnostic_source
                .as_deref()
                .map_or(payload.clone(), |source| {
                    crate::operation_diagnostics::persist_failure(
                        source,
                        DiagnosticOperationV6::SessionMutation,
                        DiagnosticPhaseV6::Checkpoint,
                        "session.mutation_failed",
                        EvidenceEffectV6::NoneCommitted,
                        Vec::new(),
                        payload,
                    )
                }));
        }
    };
    if view.lifecycle != SessionLifecycleV2::Running {
        controller_state.revoke();
        let (source, _) = active_session_source(active_state.inner())?;
        wsjtx_state.stop_for_source(
            &source,
            "WSJT-X reception stopped because the durable session is not running.",
        );
    } else {
        schedule_automatic_coordinator(app.clone());
        schedule_transition_coordinator(app);
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    use antennabench_core::{
        v2::{AdapterInput, SessionLifecycleV2, V2_BUNDLE_SUFFIX},
        v3::{
            reduce_operator_events_v3, CorrectableOperatorEventPayloadV3, OperatorEventPayloadV3,
            SignalModeV3,
        },
        Band, ExperimentMode, PlannedSlot,
    };
    use antennabench_storage::{
        BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceHooks,
        LivePersistencePoint, LiveStreamV2, RecoveryDispositionV2,
    };
    use chrono::{DateTime, TimeZone, Utc};
    use tempfile::TempDir;

    use super::{
        build_view_v3, mutate_conductor_with_hooks, read_conductor_with_hooks,
        requires_wsjtx_receiver, ConductorAction, ConductorMutationRequest, ConductorPhase,
        ConductorSessionState, CorrectableAction, SignalEvidenceStatus, SlotEvidenceStatus,
    };
    use crate::{
        open_session::{
            activate_created_bundle, e2e_report_snapshot, export_e2e_snapshots,
            open_session_at_path, ActiveSessionState, SessionErrorKind,
        },
        setup::{create_e2e_session, create_e2e_signal_session},
        wsjtx_session::inject_e2e_wsjtx_sequence,
    };

    #[derive(Debug)]
    struct TestHooks {
        now: Mutex<DateTime<Utc>>,
        next_id: Mutex<u64>,
        fail_once: Mutex<Option<LivePersistencePoint>>,
    }

    impl TestHooks {
        fn new(now: DateTime<Utc>) -> Self {
            Self {
                now: Mutex::new(now),
                next_id: Mutex::new(1),
                fail_once: Mutex::new(None),
            }
        }

        fn set_now(&self, now: DateTime<Utc>) {
            *self.now.lock().unwrap() = now;
        }

        fn fail_once_at(&self, point: LivePersistencePoint) {
            *self.fail_once.lock().unwrap() = Some(point);
        }
    }

    #[test]
    fn receive_capable_wspr_sessions_require_udp_only_when_public_collection_is_off() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let wspr = create_e2e_session(temp.path(), &active);
        let mut wspr_bundle = BundleStore::new(wspr.path).read_v3_checkpointed().unwrap();
        for mode in [
            ExperimentMode::WholeStationAb,
            ExperimentMode::RxFocused,
            ExperimentMode::SingleAntennaProfiling,
        ] {
            wspr_bundle.schedule.mode = mode;
            wspr_bundle.session_state.wspr_live_acquisition_enabled = false;
            assert!(requires_wsjtx_receiver(&wspr_bundle));
            wspr_bundle.session_state.wspr_live_acquisition_enabled = true;
            assert!(!requires_wsjtx_receiver(&wspr_bundle));
        }
        wspr_bundle.schedule.mode = ExperimentMode::TxFocused;
        wspr_bundle.session_state.wspr_live_acquisition_enabled = false;
        assert!(!requires_wsjtx_receiver(&wspr_bundle));

        let readiness = read_conductor_with_hooks(
            &active,
            &ConductorSessionState::default(),
            Arc::new(TestHooks::new(
                Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
            )),
        )
        .unwrap()
        .wsjtx_readiness
        .unwrap();
        assert_eq!(readiness.band, "20m");
        assert_eq!(readiness.power_watts, Some(5.0));
        assert!(!readiness.wspr_live_acquisition_enabled);
        assert!(readiness.has_receive_periods);
        assert_eq!(
            readiness.next_direction,
            antennabench_core::v3::WsprCycleDirection::Receive
        );

        let signal = create_e2e_signal_session(temp.path(), &active);
        let signal_bundle = BundleStore::new(signal.path)
            .read_v3_checkpointed()
            .unwrap();
        assert!(!requires_wsjtx_receiver(&signal_bundle));
        assert!(read_conductor_with_hooks(
            &active,
            &ConductorSessionState::default(),
            Arc::new(TestHooks::new(
                Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
            )),
        )
        .unwrap()
        .wsjtx_readiness
        .is_none());
    }

    impl LivePersistenceHooks for TestHooks {
        fn now(&self) -> DateTime<Utc> {
            *self.now.lock().unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.next_id.lock().unwrap();
            let value = format!("{kind}-{next:04}");
            *next += 1;
            value
        }

        fn check(&self, point: LivePersistencePoint) -> io::Result<()> {
            let mut fail = self.fail_once.lock().unwrap();
            if fail.as_ref() == Some(&point) {
                *fail = None;
                Err(io::Error::other("injected lost acknowledgement"))
            } else {
                Ok(())
            }
        }
    }

    fn fixture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle")
    }

    fn ready_store(temp: &TempDir, start: DateTime<Utc>) -> BundleStore {
        let upgraded = BundleStore::new(fixture_root())
            .upgrade_v1_to_v2(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
            .unwrap();
        let mut bundle = upgraded.read_v2().unwrap();
        bundle.events.clear();
        bundle.adapter_records.clear();
        bundle.observations.clear();
        bundle.rig.clear();
        bundle.propagation.clear();
        bundle.session_state.lifecycle = SessionLifecycleV2::Ready;
        bundle.session_state.revision = 0;
        bundle.session_state.last_committed_mutation_id = None;
        let labels = bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect::<Vec<_>>();
        assert!(labels.len() >= 2);
        bundle.schedule.slots = vec![
            PlannedSlot {
                slot_id: "slot-1".into(),
                sequence_number: 1,
                starts_at: start,
                duration_seconds: 120,
                guard_seconds: 10,
                band: Band::M20,
                antenna_label: labels[0].clone(),
            },
            PlannedSlot {
                slot_id: "slot-2".into(),
                sequence_number: 2,
                starts_at: start + chrono::Duration::seconds(120),
                duration_seconds: 120,
                guard_seconds: 10,
                band: Band::M20,
                antenna_label: labels[1].clone(),
            },
        ];
        BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
        let store = BundleStore::new(temp.path().join(format!("live{V2_BUNDLE_SUFFIX}")));
        store.write_v2(&bundle).unwrap();
        store
    }

    fn activate(store: &BundleStore) -> ActiveSessionState {
        let state = ActiveSessionState::default();
        activate_created_bundle(&state, store.root().to_path_buf()).unwrap();
        state
    }

    #[cfg(unix)]
    #[test]
    fn routine_conductor_poll_does_not_reopen_growing_evidence_streams() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let store = BundleStore::new(&created.path);
        let bundle = store.read_v3_checkpointed().unwrap();
        let adapter_path = store.root().join(&bundle.manifest.files.adapter_records);
        let hooks = Arc::new(TestHooks::new(
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
        ));
        let conductor = ConductorSessionState::default();
        let initial = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();

        let original_mode = fs::metadata(&adapter_path).unwrap().permissions().mode();
        fs::set_permissions(&adapter_path, fs::Permissions::from_mode(0o000)).unwrap();
        let projected = read_conductor_with_hooks(&active, &conductor, hooks).unwrap();
        fs::set_permissions(&adapter_path, fs::Permissions::from_mode(original_mode)).unwrap();

        assert_eq!(projected.revision, initial.revision);
        assert_eq!(projected.session_id, initial.session_id);
    }

    fn snapshot_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn collect(root: &Path, current: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        collect(root, root, &mut files);
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }

    fn request(view: &super::ConductorView, action: ConductorAction) -> ConductorMutationRequest {
        ConductorMutationRequest {
            action_token: view.action_token.clone(),
            expected_revision: view.revision,
            action,
        }
    }

    #[test]
    fn schema_v3_conductor_records_and_projects_actual_signal_state() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_signal_session(temp.path(), &active);
        let store = BundleStore::new(&created.path);
        let hooks = Arc::new(TestHooks::new(
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
        ));
        let conductor = ConductorSessionState::default();

        let ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        assert!(ready.slots.is_empty());
        assert_eq!(ready.guidance, "Start when you are ready.");
        assert_eq!(
            ready.next_intent.as_ref().unwrap().intent_id,
            created.slot_ids[0]
        );
        let started = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(started.phase, ConductorPhase::BetweenSlots);
        assert_eq!(
            started.guidance,
            "In WSJT-X, set Tx Pct to 100% and turn Enable Tx on. Switch to Vertical, then click Transmit on Vertical ready."
        );
        let armed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &started,
                ConductorAction::ArmWsprCycle {
                    intent_id: created.slot_ids[0].clone(),
                    antenna_label: created.antenna_labels[0].clone(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(armed.phase, ConductorPhase::AwaitingSlot);
        assert!(armed.guidance.contains("next even-minute WSPR time"));
        assert!(!armed.guidance.contains("protocol boundary"));
        assert_eq!(armed.slots[0].signal_status, SignalEvidenceStatus::Missing);
        assert_eq!(
            armed.slots[0].planned_signal.as_ref().unwrap().mode,
            SignalModeV3::Cw
        );
        let confirmed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &armed,
                ConductorAction::ConfirmSignal {
                    slot_id: created.slot_ids[0].clone(),
                    frequency_hz: Some(14_050_000),
                    mode: Some(SignalModeV3::Cw),
                    power_watts: Some(5.0),
                    transmitted_callsign: Some("N1RWJ".into()),
                    cadence_followed: Some(true),
                    note: Some("operator confirmed actual transmission".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();

        assert_eq!(confirmed.revision, 3);
        assert_eq!(
            confirmed.slots[0].signal_status,
            SignalEvidenceStatus::Confirmed
        );
        assert_eq!(
            confirmed.slots[0]
                .actual_signal
                .as_ref()
                .unwrap()
                .frequency_hz,
            Some(14_050_000)
        );
        let persisted = store.read_v3_checkpointed().unwrap();
        assert_eq!(persisted.events.len(), 3);
        assert_eq!(persisted.session_state.revision, 3);

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 4, 0).unwrap());
        let next_transmit = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        assert!(!next_transmit.guidance.contains("Enable Tx"));
        assert!(next_transmit.guidance.contains("Transmit on"));
    }

    #[test]
    fn historical_switch_start_events_remain_readable_and_conservative() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let store = BundleStore::new(&created.path);
        let hooks = Arc::new(TestHooks::new(
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
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
        let armed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &started,
                ConductorAction::ArmWsprCycle {
                    intent_id: created.slot_ids[0].clone(),
                    antenna_label: created.antenna_labels[0].clone(),
                },
            ),
            hooks,
        )
        .unwrap();

        let mut bundle = store.read_v3_checkpointed().unwrap();
        let mut historical = bundle.events.last().unwrap().clone();
        historical.event_id = "historical-switch-start".into();
        historical.meta.mutation.mutation_id = "historical-switch-mutation".into();
        historical.occurred_at = armed.slots[0].ends_at;
        historical.slot_id = None;
        historical.payload = OperatorEventPayloadV3::AntennaSwitchStarted {
            note: Some("Imported from an earlier AntennaBench run.".into()),
        };
        bundle.events.push(historical);

        let view = build_view_v3(
            "historical.session.wsprabundle".into(),
            &bundle,
            armed.slots[0].ends_at + chrono::Duration::seconds(1),
            "read-only-action-token".into(),
            None,
        );
        assert_eq!(view.phase, ConductorPhase::Switching);
        assert_eq!(view.antenna_in_use, None);
        assert!(view.diagnostics.is_empty());
        assert!(view.effective_events.iter().any(|event| {
            event.kind == "antenna_switch_started"
                && event.summary == "Imported from an earlier AntennaBench run."
        }));
    }

    #[test]
    fn schema_v3_unarmed_cycle_can_be_skipped_and_corrected() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let hooks = Arc::new(TestHooks::new(
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap(),
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
        let skipped = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &started,
                ConductorAction::SkipWsprCycle {
                    intent_id: created.slot_ids[0].clone(),
                    reason: Some("operator chose to wait".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            skipped.next_intent.as_ref().unwrap().intent_id,
            created.slot_ids[1]
        );
        let skipped_event = skipped
            .effective_events
            .iter()
            .find(|event| event.kind == "slot_missed")
            .unwrap()
            .source_event_id
            .clone();

        let corrected = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &skipped,
                ConductorAction::RetractEvent {
                    target_event_id: skipped_event,
                    reason: "operator had not intended to skip".into(),
                },
            ),
            hooks,
        )
        .unwrap();
        assert_eq!(
            corrected.next_intent.as_ref().unwrap().intent_id,
            created.slot_ids[0]
        );
    }

    struct FailureArtifacts {
        source: PathBuf,
        seed: &'static str,
    }

    impl Drop for FailureArtifacts {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                return;
            }
            let destination = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/desktop-e2e-failures")
                .join(self.seed);
            let _ = fs::remove_dir_all(&destination);
            let _ = copy_tree(&self.source, &destination);
            eprintln!(
                "desktop-e2e failure-artifacts={} seed={}",
                destination.display(),
                self.seed
            );
        }
    }

    fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let target = destination.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }

    #[test]
    fn deterministic_clock_projects_schedule_boundaries_without_persisting_timer_state() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start - chrono::Duration::seconds(30)));

        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(ready.phase, ConductorPhase::Ready);
        let awaiting = mutate_conductor_with_hooks(
            &active,
            &state,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(awaiting.phase, ConductorPhase::AwaitingSlot);
        assert_eq!(awaiting.seconds_to_transition, Some(30));

        hooks.set_now(start);
        let guard = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(guard.phase, ConductorPhase::Guard);
        assert_eq!(guard.seconds_to_transition, Some(10));

        hooks.set_now(start + chrono::Duration::seconds(10));
        let active_slot = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(active_slot.phase, ConductorPhase::Active);
        assert_eq!(active_slot.current_slot.unwrap().slot_id, "slot-1");

        hooks.set_now(start + chrono::Duration::seconds(240));
        let complete = read_conductor_with_hooks(&active, &state, hooks).unwrap();
        assert_eq!(complete.phase, ConductorPhase::Complete);
        assert_eq!(complete.revision, awaiting.revision);
    }

    #[test]
    fn confirmed_final_slot_projects_the_ingestion_grace_as_finalizing() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let mut bundle = store.read_v2().unwrap();
        bundle.session_state.wspr_live_acquisition_enabled = true;
        fs::write(
            store.root().join("session-state.json"),
            serde_json::to_vec_pretty(&bundle.session_state).unwrap(),
        )
        .unwrap();
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start - chrono::Duration::seconds(30)));
        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        mutate_conductor_with_hooks(
            &active,
            &state,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();

        hooks.set_now(start + chrono::Duration::seconds(120));
        let second_slot = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let antenna_label = second_slot
            .current_slot
            .as_ref()
            .unwrap()
            .planned_antenna
            .clone();
        mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &second_slot,
                ConductorAction::ConfirmAntenna {
                    slot_id: "slot-2".into(),
                    antenna_label,
                    note: None,
                },
            ),
            hooks.clone(),
        )
        .unwrap();

        hooks.set_now(start + chrono::Duration::seconds(240));
        let waiting = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        assert_eq!(waiting.phase, ConductorPhase::Finalizing);
        assert_eq!(waiting.seconds_to_transition, Some(300));

        hooks.set_now(start + chrono::Duration::seconds(540));
        let due = read_conductor_with_hooks(&active, &state, hooks).unwrap();
        assert_eq!(due.phase, ConductorPhase::Finalizing);
        assert_eq!(due.seconds_to_transition, Some(0));
    }

    #[test]
    fn desktop_e2e_manual_conductor_records_actual_state_corrections_and_lifecycle() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let labels = store
            .read_v2()
            .unwrap()
            .antennas
            .antennas
            .iter()
            .map(|antenna| antenna.label.clone())
            .collect::<Vec<_>>();
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start + chrono::Duration::seconds(20)));

        let ready = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let running = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &ready,
                ConductorAction::Start {
                    note: Some("manual no-rig run".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let confirmed = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &running,
                ConductorAction::ConfirmAntenna {
                    slot_id: "slot-1".into(),
                    antenna_label: labels[1].clone(),
                    note: Some("operator checked the switch".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            confirmed.current_slot.as_ref().unwrap().actual_antenna,
            Some(labels[1].clone())
        );
        assert_ne!(
            confirmed
                .current_slot
                .as_ref()
                .unwrap()
                .planned_antenna
                .as_str(),
            confirmed
                .current_slot
                .as_ref()
                .unwrap()
                .actual_antenna
                .as_deref()
                .unwrap()
        );
        let original = confirmed.effective_events[0].source_event_id.clone();

        let replaced = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &confirmed,
                ConductorAction::ReplaceEvent {
                    target_event_id: original.clone(),
                    slot_id: Some("slot-1".into()),
                    replacement: CorrectableAction::ConfirmAntenna {
                        antenna_label: labels[0].clone(),
                        note: Some("corrected after inspection".into()),
                    },
                    reason: "wrong antenna selected in the first entry".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            replaced.current_slot.as_ref().unwrap().actual_antenna,
            Some(labels[0].clone())
        );
        assert_ne!(
            replaced.effective_events[0].source_event_id,
            replaced.effective_events[0].effective_through_event_id
        );

        let retracted = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &replaced,
                ConductorAction::RetractEvent {
                    target_event_id: original,
                    reason: "confirmation could not be verified".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            retracted.current_slot.as_ref().unwrap().evidence_status,
            SlotEvidenceStatus::Unknown
        );

        let interrupted = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &retracted,
                ConductorAction::Interrupt {
                    reason: Some("operator pause".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(interrupted.lifecycle, SessionLifecycleV2::Interrupted);
        let resumed = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &interrupted,
                ConductorAction::Resume {
                    note: Some("manual checks complete".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let ended = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &resumed,
                ConductorAction::End {
                    reason: Some("planned run complete".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(ended.lifecycle, SessionLifecycleV2::Ended);

        let terminal = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &ended,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "too late".into(),
                },
            ),
            hooks,
        )
        .unwrap_err();
        assert_eq!(terminal.kind, SessionErrorKind::Validation);
        println!(
            "desktop-e2e result=manual-conductor revision={} lifecycle={:?}",
            ended.revision, ended.lifecycle
        );
    }

    #[test]
    fn desktop_e2e_complete_local_workflow_is_coherent_recoverable_and_exportable() {
        const SEED: &str = "complete-workflow-v1";
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("scenario-seed.txt"), SEED).unwrap();
        let _failure_artifacts = FailureArtifacts {
            source: temp.path().to_path_buf(),
            seed: SEED,
        };
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        assert_eq!(created.slot_ids.len(), 8);
        assert_eq!(created.antenna_labels, ["Vertical", "Dipole"]);
        let store = BundleStore::new(&created.path);
        let initial = store.read_v3_checkpointed().unwrap();
        assert_eq!(initial.manifest.session_id, created.session_id);
        assert_eq!(initial.session_state.revision, 0);
        assert_eq!(initial.session_state.lifecycle, SessionLifecycleV2::Ready);

        let at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 20).unwrap();
        let hooks = Arc::new(TestHooks::new(at));
        let conductor = ConductorSessionState::default();
        let ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let start_request = request(
            &ready,
            ConductorAction::Start {
                note: Some("deterministic complete workflow".into()),
            },
        );
        hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
        let started =
            mutate_conductor_with_hooks(&active, &conductor, start_request.clone(), hooks.clone())
                .unwrap();
        let retried =
            mutate_conductor_with_hooks(&active, &conductor, start_request, hooks.clone()).unwrap();
        assert_eq!(started.revision, 1);
        assert_eq!(retried.revision, 1);
        assert_eq!(store.read_v3_checkpointed().unwrap().events.len(), 1);

        let armed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &retried,
                ConductorAction::ArmWsprCycle {
                    intent_id: created.slot_ids[0].clone(),
                    antenna_label: created.antenna_labels[0].clone(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(
            armed.slots[0].starts_at,
            Utc.with_ymd_and_hms(2026, 7, 15, 20, 2, 1).unwrap()
        );
        assert_eq!(armed.antenna_in_use.as_deref(), Some("Vertical"));
        assert!(armed.effective_events.iter().any(|event| {
            event.kind == "wspr_cycle_armed"
                && event
                    .summary
                    .starts_with("Operator confirmation made Vertical ready")
        }));
        let confirmed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &armed,
                ConductorAction::ConfirmAntenna {
                    slot_id: created.slot_ids[0].clone(),
                    antenna_label: created.antenna_labels[1].clone(),
                    note: Some("operator verified the actual switch".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let original_confirmation = confirmed
            .effective_events
            .iter()
            .find(|event| event.kind == "antenna_state_confirmed")
            .unwrap()
            .source_event_id
            .clone();
        let missed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &confirmed,
                ConductorAction::MarkMissed {
                    slot_id: created.slot_ids[1].clone(),
                    reason: Some("operator was unavailable".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let bad = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &missed,
                ConductorAction::MarkBad {
                    slot_id: created.slot_ids[2].clone(),
                    reason: "feedline connection was suspect".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let noted = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &bad,
                ConductorAction::AddNote {
                    slot_id: Some(created.slot_ids[0].clone()),
                    note: "manual evidence remains available without WSJT-X".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let corrected = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &noted,
                ConductorAction::ReplaceEvent {
                    target_event_id: original_confirmation,
                    slot_id: Some(created.slot_ids[0].clone()),
                    replacement: CorrectableAction::ConfirmAntenna {
                        antenna_label: created.antenna_labels[0].clone(),
                        note: Some("corrected after inspecting the switch".into()),
                    },
                    reason: "first actual-state entry selected the wrong label".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let interrupted = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &corrected,
                ConductorAction::Interrupt {
                    reason: Some("planned operator pause".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(interrupted.lifecycle, SessionLifecycleV2::Interrupted);
        let resumed = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &interrupted,
                ConductorAction::Resume {
                    note: Some("operator returned".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(resumed.lifecycle, SessionLifecycleV2::Running);

        let wsjtx_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 3, 55).unwrap();
        let intake = inject_e2e_wsjtx_sequence(&active, &created.path, wsjtx_at);
        assert_eq!(intake.adapter_records, 4);
        assert_eq!(intake.observations, 0);
        assert_eq!(intake.gaps, 1);
        assert!(intake.revision > resumed.revision);

        hooks.set_now(Utc.with_ymd_and_hms(2026, 7, 15, 20, 4, 30).unwrap());
        let current = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        hooks.fail_once_at(LivePersistencePoint::MidStreamWrite(LiveStreamV2::Events));
        let crash = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(
                &current,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "torn crash mutation must not become evidence".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap_err();
        assert_eq!(crash.kind, SessionErrorKind::Filesystem);
        assert_eq!(
            store.read_v3_checkpointed().unwrap().session_state.revision,
            intake.revision
        );

        let crash_recovery = store.recover_v3_with_hooks(hooks.clone()).unwrap();
        assert_eq!(crash_recovery.disposition, RecoveryDispositionV2::Clean);
        assert_eq!(crash_recovery.starting_revision, intake.revision);
        assert_eq!(crash_recovery.recovered_revision, intake.revision);
        assert_eq!(crash_recovery.final_revision, intake.revision + 2);
        assert!(crash_recovery.interruption.is_some());

        let reopened_active = ActiveSessionState::default();
        activate_created_bundle(&reopened_active, created.path.clone()).unwrap();
        let recovered_conductor = ConductorSessionState::default();
        let recovered =
            read_conductor_with_hooks(&reopened_active, &recovered_conductor, hooks.clone())
                .unwrap();
        assert_eq!(recovered.lifecycle, SessionLifecycleV2::Interrupted);
        let recovery = recovered
            .recovery
            .as_ref()
            .expect("process recovery details");
        assert_eq!(recovery.disposition, "clean");
        assert_eq!(recovery.starting_revision, intake.revision + 2);
        assert_eq!(recovery.final_revision, intake.revision + 2);
        assert!(!recovery.interruption_recorded);
        let resumed = mutate_conductor_with_hooks(
            &reopened_active,
            &recovered_conductor,
            request(
                &recovered,
                ConductorAction::Resume {
                    note: Some("resumed after deterministic crash recovery".into()),
                },
            ),
            hooks.clone(),
        )
        .unwrap();
        let ended = mutate_conductor_with_hooks(
            &reopened_active,
            &recovered_conductor,
            request(
                &resumed,
                ConductorAction::End {
                    reason: Some("complete workflow finished".into()),
                },
            ),
            hooks,
        )
        .unwrap();
        assert_eq!(ended.lifecycle, SessionLifecycleV2::Ended);

        let final_bundle = store.read_v3_checkpointed().unwrap();
        assert_eq!(
            final_bundle.session_state.lifecycle,
            SessionLifecycleV2::Ended
        );
        assert_eq!(final_bundle.session_state.revision, ended.revision);
        assert_eq!(final_bundle.adapter_records.len(), 4);
        assert!(final_bundle.observations.is_empty());
        assert!(final_bundle
            .adapter_records
            .iter()
            .any(|record| record.reason.as_str() == "wsjtx.direction-filtered"));
        assert!(final_bundle.adapter_records.iter().take(3).all(|record| {
            matches!(
                &record.input,
                AdapterInput::Inline {
                    data,
                    encoding: Some(encoding),
                    ..
                } if !data.is_empty() && encoding == "hex"
            )
        }));
        assert!(!final_bundle.events.iter().any(|event| {
            matches!(
                &event.payload,
                antennabench_core::v3::OperatorEventPayloadV3::NoteAdded { note }
                    if note.contains("torn crash mutation")
            )
        }));
        let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &final_bundle.events);
        assert_eq!(reduction.lifecycle, SessionLifecycleV2::Ended);
        assert!(reduction.diagnostics.is_empty());
        assert!(reduction.effective_events.iter().any(|event| {
            event.payload
                == CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
                    antenna_label: created.antenna_labels[0].clone(),
                    note: Some("corrected after inspecting the switch".into()),
                }
        }));

        let exported = export_e2e_snapshots(&reopened_active, temp.path());
        assert_eq!(exported.revision, ended.revision);
        assert!(exported.presentation_id > 0);
        assert!(exported.report_path.exists());
        assert!(exported.compact_summary_path.exists());
        let document = scraper::Html::parse_document(&exported.report_html);
        let fact_selector = scraper::Selector::parse(".fact").unwrap();
        assert!(document.select(&fact_selector).any(|fact| {
            let rendered_text = fact.text().collect::<String>();
            rendered_text.contains("Checkpoint revision")
                && rendered_text.contains(&ended.revision.to_string())
        }));
        assert!(exported.report_html.contains("Ended / final"));
        assert!(exported
            .report_html
            .contains("1 recorded acquisition gap; inspect the durable adapter evidence"));
        assert!(exported
            .report_html
            .contains("Lifecycle and interruption history"));
        assert!(exported
            .report_html
            .contains("Intended WSPR order and observed antenna use"));
        assert!(exported
            .report_html
            .contains("Unknown — antenna changed during transmission"));
        assert!(exported
            .compact_summary_html
            .contains("AntennaBench compact share summary"));
        assert!(exported.compact_summary_html.contains(&format!(
            "committed revision <strong>{}</strong>",
            ended.revision
        )));
        assert!(exported
            .compact_summary_html
            .contains("full evidence report and lossless session bundle"));
        assert!(!exported
            .compact_summary_html
            .contains("Complete operator note and correction history"));
        for expected in [
            "Run quality and answerability",
            "Planned versus actual",
            "Explicit acquisition gap",
            "Complete operator note and correction history",
            "Corrected",
            "corrected after inspecting the switch",
            "first actual-state entry selected the wrong label",
            "planned operator pause",
            "operator returned",
            "state-corrected",
            "state-interrupted",
        ] {
            assert!(
                exported.report_html.contains(expected),
                "missing run-quality audit evidence: {expected}"
            );
        }

        let exported_store = BundleStore::new(&exported.bundle_path);
        let reopened_bundle = exported_store.read_v3_checkpointed().unwrap();
        assert_eq!(reopened_bundle, final_bundle);
        let final_active = ActiveSessionState::default();
        activate_created_bundle(&final_active, exported.bundle_path.clone()).unwrap();
        let (report_revision, presentation_id, reopened_html) = e2e_report_snapshot(&final_active);
        assert_eq!(report_revision, ended.revision);
        assert!(presentation_id > 0);
        assert_eq!(reopened_html, exported.report_html);
        fs::write(
            temp.path().join("scenario-result.txt"),
            format!(
                "seed={SEED}\nrevision={}\nevents={}\nadapter_records={}\nobservations={}\n",
                ended.revision,
                final_bundle.events.len(),
                final_bundle.adapter_records.len(),
                final_bundle.observations.len()
            ),
        )
        .unwrap();
        eprintln!(
            "desktop-e2e result=complete-workflow seed={SEED} revision={} report={} bundle={}",
            ended.revision,
            exported.report_path.display(),
            exported.bundle_path.display()
        );
    }

    #[test]
    fn idempotent_retry_lost_ack_stale_revision_and_recovery_are_explicit() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let active = activate(&store);
        let state = ConductorSessionState::default();
        let hooks = Arc::new(TestHooks::new(start));
        let first = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let competing = read_conductor_with_hooks(&active, &state, hooks.clone()).unwrap();
        let start_request = request(&first, ConductorAction::Start { note: None });
        hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
        let started =
            mutate_conductor_with_hooks(&active, &state, start_request.clone(), hooks.clone())
                .unwrap();
        assert_eq!(started.revision, 1);

        let retried =
            mutate_conductor_with_hooks(&active, &state, start_request, hooks.clone()).unwrap();
        assert_eq!(retried.revision, 1);
        assert_eq!(store.read_v2().unwrap().events.len(), 1);

        let stale = mutate_conductor_with_hooks(
            &active,
            &state,
            request(
                &competing,
                ConductorAction::AddNote {
                    slot_id: None,
                    note: "stale action".into(),
                },
            ),
            hooks.clone(),
        )
        .unwrap_err();
        assert_eq!(stale.kind, SessionErrorKind::StaleRevision);

        let restarted_state = ConductorSessionState::default();
        let recovered = read_conductor_with_hooks(&active, &restarted_state, hooks).unwrap();
        assert_eq!(recovered.lifecycle, SessionLifecycleV2::Interrupted);
        assert!(recovered.recovery.unwrap().interruption_recorded);
        assert_eq!(store.read_v2().unwrap().events.len(), 2);
    }

    #[test]
    fn report_open_is_observational_and_work_recovery_interrupts_once() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let active = activate(&store);
        let hooks = Arc::new(TestHooks::new(start));
        let conductor = ConductorSessionState::default();
        let ready = read_conductor_with_hooks(&active, &conductor, hooks.clone()).unwrap();
        let running = mutate_conductor_with_hooks(
            &active,
            &conductor,
            request(&ready, ConductorAction::Start { note: None }),
            hooks.clone(),
        )
        .unwrap();
        assert_eq!(running.lifecycle, SessionLifecycleV2::Running);

        let before_open = snapshot_files(store.root());
        let reopened = ActiveSessionState::default();
        open_session_at_path(&reopened, store.root().to_path_buf()).unwrap();
        assert_eq!(snapshot_files(store.root()), before_open);

        let restarted_conductor = ConductorSessionState::default();
        let recovered =
            read_conductor_with_hooks(&reopened, &restarted_conductor, hooks.clone()).unwrap();
        assert_eq!(recovered.lifecycle, SessionLifecycleV2::Interrupted);
        assert_eq!(recovered.revision, running.revision + 1);
        assert!(recovered.recovery.unwrap().interruption_recorded);
        let after_recovery = snapshot_files(store.root());
        assert_ne!(after_recovery, before_open);

        let reread = read_conductor_with_hooks(&reopened, &restarted_conductor, hooks).unwrap();
        assert_eq!(reread.lifecycle, SessionLifecycleV2::Interrupted);
        assert_eq!(reread.revision, recovered.revision);
        assert_eq!(snapshot_files(store.root()), after_recovery);
    }

    #[test]
    fn conflicting_effective_slot_facts_are_conservative() {
        let confirmed =
            antennabench_core::v2::CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
                antenna_label: "A".into(),
                note: None,
            };
        let missed =
            antennabench_core::v2::CorrectableOperatorEventPayloadV2::SlotMissed { reason: None };

        assert_eq!(
            super::slot_evidence(&[&confirmed, &missed]),
            (SlotEvidenceStatus::Conflicting, None)
        );
    }

    #[test]
    fn writer_primitives_remain_the_only_durable_event_path() {
        let start = Utc.with_ymd_and_hms(2026, 7, 15, 2, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = ready_store(&temp, start);
        let mut writer = store.open_v2_writer().unwrap();
        let revision = writer.checkpoint().revision;
        let mutation_id = "mutation-direct-proof".to_string();
        let pending = super::PendingAction {
            token: mutation_id.clone(),
            session_id: store.read_v2().unwrap().manifest.session_id.clone(),
            expected_revision: revision,
            occurred_at: Some(start),
        };
        let event = super::event_for_action(
            &pending.session_id,
            &pending,
            ConductorAction::Start { note: None },
        )
        .unwrap();
        writer
            .append(LiveMutationV2 {
                expected_revision: revision,
                mutation_id,
                members: vec![LiveMutationMemberV2::Event(event)],
            })
            .unwrap();
        assert_eq!(writer.checkpoint().lifecycle, SessionLifecycleV2::Running);
    }
}
