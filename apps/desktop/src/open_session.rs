use std::{
    error::Error as StdError,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_analysis::AnalysisError;
use antennabench_core::BundleContents;
use antennabench_report::{build_report, render_standalone_html, ReportError};
use antennabench_storage::{BundleStore, BundleStoreError};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OpenSessionErrorKind {
    Selection,
    Filesystem,
    JsonParse,
    Validation,
    Analysis,
    ReportPipeline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct OpenSessionErrorPayload {
    kind: OpenSessionErrorKind,
    message: String,
    detail: String,
}

impl OpenSessionErrorPayload {
    fn new(
        kind: OpenSessionErrorKind,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: detail.into(),
        }
    }

    fn report_pipeline(detail: impl Into<String>) -> Self {
        Self::new(
            OpenSessionErrorKind::ReportPipeline,
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

impl From<OpenSessionError> for OpenSessionErrorPayload {
    fn from(error: OpenSessionError) -> Self {
        match error {
            OpenSessionError::InvalidBundleSelection { name } => Self::new(
                OpenSessionErrorKind::Selection,
                "Choose a .session.wsprabundle directory.",
                format!("Selected directory: {name}"),
            ),
            OpenSessionError::Storage(error) => storage_error_payload(error),
            OpenSessionError::Report(error) => report_error_payload(error),
        }
    }
}

fn storage_error_payload(error: BundleStoreError) -> OpenSessionErrorPayload {
    match error {
        BundleStoreError::ParseJson { path, source } => OpenSessionErrorPayload::new(
            OpenSessionErrorKind::JsonParse,
            "A bundle file contains invalid JSON.",
            format!("{}: {source}", path.display()),
        ),
        BundleStoreError::Validation { source } => OpenSessionErrorPayload::new(
            OpenSessionErrorKind::Validation,
            "The session bundle did not pass validation.",
            format!("{} validation issue(s): {source}", source.issues().len()),
        ),
        error => OpenSessionErrorPayload::new(
            OpenSessionErrorKind::Filesystem,
            "The session bundle could not be read.",
            error
                .source()
                .map_or_else(|| error.to_string(), |source| format!("{error}: {source}")),
        ),
    }
}

fn report_error_payload(error: ReportError) -> OpenSessionErrorPayload {
    match error {
        ReportError::Analysis(AnalysisError::InvalidBundle(source)) => {
            OpenSessionErrorPayload::new(
                OpenSessionErrorKind::Validation,
                "The normalized session was not valid for reporting.",
                format!("{} validation issue(s): {source}", source.issues().len()),
            )
        }
        ReportError::Analysis(error) => OpenSessionErrorPayload::new(
            OpenSessionErrorKind::Analysis,
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
) -> Result<OpenSessionOutcome, OpenSessionErrorPayload> {
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
        OpenSessionErrorPayload::new(
            OpenSessionErrorKind::Selection,
            "The selected directory is not available as a local path.",
            error.to_string(),
        )
    })?;
    let session = open_bundle(&path).map_err(OpenSessionErrorPayload::from)?;
    let summary = session.summary.clone();

    let mut active = state.0.lock().map_err(|_| {
        OpenSessionErrorPayload::report_pipeline("active session state is unavailable")
    })?;
    *active = Some(session);

    Ok(OpenSessionOutcome::Opened { session: summary })
}

#[tauri::command]
pub(crate) fn active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<String, OpenSessionErrorPayload> {
    let active = state.0.lock().map_err(|_| {
        OpenSessionErrorPayload::report_pipeline("active session state is unavailable")
    })?;
    let session = active.as_ref().ok_or_else(|| {
        OpenSessionErrorPayload::report_pipeline("no active session report is available")
    })?;

    let source_name = session
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            OpenSessionErrorPayload::report_pipeline(
                "the active session source name is unavailable",
            )
        })?;
    if source_name != session.summary.bundle_name {
        return Err(OpenSessionErrorPayload::report_pipeline(
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
    use tempfile::TempDir;

    use super::{open_bundle, report_error_payload, OpenSessionErrorKind, OpenSessionErrorPayload};

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
        let error: OpenSessionErrorPayload = open_bundle(Path::new("ordinary-directory"))
            .expect_err("reject an ordinary directory")
            .into();

        assert_eq!(error.kind, OpenSessionErrorKind::Selection);
    }

    #[test]
    fn missing_bundle_is_a_filesystem_error() {
        let error: OpenSessionErrorPayload = open_bundle(Path::new("missing.session.wsprabundle"))
            .expect_err("reject a missing bundle")
            .into();

        assert_eq!(error.kind, OpenSessionErrorKind::Filesystem);
        assert!(error.detail.contains("manifest.json"));
    }

    #[test]
    fn malformed_bundle_json_has_a_specific_error_kind() {
        let temp = TempDir::new().unwrap();
        let bundle = copy_fixture(&temp);
        fs::write(bundle.join("station.json"), b"{not json").unwrap();

        let error: OpenSessionErrorPayload = open_bundle(&bundle)
            .expect_err("reject malformed JSON")
            .into();

        assert_eq!(error.kind, OpenSessionErrorKind::JsonParse);
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

        let error: OpenSessionErrorPayload = open_bundle(&bundle)
            .expect_err("reject invalid bundle")
            .into();

        assert_eq!(error.kind, OpenSessionErrorKind::Validation);
        assert!(error.detail.contains("validation issue"));
    }

    #[test]
    fn analysis_and_report_pipeline_failures_are_typed() {
        let analysis = report_error_payload(ReportError::Analysis(AnalysisError::NonFiniteSnr {
            observation_id: "observation-7".into(),
        }));
        let pipeline = OpenSessionErrorPayload::report_pipeline("renderer unavailable");

        assert_eq!(analysis.kind, OpenSessionErrorKind::Analysis);
        assert!(analysis.detail.contains("observation-7"));
        assert_eq!(pipeline.kind, OpenSessionErrorKind::ReportPipeline);
    }
}
