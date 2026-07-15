use std::{fs, io::Cursor, path::Path};

use antennabench_core::{Band, BundleV3Contents, SessionLifecycleV2, SCHEMA_VERSION_V3};
use antennabench_rbn::{
    parse_rbn_zip, prepare_rbn_import, RbnImportConfig, RbnImportPreparationConfig,
    RbnPreparedSummary, RBN_ARCHIVE_LIMITS,
};
use antennabench_storage::{BundleStore, LiveEvidenceMutationV3};
use chrono::{Duration, Utc};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::open_session::{
    active_session_source, reload_active_session, with_foreground_operation, ActiveSessionState,
    OpenedSession, SessionErrorKind, SessionErrorPayload,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum RbnImportOutcome {
    Cancelled,
    Imported {
        session: Box<OpenedSession>,
        revision: u64,
        total: u64,
        accepted: u64,
        malformed: u64,
        filtered: u64,
        unsupported: u64,
        duplicate: u64,
        conflict: u64,
        omitted: u64,
        #[serde(rename = "observationsCreated")]
        observations_created: u64,
    },
}

#[tauri::command]
pub(crate) async fn import_active_session_rbn(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
) -> Result<RbnImportOutcome, SessionErrorPayload> {
    let Some(selection) = app
        .dialog()
        .file()
        .set_title("Import a Reverse Beacon Network daily archive")
        .set_can_create_directories(false)
        .add_filter("RBN daily ZIP", &["zip"])
        .blocking_pick_file()
    else {
        return Ok(RbnImportOutcome::Cancelled);
    };
    let path = selection.into_path().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "The selected RBN archive is not available as a local path.",
            error.to_string(),
        )
    })?;
    with_foreground_operation(state.inner(), || import_file(state.inner(), &path))
}

fn import_file(
    state: &ActiveSessionState,
    path: &Path,
) -> Result<RbnImportOutcome, SessionErrorPayload> {
    let metadata = fs::metadata(path).map_err(|error| file_error(path, error))?;
    if !metadata.is_file() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "Choose a regular local RBN ZIP archive.",
            format!("selected path: {}", path.display()),
        ));
    }
    if metadata.len() > RBN_ARCHIVE_LIMITS.compressed_bytes {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.rbn.compressed_bytes",
            "admission",
            RBN_ARCHIVE_LIMITS.compressed_bytes,
            Some(metadata.len()),
            "bytes",
        ));
    }
    let bytes = fs::read(path).map_err(|error| file_error(path, error))?;
    let byte_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if byte_size > RBN_ARCHIVE_LIMITS.compressed_bytes {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.rbn.compressed_bytes",
            "read",
            RBN_ARCHIVE_LIMITS.compressed_bytes,
            Some(byte_size),
            "bytes",
        ));
    }
    let (bundle_path, _) = active_session_source(state)?;
    let store = BundleStore::new(&bundle_path);
    if store
        .schema_version()
        .map_err(|error| crate::conductor::live_error_payload(error.into()))?
        != SCHEMA_VERSION_V3
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "RBN archive import requires a schema-v3 signal session.",
            "open or create a schema-v3 session with an explicit signal plan",
        ));
    }
    let current = store
        .read_v3_checkpointed()
        .map_err(crate::conductor::live_error_payload)?;
    if matches!(
        current.session_state.lifecycle,
        SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
    ) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "RBN evidence can be imported only after the session has started.",
            format!("current lifecycle: {:?}", current.session_state.lifecycle),
        ));
    }
    let config = import_config(&current)?;
    let parsed = parse_rbn_zip(Cursor::new(&bytes), byte_size, &config).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The RBN daily archive could not be imported.",
            error.to_string(),
        )
    })?;
    let captured_at = Utc::now();
    let source_locator = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let preparation = RbnImportPreparationConfig {
        captured_at,
        source_locator: source_locator.clone(),
    };
    let mut writer = store
        .open_v3_writer()
        .map_err(crate::conductor::live_error_payload)?;
    let import_id = writer.allocate_id("rbn-import");
    let expected_revision = writer.checkpoint().revision;
    let mut summary = None::<RbnPreparedSummary>;
    let (_, receipt) = writer
        .append_evidence_with_attachment(
            &bytes,
            "application/zip",
            None,
            Some("zip-single-csv".into()),
            source_locator,
            |attachment| {
                let prepared = prepare_rbn_import(
                    &parsed,
                    &config,
                    &preparation,
                    &current.manifest.session_id,
                    &import_id,
                    attachment,
                    &current.adapter_records,
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
    let summary = summary.expect("attachment mutation builder runs before append");
    Ok(RbnImportOutcome::Imported {
        session: Box::new(reload_active_session(state, &bundle_path)?),
        revision: receipt.revision,
        total: summary.total,
        accepted: summary.accepted,
        malformed: summary.malformed,
        filtered: summary.filtered,
        unsupported: summary.unsupported,
        duplicate: summary.duplicate,
        conflict: summary.conflict,
        omitted: summary.omitted,
        observations_created: summary.observations_created,
    })
}

fn import_config(bundle: &BundleV3Contents) -> Result<RbnImportConfig, SessionErrorPayload> {
    let window_start = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| slot.starts_at)
        .min()
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Unsupported,
                "The active signal session has no RBN import window.",
                "the schedule has no slots",
            )
        })?;
    let window_end = bundle
        .schedule
        .slots
        .iter()
        .filter_map(|slot| {
            let seconds = slot.duration_seconds.checked_add(slot.guard_seconds)?;
            slot.starts_at
                .checked_add_signed(Duration::seconds(i64::from(seconds)))
        })
        .max()
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Unsupported,
                "The active signal session has no valid RBN import window.",
                "a scheduled slot end overflowed",
            )
        })?;
    let mut selected_bands = Vec::<Band>::new();
    for band in bundle.schedule.slots.iter().map(|slot| slot.band) {
        if !selected_bands.contains(&band) {
            selected_bands.push(band);
        }
    }
    Ok(RbnImportConfig {
        heard_callsign: bundle.station.callsign.clone(),
        window_start,
        window_end,
        selected_bands,
    })
}

fn file_error(path: &Path, error: std::io::Error) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Filesystem,
        "The selected RBN ZIP archive could not be read.",
        format!("{}: {error}", path.display()),
    )
}
