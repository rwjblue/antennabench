use std::{collections::BTreeSet, path::PathBuf, sync::Mutex};

use antennabench_core::{
    reduce_operator_events_v2, AdapterInput, BundleV2Contents, CorrectableOperatorEventPayloadV2,
    SessionLifecycleV2,
};
use antennabench_storage::BundleStore;
use antennabench_wsjtx::{
    latest_due_wspr_live_acquisition, plan_wspr_live_acquisitions_for_confirmed_slots,
    AdapterCancellationToken, ReqwestWsprLiveTransport, WsprLiveAcquirer,
    WsprLiveAcquisitionChannel, WsprLiveAcquisitionPlan, WsprLiveHttpTransport,
    WsprLiveImportConfig, WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS, WSPR_LIVE_QUERY_ENDPOINT,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    open_session::{
        active_session_source, with_foreground_operation, ActiveSessionState, OpenedSession,
        SessionErrorKind, SessionErrorPayload,
    },
    wspr_live_import::commit_wspr_live_response,
};

#[derive(Default)]
pub(crate) struct WsprLiveAcquisitionState(Mutex<WsprLiveAcquisitionRuntime>);

#[derive(Default)]
struct WsprLiveAcquisitionRuntime {
    source: Option<PathBuf>,
    last_request_started_at: Option<DateTime<Utc>>,
    failure: Option<WsprLiveFailure>,
}

#[derive(Debug, Clone)]
struct WsprLiveFailure {
    window_end: DateTime<Utc>,
    completed_slot_id: String,
    message: String,
    detail: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsprLiveAcquisitionRequest {
    #[serde(default)]
    retry: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum WsprLiveAcquisitionOutcome {
    Dormant {
        #[serde(rename = "capturedThrough")]
        captured_through: Option<DateTime<Utc>>,
    },
    Waiting {
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "notBefore")]
        not_before: DateTime<Utc>,
        #[serde(rename = "capturedThrough")]
        captured_through: Option<DateTime<Utc>>,
    },
    UpToDate {
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
    },
    Failed {
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "windowEnd")]
        window_end: DateTime<Utc>,
        message: String,
        detail: String,
    },
    Captured {
        session: Box<OpenedSession>,
        revision: u64,
        #[serde(rename = "completedSlotId")]
        completed_slot_id: String,
        #[serde(rename = "capturedThrough")]
        captured_through: DateTime<Utc>,
        total: usize,
        accepted: usize,
        duplicate: usize,
        conflict: usize,
        #[serde(rename = "observationsCreated")]
        observations_created: usize,
    },
}

impl WsprLiveAcquisitionRuntime {
    fn reset_for_source(&mut self, source: &PathBuf) {
        if self.source.as_ref() != Some(source) {
            *self = Self {
                source: Some(source.clone()),
                ..Self::default()
            };
        }
    }

    fn remember_failure(&mut self, plan: &WsprLiveAcquisitionPlan, error: &SessionErrorPayload) {
        self.failure = Some(WsprLiveFailure {
            window_end: plan.query.window_end,
            completed_slot_id: plan.completed_slot_id.clone(),
            message: error.message.clone(),
            detail: error.detail.clone(),
        });
    }
}

#[tauri::command]
pub(crate) fn advance_active_session_wspr_live(
    request: WsprLiveAcquisitionRequest,
    active_state: State<'_, ActiveSessionState>,
    acquisition_state: State<'_, WsprLiveAcquisitionState>,
) -> Result<WsprLiveAcquisitionOutcome, SessionErrorPayload> {
    let transport = ReqwestWsprLiveTransport::new().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Resource,
            "The bounded WSPR.live client could not be initialized.",
            error.to_string(),
        )
    })?;
    advance_with_transport(
        active_state.inner(),
        acquisition_state.inner(),
        request,
        Utc::now(),
        transport,
    )
}

fn advance_with_transport<T: WsprLiveHttpTransport>(
    active_state: &ActiveSessionState,
    acquisition_state: &WsprLiveAcquisitionState,
    request: WsprLiveAcquisitionRequest,
    now: DateTime<Utc>,
    transport: T,
) -> Result<WsprLiveAcquisitionOutcome, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let (source, _) = active_session_source(active_state)?;
        let store = BundleStore::new(&source);
        let snapshot = store
            .read_v2_checkpointed()
            .map_err(crate::conductor::live_error_payload)?;
        if snapshot.session_state.lifecycle != SessionLifecycleV2::Running {
            return Ok(WsprLiveAcquisitionOutcome::Dormant {
                captured_through: captured_through(&snapshot),
            });
        }

        let plans = authorized_plans(&snapshot).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The committed session cannot produce a WSPR.live acquisition plan.",
                error.to_string(),
            )
        })?;
        let captured_through = captured_through(&snapshot);
        let pending = plans
            .into_iter()
            .filter(|plan| captured_through.is_none_or(|end| plan.query.window_end > end))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(captured_through.map_or(
                WsprLiveAcquisitionOutcome::Dormant {
                    captured_through: None,
                },
                |captured_through| WsprLiveAcquisitionOutcome::UpToDate { captured_through },
            ));
        }

        let (last_request_started_at, prior_failure) = {
            let mut runtime = acquisition_state.0.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?;
            runtime.reset_for_source(&source);
            (runtime.last_request_started_at, runtime.failure.clone())
        };
        let due = latest_due_wspr_live_acquisition(&pending, now, last_request_started_at).cloned();
        let Some(plan) = due else {
            let next = pending
                .iter()
                .min_by_key(|plan| (plan.not_before, plan.segment_ended_at))
                .expect("pending acquisition is non-empty");
            let interval_deadline = last_request_started_at.and_then(|started| {
                started
                    .checked_add_signed(Duration::seconds(WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS))
            });
            return Ok(WsprLiveAcquisitionOutcome::Waiting {
                completed_slot_id: next.completed_slot_id.clone(),
                not_before: interval_deadline
                    .map_or(next.not_before, |deadline| deadline.max(next.not_before)),
                captured_through,
            });
        };
        if let Some(failure) = prior_failure
            .filter(|failure| failure.window_end == plan.query.window_end && !request.retry)
        {
            return Ok(WsprLiveAcquisitionOutcome::Failed {
                completed_slot_id: failure.completed_slot_id,
                window_end: failure.window_end,
                message: failure.message,
                detail: failure.detail,
            });
        }

        {
            let mut runtime = acquisition_state.0.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?;
            runtime.last_request_started_at = Some(now);
            runtime.failure = None;
        }
        let response = match WsprLiveAcquirer::new(transport)
            .acquire(&plan, &AdapterCancellationToken::default())
        {
            Ok(response) => response,
            Err(error) => {
                let payload = SessionErrorPayload::new(
                    SessionErrorKind::Resource,
                    "WSPR.live spots could not be fetched.",
                    error.to_string(),
                );
                acquisition_state
                    .0
                    .lock()
                    .map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "WSPR.live acquisition state is unavailable",
                        )
                    })?
                    .remember_failure(&plan, &payload);
                return Ok(failed_outcome(&plan, payload));
            }
        };
        let config = WsprLiveImportConfig {
            session_callsign: plan.query.session_callsign.clone(),
            window_start: plan.query.window_start,
            window_end: plan.query.window_end,
            selected_bands: snapshot
                .schedule
                .slots
                .iter()
                .filter(|slot| {
                    slot.starts_at
                        .checked_add_signed(Duration::seconds(i64::from(slot.duration_seconds)))
                        .is_some_and(|ends_at| ends_at <= plan.query.window_end)
                })
                .fold(Vec::new(), |mut bands, slot| {
                    if !bands.contains(&slot.band) {
                        bands.push(slot.band);
                    }
                    bands
                }),
            captured_at: response.received_at,
            source_locator: Some(WSPR_LIVE_QUERY_ENDPOINT.into()),
        };
        let committed = match commit_wspr_live_response(
            active_state,
            &source,
            &response.body,
            config,
            WsprLiveAcquisitionChannel::HttpsQuery,
        ) {
            Ok(committed) => committed,
            Err(error) => {
                acquisition_state
                    .0
                    .lock()
                    .map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "WSPR.live acquisition state is unavailable",
                        )
                    })?
                    .remember_failure(&plan, &error);
                return Ok(failed_outcome(&plan, error));
            }
        };
        acquisition_state
            .0
            .lock()
            .map_err(|_| {
                SessionErrorPayload::report_pipeline("WSPR.live acquisition state is unavailable")
            })?
            .failure = None;
        Ok(WsprLiveAcquisitionOutcome::Captured {
            session: Box::new(committed.session),
            revision: committed.revision,
            completed_slot_id: plan.completed_slot_id,
            captured_through: plan.query.window_end,
            total: committed.summary.total,
            accepted: committed.summary.accepted,
            duplicate: committed.summary.duplicate,
            conflict: committed.summary.conflict,
            observations_created: committed.summary.observations_created,
        })
    })
}

fn failed_outcome(
    plan: &WsprLiveAcquisitionPlan,
    error: SessionErrorPayload,
) -> WsprLiveAcquisitionOutcome {
    WsprLiveAcquisitionOutcome::Failed {
        completed_slot_id: plan.completed_slot_id.clone(),
        window_end: plan.query.window_end,
        message: error.message,
        detail: error.detail,
    }
}

fn authorized_plans(
    bundle: &BundleV2Contents,
) -> Result<Vec<WsprLiveAcquisitionPlan>, antennabench_wsjtx::WsprLiveImportError> {
    let confirmed_slot_ids = reduce_operator_events_v2(SessionLifecycleV2::Ready, &bundle.events)
        .effective_events
        .into_iter()
        .filter_map(|event| {
            matches!(
                event.payload,
                CorrectableOperatorEventPayloadV2::AntennaStateConfirmed { .. }
            )
            .then_some(event.slot_id)
            .flatten()
        })
        .collect::<BTreeSet<_>>();
    plan_wspr_live_acquisitions_for_confirmed_slots(
        &bundle.station.callsign,
        &bundle.schedule.slots,
        &confirmed_slot_ids,
    )
}

fn captured_through(bundle: &BundleV2Contents) -> Option<DateTime<Utc>> {
    bundle
        .adapter_records
        .iter()
        .filter(|record| record.record_type == "wspr_live_import_summary")
        .filter_map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                return None;
            };
            let summary = serde_json::from_str::<serde_json::Value>(data).ok()?;
            let window_end = summary
                .get("window_end")?
                .as_str()?
                .parse::<DateTime<Utc>>()
                .ok()?;
            let is_automatic = summary
                .get("acquisition_channel")
                .and_then(serde_json::Value::as_str)
                == Some("https-query");
            let is_mature_manual_recovery = summary
                .get("captured_at")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.parse::<DateTime<Utc>>().ok())
                .and_then(|captured_at| {
                    window_end
                        .checked_add_signed(Duration::minutes(5))
                        .map(|mature_at| captured_at >= mature_at)
                })
                .unwrap_or(false);
            (is_automatic || is_mature_manual_recovery).then_some(window_end)
        })
        .max()
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, path::Path};

    use antennabench_core::{
        EventTimeBasisV2, MutationMember, OperatorEventPayloadV2, OperatorEventV2, Provenance,
        RecordMetaV2, RecordSource, SCHEMA_VERSION_V2,
    };
    use antennabench_storage::{BundleStore, LiveMutationMemberV2, LiveMutationV2};
    use antennabench_wsjtx::{
        AdapterCancellationToken, WsprLiveAcquisitionError, WsprLiveHttpResponse,
        WsprLiveHttpTransport, WSPR_LIVE_COLUMNS,
    };
    use chrono::{DateTime, Utc};
    use serde_json::json;
    use tempfile::TempDir;

    use super::{
        advance_with_transport, WsprLiveAcquisitionOutcome, WsprLiveAcquisitionRequest,
        WsprLiveAcquisitionState,
    };
    use crate::{open_session::ActiveSessionState, setup::create_e2e_session};

    struct FakeTransport {
        calls: Cell<usize>,
        response: Result<WsprLiveHttpResponse, WsprLiveAcquisitionError>,
    }

    impl WsprLiveHttpTransport for &FakeTransport {
        fn get(
            &self,
            _url: &str,
            _body_limit: u64,
            _cancellation: &AdapterCancellationToken,
        ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
            self.calls.set(self.calls.get() + 1);
            self.response.clone()
        }
    }

    fn empty_response(received_at: DateTime<Utc>) -> WsprLiveHttpResponse {
        WsprLiveHttpResponse {
            received_at,
            status: 200,
            body: serde_json::to_vec(&json!({
                "meta": WSPR_LIVE_COLUMNS.map(|name| json!({
                    "name": name,
                    "type": "Synthetic",
                })),
                "data": [],
                "rows": 0,
            }))
            .unwrap(),
        }
    }

    fn running_confirmed_session(
        root: &Path,
    ) -> (ActiveSessionState, std::path::PathBuf, DateTime<Utc>) {
        let active = ActiveSessionState::default();
        let created = create_e2e_session(root, &active);
        let store = BundleStore::new(&created.path);
        let mut writer = store.open_v2_writer().unwrap();
        let snapshot = writer.snapshot().clone();
        let final_end = snapshot
            .schedule
            .slots
            .iter()
            .map(|slot| {
                slot.starts_at + chrono::Duration::seconds(i64::from(slot.duration_seconds))
            })
            .max()
            .unwrap();
        let mut actions = vec![(
            None,
            snapshot.schedule.slots[0].starts_at,
            OperatorEventPayloadV2::SessionStarted { note: None },
        )];
        actions.extend(snapshot.schedule.slots.iter().map(|slot| {
            (
                Some(slot.slot_id.clone()),
                slot.starts_at,
                OperatorEventPayloadV2::AntennaStateConfirmed {
                    antenna_label: slot.antenna_label.clone(),
                    note: None,
                },
            )
        }));
        for (index, (slot_id, occurred_at, payload)) in actions.into_iter().enumerate() {
            let mutation_id = format!("automatic-test-mutation-{index}");
            let event = OperatorEventV2 {
                meta: RecordMetaV2 {
                    schema_version: SCHEMA_VERSION_V2,
                    session_id: snapshot.manifest.session_id.clone(),
                    recorded_at: occurred_at,
                    provenance: Provenance::from_legacy(
                        RecordSource::Operator,
                        env!("CARGO_PKG_VERSION"),
                    ),
                    mutation: MutationMember {
                        mutation_id: mutation_id.clone(),
                        member_index: 0,
                        member_count: 1,
                    },
                },
                event_id: format!("automatic-test-event-{index}"),
                occurred_at,
                time_basis: EventTimeBasisV2::ObservedNow,
                uncertainty_seconds: None,
                slot_id,
                payload,
            };
            writer
                .append(LiveMutationV2 {
                    expected_revision: writer.checkpoint().revision,
                    mutation_id,
                    members: vec![LiveMutationMemberV2::Event(event)],
                })
                .unwrap();
        }
        drop(writer);
        (
            active,
            created.path,
            final_end + chrono::Duration::minutes(5),
        )
    }

    #[test]
    fn due_confirmations_fetch_once_and_atomically_commit_through_the_importer() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path());
        let before = BundleStore::new(&path).read_v2_checkpointed().unwrap();
        let transport = FakeTransport {
            calls: Cell::new(0),
            response: Ok(empty_response(now)),
        };

        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now,
            &transport,
        )
        .unwrap();

        assert_eq!(transport.calls.get(), 1);
        let WsprLiveAcquisitionOutcome::Captured {
            revision,
            captured_through,
            total,
            ..
        } = outcome
        else {
            panic!("due acquisition must capture")
        };
        assert_eq!(captured_through, now - chrono::Duration::minutes(5));
        assert_eq!(total, 0);
        assert_eq!(revision, before.session_state.revision + 1);
        let after = BundleStore::new(&path).read_v2_checkpointed().unwrap();
        let summary = after
            .adapter_records
            .iter()
            .find(|record| record.record_type == "wspr_live_import_summary")
            .unwrap();
        assert_eq!(
            summary.meta.provenance.acquisition_channel.as_str(),
            "https-query"
        );
        assert_eq!(
            after.adapter_records.len(),
            before.adapter_records.len() + 2
        );
    }

    #[test]
    fn failures_do_not_mutate_or_automatically_retry_but_restart_may_resume() {
        let temp = TempDir::new().unwrap();
        let (active, path, now) = running_confirmed_session(temp.path());
        let before = BundleStore::new(&path).read_v2_checkpointed().unwrap();
        let runtime = WsprLiveAcquisitionState::default();
        let offline = FakeTransport {
            calls: Cell::new(0),
            response: Err(WsprLiveAcquisitionError::Transport("offline".into())),
        };

        let first = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest::default(),
            now,
            &offline,
        )
        .unwrap();
        assert!(matches!(first, WsprLiveAcquisitionOutcome::Failed { .. }));
        assert_eq!(offline.calls.get(), 1);
        let suppressed = advance_with_transport(
            &active,
            &runtime,
            WsprLiveAcquisitionRequest::default(),
            now + chrono::Duration::seconds(10),
            &offline,
        )
        .unwrap();
        assert!(matches!(
            suppressed,
            WsprLiveAcquisitionOutcome::Failed { .. }
        ));
        assert_eq!(offline.calls.get(), 1);
        assert_eq!(
            BundleStore::new(&path).read_v2_checkpointed().unwrap(),
            before
        );

        let resumed = FakeTransport {
            calls: Cell::new(0),
            response: Ok(empty_response(now + chrono::Duration::seconds(20))),
        };
        let outcome = advance_with_transport(
            &active,
            &WsprLiveAcquisitionState::default(),
            WsprLiveAcquisitionRequest::default(),
            now + chrono::Duration::seconds(20),
            &resumed,
        )
        .unwrap();
        assert!(matches!(
            outcome,
            WsprLiveAcquisitionOutcome::Captured { .. }
        ));
        assert_eq!(resumed.calls.get(), 1);
    }
}
