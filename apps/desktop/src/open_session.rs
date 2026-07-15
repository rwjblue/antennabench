use std::{
    error::Error as StdError,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_analysis::AnalysisError;
use antennabench_core::{
    BundleContents, BundleValidationReport, V1_BUNDLE_SUFFIX, V2_BUNDLE_SUFFIX,
};
use antennabench_report::{build_report_with_validation, render_standalone_html, ReportError};
use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

const SESSION_SUMMARY_IPC_BYTES: u64 = 64 * 1024;
const REPORT_DOCUMENT_IPC_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct ActiveSessionState(Mutex<DesktopState>);

#[derive(Default)]
struct DesktopState {
    active: Option<ActiveSession>,
    export_source: Option<PathBuf>,
    foreground_busy: bool,
}

struct ForegroundGuard<'a>(&'a ActiveSessionState);

impl ActiveSessionState {
    fn begin_foreground(&self) -> Result<ForegroundGuard<'_>, SessionErrorPayload> {
        let mut state = self.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        if state.foreground_busy {
            return Err(SessionErrorPayload::resource(
                SessionErrorKind::Busy,
                "resource.operation.busy",
                "foreground",
                1,
                Some(2),
                "operations",
            ));
        }
        state.foreground_busy = true;
        Ok(ForegroundGuard(self))
    }
}

pub(crate) fn with_foreground_operation<T>(
    state: &ActiveSessionState,
    operation: impl FnOnce() -> Result<T, SessionErrorPayload>,
) -> Result<T, SessionErrorPayload> {
    let _foreground = state.begin_foreground()?;
    operation()
}

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            state.foreground_busy = false;
        }
    }
}

#[derive(Debug)]
struct ActiveSession {
    source: PathBuf,
    summary: OpenedSession,
    report_html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenedSession {
    pub(crate) bundle_name: String,
    pub(crate) session_id: String,
    pub(crate) callsign: String,
    pub(crate) grid: String,
    pub(crate) antenna_count: usize,
    pub(crate) slot_count: usize,
    pub(crate) observation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum OpenSessionOutcome {
    Cancelled,
    Opened { session: OpenedSession },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ExportSessionOutcome {
    Cancelled,
    Exported {
        #[serde(rename = "bundleName")]
        bundle_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionErrorKind {
    Selection,
    Destination,
    Filesystem,
    JsonParse,
    Validation,
    Analysis,
    ReportPipeline,
    Verification,
    Resource,
    Busy,
    StaleRevision,
    Conflict,
    Unsupported,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SessionErrorPayload {
    pub(crate) kind: SessionErrorKind,
    pub(crate) message: String,
    pub(crate) detail: String,
}

impl SessionErrorPayload {
    pub(crate) fn new(
        kind: SessionErrorKind,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn report_pipeline(detail: impl Into<String>) -> Self {
        Self::new(
            SessionErrorKind::ReportPipeline,
            "The local report could not be prepared.",
            detail,
        )
    }

    pub(crate) fn resource(
        kind: SessionErrorKind,
        code: &str,
        stage: &str,
        limit: u64,
        observed: Option<u64>,
        unit: &str,
    ) -> Self {
        Self::new(
            kind,
            "The local operation was stopped by its resource policy.",
            format!(
                "code={code} stage={stage} limit={limit} observed={} unit={unit} complete=false",
                observed.map_or_else(|| "unknown".to_string(), |value| value.to_string())
            ),
        )
    }
}

#[derive(Debug, Error)]
enum OpenSessionError {
    #[error("selected directory is not a session bundle: {name}")]
    InvalidBundleSelection { name: String },
    #[error(transparent)]
    Storage(#[from] BundleStoreError),
    #[error(transparent)]
    Report(#[from] ReportError),
}

#[derive(Debug, Error)]
enum ExportSessionError {
    #[error("no active session is available to export")]
    NoActiveSession,
    #[error("selected destination is not a session bundle: {name}")]
    InvalidDestination { name: String },
    #[error(transparent)]
    Copy(#[from] BundleCopyError),
}

impl From<OpenSessionError> for SessionErrorPayload {
    fn from(error: OpenSessionError) -> Self {
        match error {
            OpenSessionError::InvalidBundleSelection { name } => Self::new(
                SessionErrorKind::Selection,
                "Choose a .session.antennabundle or .session.wsprabundle directory.",
                format!("Selected directory: {name}"),
            ),
            OpenSessionError::Storage(error) => storage_error_payload(error),
            OpenSessionError::Report(error) => report_error_payload(error),
        }
    }
}

impl From<ExportSessionError> for SessionErrorPayload {
    fn from(error: ExportSessionError) -> Self {
        match error {
            ExportSessionError::NoActiveSession => Self::new(
                SessionErrorKind::ReportPipeline,
                "Open a session bundle before exporting a copy.",
                "no active session is available",
            ),
            ExportSessionError::InvalidDestination { name } => Self::new(
                SessionErrorKind::Destination,
                "Keep the source bundle's .session.antennabundle or .session.wsprabundle suffix.",
                format!("Selected destination: {name}"),
            ),
            ExportSessionError::Copy(error) => copy_error_payload(error),
        }
    }
}

fn error_with_source(error: &dyn StdError) -> String {
    error
        .source()
        .map_or_else(|| error.to_string(), |source| format!("{error}: {source}"))
}

fn copy_error_payload(error: BundleCopyError) -> SessionErrorPayload {
    match error {
        BundleCopyError::Source { source } => storage_error_payload(source),
        BundleCopyError::DestinationExists { path } => SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "A file or directory already exists at that destination.",
            path.display().to_string(),
        ),
        BundleCopyError::DestinationInsideSource { path } => SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "Choose a destination outside the active source bundle.",
            path.display().to_string(),
        ),
        error @ (BundleCopyError::InspectDestination { .. }
        | BundleCopyError::CreateDestination { .. }
        | BundleCopyError::DestinationLayout { .. }) => SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "The export destination could not be prepared.",
            error_with_source(&error),
        ),
        BundleCopyError::Verification { source } => SessionErrorPayload::new(
            SessionErrorKind::Verification,
            "The exported copy did not pass verification and was removed.",
            error_with_source(&source),
        ),
        error @ BundleCopyError::UnsupportedSourceEntry { .. } => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The source contains an unsafe filesystem entry and was not exported.",
            error.to_string(),
        ),
        error @ BundleCopyError::CleanupFailed { .. } => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The export failed, and its incomplete destination could not be removed.",
            error_with_source(&error),
        ),
        error => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The session bundle could not be copied.",
            error_with_source(&error),
        ),
    }
}

pub(crate) fn storage_error_payload(error: BundleStoreError) -> SessionErrorPayload {
    match error {
        BundleStoreError::ParseJson { path, source } => SessionErrorPayload::new(
            SessionErrorKind::JsonParse,
            "A bundle file contains invalid JSON.",
            format!("{}: {source}", path.display()),
        ),
        BundleStoreError::Validation { source } => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The session bundle did not pass validation.",
            validation_error_detail(&source),
        ),
        error => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The session bundle could not be read.",
            error
                .source()
                .map_or_else(|| error.to_string(), |source| format!("{error}: {source}")),
        ),
    }
}

fn report_error_payload(error: ReportError) -> SessionErrorPayload {
    match error {
        ReportError::Analysis(AnalysisError::InvalidBundle(source)) => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The normalized session was not valid for reporting.",
            validation_error_detail(&source),
        ),
        ReportError::Analysis(error) => SessionErrorPayload::new(
            SessionErrorKind::Analysis,
            "The session evidence could not be analyzed.",
            error.to_string(),
        ),
        ReportError::Resource(error) => {
            let diagnostic = error.diagnostic;
            let kind = if diagnostic.code == "resource.operation.cancelled" {
                SessionErrorKind::Cancelled
            } else {
                SessionErrorKind::Resource
            };
            SessionErrorPayload::resource(
                kind,
                diagnostic.code,
                &format!("{:?}", diagnostic.stage),
                diagnostic.limit,
                diagnostic.observed,
                diagnostic.unit,
            )
        }
        ReportError::Serialization { message } => SessionErrorPayload::new(
            SessionErrorKind::ReportPipeline,
            "The local report could not be serialized.",
            message,
        ),
    }
}

fn validation_error_detail(source: &antennabench_core::BundleValidationError) -> String {
    const MAX_DISPLAYED_DIAGNOSTICS: usize = 5;
    let diagnostics = source
        .report()
        .diagnostics()
        .iter()
        .take(MAX_DISPLAYED_DIAGNOSTICS)
        .map(|diagnostic| {
            let field = diagnostic
                .location
                .field_path
                .as_deref()
                .map_or_else(String::new, |field| format!(" {field}"));
            format!(
                "{} at {:?}{field}",
                diagnostic.code, diagnostic.location.file
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let remaining = source
        .diagnostic_count()
        .saturating_sub(MAX_DISPLAYED_DIAGNOSTICS);
    let remainder = if remaining > 0 {
        format!("; and {remaining} more")
    } else {
        String::new()
    };
    format!(
        "{} validation issue(s): {diagnostics}{remainder}",
        source.diagnostic_count(),
    )
}

fn open_bundle(path: &Path) -> Result<ActiveSession, OpenSessionError> {
    let bundle_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| bundle_suffix(name).is_some())
        .ok_or_else(|| OpenSessionError::InvalidBundleSelection {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into(),
            ),
        })?
        .to_string();

    let (bundle, validation) = BundleStore::new(path).read_for_analysis()?;
    build_active_session(path.to_path_buf(), bundle_name, &bundle, &validation)
}

fn suggested_export_name(source: &Path) -> String {
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

fn export_bundle(source: &Path, destination: &Path) -> Result<String, ExportSessionError> {
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

    BundleStore::new(source).copy_losslessly_to(destination)?;
    Ok(bundle_name)
}

fn bundle_suffix(name: &str) -> Option<&'static str> {
    [V2_BUNDLE_SUFFIX, V1_BUNDLE_SUFFIX]
        .into_iter()
        .find(|suffix| name.ends_with(suffix))
}

fn build_active_session(
    source: PathBuf,
    bundle_name: String,
    bundle: &BundleContents,
    validation: &BundleValidationReport,
) -> Result<ActiveSession, OpenSessionError> {
    let report = build_report_with_validation(bundle, validation)?;
    let report_html = render_standalone_html(&report)?;

    Ok(ActiveSession {
        source,
        summary: OpenedSession {
            bundle_name,
            session_id: bundle.manifest.session_id.clone(),
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            antenna_count: bundle.antennas.antennas.len(),
            slot_count: bundle.schedule.slots.len(),
            observation_count: bundle.observations.len(),
        },
        report_html,
    })
}

pub(crate) fn activate_created_bundle(
    state: &ActiveSessionState,
    path: PathBuf,
) -> Result<OpenedSession, SessionErrorPayload> {
    let session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
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
    active.active = Some(session);
    active.export_source = Some(path);
    Ok(summary)
}

pub(crate) fn active_session_source(
    state: &ActiveSessionState,
) -> Result<(PathBuf, String), SessionErrorPayload> {
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.active.as_ref().ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Create or open a schema-v2 session before using the conductor.",
            "no active session is available",
        )
    })?;
    Ok((session.source.clone(), session.summary.bundle_name.clone()))
}

fn open_session_with_selection<F>(
    state: &ActiveSessionState,
    select: F,
) -> Result<OpenSessionOutcome, SessionErrorPayload>
where
    F: FnOnce() -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let _foreground = state.begin_foreground()?;
    let Some(path) = select()? else {
        return Ok(OpenSessionOutcome::Cancelled);
    };
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(bundle_suffix)
        .is_some()
    {
        state
            .0
            .lock()
            .map_err(|_| {
                SessionErrorPayload::report_pipeline("active session state is unavailable")
            })?
            .export_source = Some(path.clone());
    }
    let session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
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
    active.active = Some(session);
    active.export_source = Some(path);

    Ok(OpenSessionOutcome::Opened { session: summary })
}

fn export_active_session_with_selection<F>(
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
    let bundle_name = export_bundle(&source, &destination).map_err(SessionErrorPayload::from)?;

    Ok(ExportSessionOutcome::Exported { bundle_name })
}

fn active_session_report_for(state: &ActiveSessionState) -> Result<String, SessionErrorPayload> {
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

    if session.report_html.len() as u64 > REPORT_DOCUMENT_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            "report_document",
            REPORT_DOCUMENT_IPC_BYTES,
            Some(session.report_html.len() as u64),
            "bytes",
        ));
    }

    Ok(session.report_html.clone())
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
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    open_session_with_selection(state.inner(), || {
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
    })
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
pub(crate) fn active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<String, SessionErrorPayload> {
    active_session_report_for(state.inner())
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    use antennabench_analysis::AnalysisError;
    use antennabench_report::ReportError;
    use antennabench_storage::{BundleCopyError, BundleStoreError};
    use tempfile::TempDir;

    use super::{
        active_session_report_for, check_ipc_payload, copy_error_payload,
        export_active_session_with_selection, export_bundle, open_bundle,
        open_session_with_selection, report_error_payload, ActiveSessionState,
        ExportSessionOutcome, OpenSessionOutcome, SessionErrorKind, SessionErrorPayload,
        REPORT_DOCUMENT_IPC_BYTES,
    };

    fn canonical_fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle")
    }

    fn snapshot_files(root: &Path) -> io::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
        let mut snapshot = snapshot_files_from(root, root)?;
        snapshot.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(snapshot)
    }

    fn snapshot_files_from(
        root: &Path,
        current: &Path,
    ) -> io::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
        let mut snapshot = Vec::new();
        for entry in fs::read_dir(current)? {
            let path = entry?.path();
            if path.is_dir() {
                snapshot.extend(snapshot_files_from(root, &path)?);
            } else {
                snapshot.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&path)?,
                ));
            }
        }
        Ok(snapshot)
    }

    fn copy_fixture(temp: &TempDir) -> std::path::PathBuf {
        let target = temp.path().join("test.session.wsprabundle");
        copy_directory(&canonical_fixture(), &target).expect("copy canonical fixture");
        target
    }

    fn copy_directory(source: &Path, target: &Path) -> io::Result<()> {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let target_path = target.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_directory(&entry.path(), &target_path)?;
            } else {
                fs::copy(entry.path(), target_path)?;
            }
        }
        Ok(())
    }

    #[test]
    fn canonical_bundle_opens_without_mutating_its_source() {
        let fixture = canonical_fixture();
        let before = snapshot_files(&fixture).expect("snapshot fixture before open");

        let opened = open_bundle(&fixture).expect("open canonical fixture");

        assert_eq!(
            opened.summary.bundle_name,
            "canonical-sample-report.session.wsprabundle"
        );
        assert_eq!(
            opened.summary.session_id,
            "session-canonical-sample-2026-03-14"
        );
        assert!(!opened.summary.callsign.is_empty());
        assert!(opened.report_html.starts_with("<!doctype html>"));
        let payload = serde_json::to_value(super::OpenSessionOutcome::Opened {
            session: opened.summary.clone(),
        })
        .unwrap();
        assert!(payload["session"].get("reportHtml").is_none());
        assert_eq!(snapshot_files(&fixture).unwrap(), before);
    }

    #[test]
    fn selection_must_be_a_directory_bundle() {
        let error: SessionErrorPayload = open_bundle(Path::new("ordinary-directory"))
            .expect_err("reject an ordinary directory")
            .into();

        assert_eq!(error.kind, SessionErrorKind::Selection);
    }

    #[test]
    fn missing_bundle_is_a_filesystem_error() {
        let error: SessionErrorPayload = open_bundle(Path::new("missing.session.wsprabundle"))
            .expect_err("reject a missing bundle")
            .into();

        assert_eq!(error.kind, SessionErrorKind::Filesystem);
        assert!(error.detail.contains("manifest.json"));
    }

    #[test]
    fn malformed_bundle_json_has_a_specific_error_kind() {
        let temp = TempDir::new().unwrap();
        let bundle = copy_fixture(&temp);
        fs::write(bundle.join("station.json"), b"{not json").unwrap();

        let error: SessionErrorPayload = open_bundle(&bundle)
            .expect_err("reject malformed JSON")
            .into();

        assert_eq!(error.kind, SessionErrorKind::Validation);
        assert!(error.detail.contains("bundle.wire.invalid_json"));
        assert!(error.detail.contains("Station"));
    }

    #[test]
    fn invalid_bundle_has_a_specific_error_kind() {
        let temp = TempDir::new().unwrap();
        let bundle = copy_fixture(&temp);
        let station_path = bundle.join("station.json");
        let mut station: serde_json::Value =
            serde_json::from_slice(&fs::read(&station_path).unwrap()).unwrap();
        station["session_id"] = serde_json::Value::String("wrong-session".into());
        fs::write(&station_path, serde_json::to_vec_pretty(&station).unwrap()).unwrap();

        let error: SessionErrorPayload = open_bundle(&bundle)
            .expect_err("reject invalid bundle")
            .into();

        assert_eq!(error.kind, SessionErrorKind::Validation);
        assert!(error.detail.contains("validation issue"));
    }

    #[test]
    fn analysis_and_report_pipeline_failures_are_typed() {
        let analysis = report_error_payload(ReportError::Analysis(AnalysisError::NonFiniteSnr {
            observation_id: "observation-7".into(),
        }));
        let pipeline = SessionErrorPayload::report_pipeline("renderer unavailable");

        assert_eq!(analysis.kind, SessionErrorKind::Analysis);
        assert!(analysis.detail.contains("observation-7"));
        assert_eq!(pipeline.kind, SessionErrorKind::ReportPipeline);
    }

    #[test]
    fn desktop_busy_and_ipc_boundaries_are_typed_at_n_minus_one_n_and_n_plus_one() {
        let state = ActiveSessionState::default();
        let guard = state.begin_foreground().unwrap();
        let busy = active_session_report_for(&state).unwrap_err();
        assert_eq!(busy.kind, SessionErrorKind::Busy);
        assert!(busy.detail.contains("resource.operation.busy"));
        drop(guard);

        let payload = "bounded-payload";
        let bytes = serde_json::to_vec(payload).unwrap().len() as u64;
        let below = check_ipc_payload(&payload, bytes - 1, "test_summary").unwrap_err();
        assert_eq!(below.kind, SessionErrorKind::Resource);
        assert!(below.detail.contains("resource.desktop.ipc_bytes"));
        check_ipc_payload(&payload, bytes, "test_summary").unwrap();
        check_ipc_payload(&payload, bytes + 1, "test_summary").unwrap();

        open_session_with_selection(&state, || Ok(Some(canonical_fixture()))).unwrap();
        state.0.lock().unwrap().active.as_mut().unwrap().report_html =
            "x".repeat(REPORT_DOCUMENT_IPC_BYTES as usize + 1);
        let oversized = active_session_report_for(&state).unwrap_err();
        assert_eq!(oversized.kind, SessionErrorKind::Resource);
        assert!(oversized.detail.contains("resource.desktop.ipc_bytes"));
        assert!(oversized.detail.contains("report_document"));
    }

    #[test]
    fn exported_copy_reopens_through_the_desktop_import_path() {
        let temp = TempDir::new().unwrap();
        let source = copy_fixture(&temp);
        let opened = open_bundle(&source).expect("open source bundle");
        let destination = temp.path().join("exported.session.wsprabundle");

        let bundle_name = export_bundle(&source, &destination).expect("export source bundle");
        let reopened = open_bundle(&destination).expect("reopen exported bundle");
        let mut expected = opened.summary;
        expected.bundle_name = bundle_name.clone();

        assert_eq!(bundle_name, "exported.session.wsprabundle");
        assert_eq!(reopened.summary, expected);
    }

    #[test]
    fn export_destination_and_verification_failures_are_typed() {
        let destination: SessionErrorPayload = super::ExportSessionError::InvalidDestination {
            name: "export-directory".into(),
        }
        .into();
        let verification = copy_error_payload(BundleCopyError::Verification {
            source: BundleStoreError::InvalidBundleRoot {
                path: "exported.session.wsprabundle".into(),
            },
        });

        assert_eq!(destination.kind, SessionErrorKind::Destination);
        assert_eq!(verification.kind, SessionErrorKind::Verification);
    }

    #[test]
    fn lossless_export_is_available_without_a_derived_report() {
        let temp = TempDir::new().unwrap();
        let source = copy_fixture(&temp);
        let destination = temp.path().join("resource-safe-copy.session.wsprabundle");
        let state = ActiveSessionState::default();
        state.0.lock().unwrap().export_source = Some(source.clone());

        let outcome = export_active_session_with_selection(&state, |selected| {
            assert_eq!(selected, source);
            Ok(Some(destination.clone()))
        })
        .expect("storage-safe export does not require report eligibility");

        assert_eq!(
            outcome,
            ExportSessionOutcome::Exported {
                bundle_name: "resource-safe-copy.session.wsprabundle".into()
            }
        );
        assert_eq!(
            snapshot_files(&source).unwrap(),
            snapshot_files(&destination).unwrap()
        );
        assert!(active_session_report_for(&state).is_err());
    }

    #[test]
    fn desktop_e2e_canonical_workflow_is_lossless_and_non_mutating() {
        let temp = TempDir::new().expect("create isolated desktop workflow directory");
        let source = copy_fixture(&temp);
        let destination = temp.path().join("exported.session.wsprabundle");
        let before = snapshot_files(&source).expect("snapshot source before desktop workflow");
        let state = ActiveSessionState::default();

        println!("desktop-e2e phase=open source={}", source.display());
        let opened = open_session_with_selection(&state, || Ok(Some(source.clone())))
            .expect("open canonical source through desktop orchestration");
        let OpenSessionOutcome::Opened { session } = opened else {
            panic!("deterministic source selection unexpectedly cancelled");
        };
        assert_eq!(session.session_id, "session-canonical-sample-2026-03-14");

        println!("desktop-e2e phase=report session_id={}", session.session_id);
        let source_report = active_session_report_for(&state)
            .expect("derive active report through desktop orchestration");
        assert!(source_report.starts_with("<!doctype html>"));

        println!(
            "desktop-e2e phase=export destination={}",
            destination.display()
        );
        let exported = export_active_session_with_selection(&state, |active_source| {
            assert_eq!(active_source, source);
            Ok(Some(destination.clone()))
        })
        .expect("export canonical source through desktop orchestration");
        assert_eq!(
            exported,
            ExportSessionOutcome::Exported {
                bundle_name: "exported.session.wsprabundle".into()
            }
        );
        assert_eq!(
            snapshot_files(&destination).expect("snapshot exported desktop bundle"),
            before,
            "exported tree and file bytes must equal the selected source"
        );
        assert_eq!(
            snapshot_files(&source).expect("snapshot source after desktop export"),
            before,
            "the desktop workflow must not mutate its source"
        );

        println!(
            "desktop-e2e phase=reopen destination={}",
            destination.display()
        );
        let reopened = open_session_with_selection(&state, || Ok(Some(destination.clone())))
            .expect("reopen exported bundle through desktop orchestration");
        let OpenSessionOutcome::Opened {
            session: reopened_session,
        } = reopened
        else {
            panic!("deterministic exported selection unexpectedly cancelled");
        };
        assert_eq!(reopened_session.session_id, session.session_id);
        assert_eq!(
            active_session_report_for(&state).expect("view report after exported bundle reopen"),
            source_report
        );
        assert_eq!(
            snapshot_files(&source).expect("snapshot source after exported bundle reopen"),
            before,
            "reopening the export must not mutate the original source"
        );
        println!("desktop-e2e result=passed files={}", before.len());
    }

    #[test]
    fn desktop_e2e_cancellation_is_a_normal_outcome() {
        let state = ActiveSessionState::default();
        assert_eq!(
            open_session_with_selection(&state, || Ok(None)).expect("cancel opening"),
            OpenSessionOutcome::Cancelled
        );

        let fixture = canonical_fixture();
        open_session_with_selection(&state, || Ok(Some(fixture)))
            .expect("open canonical source before cancellation checks");
        let report = active_session_report_for(&state).expect("capture active report");
        assert_eq!(
            open_session_with_selection(&state, || Ok(None)).expect("cancel replacement open"),
            OpenSessionOutcome::Cancelled
        );
        assert_eq!(
            export_active_session_with_selection(&state, |_| Ok(None))
                .expect("cancel active export"),
            ExportSessionOutcome::Cancelled
        );
        assert_eq!(
            active_session_report_for(&state).expect("retain report after cancellations"),
            report
        );
        println!("desktop-e2e result=cancelled-normal active_report=retained");
    }

    #[test]
    fn desktop_e2e_failure_is_typed_and_diagnostic() {
        let temp = TempDir::new().expect("create isolated invalid desktop fixture");
        let source = copy_fixture(&temp);
        fs::write(source.join("station.json"), b"{not json")
            .expect("make desktop fixture deterministically invalid");
        let state = ActiveSessionState::default();

        let error = open_session_with_selection(&state, || Ok(Some(source.clone())))
            .expect_err("reject invalid JSON through desktop orchestration");

        println!(
            "desktop-e2e result=typed-failure kind={:?} detail={}",
            error.kind, error.detail
        );
        assert_eq!(error.kind, SessionErrorKind::Validation);
        assert!(error.detail.contains("bundle.wire.invalid_json"));
        assert!(error.detail.contains("Station"));
        assert!(active_session_report_for(&state).is_err());
    }
}
