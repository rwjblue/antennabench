use std::{
    io::ErrorKind,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration as StdDuration,
};

use antennabench_core::{
    annotate_bundle_observations,
    v2::{
        AdapterDisposition, AdapterInput, AdapterReasonId, AdapterRecordV2, BundleV2Contents,
        MutationMember, NormalizedRecordKind, NormalizedRecordLink, ObservationRecordV2,
        Provenance, RecordMetaV2, SessionLifecycleV2,
    },
    v3::{project_wspr_run_v3, BundleV3Contents, WsprCycleDirection},
    Band, BundleContents, ObservationRecord, RecordSource, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3,
    SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
};
use antennabench_storage::{
    BundleStore, LiveEvidenceMutationV3, LiveMutationMemberV2, LiveMutationV2,
    LivePersistenceError, LivePersistenceHooks, SystemLivePersistenceHooks,
};
use antennabench_wsjtx::{
    band_from_frequency_hz, parse_wsjtx_datagram, LiveIngestConfig, LiveIngestError,
    LiveIngestOutcome, LiveMessageDisposition, LiveRecordedMessage, LiveWsjtxIngest,
    ReceivedUdpDatagram, UdpReceiverError, WsjtxMessage, WsjtxUdpReceiver, WsprDecodeDisposition,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    conductor::live_error_payload,
    open_session::{
        active_session_source, with_foreground_operation, ActiveSessionState, SessionErrorKind,
        SessionErrorPayload,
    },
};

const RECEIVER_READ_TIMEOUT: StdDuration = StdDuration::from_millis(250);
const HEARTBEAT_STALE_AFTER_SECONDS: i64 = 45;
const WSJTX_STATUS_IPC_BYTES: u64 = 64 * 1024;

#[derive(Default)]
pub(crate) struct WsjtxSessionState(Mutex<WsjtxRuntime>);

#[derive(Default)]
struct WsjtxRuntime {
    active: Option<ReceiverHandle>,
    last_source: Option<PathBuf>,
    last_status: Option<WsjtxReceiverStatus>,
}

struct ReceiverHandle {
    source: PathBuf,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<WsjtxReceiverStatus>>,
    worker: JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WsjtxReceiverPhase {
    Idle,
    Running,
    Stale,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsjtxDiagnostic {
    code: String,
    message: String,
    evidence_complete: bool,
    stops_intake: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsjtxReceiverStatus {
    phase: WsjtxReceiverPhase,
    receiver_id: Option<String>,
    bind_address: Option<String>,
    expected_client_id: Option<String>,
    started_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    received_datagrams: u64,
    committed_mutations: u64,
    ignored_datagrams: u64,
    station_status: Option<WsjtxStationStatus>,
    setup_warnings: Vec<WsjtxSetupWarning>,
    diagnostic: Option<WsjtxDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsjtxStationStatus {
    observed_at: DateTime<Utc>,
    dial_frequency_hz: u64,
    mode: String,
    tx_enabled: bool,
    transmitting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsjtxSetupWarning {
    code: String,
    message: String,
}

impl Default for WsjtxReceiverStatus {
    fn default() -> Self {
        Self {
            phase: WsjtxReceiverPhase::Idle,
            receiver_id: None,
            bind_address: None,
            expected_client_id: None,
            started_at: None,
            last_seen_at: None,
            received_datagrams: 0,
            committed_mutations: 0,
            ignored_datagrams: 0,
            station_status: None,
            setup_warnings: Vec::new(),
            diagnostic: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartWsjtxRequest {
    bind_address: String,
    port: u16,
    expected_client_id: String,
}

struct WsjtxOrchestrator {
    store: BundleStore,
    schema_version: u16,
    ingest: LiveWsjtxIngest,
    session_id: String,
    receiver_id: String,
    expected_client_id: String,
    hooks: Arc<dyn LivePersistenceHooks>,
}

enum WsjtxSnapshot {
    V2(BundleV2Contents),
    V3(BundleV3Contents),
}

impl WsjtxSnapshot {
    fn lifecycle(&self) -> SessionLifecycleV2 {
        match self {
            Self::V2(bundle) => bundle.session_state.lifecycle,
            Self::V3(bundle) => bundle.session_state.lifecycle,
        }
    }

    fn session_id(&self) -> &str {
        match self {
            Self::V2(bundle) => &bundle.manifest.session_id,
            Self::V3(bundle) => &bundle.manifest.session_id,
        }
    }

    fn revision(&self) -> u64 {
        match self {
            Self::V2(bundle) => bundle.session_state.revision,
            Self::V3(bundle) => bundle.session_state.revision,
        }
    }

    fn last_committed_mutation_id(&self) -> Option<&str> {
        match self {
            Self::V2(bundle) => bundle.session_state.last_committed_mutation_id.as_deref(),
            Self::V3(bundle) => bundle.session_state.last_committed_mutation_id.as_deref(),
        }
    }

    fn station(&self) -> (&str, &str) {
        match self {
            Self::V2(bundle) => (&bundle.station.callsign, &bundle.station.grid),
            Self::V3(bundle) => (&bundle.station.callsign, &bundle.station.grid),
        }
    }

    fn earliest_slot_start(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::V2(bundle) => bundle
                .schedule
                .slots
                .iter()
                .map(|slot| slot.starts_at)
                .min(),
            Self::V3(bundle) => bundle
                .clone()
                .into_current()
                .bundle
                .schedule
                .slots
                .iter()
                .map(|slot| slot.starts_at)
                .min(),
        }
    }

    fn current_bundle(&self, observed_at: DateTime<Utc>) -> BundleContents {
        match self {
            Self::V2(bundle) => bundle.clone().into_current().bundle,
            Self::V3(bundle) => {
                let receive_intents = bundle
                    .schedule
                    .wspr_cycle_intents
                    .iter()
                    .filter(|intent| {
                        intent.direction.is_none()
                            || intent.direction == Some(WsprCycleDirection::Receive)
                    })
                    .map(|intent| intent.intent_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
                let attributable = projection
                    .cycles
                    .iter()
                    .filter(|cycle| {
                        cycle.occupancy_fully_covers_transmission
                            && cycle.window.transmission_ends_at <= observed_at
                            && receive_intents.contains(cycle.intent_id.as_str())
                    })
                    .map(|cycle| cycle.intent_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut current = bundle.clone().into_current().bundle;
                current
                    .schedule
                    .slots
                    .retain(|slot| attributable.contains(slot.slot_id.as_str()));
                current
            }
        }
    }
}

#[derive(Clone)]
struct PendingWsjtxMutation {
    expected_revision: u64,
    mutation_id: String,
    adapter_records: Vec<AdapterRecordV2>,
    observations: Vec<ObservationRecordV2>,
}

fn read_wsjtx_snapshot(store: &BundleStore) -> Result<WsjtxSnapshot, LivePersistenceError> {
    match store.schema_version()? {
        SCHEMA_VERSION_V2 => store.read_v2_checkpointed().map(WsjtxSnapshot::V2),
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => {
            store.read_v3_checkpointed().map(WsjtxSnapshot::V3)
        }
        actual => {
            Err(antennabench_storage::BundleStoreError::UnsupportedSchemaVersion { actual }.into())
        }
    }
}

fn setup_warning_target(
    snapshot: &WsjtxSnapshot,
    now: DateTime<Utc>,
) -> Option<(Band, WsprCycleDirection)> {
    let WsjtxSnapshot::V3(bundle) = snapshot else {
        return None;
    };
    if !matches!(
        bundle.session_state.lifecycle,
        SessionLifecycleV2::Ready | SessionLifecycleV2::Running
    ) {
        return None;
    }
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let upcoming_cycle = projection
        .cycles
        .iter()
        .find(|cycle| cycle.window.transmission_ends_at > now)
        .map(|cycle| cycle.intent_id.as_str());
    let intent = upcoming_cycle
        .and_then(|intent_id| {
            bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .find(|intent| intent.intent_id == intent_id)
        })
        .or_else(|| {
            bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
                !projection
                    .cycles
                    .iter()
                    .any(|cycle| cycle.intent_id == intent.intent_id)
                    && !projection
                        .skipped_intent_ids
                        .iter()
                        .any(|intent_id| intent_id == &intent.intent_id)
            })
        })?;
    Some((intent.band, intent.direction?))
}

fn setup_warnings_for(
    station: &WsjtxStationStatus,
    band: Band,
    direction: WsprCycleDirection,
) -> Vec<WsjtxSetupWarning> {
    let mut warnings = Vec::new();
    if !station.mode.trim().eq_ignore_ascii_case("WSPR") {
        warnings.push(WsjtxSetupWarning {
            code: "wsjtx.setup.mode_mismatch".into(),
            message: format!(
                "WSJT-X reports mode {:?}; the current instruction requires WSPR.",
                station.mode
            ),
        });
    }
    if band_from_frequency_hz(station.dial_frequency_hz) != Some(band) {
        let band = serde_json::to_value(band)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{band:?}"));
        warnings.push(WsjtxSetupWarning {
            code: "wsjtx.setup.band_mismatch".into(),
            message: format!(
                "WSJT-X reports dial frequency {} Hz, outside the instructed {band} band.",
                station.dial_frequency_hz
            ),
        });
    }
    match direction {
        WsprCycleDirection::Receive => {
            if station.transmitting {
                warnings.push(WsjtxSetupWarning {
                    code: "wsjtx.setup.transmitting_during_receive".into(),
                    message: "WSJT-X reports transmitting during a receive instruction.".into(),
                });
            }
            if station.tx_enabled {
                warnings.push(WsjtxSetupWarning {
                    code: "wsjtx.setup.tx_enabled_during_receive".into(),
                    message:
                        "WSJT-X reports Enable Tx on; turn it off for this receive instruction."
                            .into(),
                });
            }
        }
        WsprCycleDirection::Transmit if !station.tx_enabled => {
            warnings.push(WsjtxSetupWarning {
                code: "wsjtx.setup.tx_disabled_during_transmit".into(),
                message: "WSJT-X reports Enable Tx off; turn it on for this transmit instruction."
                    .into(),
            });
        }
        WsprCycleDirection::Transmit => {}
    }
    warnings
}

fn project_setup_warnings(
    snapshot: &WsjtxSnapshot,
    status: &mut WsjtxReceiverStatus,
    now: DateTime<Utc>,
) {
    status.setup_warnings.clear();
    let Some(station) = status.station_status.as_ref() else {
        return;
    };
    let stale = now.signed_duration_since(station.observed_at)
        > Duration::seconds(HEARTBEAT_STALE_AFTER_SECONDS);
    if status.phase != WsjtxReceiverPhase::Running || stale {
        status.station_status = None;
        return;
    }
    let Some((band, direction)) = setup_warning_target(snapshot, now) else {
        return;
    };
    status.setup_warnings = setup_warnings_for(station, band, direction);
}

enum IntakeDecision {
    Continue,
    Stop,
}

impl WsjtxSessionState {
    pub(crate) fn is_running_for_source(&self, source: &Path, now: DateTime<Utc>) -> bool {
        matches!(
            self.status_for_source(source, now).phase,
            WsjtxReceiverPhase::Running
        )
    }

    pub(crate) fn stop_all(&self, reason: &str) {
        let handle = self
            .0
            .lock()
            .ok()
            .and_then(|mut runtime| runtime.active.take());
        if let Some(handle) = handle {
            self.finish_stop(handle, reason);
        }
    }

    pub(crate) fn stop_for_source(&self, source: &Path, reason: &str) {
        let handle = self.0.lock().ok().and_then(|mut runtime| {
            runtime
                .active
                .as_ref()
                .is_some_and(|handle| handle.source == source)
                .then(|| runtime.active.take())
                .flatten()
        });
        if let Some(handle) = handle {
            self.finish_stop(handle, reason);
        }
    }

    fn finish_stop(&self, handle: ReceiverHandle, reason: &str) -> WsjtxReceiverStatus {
        let source = handle.source.clone();
        handle.stop.store(true, Ordering::Release);
        let status = handle.status.clone();
        let _ = handle.worker.join();
        let mut snapshot = status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default();
        if !matches!(snapshot.phase, WsjtxReceiverPhase::Failed) {
            snapshot.phase = WsjtxReceiverPhase::Stopped;
            snapshot.diagnostic = Some(WsjtxDiagnostic {
                code: "wsjtx.receiver.stopped".into(),
                message: reason.into(),
                evidence_complete: true,
                stops_intake: true,
            });
        }
        snapshot.station_status = None;
        snapshot.setup_warnings.clear();
        if let Ok(mut runtime) = self.0.lock() {
            runtime.last_source = Some(source);
            runtime.last_status = Some(snapshot.clone());
        }
        snapshot
    }

    fn status_for_source(&self, source: &Path, now: DateTime<Utc>) -> WsjtxReceiverStatus {
        let mut status = self
            .0
            .lock()
            .ok()
            .and_then(|runtime| {
                runtime
                    .active
                    .as_ref()
                    .filter(|handle| handle.source == source)
                    .and_then(|handle| handle.status.lock().ok().map(|status| status.clone()))
                    .or_else(|| {
                        (runtime.last_source.as_deref() == Some(source))
                            .then(|| runtime.last_status.clone())
                            .flatten()
                    })
            })
            .unwrap_or_default();
        if status.phase == WsjtxReceiverPhase::Running {
            let reference = status.last_seen_at.or(status.started_at);
            if reference.is_some_and(|seen| {
                now.signed_duration_since(seen) > Duration::seconds(HEARTBEAT_STALE_AFTER_SECONDS)
            }) {
                status.phase = WsjtxReceiverPhase::Stale;
                status.diagnostic = Some(WsjtxDiagnostic {
                    code: "wsjtx.client.stale_heartbeat".into(),
                    message:
                        "No datagram from the expected WSJT-X client has arrived within 45 seconds."
                            .into(),
                    evidence_complete: true,
                    stops_intake: false,
                });
            }
        }
        if status.phase != WsjtxReceiverPhase::Running {
            status.station_status = None;
            status.setup_warnings.clear();
        }
        status
    }
}

impl Drop for WsjtxSessionState {
    fn drop(&mut self) {
        if let Ok(runtime) = self.0.get_mut() {
            if let Some(handle) = runtime.active.take() {
                handle.stop.store(true, Ordering::Release);
                let _ = handle.worker.join();
            }
        }
    }
}

fn validate_start_request(
    request: StartWsjtxRequest,
) -> Result<(SocketAddr, String), SessionErrorPayload> {
    let ip = request.bind_address.trim().parse::<IpAddr>().map_err(|_| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Enter a numeric loopback address for WSJT-X reception.",
            "bindAddress must be 127.0.0.1 or ::1",
        )
    })?;
    if !ip.is_loopback() || request.port == 0 {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Use a loopback address and a non-zero UDP port.",
            "the desktop receiver never exposes arbitrary network binding",
        ));
    }
    let expected = request.expected_client_id.trim();
    if expected.is_empty() || expected.len() > 128 || !expected.is_ascii() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Enter the expected WSJT-X client identity.",
            "expectedClientId must contain 1 to 128 ASCII bytes",
        ));
    }
    Ok((SocketAddr::new(ip, request.port), expected.to_string()))
}

fn start_receiver(
    state: &WsjtxSessionState,
    source: PathBuf,
    request: StartWsjtxRequest,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    let (bind, expected_client_id) = validate_start_request(request)?;
    let store = BundleStore::new(&source);
    let schema_version = store
        .schema_version()
        .map_err(crate::open_session::storage_error_payload)?;
    let bundle = read_wsjtx_snapshot(&store).map_err(live_error_payload)?;
    if !matches!(
        bundle.lifecycle(),
        SessionLifecycleV2::Ready | SessionLifecycleV2::Running
    ) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "WSJT-X reception can start only while the session is ready or running.",
            format!("current lifecycle is {:?}", bundle.lifecycle()),
        ));
    }
    if state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("WSJT-X receiver state is unavailable"))?
        .active
        .is_some()
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "Stop the active WSJT-X receiver before starting another one.",
            "one desktop process owns at most one receiver",
        ));
    }

    let mut receiver = WsjtxUdpReceiver::bind(bind).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Resource,
            "The WSJT-X UDP receiver could not bind to that local address.",
            format!("code=wsjtx.receiver.bind_failed address={bind} error={error}"),
        )
    })?;
    receiver
        .set_read_timeout(Some(RECEIVER_READ_TIMEOUT))
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Resource,
                "The WSJT-X UDP receiver could not configure bounded polling.",
                format!("code=wsjtx.receiver.timeout_failed error={error}"),
            )
        })?;
    let local_address = receiver.local_addr().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Resource,
            "The WSJT-X UDP receiver address could not be inspected.",
            error.to_string(),
        )
    })?;
    let receiver_id = hooks.new_id("wsjtx-receiver");
    let session_started_at = bundle.earliest_slot_start().unwrap_or_else(|| hooks.now());
    let (station_callsign, station_grid) = bundle.station();
    let ingest = LiveWsjtxIngest::new(LiveIngestConfig {
        session_id: bundle.session_id().to_string(),
        receiver_id: receiver_id.clone(),
        station_callsign: station_callsign.to_string(),
        station_grid: station_grid.to_string(),
        session_started_at,
    })
    .map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The active station cannot configure WSJT-X ingestion.",
            error.to_string(),
        )
    })?;
    let started_at = hooks.now();
    let status = Arc::new(Mutex::new(WsjtxReceiverStatus {
        phase: WsjtxReceiverPhase::Running,
        receiver_id: Some(receiver_id.clone()),
        bind_address: Some(local_address.to_string()),
        expected_client_id: Some(expected_client_id.clone()),
        started_at: Some(started_at),
        ..WsjtxReceiverStatus::default()
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let worker_status = status.clone();
    let worker_stop = stop.clone();
    let worker = thread::Builder::new()
        .name("antennabench-wsjtx".into())
        .spawn(move || {
            let mut orchestrator = WsjtxOrchestrator {
                store,
                schema_version,
                ingest,
                session_id: bundle.session_id().to_string(),
                receiver_id,
                expected_client_id,
                hooks,
            };
            receiver_loop(
                &mut receiver,
                &mut orchestrator,
                &worker_stop,
                &worker_status,
            );
        })
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Resource,
                "The WSJT-X receiver task could not start.",
                error.to_string(),
            )
        })?;
    let snapshot = status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_default();
    state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("WSJT-X receiver state is unavailable"))?
        .active = Some(ReceiverHandle {
        source,
        stop,
        status,
        worker,
    });
    Ok(snapshot)
}

fn receiver_loop(
    receiver: &mut WsjtxUdpReceiver,
    orchestrator: &mut WsjtxOrchestrator,
    stop: &AtomicBool,
    status: &Mutex<WsjtxReceiverStatus>,
) {
    while !stop.load(Ordering::Acquire) {
        match receiver.receive() {
            Ok(datagram) => {
                let decision = orchestrator.process(datagram, status);
                if matches!(decision, IntakeDecision::Stop) {
                    break;
                }
            }
            Err(UdpReceiverError::Receive(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(UdpReceiverError::Receive(error)) => {
                orchestrator.record_receiver_gap(
                    "wsjtx.receiver.receive_failed",
                    error.to_string(),
                    status,
                );
                break;
            }
            Err(UdpReceiverError::Shutdown) => break,
        }
    }
}

impl WsjtxOrchestrator {
    fn process(
        &mut self,
        datagram: ReceivedUdpDatagram,
        status: &Mutex<WsjtxReceiverStatus>,
    ) -> IntakeDecision {
        update_status(status, |status| status.received_datagrams += 1);
        let snapshot = match read_wsjtx_snapshot(&self.store) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                set_failed(
                    status,
                    "wsjtx.persistence.read_failed",
                    error.to_string(),
                    false,
                );
                return IntakeDecision::Stop;
            }
        };
        if snapshot.session_id() != self.session_id {
            set_failed(
                status,
                "wsjtx.receiver.lifecycle_stopped",
                "intake stopped because the active session changed".into(),
                true,
            );
            return IntakeDecision::Stop;
        }

        if snapshot.lifecycle() == SessionLifecycleV2::Ready {
            let Ok(parsed) = parse_wsjtx_datagram(&datagram.bytes) else {
                update_status(status, |status| status.ignored_datagrams += 1);
                return IntakeDecision::Continue;
            };
            if parsed.message.client_id() != self.expected_client_id {
                update_status(status, |status| status.ignored_datagrams += 1);
                return IntakeDecision::Continue;
            }
            update_status(status, |status| {
                status.last_seen_at = Some(datagram.received_at);
                status.phase = WsjtxReceiverPhase::Running;
                status.diagnostic = None;
            });
            let status_message = matches!(parsed.message, WsjtxMessage::Status(_));
            let outcome = self
                .ingest
                .ingest_datagram(&datagram.bytes, datagram.received_at);
            if outcome.is_ok() {
                self.sync_station_status(status, datagram.received_at, status_message);
            }
            return IntakeDecision::Continue;
        }
        if snapshot.lifecycle() != SessionLifecycleV2::Running {
            set_failed(
                status,
                "wsjtx.receiver.lifecycle_stopped",
                format!("intake stopped at lifecycle {:?}", snapshot.lifecycle()),
                true,
            );
            return IntakeDecision::Stop;
        }

        let parsed = match parse_wsjtx_datagram(&datagram.bytes) {
            Ok(parsed) => parsed,
            Err(error) => {
                let mutation = self.adapter_only_mutation(
                    &snapshot,
                    &datagram,
                    "udp_malformed",
                    AdapterDisposition::Malformed,
                    "wsjtx.malformed",
                    None,
                );
                return self.commit_or_stop(
                    mutation,
                    status,
                    Some(("wsjtx.protocol.malformed", error.to_string(), true)),
                );
            }
        };
        if parsed.message.client_id() != self.expected_client_id {
            if let Err(LiveIngestError::Resource(error)) = self
                .ingest
                .ingest_datagram(&datagram.bytes, datagram.received_at)
            {
                return self.stop_for_resource_gap(&snapshot, &datagram, status, error.diagnostic);
            }
            let mutation = self.adapter_only_mutation(
                &snapshot,
                &datagram,
                "udp_client_mismatch",
                AdapterDisposition::Filtered,
                "wsjtx.client-mismatch",
                None,
            );
            return self.commit_or_stop(
                mutation,
                status,
                Some((
                    "wsjtx.client.mismatch",
                    format!(
                        "ignored client {:?}; expected {:?}",
                        parsed.message.client_id(),
                        self.expected_client_id
                    ),
                    true,
                )),
            );
        }

        update_status(status, |status| {
            status.last_seen_at = Some(datagram.received_at);
            status.phase = WsjtxReceiverPhase::Running;
            status.diagnostic = None;
        });
        let status_message = matches!(parsed.message, WsjtxMessage::Status(_));
        match self
            .ingest
            .ingest_datagram(&datagram.bytes, datagram.received_at)
        {
            Ok(LiveIngestOutcome::IgnoredUnsupported { .. }) => {
                let mutation = self.adapter_only_mutation(
                    &snapshot,
                    &datagram,
                    "udp_unsupported",
                    AdapterDisposition::Unsupported,
                    "wsjtx.unsupported",
                    None,
                );
                self.commit_or_stop(mutation, status, None)
            }
            Ok(LiveIngestOutcome::Recorded(message)) => {
                self.sync_station_status(status, datagram.received_at, status_message);
                let mutation = self.recorded_mutation(&snapshot, &datagram, *message);
                self.commit_or_stop(mutation, status, None)
            }
            Err(LiveIngestError::Parse(error)) => {
                let mutation = self.adapter_only_mutation(
                    &snapshot,
                    &datagram,
                    "udp_malformed",
                    AdapterDisposition::Malformed,
                    "wsjtx.malformed",
                    None,
                );
                self.commit_or_stop(
                    mutation,
                    status,
                    Some(("wsjtx.protocol.malformed", error.to_string(), true)),
                )
            }
            Err(LiveIngestError::Resource(error)) => {
                self.stop_for_resource_gap(&snapshot, &datagram, status, error.diagnostic)
            }
        }
    }

    fn sync_station_status(
        &self,
        receiver_status: &Mutex<WsjtxReceiverStatus>,
        received_at: DateTime<Utc>,
        received_status_message: bool,
    ) {
        let client_status = self
            .ingest
            .client_state(&self.expected_client_id)
            .and_then(|client| client.status.as_ref());
        update_status(receiver_status, |receiver| {
            receiver.setup_warnings.clear();
            receiver.station_status = client_status.and_then(|current| {
                let observed_at = received_status_message.then_some(received_at).or_else(|| {
                    receiver
                        .station_status
                        .as_ref()
                        .map(|status| status.observed_at)
                })?;
                Some(WsjtxStationStatus {
                    observed_at,
                    dial_frequency_hz: current.dial_frequency_hz,
                    mode: current.mode.clone(),
                    tx_enabled: current.tx_enabled,
                    transmitting: current.transmitting,
                })
            });
        });
    }

    fn stop_for_resource_gap(
        &self,
        snapshot: &WsjtxSnapshot,
        datagram: &ReceivedUdpDatagram,
        status: &Mutex<WsjtxReceiverStatus>,
        diagnostic: antennabench_wsjtx::AdapterResourceDiagnostic,
    ) -> IntakeDecision {
        let message = format!("{diagnostic:?}");
        let mutation = self.adapter_only_mutation(
            snapshot,
            datagram,
            "acquisition_gap",
            AdapterDisposition::PartiallyNormalized,
            "wsjtx.acquisition-gap",
            None,
        );
        let decision = self.commit_or_stop(
            mutation,
            status,
            Some((diagnostic.code, message.clone(), true)),
        );
        set_failed(status, diagnostic.code, message, true);
        if matches!(decision, IntakeDecision::Continue) {
            IntakeDecision::Stop
        } else {
            decision
        }
    }

    fn recorded_mutation(
        &self,
        snapshot: &WsjtxSnapshot,
        datagram: &ReceivedUdpDatagram,
        recorded: LiveRecordedMessage,
    ) -> PendingWsjtxMutation {
        let mutation_id = self.hooks.new_id("wsjtx-mutation");
        let adapter_id = recorded.wsjtx_record.record_id;
        let mut direction_filtered = false;
        let observation = recorded.observation.and_then(|observation| {
            let mut current = snapshot.current_bundle(datagram.received_at);
            current.observations.push(observation);
            annotate_bundle_observations(&mut current);
            let observation = current
                .observations
                .pop()
                .expect("the just-appended observation remains present");
            if matches!(snapshot, WsjtxSnapshot::V3(_)) && observation.slot_id.is_none() {
                direction_filtered = true;
                return None;
            }
            Some(observation_v2(observation, &adapter_id, &mutation_id, 1, 2))
        });
        let disposition = if direction_filtered {
            AdapterDisposition::Filtered
        } else {
            disposition(&recorded.disposition)
        };
        let reason = if direction_filtered {
            "wsjtx.direction-filtered".into()
        } else {
            reason(&recorded.disposition)
        };
        let normalized_records = observation
            .as_ref()
            .map(|observation| {
                vec![NormalizedRecordLink {
                    record_kind: NormalizedRecordKind::Observation,
                    record_id: observation.observation_id.clone(),
                }]
            })
            .unwrap_or_default();
        let member_count = if observation.is_some() { 2 } else { 1 };
        let adapter = adapter_record(
            &self.session_id,
            &self.receiver_id,
            &mutation_id,
            0,
            member_count,
            adapter_id,
            datagram,
            recorded.wsjtx_record.meta.timestamp,
            recorded.wsjtx_record.message_type,
            disposition,
            reason,
            normalized_records,
        );
        PendingWsjtxMutation {
            expected_revision: snapshot.revision(),
            mutation_id,
            adapter_records: vec![adapter],
            observations: observation.into_iter().collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn adapter_only_mutation(
        &self,
        snapshot: &WsjtxSnapshot,
        datagram: &ReceivedUdpDatagram,
        record_type: &str,
        disposition: AdapterDisposition,
        reason_value: &str,
        source_time: Option<DateTime<Utc>>,
    ) -> PendingWsjtxMutation {
        let mutation_id = self.hooks.new_id("wsjtx-mutation");
        let record_id = self.hooks.new_id("wsjtx-record");
        PendingWsjtxMutation {
            expected_revision: snapshot.revision(),
            mutation_id: mutation_id.clone(),
            adapter_records: vec![adapter_record(
                &self.session_id,
                &self.receiver_id,
                &mutation_id,
                0,
                1,
                record_id,
                datagram,
                source_time.unwrap_or(datagram.received_at),
                record_type.into(),
                disposition,
                reason_value.into(),
                vec![],
            )],
            observations: vec![],
        }
    }

    fn commit_or_stop(
        &self,
        mut mutation: PendingWsjtxMutation,
        status: &Mutex<WsjtxReceiverStatus>,
        diagnostic: Option<(&str, String, bool)>,
    ) -> IntakeDecision {
        for _ in 0..3 {
            let result = match self.schema_version {
                SCHEMA_VERSION_V2 => self
                    .store
                    .open_v2_writer_with_hooks(self.hooks.clone())
                    .and_then(|mut writer| {
                        let mut members = mutation
                            .adapter_records
                            .clone()
                            .into_iter()
                            .map(LiveMutationMemberV2::AdapterRecord)
                            .collect::<Vec<_>>();
                        members.extend(
                            mutation
                                .observations
                                .clone()
                                .into_iter()
                                .map(LiveMutationMemberV2::Observation),
                        );
                        writer.append(LiveMutationV2 {
                            expected_revision: mutation.expected_revision,
                            mutation_id: mutation.mutation_id.clone(),
                            members,
                        })
                    }),
                SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => self
                    .store
                    .open_v3_writer_with_hooks(self.hooks.clone())
                    .and_then(|mut writer| {
                        writer.append_evidence(LiveEvidenceMutationV3 {
                            expected_revision: mutation.expected_revision,
                            mutation_id: mutation.mutation_id.clone(),
                            adapter_records: mutation.adapter_records.clone(),
                            observations: mutation.observations.clone(),
                        })
                    }),
                actual => Err(
                    antennabench_storage::BundleStoreError::UnsupportedSchemaVersion { actual }
                        .into(),
                ),
            };
            match result {
                Ok(_) => {
                    update_status(status, |status| {
                        status.committed_mutations += 1;
                        if let Some((code, message, _)) = &diagnostic {
                            status.ignored_datagrams += 1;
                            status.diagnostic = Some(WsjtxDiagnostic {
                                code: (*code).into(),
                                message: message.clone(),
                                evidence_complete: true,
                                stops_intake: false,
                            });
                        }
                    });
                    return IntakeDecision::Continue;
                }
                Err(LivePersistenceError::StaleRevision { actual, .. }) => {
                    mutation.expected_revision = actual;
                }
                Err(error) => {
                    let committed = read_wsjtx_snapshot(&self.store).ok().is_some_and(|bundle| {
                        bundle.last_committed_mutation_id() == Some(mutation.mutation_id.as_str())
                    });
                    if committed {
                        update_status(status, |status| status.committed_mutations += 1);
                        return IntakeDecision::Continue;
                    }
                    set_failed(
                        status,
                        "wsjtx.persistence.commit_failed",
                        error.to_string(),
                        false,
                    );
                    return IntakeDecision::Stop;
                }
            }
        }
        set_failed(
            status,
            "wsjtx.persistence.stale_revision",
            "the checkpoint kept advancing during three bounded retries".into(),
            false,
        );
        IntakeDecision::Stop
    }

    fn record_receiver_gap(
        &self,
        code: &str,
        message: String,
        status: &Mutex<WsjtxReceiverStatus>,
    ) {
        let snapshot = read_wsjtx_snapshot(&self.store);
        if let Ok(snapshot) = snapshot {
            if snapshot.lifecycle() == SessionLifecycleV2::Running {
                let at = self.hooks.now();
                let datagram = ReceivedUdpDatagram {
                    bytes: Vec::new(),
                    source: "127.0.0.1:0".parse().expect("static socket address"),
                    received_at: at,
                };
                let mutation = self.adapter_only_mutation(
                    &snapshot,
                    &datagram,
                    "acquisition_gap",
                    AdapterDisposition::PartiallyNormalized,
                    "wsjtx.acquisition-gap",
                    Some(at),
                );
                if matches!(
                    self.commit_or_stop(mutation, status, None),
                    IntakeDecision::Continue
                ) {
                    set_failed(status, code, message, true);
                    return;
                }
            }
        }
        set_failed(status, code, message, false);
    }
}

#[allow(clippy::too_many_arguments)]
fn adapter_record(
    session_id: &str,
    receiver_id: &str,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
    record_id: String,
    datagram: &ReceivedUdpDatagram,
    source_time: DateTime<Utc>,
    record_type: String,
    disposition: AdapterDisposition,
    reason_value: String,
    normalized_records: Vec<NormalizedRecordLink>,
) -> AdapterRecordV2 {
    AdapterRecordV2 {
        meta: record_meta(
            session_id,
            mutation_id,
            member_index,
            member_count,
            source_time,
        ),
        record_id,
        source_time: Some(source_time),
        record_type,
        disposition,
        reason: AdapterReasonId::new(reason_value).expect("static WSJT-X reason identity"),
        normalized_records,
        input: AdapterInput::Inline {
            data: encode_hex(&datagram.bytes),
            media_type: "application/vnd.wsjt-x.udp".into(),
            encoding: Some("hex".into()),
            source_locator: Some(format!("udp://{}?receiver={receiver_id}", datagram.source)),
        },
    }
}

fn observation_v2(
    observation: ObservationRecord,
    adapter_id: &str,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
) -> ObservationRecordV2 {
    ObservationRecordV2 {
        meta: record_meta(
            &observation.meta.session_id,
            mutation_id,
            member_index,
            member_count,
            observation.meta.timestamp,
        ),
        observation_id: observation.observation_id,
        adapter_record_ids: vec![adapter_id.into()],
        observation_kind: observation.observation_kind,
        band: observation.band,
        frequency_hz: observation.frequency_hz,
        mode: observation.mode,
        reporter_call: observation.reporter_call,
        heard_call: observation.heard_call,
        reporter_grid: observation.reporter_grid,
        heard_grid: observation.heard_grid,
        distance_km: observation.distance_km,
        azimuth_degrees: observation.azimuth_degrees,
        snr_db: observation.snr_db,
        drift_hz_per_minute: observation.drift_hz_per_minute,
        power_watts: observation.power_watts,
        slot_id: observation.slot_id,
        slot_label: observation.slot_label,
        slot_confidence: observation.slot_confidence,
        raw: observation.raw,
    }
}

fn record_meta(
    session_id: &str,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
    recorded_at: DateTime<Utc>,
) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.into(),
        recorded_at,
        provenance: Provenance::from_legacy(RecordSource::WsjtxUdp, env!("CARGO_PKG_VERSION")),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count,
        },
    }
}

fn disposition(disposition: &LiveMessageDisposition) -> AdapterDisposition {
    match disposition {
        LiveMessageDisposition::Heartbeat
        | LiveMessageDisposition::Status
        | LiveMessageDisposition::Close
        | LiveMessageDisposition::WsprDecode(WsprDecodeDisposition::ObservationProduced) => {
            AdapterDisposition::Accepted
        }
        LiveMessageDisposition::WsprDecode(WsprDecodeDisposition::Duplicate) => {
            AdapterDisposition::Duplicate
        }
        LiveMessageDisposition::WsprDecode(
            WsprDecodeDisposition::Replay | WsprDecodeDisposition::OffAir,
        ) => AdapterDisposition::Filtered,
        LiveMessageDisposition::WsprDecode(_) => AdapterDisposition::PartiallyNormalized,
    }
}

fn reason(disposition: &LiveMessageDisposition) -> String {
    match disposition {
        LiveMessageDisposition::Heartbeat => "wsjtx.heartbeat".into(),
        LiveMessageDisposition::Status => "wsjtx.status".into(),
        LiveMessageDisposition::Close => "wsjtx.close".into(),
        LiveMessageDisposition::WsprDecode(disposition) => {
            format!("wsjtx.{}", disposition.as_str().replace('_', "-"))
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn update_status(
    status: &Mutex<WsjtxReceiverStatus>,
    update: impl FnOnce(&mut WsjtxReceiverStatus),
) {
    if let Ok(mut status) = status.lock() {
        update(&mut status);
    }
}

fn set_failed(
    status: &Mutex<WsjtxReceiverStatus>,
    code: &str,
    message: String,
    evidence_complete: bool,
) {
    update_status(status, |status| {
        status.phase = WsjtxReceiverPhase::Failed;
        status.diagnostic = Some(WsjtxDiagnostic {
            code: code.into(),
            message,
            evidence_complete,
            stops_intake: true,
        });
    });
}

fn check_status_ipc(status: &WsjtxReceiverStatus) -> Result<(), SessionErrorPayload> {
    let size = serde_json::to_vec(status)
        .map_err(|error| SessionErrorPayload::report_pipeline(error.to_string()))?
        .len() as u64;
    if size > WSJTX_STATUS_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            "wsjtx_status",
            WSJTX_STATUS_IPC_BYTES,
            Some(size),
            "bytes",
        ));
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn active_session_wsjtx_status(
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let now = Utc::now();
    let mut status = wsjtx_state.status_for_source(&source, now);
    if let Ok(snapshot) = read_wsjtx_snapshot(&BundleStore::new(source)) {
        project_setup_warnings(&snapshot, &mut status, now);
    }
    check_status_ipc(&status)?;
    Ok(status)
}

#[tauri::command]
pub(crate) fn start_active_session_wsjtx(
    request: StartWsjtxRequest,
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    with_foreground_operation(active_state.inner(), || {
        let (source, _) = active_session_source(active_state.inner())?;
        let status = start_receiver(
            wsjtx_state.inner(),
            source,
            request,
            Arc::new(SystemLivePersistenceHooks),
        )?;
        check_status_ipc(&status)?;
        Ok(status)
    })
}

#[tauri::command]
pub(crate) fn stop_active_session_wsjtx(
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
) -> Result<WsjtxReceiverStatus, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    wsjtx_state.stop_for_source(&source, "The operator stopped WSJT-X reception.");
    let status = wsjtx_state.status_for_source(&source, Utc::now());
    check_status_ipc(&status)?;
    Ok(status)
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct E2eWsjtxResult {
    pub(crate) revision: u64,
    pub(crate) adapter_records: usize,
    pub(crate) observations: usize,
    pub(crate) gaps: usize,
}

#[cfg(test)]
pub(crate) fn inject_e2e_wsjtx_sequence(
    source: &Path,
    received_at: DateTime<Utc>,
) -> E2eWsjtxResult {
    use antennabench_wsjtx::WsjtxAdapterLimits;

    #[derive(Debug)]
    struct DeterministicHooks(Mutex<u64>, DateTime<Utc>);

    impl LivePersistenceHooks for DeterministicHooks {
        fn now(&self) -> DateTime<Utc> {
            self.1
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.0.lock().unwrap();
            let id = format!("e2e-{kind}-{next:04}");
            *next += 1;
            id
        }
    }

    let store = BundleStore::new(source);
    let bundle = store.read_v3_checkpointed().expect("running checkpoint");
    let hooks = Arc::new(DeterministicHooks(Mutex::new(1), received_at));
    let config = LiveIngestConfig {
        session_id: bundle.manifest.session_id.clone(),
        receiver_id: "e2e-fixture-receiver".into(),
        station_callsign: bundle.station.callsign,
        station_grid: bundle.station.grid,
        session_started_at: received_at - Duration::minutes(2),
    };
    let mut limits = WsjtxAdapterLimits::testing(512);
    limits.udp_rate_burst = 3;
    limits.udp_rate_per_second = 3;
    let mut orchestrator = WsjtxOrchestrator {
        store: store.clone(),
        schema_version: bundle.manifest.schema_version,
        ingest: LiveWsjtxIngest::new_with_limits(config, limits).expect("bounded live ingest"),
        session_id: bundle.manifest.session_id,
        receiver_id: "e2e-fixture-receiver".into(),
        expected_client_id: "WSJT-X".into(),
        hooks,
    };
    let fixture = include_str!("../../../fixtures/wsjtx/udp/schema3-live-sequence.hex");
    let datagrams = fixture
        .lines()
        .map(|line| {
            line.split_once('=')
                .expect("named fixture datagram")
                .1
                .as_bytes()
                .chunks_exact(2)
                .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let status = Mutex::new(WsjtxReceiverStatus::default());
    for bytes in &datagrams {
        assert!(matches!(
            orchestrator.process(
                ReceivedUdpDatagram {
                    bytes: bytes.clone(),
                    source: "127.0.0.1:2237".parse().unwrap(),
                    received_at,
                },
                &status,
            ),
            IntakeDecision::Continue
        ));
    }
    assert!(matches!(
        orchestrator.process(
            ReceivedUdpDatagram {
                bytes: datagrams[0].clone(),
                source: "127.0.0.1:2237".parse().unwrap(),
                received_at,
            },
            &status,
        ),
        IntakeDecision::Stop
    ));
    let bundle = store.read_v3_checkpointed().expect("ingested checkpoint");
    E2eWsjtxResult {
        revision: bundle.session_state.revision,
        adapter_records: bundle.adapter_records.len(),
        observations: bundle.observations.len(),
        gaps: bundle
            .adapter_records
            .iter()
            .filter(|record| record.record_type == "acquisition_gap")
            .count(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
    };

    use antennabench_core::{
        v2::{
            EventTimeBasisV2, MutationMember, OperatorEventPayloadV2, OperatorEventV2, Provenance,
            SessionLifecycleV2, V2_BUNDLE_SUFFIX,
        },
        v3::{OperatorEventPayloadV3, OperatorEventV3, RecordMetaV3, WsprCycleDirection},
        Band, RecordSource, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, WSPR_TRANSMISSION_MILLISECONDS,
    };
    use antennabench_storage::{
        BundleStore, LiveMutationMemberV2, LiveMutationV2, LivePersistenceHooks,
        LivePersistencePoint,
    };
    use antennabench_wsjtx::{LiveIngestConfig, LiveWsjtxIngest, WsjtxAdapterLimits};
    use chrono::{DateTime, TimeZone, Utc};
    use tempfile::TempDir;

    use super::{
        project_setup_warnings, setup_warnings_for, IntakeDecision, ReceivedUdpDatagram,
        WsjtxOrchestrator, WsjtxReceiverPhase, WsjtxReceiverStatus, WsjtxSessionState,
        WsjtxSnapshot, WsjtxStationStatus,
    };
    use crate::{open_session::ActiveSessionState, setup::create_e2e_session};

    #[derive(Debug)]
    struct TestHooks {
        now: DateTime<Utc>,
        next_id: Mutex<u64>,
        fail_once: Mutex<Option<LivePersistencePoint>>,
    }

    impl TestHooks {
        fn new(now: DateTime<Utc>) -> Self {
            Self {
                now,
                next_id: Mutex::new(1),
                fail_once: Mutex::new(None),
            }
        }

        fn fail_once_at(&self, point: LivePersistencePoint) {
            *self.fail_once.lock().unwrap() = Some(point);
        }
    }

    impl LivePersistenceHooks for TestHooks {
        fn now(&self) -> DateTime<Utc> {
            self.now
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.next_id.lock().unwrap();
            let id = format!("{kind}-{next:04}");
            *next += 1;
            id
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

    fn v3_event(
        session_id: &str,
        event_id: &str,
        occurred_at: DateTime<Utc>,
        slot_id: Option<String>,
        payload: OperatorEventPayloadV3,
    ) -> OperatorEventV3 {
        OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: SCHEMA_VERSION_V3,
                session_id: session_id.into(),
                recorded_at: occurred_at,
                provenance: Provenance::from_legacy(
                    RecordSource::Operator,
                    env!("CARGO_PKG_VERSION"),
                ),
                mutation: MutationMember {
                    mutation_id: format!("mutation-{event_id}"),
                    member_index: 0,
                    member_count: 1,
                },
            },
            event_id: event_id.into(),
            occurred_at,
            time_basis: EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id,
            payload,
        }
    }

    #[test]
    fn directed_wsjt_x_attribution_requires_receive_and_full_antenna_occupancy() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let mut bundle = BundleStore::new(created.path)
            .read_v3_checkpointed()
            .unwrap();
        let intent = bundle.schedule.wspr_cycle_intents[0].clone();
        let cycle_starts_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 1).unwrap();
        bundle.events.push(v3_event(
            &bundle.manifest.session_id,
            "ready",
            cycle_starts_at - chrono::Duration::seconds(1),
            Some(intent.intent_id.clone()),
            OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: intent.antenna_label,
                cycle_starts_at,
                readiness: Some(antennabench_core::v5::WsprReadinessBasisV5::OperatorConfirmed),
            },
        ));
        let transmission_ends_at =
            cycle_starts_at + chrono::Duration::milliseconds(WSPR_TRANSMISSION_MILLISECONDS);

        let before_decode = WsjtxSnapshot::V3(bundle.clone())
            .current_bundle(transmission_ends_at - chrono::Duration::milliseconds(1));
        assert!(before_decode.schedule.slots.is_empty());
        let attributable = WsjtxSnapshot::V3(bundle.clone()).current_bundle(transmission_ends_at);
        assert_eq!(attributable.schedule.slots.len(), 1);

        let mut early_switched_bundle = bundle.clone();
        early_switched_bundle.events.push(v3_event(
            &early_switched_bundle.manifest.session_id,
            "switch-early",
            cycle_starts_at + chrono::Duration::seconds(30),
            None,
            OperatorEventPayloadV3::AntennaSwitchStarted { note: None },
        ));
        let unknown = WsjtxSnapshot::V3(early_switched_bundle)
            .current_bundle(transmission_ends_at + chrono::Duration::seconds(30));
        assert!(unknown.schedule.slots.is_empty());

        let second_receive_intent = bundle.schedule.wspr_cycle_intents[1].clone();
        let second_receive_starts_at = cycle_starts_at + chrono::Duration::minutes(2);
        bundle.events.push(v3_event(
            &bundle.manifest.session_id,
            "second-receive-ready",
            second_receive_starts_at - chrono::Duration::seconds(1),
            Some(second_receive_intent.intent_id),
            OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: second_receive_intent.antenna_label,
                cycle_starts_at: second_receive_starts_at,
                readiness: Some(antennabench_core::v5::WsprReadinessBasisV5::OperatorConfirmed),
            },
        ));
        let after_second_receive = WsjtxSnapshot::V3(bundle.clone()).current_bundle(
            second_receive_starts_at
                + chrono::Duration::milliseconds(WSPR_TRANSMISSION_MILLISECONDS),
        );
        assert_eq!(after_second_receive.schedule.slots.len(), 2);

        let transmit_intent = bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .find(|intent| intent.direction == Some(WsprCycleDirection::Transmit))
            .unwrap()
            .clone();
        let transmit_starts_at = cycle_starts_at + chrono::Duration::minutes(4);
        bundle.events.push(v3_event(
            &bundle.manifest.session_id,
            "transmit-ready",
            transmit_starts_at - chrono::Duration::seconds(1),
            Some(transmit_intent.intent_id),
            OperatorEventPayloadV3::WsprCycleArmed {
                antenna_label: transmit_intent.antenna_label,
                cycle_starts_at: transmit_starts_at,
                readiness: Some(antennabench_core::v5::WsprReadinessBasisV5::OperatorConfirmed),
            },
        ));
        let after_transmit = WsjtxSnapshot::V3(bundle.clone()).current_bundle(
            transmit_starts_at + chrono::Duration::milliseconds(WSPR_TRANSMISSION_MILLISECONDS),
        );
        assert_eq!(after_transmit.schedule.slots.len(), 2);
    }

    #[test]
    fn setup_warning_matrix_is_advisory_and_does_not_infer_transmission() {
        let observed_at = Utc.with_ymd_and_hms(2026, 7, 16, 20, 0, 0).unwrap();
        let mut station = WsjtxStationStatus {
            observed_at,
            dial_frequency_hz: 14_095_600,
            mode: "WSPR".into(),
            tx_enabled: true,
            transmitting: false,
        };
        assert_eq!(
            setup_warnings_for(&station, Band::M20, WsprCycleDirection::Receive)
                .into_iter()
                .map(|warning| warning.code)
                .collect::<Vec<_>>(),
            vec!["wsjtx.setup.tx_enabled_during_receive"]
        );

        station.mode = "FT8".into();
        station.dial_frequency_hz = 7_040_000;
        station.transmitting = true;
        assert_eq!(
            setup_warnings_for(&station, Band::M20, WsprCycleDirection::Receive)
                .into_iter()
                .map(|warning| warning.code)
                .collect::<Vec<_>>(),
            vec![
                "wsjtx.setup.mode_mismatch",
                "wsjtx.setup.band_mismatch",
                "wsjtx.setup.transmitting_during_receive",
                "wsjtx.setup.tx_enabled_during_receive",
            ]
        );

        station.mode = " Wspr ".into();
        station.dial_frequency_hz = 14_095_600;
        station.transmitting = false;
        station.tx_enabled = false;
        assert_eq!(
            setup_warnings_for(&station, Band::M20, WsprCycleDirection::Transmit)
                .into_iter()
                .map(|warning| warning.code)
                .collect::<Vec<_>>(),
            vec!["wsjtx.setup.tx_disabled_during_transmit"]
        );
        station.tx_enabled = true;
        assert!(setup_warnings_for(&station, Band::M20, WsprCycleDirection::Transmit).is_empty());
    }

    #[test]
    fn setup_warning_projection_fails_closed_for_stale_and_legacy_status() {
        let temp = TempDir::new().unwrap();
        let active = ActiveSessionState::default();
        let created = create_e2e_session(temp.path(), &active);
        let bundle = BundleStore::new(created.path)
            .read_v3_checkpointed()
            .unwrap();
        let at = Utc.with_ymd_and_hms(2026, 7, 16, 20, 0, 0).unwrap();
        let mut status = WsjtxReceiverStatus {
            phase: WsjtxReceiverPhase::Running,
            station_status: Some(WsjtxStationStatus {
                observed_at: at,
                dial_frequency_hz: 14_095_600,
                mode: "WSPR".into(),
                tx_enabled: true,
                transmitting: false,
            }),
            ..WsjtxReceiverStatus::default()
        };
        project_setup_warnings(&WsjtxSnapshot::V3(bundle), &mut status, at);
        assert_eq!(
            status
                .setup_warnings
                .iter()
                .map(|warning| warning.code.as_str())
                .collect::<Vec<_>>(),
            vec!["wsjtx.setup.tx_enabled_during_receive"]
        );

        project_setup_warnings(
            &WsjtxSnapshot::V2(running_store(&TempDir::new().unwrap()).read_v2().unwrap()),
            &mut status,
            at,
        );
        assert!(status.setup_warnings.is_empty());
        project_setup_warnings(
            &WsjtxSnapshot::V2(running_store(&TempDir::new().unwrap()).read_v2().unwrap()),
            &mut status,
            at + chrono::Duration::seconds(46),
        );
        assert!(status.station_status.is_none());
    }

    fn running_store(temp: &TempDir) -> BundleStore {
        let upgraded = BundleStore::new(fixture_root())
            .upgrade_v1_to_v2(temp.path().join(format!("upgraded{V2_BUNDLE_SUFFIX}")))
            .unwrap();
        let mut bundle = upgraded.read_v2().unwrap();
        bundle.events.clear();
        bundle.adapter_records.clear();
        bundle.observations.clear();
        bundle.rig.clear();
        bundle.propagation.clear();
        let started_at = bundle.schedule.slots[0].starts_at;
        bundle.events.push(OperatorEventV2 {
            meta: super::record_meta(
                &bundle.manifest.session_id,
                "mutation-start",
                0,
                1,
                started_at,
            ),
            event_id: "event-start".into(),
            occurred_at: started_at,
            time_basis: EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id: None,
            payload: OperatorEventPayloadV2::SessionStarted { note: None },
        });
        bundle.session_state.lifecycle = SessionLifecycleV2::Running;
        bundle.session_state.revision = 1;
        bundle.session_state.last_committed_mutation_id = Some("mutation-start".into());
        BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
        let store = BundleStore::new(temp.path().join(format!("live{V2_BUNDLE_SUFFIX}")));
        store.write_v2(&bundle).unwrap();
        store
    }

    fn fixture_datagrams() -> Vec<Vec<u8>> {
        let fixture = include_str!("../../../fixtures/wsjtx/udp/schema3-live-sequence.hex");
        fixture
            .lines()
            .map(|line| decode_hex(line.split_once('=').unwrap().1))
            .collect()
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
            .collect()
    }

    fn orchestrator(
        store: BundleStore,
        hooks: Arc<TestHooks>,
        limits: Option<WsjtxAdapterLimits>,
    ) -> WsjtxOrchestrator {
        let bundle = store.read_v2_checkpointed().unwrap();
        let config = LiveIngestConfig {
            session_id: bundle.manifest.session_id.clone(),
            receiver_id: "fixture-receiver".into(),
            station_callsign: bundle.station.callsign,
            station_grid: bundle.station.grid,
            session_started_at: hooks.now(),
        };
        let ingest = match limits {
            Some(limits) => LiveWsjtxIngest::new_with_limits(config, limits).unwrap(),
            None => LiveWsjtxIngest::new(config).unwrap(),
        };
        WsjtxOrchestrator {
            store,
            schema_version: SCHEMA_VERSION_V2,
            ingest,
            session_id: bundle.manifest.session_id,
            receiver_id: "fixture-receiver".into(),
            expected_client_id: "WSJT-X".into(),
            hooks,
        }
    }

    fn received(bytes: Vec<u8>, at: DateTime<Utc>) -> ReceivedUdpDatagram {
        ReceivedUdpDatagram {
            bytes,
            source: "127.0.0.1:2237".parse().unwrap(),
            received_at: at,
        }
    }

    #[test]
    fn desktop_e2e_captured_sequence_commits_raw_evidence_and_observation_atomically() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut orchestrator = orchestrator(store.clone(), hooks, None);
        let status = Mutex::new(WsjtxReceiverStatus::default());

        for (offset, bytes) in fixture_datagrams().into_iter().enumerate() {
            let decision = orchestrator.process(
                received(bytes, at + chrono::Duration::seconds(offset as i64)),
                &status,
            );
            assert!(
                matches!(decision, IntakeDecision::Continue),
                "offset {offset}: {:?}",
                status.lock().unwrap()
            );
        }

        let bundle = store.read_v2_checkpointed().unwrap();
        assert_eq!(bundle.adapter_records.len(), 3);
        assert_eq!(bundle.observations.len(), 1);
        assert_eq!(bundle.session_state.revision, 4);
        let observation = &bundle.observations[0];
        assert_eq!(observation.adapter_record_ids.len(), 1);
        let evidence = bundle
            .adapter_records
            .iter()
            .find(|record| record.record_id == observation.adapter_record_ids[0])
            .unwrap();
        assert_eq!(
            evidence.normalized_records[0].record_id,
            observation.observation_id
        );
        assert_eq!(
            evidence.meta.mutation.mutation_id,
            observation.meta.mutation.mutation_id
        );
        assert_eq!(evidence.meta.mutation.member_count, 2);
        let status = status.lock().unwrap();
        let station = status.station_status.as_ref().unwrap();
        assert_eq!(station.observed_at, at + chrono::Duration::seconds(1));
        assert_eq!(station.dial_frequency_hz, 14_095_600);
        assert_eq!(station.mode, "WSPR");
        eprintln!(
            "desktop-e2e result=wsjtx-ingest revision={} adapter_records={} observations={}",
            bundle.session_state.revision,
            bundle.adapter_records.len(),
            bundle.observations.len()
        );
    }

    #[test]
    fn close_and_client_generation_reset_clear_observed_station_status() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut orchestrator = orchestrator(store, hooks, None);
        let status = Mutex::new(WsjtxReceiverStatus::default());
        let fixtures = fixture_datagrams();

        orchestrator.process(received(fixtures[1].clone(), at), &status);
        assert!(status.lock().unwrap().station_status.is_some());
        let mut close = fixtures[0][..22].to_vec();
        close[8..12].copy_from_slice(&6_u32.to_be_bytes());
        orchestrator.process(received(close, at + chrono::Duration::seconds(1)), &status);
        assert!(status.lock().unwrap().station_status.is_none());

        orchestrator.process(
            received(fixtures[1].clone(), at + chrono::Duration::seconds(2)),
            &status,
        );
        assert!(status.lock().unwrap().station_status.is_some());
        orchestrator.process(
            received(fixtures[0].clone(), at + chrono::Duration::seconds(48)),
            &status,
        );
        assert!(status.lock().unwrap().station_status.is_none());
    }

    #[test]
    fn malformed_mismatch_duplicate_and_lost_ack_remain_explicit_and_idempotent() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut orchestrator = orchestrator(store.clone(), hooks.clone(), None);
        let status = Mutex::new(WsjtxReceiverStatus::default());
        let fixtures = fixture_datagrams();

        orchestrator.process(received(vec![1, 2, 3], at), &status);
        let mut mismatch = fixtures[0].clone();
        mismatch[16..22].copy_from_slice(b"OTHER!");
        orchestrator.process(received(mismatch, at), &status);
        orchestrator.process(received(fixtures[1].clone(), at), &status);
        hooks.fail_once_at(LivePersistencePoint::BeforeAcknowledge);
        orchestrator.process(received(fixtures[2].clone(), at), &status);
        orchestrator.process(received(fixtures[2].clone(), at), &status);

        let bundle = store.read_v2_checkpointed().unwrap();
        assert_eq!(bundle.adapter_records.len(), 5);
        assert_eq!(bundle.observations.len(), 1);
        assert_eq!(bundle.session_state.revision, 6);
        assert_eq!(
            bundle.adapter_records[0].disposition,
            antennabench_core::v2::AdapterDisposition::Malformed
        );
        assert_eq!(
            bundle.adapter_records[1].disposition,
            antennabench_core::v2::AdapterDisposition::Filtered
        );
        assert_eq!(
            bundle.adapter_records[4].disposition,
            antennabench_core::v2::AdapterDisposition::Duplicate
        );
    }

    #[test]
    fn resource_gap_is_durable_and_stops_intake_without_hiding_incompleteness() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut limits = WsjtxAdapterLimits::testing(512);
        limits.udp_rate_burst = 1;
        limits.udp_rate_per_second = 1;
        let mut orchestrator = orchestrator(store.clone(), hooks, Some(limits));
        let status = Mutex::new(WsjtxReceiverStatus::default());
        let fixtures = fixture_datagrams();

        assert!(matches!(
            orchestrator.process(received(fixtures[0].clone(), at), &status),
            IntakeDecision::Continue
        ));
        assert!(matches!(
            orchestrator.process(received(fixtures[1].clone(), at), &status),
            IntakeDecision::Stop
        ));

        let bundle = store.read_v2_checkpointed().unwrap();
        assert_eq!(bundle.adapter_records.len(), 2);
        assert_eq!(bundle.adapter_records[1].record_type, "acquisition_gap");
        let status = status.lock().unwrap();
        assert_eq!(status.phase, WsjtxReceiverPhase::Failed);
        assert!(status.diagnostic.as_ref().unwrap().stops_intake);
        assert!(status.diagnostic.as_ref().unwrap().evidence_complete);
    }

    #[test]
    fn unexpected_clients_consume_the_bounded_admission_budget() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut limits = WsjtxAdapterLimits::testing(512);
        limits.udp_rate_burst = 1;
        limits.udp_rate_per_second = 1;
        let mut orchestrator = orchestrator(store.clone(), hooks, Some(limits));
        let status = Mutex::new(WsjtxReceiverStatus::default());
        let mut mismatch = fixture_datagrams()[0].clone();
        mismatch[16..22].copy_from_slice(b"OTHER!");

        assert!(matches!(
            orchestrator.process(received(mismatch.clone(), at), &status),
            IntakeDecision::Continue
        ));
        assert!(matches!(
            orchestrator.process(received(mismatch, at), &status),
            IntakeDecision::Stop
        ));

        let bundle = store.read_v2_checkpointed().unwrap();
        assert_eq!(bundle.adapter_records.len(), 2);
        assert_eq!(
            bundle.adapter_records[0].disposition,
            antennabench_core::v2::AdapterDisposition::Filtered
        );
        assert_eq!(bundle.adapter_records[1].record_type, "acquisition_gap");
    }

    #[test]
    fn terminal_lifecycle_and_stale_heartbeat_are_typed_without_network_access() {
        let at = Utc.with_ymd_and_hms(2026, 7, 15, 3, 0, 0).unwrap();
        let temp = TempDir::new().unwrap();
        let store = running_store(&temp);
        let hooks = Arc::new(TestHooks::new(at));
        let mut orchestrator = orchestrator(store.clone(), hooks, None);
        let snapshot = store.read_v2_checkpointed().unwrap();
        let ended_at = at + chrono::Duration::seconds(1);
        store
            .open_v2_writer()
            .unwrap()
            .append(LiveMutationV2 {
                expected_revision: snapshot.session_state.revision,
                mutation_id: "mutation-end".into(),
                members: vec![LiveMutationMemberV2::Event(OperatorEventV2 {
                    meta: super::record_meta(
                        &snapshot.manifest.session_id,
                        "mutation-end",
                        0,
                        1,
                        ended_at,
                    ),
                    event_id: "event-end".into(),
                    occurred_at: ended_at,
                    time_basis: EventTimeBasisV2::ObservedNow,
                    uncertainty_seconds: None,
                    slot_id: None,
                    payload: OperatorEventPayloadV2::SessionEnded { reason: None },
                })],
            })
            .unwrap();
        let status = Mutex::new(WsjtxReceiverStatus::default());
        assert!(matches!(
            orchestrator.process(received(fixture_datagrams()[0].clone(), at), &status),
            IntakeDecision::Stop
        ));
        assert_eq!(status.lock().unwrap().phase, WsjtxReceiverPhase::Failed);

        let state = WsjtxSessionState::default();
        {
            let mut runtime = state.0.lock().unwrap();
            runtime.last_source = Some(store.root().to_path_buf());
            runtime.last_status = Some(WsjtxReceiverStatus {
                phase: WsjtxReceiverPhase::Running,
                started_at: Some(at),
                expected_client_id: Some("WSJT-X".into()),
                ..WsjtxReceiverStatus::default()
            });
        }
        let stale = state.status_for_source(store.root(), at + chrono::Duration::seconds(46));
        assert_eq!(stale.phase, WsjtxReceiverPhase::Stale);
        assert_eq!(
            stale.diagnostic.unwrap().code,
            "wsjtx.client.stale_heartbeat"
        );
        assert_eq!(
            state.status_for_source(&store.root().join("other"), at),
            WsjtxReceiverStatus::default()
        );
    }
}
