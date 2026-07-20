use super::*;

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
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
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

pub(crate) fn export_session_bundle_at_path(
    source: &Path,
    destination: &Path,
) -> Result<(String, Option<u64>), SessionErrorPayload> {
    export_bundle(source, destination).map_err(SessionErrorPayload::from)
}

pub(crate) fn validate_portable_session_at_path(path: &Path) -> Result<(), SessionErrorPayload> {
    open_bundle(path)
        .map(|_| ())
        .map_err(SessionErrorPayload::from)
}

pub(super) fn bundle_suffix(name: &str) -> Option<&'static str> {
    [V2_BUNDLE_SUFFIX, V1_BUNDLE_SUFFIX]
        .into_iter()
        .find(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
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
            session: Box::new(summary.clone()),
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

    Ok(OpenSessionOutcome::Opened {
        session: Box::new(summary),
    })
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

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn export_active_report_with_selection<F>(
    state: &ActiveSessionState,
    format: ReportExportFormat,
    controller_evidence: ControllerEvidenceHandling,
    select: F,
) -> Result<ExportReportOutcome, SessionErrorPayload>
where
    F: FnOnce(&Path) -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    export_active_report_with_selection_and_disclosure(
        state,
        format,
        controller_evidence,
        OperationalHistoryHandling::Omitted,
        select,
    )
}

pub(super) fn export_active_report_with_selection_and_disclosure<F>(
    state: &ActiveSessionState,
    format: ReportExportFormat,
    controller_evidence: ControllerEvidenceHandling,
    operational_history: OperationalHistoryHandling,
    select: F,
) -> Result<ExportReportOutcome, SessionErrorPayload>
where
    F: FnOnce(&Path) -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let _foreground = state.begin_foreground()?;
    let (source, presentation) = {
        let mut active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        active.pending_report_export = None;
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
    let file_name = report_display_file_name(&destination)?;
    let presentation_id = presentation.presentation_id;
    let session_id = presentation.session_id.clone();
    let revision = presentation.revision;
    let controller_evidence =
        if format == ReportExportFormat::FullEvidenceHtml && presentation.has_controller_evidence {
            controller_evidence
        } else {
            ControllerEvidenceHandling::Complete
        };
    let operational_history = if format == ReportExportFormat::FullEvidenceHtml {
        operational_history
    } else {
        OperationalHistoryHandling::Omitted
    };
    let html = match (format, controller_evidence, operational_history) {
        (ReportExportFormat::CompactSummaryHtml, _, _) => presentation.compact_summary_html,
        (
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            OperationalHistoryHandling::Omitted,
        ) => presentation.report_html,
        (
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::OmittedAtExport,
            OperationalHistoryHandling::Omitted,
        ) => presentation.controller_omitted_report_html.ok_or_else(|| {
            SessionErrorPayload::report_pipeline(
                "controller omission export is unavailable for this report snapshot",
            )
        })?,
        (
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            OperationalHistoryHandling::IncludedRedacted,
        ) => presentation.operational_history_report_html,
        (
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::OmittedAtExport,
            OperationalHistoryHandling::IncludedRedacted,
        ) => presentation
            .operational_history_controller_omitted_report_html
            .ok_or_else(|| {
                SessionErrorPayload::report_pipeline(
                    "combined controller omission and operational-history export is unavailable",
                )
            })?,
    };
    match selected_report_destination(&destination, &file_name)? {
        None => {
            write_new_report(&destination, &file_name, html.as_bytes())?;
            Ok(ExportReportOutcome::Exported {
                file_name,
                revision,
                format,
            })
        }
        Some(destination_identity) => {
            let pending_export_id = Uuid::new_v4().to_string();
            let mut active = state.0.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("active session state is unavailable")
            })?;
            let current = active
                .active
                .as_ref()
                .and_then(|session| session.presentation.as_ref());
            if !current.is_some_and(|current| {
                current.presentation_id == presentation_id
                    && current.session_id == session_id
                    && current.revision == revision
            }) {
                return Err(stale_pending_report_error(
                    "the active report presentation changed during destination selection",
                ));
            }
            active.pending_report_export = Some(PendingReportExport {
                pending_export_id: pending_export_id.clone(),
                destination,
                destination_identity,
                file_name: file_name.clone(),
                presentation_id,
                session_id,
                revision,
                format,
                html,
            });
            Ok(ExportReportOutcome::ConfirmationRequired {
                pending_export_id,
                file_name,
                revision,
                format,
            })
        }
    }
}

const REPORT_DISPLAY_FILE_NAME_CHARS: usize = 160;

fn report_display_file_name(destination: &Path) -> Result<String, SessionErrorPayload> {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(".html"))
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "Choose an .html destination for the standalone report.",
                "the selected destination does not have a usable .html file name",
            )
        })?;
    let mut chars = name.chars();
    let bounded = chars
        .by_ref()
        .take(REPORT_DISPLAY_FILE_NAME_CHARS)
        .collect::<String>();
    if chars.next().is_some() {
        Ok(format!("{bounded}…"))
    } else {
        Ok(bounded)
    }
}

fn selected_report_destination(
    destination: &Path,
    file_name: &str,
) -> Result<Option<ReportDestinationIdentity>, SessionErrorPayload> {
    match std::fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(report_destination_error(
            "The selected report destination could not be inspected.",
            file_name,
            error,
        )),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "Choose a new destination or an existing regular HTML file.",
                format!("{file_name} is a directory, symbolic link, or unsupported entry"),
            ))
        }
        Ok(_) => inspect_report_destination(destination, file_name).map(Some),
    }
}

fn inspect_report_destination(
    destination: &Path,
    file_name: &str,
) -> Result<ReportDestinationIdentity, SessionErrorPayload> {
    let before = std::fs::symlink_metadata(destination).map_err(|error| {
        report_destination_error(
            "The existing report destination is no longer available.",
            file_name,
            error,
        )
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "Only an existing regular HTML file can be replaced.",
            format!("{file_name} is a directory, symbolic link, or unsupported entry"),
        ));
    }
    let mut file = File::open(destination).map_err(|error| {
        report_destination_error(
            "The existing report destination could not be read safely.",
            file_name,
            error,
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        report_destination_error(
            "The existing report destination could not be verified.",
            file_name,
            error,
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            report_destination_error(
                "The existing report destination could not be verified.",
                file_name,
                error,
            )
        })?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let digest: [u8; 32] = digest.finalize().into();
    let opened_identity = report_destination_identity(&opened, digest);
    let after = std::fs::symlink_metadata(destination).map_err(|error| {
        report_destination_error(
            "The existing report destination changed while it was being inspected.",
            file_name,
            error,
        )
    })?;
    if after.file_type().is_symlink()
        || !after.is_file()
        || report_destination_identity(&after, digest) != opened_identity
    {
        return Err(stale_pending_report_error(
            "the selected destination changed while it was being inspected",
        ));
    }
    Ok(opened_identity)
}

fn report_destination_identity(
    metadata: &std::fs::Metadata,
    content_digest: [u8; 32],
) -> ReportDestinationIdentity {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;

    ReportDestinationIdentity {
        length: metadata.len(),
        modified_nanos: metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos()),
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(unix)]
        changed_seconds: metadata.ctime(),
        #[cfg(unix)]
        changed_nanos: metadata.ctime_nsec(),
        content_digest,
    }
}

fn write_new_report(
    destination: &Path,
    file_name: &str,
    html: &[u8],
) -> Result<(), SessionErrorPayload> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| {
            report_destination_error(
                "The report destination could not be created without overwriting a file.",
                file_name,
                error,
            )
        })?;
    if let Err(error) = file.write_all(html).and_then(|()| file.sync_all()) {
        drop(file);
        let cleanup = std::fs::remove_file(destination);
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The standalone report could not be written completely.",
            cleanup.map_or_else(
                |cleanup| {
                    format!(
                        "{file_name}: {error}; incomplete destination cleanup failed: {cleanup}"
                    )
                },
                |()| format!("{file_name}: {error}"),
            ),
        ));
    }
    Ok(())
}

pub(super) trait ReportReplacePort {
    fn replace(&self, temporary: tempfile::NamedTempFile, destination: &Path)
        -> Result<(), String>;
}

pub(super) struct SystemReportReplacePort;

impl ReportReplacePort for SystemReportReplacePort {
    fn replace(
        &self,
        temporary: tempfile::NamedTempFile,
        destination: &Path,
    ) -> Result<(), String> {
        match temporary.persist(destination) {
            Ok(_) => Ok(()),
            Err(error) => {
                let replacement_error = error.error.to_string();
                match error.file.close() {
                    Ok(()) => Err(replacement_error),
                    Err(cleanup) => Err(format!(
                        "{replacement_error}; temporary output cleanup failed: {cleanup}"
                    )),
                }
            }
        }
    }
}

fn atomic_replace_report(
    pending: &PendingReportExport,
    replace_port: &dyn ReportReplacePort,
) -> Result<(), SessionErrorPayload> {
    let current = inspect_report_destination(&pending.destination, &pending.file_name)?;
    if current != pending.destination_identity {
        return Err(stale_pending_report_error(
            "the existing report changed after replacement was requested",
        ));
    }
    let parent = pending.destination.parent().ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "The report destination has no writable parent directory.",
            pending.file_name.clone(),
        )
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        report_destination_error(
            "A sibling temporary report could not be created.",
            &pending.file_name,
            error,
        )
    })?;
    if let Err(error) = temporary
        .write_all(pending.html.as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
    {
        let cleanup = temporary.close();
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The replacement report could not be written completely.",
            cleanup.map_or_else(
                |cleanup| {
                    format!(
                        "{}: {error}; temporary output cleanup failed: {cleanup}",
                        pending.file_name
                    )
                },
                |()| format!("{}: {error}", pending.file_name),
            ),
        ));
    }
    let current = inspect_report_destination(&pending.destination, &pending.file_name)?;
    if current != pending.destination_identity {
        temporary.close().map_err(|cleanup| {
            SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The existing report changed and the temporary output could not be cleaned up.",
                format!("{}: {cleanup}", pending.file_name),
            )
        })?;
        return Err(stale_pending_report_error(
            "the existing report changed before atomic replacement",
        ));
    }
    replace_port
        .replace(temporary, &pending.destination)
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The existing report could not be replaced atomically.",
                format!("{}: {error}", pending.file_name),
            )
        })
}

pub(super) fn confirm_pending_report_export_with(
    state: &ActiveSessionState,
    pending_export_id: &str,
    replace_port: &dyn ReportReplacePort,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    let pending = {
        let mut active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        if active
            .pending_report_export
            .as_ref()
            .is_none_or(|pending| pending.pending_export_id != pending_export_id)
        {
            return Err(stale_pending_report_error(
                "the pending report replacement identity is unknown or already consumed",
            ));
        }
        active
            .pending_report_export
            .take()
            .expect("checked pending export")
    };
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let presentation = active
        .active
        .as_ref()
        .and_then(|session| session.presentation.as_ref());
    if !presentation.is_some_and(|presentation| {
        presentation.presentation_id == pending.presentation_id
            && presentation.session_id == pending.session_id
            && presentation.revision == pending.revision
    }) {
        return Err(stale_pending_report_error(
            "the active report presentation changed before replacement was confirmed",
        ));
    }
    drop(active);
    atomic_replace_report(&pending, replace_port)?;
    Ok(ExportReportOutcome::Exported {
        file_name: pending.file_name,
        revision: pending.revision,
        format: pending.format,
    })
}

pub(super) fn cancel_pending_report_export_for(
    state: &ActiveSessionState,
    pending_export_id: &str,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    let mut active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    if active
        .pending_report_export
        .as_ref()
        .is_some_and(|pending| pending.pending_export_id == pending_export_id)
    {
        active.pending_report_export = None;
    }
    Ok(ExportReportOutcome::Cancelled)
}

fn report_destination_error(
    message: &str,
    file_name: &str,
    error: std::io::Error,
) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Destination,
        message,
        format!("{file_name}: {error}"),
    )
}

fn stale_pending_report_error(detail: impl Into<String>) -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Conflict,
        "The pending report replacement is no longer current.",
        detail,
    )
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
            operational_history: snapshot.operational_history.clone(),
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
        active.pending_report_export = None;
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
pub(crate) async fn export_active_session_report(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
    format: Option<ReportExportFormat>,
    controller_evidence: Option<ControllerEvidenceHandling>,
    operational_history: Option<OperationalHistoryHandling>,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    let format = format.unwrap_or_default();
    let result = export_active_report_with_selection_and_disclosure(
        state.inner(),
        format,
        controller_evidence.unwrap_or_default(),
        operational_history.unwrap_or_default(),
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
    );
    result.map_err(|payload| persist_report_export_failure(state.inner(), payload))
}

#[tauri::command]
pub(crate) fn confirm_report_export(
    state: State<'_, ActiveSessionState>,
    pending_export_id: String,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    confirm_pending_report_export_with(state.inner(), &pending_export_id, &SystemReportReplacePort)
        .map_err(|payload| persist_report_export_failure(state.inner(), payload))
}

#[tauri::command]
pub(crate) fn cancel_report_export(
    state: State<'_, ActiveSessionState>,
    pending_export_id: String,
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    cancel_pending_report_export_for(state.inner(), &pending_export_id)
}

#[tauri::command]
pub(crate) fn active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<ReportPresentation, SessionErrorPayload> {
    active_session_report_for(state.inner()).map_err(|payload| {
        persist_active_operation_failure(
            state.inner(),
            DiagnosticOperationV6::ReportRender,
            DiagnosticPhaseV6::Render,
            "report.render_failed",
            payload,
        )
    })
}

#[tauri::command]
pub(crate) fn refresh_active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<ReportPresentation, SessionErrorPayload> {
    refresh_active_session_report_for(state.inner()).map_err(|payload| {
        persist_active_operation_failure(
            state.inner(),
            DiagnosticOperationV6::ReportRender,
            DiagnosticPhaseV6::Render,
            "report.render_failed",
            payload,
        )
    })
}

fn persist_active_operation_failure(
    state: &ActiveSessionState,
    operation: DiagnosticOperationV6,
    phase: DiagnosticPhaseV6,
    code: &str,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    let Ok((source, _)) = active_session_source(state) else {
        return payload;
    };
    crate::operation_diagnostics::persist_failure(
        &source,
        operation,
        phase,
        code,
        EvidenceEffectV6::NoneCommitted,
        Vec::new(),
        payload,
    )
}

fn persist_report_export_failure(
    state: &ActiveSessionState,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    persist_active_operation_failure(
        state,
        DiagnosticOperationV6::ReportExport,
        DiagnosticPhaseV6::WriteDestination,
        "report.export_failed",
        payload,
    )
}
