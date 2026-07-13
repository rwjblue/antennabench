use std::{
    error::Error as StdError,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_analysis::AnalysisError;
use antennabench_core::BundleContents;
use antennabench_report::{build_report, render_standalone_html, ReportError};
use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

const BUNDLE_SUFFIX: &str = ".session.wsprabundle";

#[derive(Default)]
pub(crate) struct ActiveSessionState(Mutex<Option<ActiveSession>>);

#[derive(Debug)]
struct ActiveSession {
    source: PathBuf,
    summary: OpenedSession,
    report_html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenedSession {
    bundle_name: String,
    session_id: String,
    callsign: String,
    grid: String,
    antenna_count: usize,
    slot_count: usize,
    observation_count: usize,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SessionErrorPayload {
    kind: SessionErrorKind,
    message: String,
    detail: String,
}

impl SessionErrorPayload {
    fn new(kind: SessionErrorKind, message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: detail.into(),
        }
    }

    fn report_pipeline(detail: impl Into<String>) -> Self {
        Self::new(
            SessionErrorKind::ReportPipeline,
            "The local report could not be prepared.",
            detail,
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
                "Choose a .session.wsprabundle directory.",
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
                "Save the copy as a .session.wsprabundle directory.",
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

fn storage_error_payload(error: BundleStoreError) -> SessionErrorPayload {
    match error {
        BundleStoreError::ParseJson { path, source } => SessionErrorPayload::new(
            SessionErrorKind::JsonParse,
            "A bundle file contains invalid JSON.",
            format!("{}: {source}", path.display()),
        ),
        BundleStoreError::Validation { source } => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The session bundle did not pass validation.",
            format!("{} validation issue(s): {source}", source.issues().len()),
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
            format!("{} validation issue(s): {source}", source.issues().len()),
        ),
        ReportError::Analysis(error) => SessionErrorPayload::new(
            SessionErrorKind::Analysis,
            "The session evidence could not be analyzed.",
            error.to_string(),
        ),
    }
}

fn open_bundle(path: &Path) -> Result<ActiveSession, OpenSessionError> {
    let bundle_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(BUNDLE_SUFFIX))
        .ok_or_else(|| OpenSessionError::InvalidBundleSelection {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into(),
            ),
        })?
        .to_string();

    let bundle = BundleStore::new(path).read_normalized_validated()?;
    build_active_session(path.to_path_buf(), bundle_name, &bundle)
}

fn suggested_export_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(BUNDLE_SUFFIX))
        .map_or_else(
            || format!("session-copy{BUNDLE_SUFFIX}"),
            |stem| format!("{stem}-copy{BUNDLE_SUFFIX}"),
        )
}

fn export_bundle(source: &Path, destination: &Path) -> Result<String, ExportSessionError> {
    let bundle_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(BUNDLE_SUFFIX))
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

fn build_active_session(
    source: PathBuf,
    bundle_name: String,
    bundle: &BundleContents,
) -> Result<ActiveSession, OpenSessionError> {
    let report = build_report(bundle)?;
    let report_html = render_standalone_html(&report);

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

#[tauri::command]
pub(crate) async fn open_session_bundle(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    let Some(selection) = app
        .dialog()
        .file()
        .set_title("Open an AntennaBench session bundle")
        .set_can_create_directories(false)
        .blocking_pick_folder()
    else {
        return Ok(OpenSessionOutcome::Cancelled);
    };

    let path = selection.into_path().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "The selected directory is not available as a local path.",
            error.to_string(),
        )
    })?;
    let session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
    let summary = session.summary.clone();

    let mut active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    *active = Some(session);

    Ok(OpenSessionOutcome::Opened { session: summary })
}

#[tauri::command]
pub(crate) async fn export_active_session(
    app: AppHandle,
    state: State<'_, ActiveSessionState>,
) -> Result<ExportSessionOutcome, SessionErrorPayload> {
    let source = {
        let active = state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("active session state is unavailable")
        })?;
        active
            .as_ref()
            .map(|session| session.source.clone())
            .ok_or(ExportSessionError::NoActiveSession)
            .map_err(SessionErrorPayload::from)?
    };

    let mut dialog = app
        .dialog()
        .file()
        .set_title("Export an AntennaBench session bundle copy")
        .set_file_name(suggested_export_name(&source))
        .set_can_create_directories(true)
        .add_filter("AntennaBench session bundle", &["wsprabundle"]);
    if let Some(parent) = source.parent() {
        dialog = dialog.set_directory(parent);
    }

    let Some(selection) = dialog.blocking_save_file() else {
        return Ok(ExportSessionOutcome::Cancelled);
    };
    let destination = selection.into_path().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "The selected destination is not available as a local path.",
            error.to_string(),
        )
    })?;
    let bundle_name = export_bundle(&source, &destination).map_err(SessionErrorPayload::from)?;

    Ok(ExportSessionOutcome::Exported { bundle_name })
}

#[tauri::command]
pub(crate) fn active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<String, SessionErrorPayload> {
    let active = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.as_ref().ok_or_else(|| {
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

    Ok(session.report_html.clone())
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    use antennabench_analysis::AnalysisError;
    use antennabench_report::ReportError;
    use antennabench_storage::{BundleCopyError, BundleStoreError};
    use tempfile::TempDir;

    use super::{
        copy_error_payload, export_bundle, open_bundle, report_error_payload, SessionErrorKind,
        SessionErrorPayload,
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

        assert_eq!(error.kind, SessionErrorKind::JsonParse);
        assert!(error.detail.contains("station.json"));
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
}
