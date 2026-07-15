use std::{fs, path::Path};

use antennabench_core::SessionLifecycleV2;
use antennabench_storage::{BundleStore, LiveMutationMemberV2, LiveMutationV2};
use antennabench_wsjtx::{
    derive_wspr_live_query_scope, parse_wspr_live_json, prepare_wspr_live_acquisition,
    AdapterCancellationToken, WsprLiveAcquisitionChannel, WsprLiveImportConfig,
    WsprLiveImportSummary, WSPR_LIVE_IMPORT_LIMITS,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WsprLiveImportRequest {
    authority_confirmed: bool,
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
    request: WsprLiveImportRequest,
) -> Result<WsprLiveImportOutcome, SessionErrorPayload> {
    if !request.authority_confirmed {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Confirm that you may use the supplied WSPR.live response.",
            "authorityConfirmed must be true before the native file picker opens",
        ));
    }
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
    let snapshot = store
        .read_v2_checkpointed()
        .map_err(crate::conductor::live_error_payload)?;
    if snapshot.session_state.lifecycle != SessionLifecycleV2::Running {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "WSPR.live evidence can be imported only while the session is running.",
            format!("current lifecycle: {:?}", snapshot.session_state.lifecycle),
        ));
    }
    let scope = derive_wspr_live_query_scope(&snapshot.station.callsign, &snapshot.schedule.slots)
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Unsupported,
                "The active session has no valid scheduled WSPR.live query scope.",
                error.to_string(),
            )
        })?;
    let captured_at = Utc::now();
    let source_locator = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let config = WsprLiveImportConfig {
        session_callsign: scope.session_callsign,
        window_start: scope.window_start,
        window_end: scope.window_end,
        selected_bands: scope.selected_bands,
        captured_at,
        source_locator: source_locator.clone(),
    };
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
                    &parsed,
                    &config,
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

fn file_error(path: &Path, error: std::io::Error) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Filesystem,
        "The selected WSPR.live JSON file could not be read.",
        format!("{}: {error}", path.display()),
    )
}
