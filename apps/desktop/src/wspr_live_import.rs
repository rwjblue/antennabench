use std::{fs, path::Path};

use antennabench_core::{
    v2::SessionLifecycleV2, v3::project_wspr_run_v3, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3,
    SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
};
use antennabench_storage::{
    BundleStore, LiveEvidenceMutationV3, LiveMutationMemberV2, LiveMutationV2, LivePersistenceError,
};
use antennabench_wsjtx::{
    derive_wspr_live_query_scope, parse_wspr_live_json, prepare_wspr_live_acquisition,
    AdapterCancellationToken, ParsedWsprLiveJson, WsprLiveAcquisitionChannel,
    WsprLiveConfirmedCycle, WsprLiveImportConfig, WsprLiveImportSummary, WSPR_LIVE_IMPORT_LIMITS,
};
use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::open_session::{
    active_session_source, reload_active_session, with_foreground_operation, ActiveSessionState,
    OpenedSession, SessionErrorKind, SessionErrorPayload,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum WsprLiveImportOutcome {
    Cancelled,
    Imported {
        session: Box<OpenedSession>,
        revision: u64,
        total: usize,
        accepted: usize,
        malformed: usize,
        filtered: usize,
        unsupported: usize,
        duplicate: usize,
        conflict: usize,
        #[serde(rename = "observationsCreated")]
        observations_created: usize,
        #[serde(rename = "evidenceCompletenessKnown")]
        evidence_completeness_known: bool,
    },
}

pub(crate) struct CommittedWsprLiveResponse {
    pub(crate) session: OpenedSession,
    pub(crate) revision: u64,
    pub(crate) summary: WsprLiveImportSummary,
}

#[tauri::command]
pub(crate) async fn import_active_session_wspr_live(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
) -> Result<WsprLiveImportOutcome, SessionErrorPayload> {
    let Some(selection) = app
        .dialog()
        .file()
        .set_title("Import a WSPR.live ClickHouse JSON response")
        .set_can_create_directories(false)
        .add_filter("WSPR.live JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(WsprLiveImportOutcome::Cancelled);
    };
    let path = selection.into_path().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "The selected WSPR.live file is not available as a local path.",
            error.to_string(),
        )
    })?;
    with_foreground_operation(state.inner(), || import_file(state.inner(), &path))
}

fn import_file(
    state: &ActiveSessionState,
    path: &Path,
) -> Result<WsprLiveImportOutcome, SessionErrorPayload> {
    let metadata = fs::metadata(path).map_err(|error| file_error(path, error))?;
    if !metadata.is_file() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "Choose a regular local WSPR.live JSON file.",
            format!("selected path: {}", path.display()),
        ));
    }
    if metadata.len() > WSPR_LIVE_IMPORT_LIMITS.source_bytes {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.adapter.source_bytes",
            "admission",
            WSPR_LIVE_IMPORT_LIMITS.source_bytes,
            Some(metadata.len()),
            "bytes",
        ));
    }
    let bytes = fs::read(path).map_err(|error| file_error(path, error))?;
    let (bundle_path, _) = active_session_source(state)?;
    let store = BundleStore::new(&bundle_path);
    let captured_at = Utc::now();
    let source_locator = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let config = import_config(&store, captured_at, source_locator.clone())?;
    let committed = commit_wspr_live_response(
        state,
        &bundle_path,
        &bytes,
        config,
        WsprLiveAcquisitionChannel::FileImport,
    )?;
    let summary = committed.summary;
    Ok(WsprLiveImportOutcome::Imported {
        session: Box::new(committed.session),
        revision: committed.revision,
        total: summary.total,
        accepted: summary.accepted,
        malformed: summary.malformed,
        filtered: summary.filtered,
        unsupported: summary.unsupported,
        duplicate: summary.duplicate,
        conflict: summary.conflict,
        observations_created: summary.observations_created,
        evidence_completeness_known: summary.evidence_completeness_known,
    })
}

fn import_config(
    store: &BundleStore,
    captured_at: chrono::DateTime<Utc>,
    source_locator: Option<String>,
) -> Result<WsprLiveImportConfig, SessionErrorPayload> {
    let schema_version = store
        .schema_version()
        .map_err(LivePersistenceError::from)
        .map_err(crate::conductor::live_error_payload)?;
    let (callsign, slots, confirmed_cycles, lifecycle) = match schema_version {
        SCHEMA_VERSION_V2 => {
            let bundle = store
                .read_v2_checkpointed()
                .map_err(crate::conductor::live_error_payload)?;
            (
                bundle.station.callsign,
                bundle.schedule.slots,
                None,
                bundle.session_state.lifecycle,
            )
        }
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => {
            let bundle = store
                .read_v3_checkpointed()
                .map_err(crate::conductor::live_error_payload)?;
            let directions = bundle
                .schedule
                .wspr_cycle_intents
                .iter()
                .map(|intent| (intent.intent_id.as_str(), intent.direction))
                .collect::<std::collections::BTreeMap<_, _>>();
            let confirmed_cycles =
                (bundle.manifest.schema_version >= SCHEMA_VERSION_V4).then(|| {
                    project_wspr_run_v3(&bundle.schedule, &bundle.events)
                        .cycles
                        .into_iter()
                        .filter(|cycle| cycle.occupancy_fully_covers_transmission)
                        .map(|cycle| WsprLiveConfirmedCycle {
                            slot_id: cycle.intent_id.clone(),
                            antenna_label: cycle.antenna_label,
                            starts_at: cycle.window.starts_at,
                            transmission_ends_at: cycle.window.transmission_ends_at,
                            band: cycle.band,
                            direction: directions.get(cycle.intent_id.as_str()).copied().flatten(),
                        })
                        .collect()
                });
            drop(directions);
            let lifecycle = bundle.session_state.lifecycle;
            let callsign = bundle.station.callsign.clone();
            let slots = bundle.into_current().bundle.schedule.slots;
            (callsign, slots, confirmed_cycles, lifecycle)
        }
        actual => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "This session format cannot import WSPR.live evidence.",
                format!("unsupported schema version {actual}"),
            ));
        }
    };
    if lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "WSPR.live evidence can be imported only while the session is running.",
            format!("current lifecycle: {lifecycle:?}"),
        ));
    }
    let scope = derive_wspr_live_query_scope(&callsign, &slots).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "The active session has no valid scheduled WSPR.live query scope.",
            error.to_string(),
        )
    })?;
    Ok(WsprLiveImportConfig {
        session_callsign: scope.session_callsign,
        window_start: scope.window_start,
        window_end: scope.window_end,
        selected_bands: scope.selected_bands,
        captured_at,
        source_locator,
        confirmed_cycles,
    })
}

pub(crate) fn commit_wspr_live_response(
    state: &ActiveSessionState,
    bundle_path: &Path,
    bytes: &[u8],
    config: WsprLiveImportConfig,
    channel: WsprLiveAcquisitionChannel,
) -> Result<CommittedWsprLiveResponse, SessionErrorPayload> {
    let parsed = parse_wspr_live_json(bytes, &config, &AdapterCancellationToken::default())
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The WSPR.live JSON response could not be imported.",
                error.to_string(),
            )
        })?;
    let store = BundleStore::new(bundle_path);
    match store
        .schema_version()
        .map_err(LivePersistenceError::from)
        .map_err(crate::conductor::live_error_payload)?
    {
        SCHEMA_VERSION_V2 => commit_v2_wspr_live_response(
            state,
            bundle_path,
            bytes,
            &config,
            channel,
            &parsed,
            &store,
        ),
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => commit_v3_wspr_live_response(
            state,
            bundle_path,
            bytes,
            &config,
            channel,
            &parsed,
            &store,
        ),
        actual => Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "This session format cannot import WSPR.live evidence.",
            format!("unsupported schema version {actual}"),
        )),
    }
}

fn commit_v2_wspr_live_response(
    state: &ActiveSessionState,
    bundle_path: &Path,
    bytes: &[u8],
    config: &WsprLiveImportConfig,
    channel: WsprLiveAcquisitionChannel,
    parsed: &ParsedWsprLiveJson,
    store: &BundleStore,
) -> Result<CommittedWsprLiveResponse, SessionErrorPayload> {
    let mut writer = store
        .open_v2_writer()
        .map_err(crate::conductor::live_error_payload)?;
    let current = writer.snapshot().clone();
    if current.session_state.lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The session stopped before the WSPR.live response could commit.",
            "no attachment, adapter, or observation records were appended",
        ));
    }
    let import_id = writer.allocate_id("import");
    let expected_revision = writer.checkpoint().revision;
    let source_locator = config.source_locator.clone();
    let mut summary = None::<WsprLiveImportSummary>;
    let (_, receipt) = writer
        .append_with_attachment(
            bytes,
            "application/json",
            None,
            Some("clickhouse-format-json".into()),
            source_locator,
            |attachment| {
                let prepared = prepare_wspr_live_acquisition(
                    parsed,
                    config,
                    &current.manifest.session_id,
                    &import_id,
                    attachment,
                    &current.adapter_records,
                    channel,
                );
                summary = Some(prepared.summary);
                let members = prepared
                    .adapter_records
                    .into_iter()
                    .map(LiveMutationMemberV2::AdapterRecord)
                    .chain(
                        prepared
                            .observations
                            .into_iter()
                            .map(LiveMutationMemberV2::Observation),
                    )
                    .collect();
                LiveMutationV2 {
                    expected_revision,
                    mutation_id: prepared.mutation_id,
                    members,
                }
            },
        )
        .map_err(crate::conductor::live_error_payload)?;
    drop(writer);
    Ok(CommittedWsprLiveResponse {
        session: reload_active_session(state, bundle_path)?,
        revision: receipt.revision,
        summary: summary.expect("attachment mutation builder runs before append"),
    })
}

fn commit_v3_wspr_live_response(
    state: &ActiveSessionState,
    bundle_path: &Path,
    bytes: &[u8],
    config: &WsprLiveImportConfig,
    channel: WsprLiveAcquisitionChannel,
    parsed: &ParsedWsprLiveJson,
    store: &BundleStore,
) -> Result<CommittedWsprLiveResponse, SessionErrorPayload> {
    let mut writer = store
        .open_v3_writer()
        .map_err(crate::conductor::live_error_payload)?;
    let current = writer.snapshot().clone();
    if current.session_state.lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The session stopped before the WSPR.live response could commit.",
            "no attachment, adapter, or observation records were appended",
        ));
    }
    let import_id = writer.allocate_id("import");
    let expected_revision = writer.checkpoint().revision;
    let source_locator = config.source_locator.clone();
    let mut summary = None::<WsprLiveImportSummary>;
    let (_, receipt) = writer
        .append_evidence_with_attachment(
            bytes,
            "application/json",
            None,
            Some("clickhouse-format-json".into()),
            source_locator,
            |attachment| {
                let prepared = prepare_wspr_live_acquisition(
                    parsed,
                    config,
                    &current.manifest.session_id,
                    &import_id,
                    attachment,
                    &current.adapter_records,
                    channel,
                );
                summary = Some(prepared.summary);
                LiveEvidenceMutationV3 {
                    expected_revision,
                    mutation_id: prepared.mutation_id,
                    adapter_records: prepared.adapter_records,
                    observations: prepared.observations,
                }
            },
        )
        .map_err(crate::conductor::live_error_payload)?;
    drop(writer);
    Ok(CommittedWsprLiveResponse {
        session: reload_active_session(state, bundle_path)?,
        revision: receipt.revision,
        summary: summary.expect("attachment mutation builder runs before append"),
    })
}

fn file_error(path: &Path, error: std::io::Error) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Filesystem,
        "The selected WSPR.live JSON file could not be read.",
        format!("{}: {error}", path.display()),
    )
}
