use super::*;

pub(super) fn suggested_export_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| bundle_suffix(name).map(|suffix| (name, suffix)))
        .map_or_else(
            || format!("session-copy{V2_BUNDLE_SUFFIX}"),
            |(name, suffix)| {
                let stem = name.strip_suffix(suffix).expect("matched suffix");
                format!("{stem}-copy{suffix}")
            },
        )
}

pub(super) fn suggested_report_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| bundle_suffix(name).map(|suffix| name.trim_end_matches(suffix)))
        .map_or_else(
            || "antennabench-report.html".into(),
            |stem| format!("{stem}-report.html"),
        )
}

pub(super) fn suggested_compact_summary_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| bundle_suffix(name).map(|suffix| name.trim_end_matches(suffix)))
        .map_or_else(
            || "antennabench-compact-summary.html".into(),
            |stem| format!("{stem}-compact-summary.html"),
        )
}

pub(super) fn export_bundle(
    source: &Path,
    destination: &Path,
) -> Result<(String, Option<u64>), ExportSessionError> {
    let bundle_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            source
                .file_name()
                .and_then(|source| source.to_str())
                .and_then(bundle_suffix)
                .is_some_and(|suffix| name.ends_with(suffix))
        })
        .ok_or_else(|| ExportSessionError::InvalidDestination {
            name: destination.file_name().map_or_else(
                || destination.display().to_string(),
                |name| name.to_string_lossy().into(),
            ),
        })?
        .to_string();

    let source_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let revision = if source_name.ends_with(V2_BUNDLE_SUFFIX) {
        let store = BundleStore::new(source);
        match store.schema_version().map_err(LivePersistenceError::from)? {
            SCHEMA_VERSION_V2 => {
                let exported = store.export_v2_checkpointed_to(destination)?;
                Some(exported.read_v2_checkpointed()?.session_state.revision)
            }
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => {
                let exported = store.export_v3_checkpointed_to(destination)?;
                Some(exported.read_v3_checkpointed()?.session_state.revision)
            }
            actual => {
                return Err(LivePersistenceError::from(
                    BundleStoreError::UnsupportedSchemaVersion { actual },
                )
                .into());
            }
        }
    } else {
        BundleStore::new(source).copy_losslessly_to(destination)?;
        None
    };
    Ok((bundle_name, revision))
}

pub(super) fn bundle_suffix(name: &str) -> Option<&'static str> {
    [V2_BUNDLE_SUFFIX, V1_BUNDLE_SUFFIX]
        .into_iter()
        .find(|suffix| name.ends_with(suffix))
}

pub(super) fn open_session_with_selection<F>(
    state: &ActiveSessionState,
    select: F,
) -> Result<OpenSessionOutcome, SessionErrorPayload>
where
    F: FnOnce() -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    open_session_with_selection_and_verification(state, select, |_| Ok(()))
}

fn open_session_with_selection_and_verification<F, V>(
    state: &ActiveSessionState,
    select: F,
    verify: V,
) -> Result<OpenSessionOutcome, SessionErrorPayload>
where
    F: FnOnce() -> Result<Option<PathBuf>, SessionErrorPayload>,
    V: FnOnce(&Path) -> Result<(), SessionErrorPayload>,
{
    let _foreground = state.begin_foreground()?;
    let Some(path) = select()? else {
        return Ok(OpenSessionOutcome::Cancelled);
    };
    let mut session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
    verify(&path)?;
    let summary = session.summary.clone();
    check_ipc_payload(
        &OpenSessionOutcome::Opened {
            session: summary.clone(),
        },
        SESSION_SUMMARY_IPC_BYTES,
        "session_summary",
    )?;

    let mut active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    assign_presentation_id(&mut active, &mut session);
    active.active = Some(session);
    active.export_source = Some(path);

    Ok(OpenSessionOutcome::Opened { session: summary })
}

#[cfg(test)]
pub(crate) fn open_session_at_path(
    state: &ActiveSessionState,
    path: PathBuf,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    open_session_with_selection(state, || Ok(Some(path)))
}

pub(crate) fn open_session_at_path_verified(
    state: &ActiveSessionState,
    path: PathBuf,
    verify: impl FnOnce(&Path) -> Result<(), SessionErrorPayload>,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    open_session_with_selection_and_verification(state, || Ok(Some(path)), verify)
}

pub(crate) fn finish_open_side_effects(
    controller_state: &AntennaControllerState,
    wsjtx_state: &WsjtxSessionState,
) {
    controller_state.revoke();
    wsjtx_state.stop_all("WSJT-X reception stopped because a different session was opened.");
}

pub(super) fn export_active_session_with_selection<F>(
    state: &ActiveSessionState,
    select: F,
) -> Result<ExportSessionOutcome, SessionErrorPayload>
where
    F: FnOnce(&Path) -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let _foreground = state.begin_foreground()?;
    let source = {
        let active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        active
            .active
            .as_ref()
            .map(|session| session.source.clone())
            .or_else(|| active.export_source.clone())
            .ok_or(ExportSessionError::NoActiveSession)
            .map_err(SessionErrorPayload::from)?
    };

    let Some(destination) = select(&source)? else {
        return Ok(ExportSessionOutcome::Cancelled);
    };
    let (bundle_name, revision) =
        export_bundle(&source, &destination).map_err(SessionErrorPayload::from)?;

    Ok(ExportSessionOutcome::Exported {
        bundle_name,
        revision,
    })
}

pub(super) fn export_active_report_with_selection<F>(
    state: &ActiveSessionState,
    format: ReportExportFormat,
    controller_evidence: ControllerEvidenceHandling,
    select: F,
) -> Result<ExportReportOutcome, SessionErrorPayload>
where
    F: FnOnce(&Path) -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let _foreground = state.begin_foreground()?;
    let (source, presentation) = {
        let active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        let session = active.active.as_ref().ok_or_else(|| {
            SessionErrorPayload::report_pipeline("no active session report is available")
        })?;
        let presentation = session.presentation.clone().ok_or_else(|| {
            SessionErrorPayload::report_pipeline("no coherent report snapshot is available")
        })?;
        (session.source.clone(), presentation)
    };
    let Some(destination) = select(&source)? else {
        return Ok(ExportReportOutcome::Cancelled);
    };
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(".html"))
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "Choose an .html destination for the standalone report.",
                destination.display().to_string(),
            )
        })?
        .to_string();
    let controller_evidence =
        if format == ReportExportFormat::FullEvidenceHtml && presentation.has_controller_evidence {
            controller_evidence
        } else {
            ControllerEvidenceHandling::Complete
        };
    let html = match (format, controller_evidence) {
        (ReportExportFormat::CompactSummaryHtml, _) => &presentation.compact_summary_html,
        (ReportExportFormat::FullEvidenceHtml, ControllerEvidenceHandling::Complete) => {
            &presentation.report_html
        }
        (ReportExportFormat::FullEvidenceHtml, ControllerEvidenceHandling::OmittedAtExport) => {
            presentation
                .controller_omitted_report_html
                .as_ref()
                .ok_or_else(|| {
                    SessionErrorPayload::report_pipeline(
                        "controller omission export is unavailable for this report snapshot",
                    )
                })?
        }
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "The report destination could not be created without overwriting a file.",
                format!("{}: {error}", destination.display()),
            )
        })?;
    if let Err(error) = file
        .write_all(html.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let cleanup = std::fs::remove_file(&destination);
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The standalone report could not be written completely.",
            cleanup.map_or_else(
                |cleanup| {
                    format!(
                        "{}: {error}; incomplete destination cleanup failed: {cleanup}",
                        destination.display()
                    )
                },
                |()| format!("{}: {error}", destination.display()),
            ),
        ));
    }
    Ok(ExportReportOutcome::Exported {
        file_name,
        revision: presentation.revision,
        format,
    })
}

pub(super) fn active_session_report_for(
    state: &ActiveSessionState,
) -> Result<ReportPresentation, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.active.as_ref().ok_or_else(|| {
        SessionErrorPayload::report_pipeline("no active session report is available")
    })?;

    let source_name = session
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            SessionErrorPayload::report_pipeline("the active session source name is unavailable")
        })?;
    if source_name != session.summary.bundle_name {
        return Err(SessionErrorPayload::report_pipeline(
            "the active session source does not match its derived report",
        ));
    }

    let presentation = session.presentation.as_ref().ok_or_else(|| {
        SessionErrorPayload::report_pipeline(
            "the active snapshot is available for lossless export, but report rendering is unavailable",
        )
    })?;
    if presentation.report_html.len() as u64 > REPORT_DOCUMENT_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            "report_document",
            REPORT_DOCUMENT_IPC_BYTES,
            Some(presentation.report_html.len() as u64),
            "bytes",
        ));
    }

    Ok(presentation.clone())
}

pub(super) fn refresh_active_session_report_for(
    state: &ActiveSessionState,
) -> Result<ReportPresentation, SessionErrorPayload> {
    const MAX_REFRESH_ATTEMPTS: usize = 3;
    let _foreground = state.begin_foreground()?;
    let (source, bundle_name) = {
        let active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        let session = active.active.as_ref().ok_or_else(|| {
            SessionErrorPayload::report_pipeline("no active session report is available")
        })?;
        (session.source.clone(), session.summary.bundle_name.clone())
    };

    for _ in 0..MAX_REFRESH_ATTEMPTS {
        let snapshot = load_snapshot(&source, &bundle_name).map_err(SessionErrorPayload::from)?;
        if let Some(existing) = state
            .0
            .lock()
            .map_err(|_| {
                SessionErrorPayload::report_pipeline("active session state is unavailable")
            })?
            .active
            .as_ref()
            .filter(|session| session.source == source)
            .and_then(|session| session.presentation.as_ref())
            .filter(|presentation| {
                presentation.revision == snapshot.revision
                    && presentation.session_id == snapshot.bundle.manifest.session_id
            })
            .cloned()
        {
            return Ok(existing);
        }
        let mut presentation = prepare_presentation(&snapshot).map_err(report_error_payload)?;
        let verified = load_snapshot(&source, &bundle_name).map_err(SessionErrorPayload::from)?;
        if snapshot.revision != verified.revision {
            continue;
        }
        let summary = OpenedSession {
            bundle_name: bundle_name.clone(),
            session_id: snapshot.bundle.manifest.session_id.clone(),
            callsign: snapshot.bundle.station.callsign.clone(),
            grid: snapshot.bundle.station.grid.clone(),
            antenna_count: snapshot.bundle.antennas.antennas.len(),
            slot_count: snapshot.intended_cycle_count,
            observation_count: snapshot.bundle.observations.len(),
            schema_version: snapshot.schema_version,
            revision: snapshot.revision,
            lifecycle: snapshot.lifecycle,
            report_available: true,
        };
        let mut active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        let next_id = active.next_presentation_id.saturating_add(1);
        let session = active.active.as_mut().ok_or_else(|| {
            SessionErrorPayload::report_pipeline("no active session report is available")
        })?;
        if session.source != source {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The active session changed while its report was refreshing.",
                "the prior coherent presentation remains visible",
            ));
        }
        presentation.presentation_id = next_id;
        session.summary = summary;
        session.presentation = Some(presentation.clone());
        active.next_presentation_id = next_id;
        return Ok(presentation);
    }

    Err(SessionErrorPayload::new(
        SessionErrorKind::StaleRevision,
        "The session kept changing while its report was refreshing.",
        "no stale presentation was published; retry after the current intake burst",
    ))
}

pub(crate) fn check_ipc_payload(
    payload: &impl Serialize,
    limit: u64,
    role: &'static str,
) -> Result<(), SessionErrorPayload> {
    let bytes = serde_json::to_vec(payload).map_err(|error| {
        SessionErrorPayload::report_pipeline(format!("IPC serialization failed: {error}"))
    })?;
    if bytes.len() as u64 > limit {
        Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            role,
            limit,
            Some(bytes.len() as u64),
            "bytes",
        ))
    } else {
        Ok(())
    }
}

#[tauri::command]
pub(crate) async fn open_session_bundle(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    let outcome = open_session_with_selection(state.inner(), || {
        let Some(selection) = app
            .dialog()
            .file()
            .set_title("Open an AntennaBench session bundle")
            .set_can_create_directories(false)
            .blocking_pick_folder()
        else {
            return Ok(None);
        };

        selection.into_path().map(Some).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Selection,
                "The selected directory is not available as a local path.",
                error.to_string(),
            )
        })
    })?;
    if matches!(outcome, OpenSessionOutcome::Opened { .. }) {
        finish_open_side_effects(controller_state.inner(), wsjtx_state.inner());
    }
    Ok(outcome)
}

#[tauri::command]
pub(crate) async fn export_active_session(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
) -> Result<ExportSessionOutcome, SessionErrorPayload> {
    export_active_session_with_selection(state.inner(), |source| {
        let mut dialog = app
            .dialog()
            .file()
            .set_title("Export an AntennaBench session bundle copy")
            .set_file_name(suggested_export_name(source))
            .set_can_create_directories(true)
            .add_filter(
                "AntennaBench session bundle",
                &["antennabundle", "wsprabundle"],
            );
        if let Some(parent) = source.parent() {
            dialog = dialog.set_directory(parent);
        }

        let Some(selection) = dialog.blocking_save_file() else {
            return Ok(None);
        };
        selection.into_path().map(Some).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "The selected destination is not available as a local path.",
                error.to_string(),
            )
        })
    })
}

#[tauri::command]
pub(crate) async fn export_active_session_report(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
    format: Option<ReportExportFormat>,
    controller_evidence: Option<ControllerEvidenceHandling>,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    let format = format.unwrap_or_default();
    export_active_report_with_selection(
        state.inner(),
        format,
        controller_evidence.unwrap_or_default(),
        |source| {
            let mut dialog = app
                .dialog()
                .file()
                .set_title(match format {
                    ReportExportFormat::CompactSummaryHtml => {
                        "Export compact AntennaBench share summary (not the full audit report)"
                    }
                    ReportExportFormat::FullEvidenceHtml => {
                        "Export full AntennaBench evidence report snapshot"
                    }
                })
                .set_file_name(match format {
                    ReportExportFormat::CompactSummaryHtml => {
                        suggested_compact_summary_name(source)
                    }
                    ReportExportFormat::FullEvidenceHtml => suggested_report_name(source),
                })
                .set_can_create_directories(true)
                .add_filter(
                    match format {
                        ReportExportFormat::CompactSummaryHtml => "Compact summary HTML",
                        ReportExportFormat::FullEvidenceHtml => "Full evidence HTML report",
                    },
                    &["html"],
                );
            if let Some(parent) = source.parent() {
                dialog = dialog.set_directory(parent);
            }
            let Some(selection) = dialog.blocking_save_file() else {
                return Ok(None);
            };
            selection.into_path().map(Some).map_err(|error| {
                SessionErrorPayload::new(
                    SessionErrorKind::Destination,
                    "The selected destination is not available as a local path.",
                    error.to_string(),
                )
            })
        },
    )
}

#[tauri::command]
pub(crate) fn active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<ReportPresentation, SessionErrorPayload> {
    active_session_report_for(state.inner())
}

#[tauri::command]
pub(crate) fn refresh_active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<ReportPresentation, SessionErrorPayload> {
    refresh_active_session_report_for(state.inner())
}
