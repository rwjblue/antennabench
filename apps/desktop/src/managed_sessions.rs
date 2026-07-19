use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use antennabench_core::{v2::SessionLifecycleV2, Band, ExperimentMode};
use antennabench_storage::CatalogDirectionCoverage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    antenna_control::AntennaControllerState,
    open_session::{
        finish_open_side_effects, open_session_at_path_verified, ActiveSessionState,
        OpenSessionOutcome, SessionErrorKind, SessionErrorPayload,
    },
    setup::{managed_sessions_dir, prepare_managed_sessions_dir, resolved_app_data_dir},
    wsjtx_session::WsjtxSessionState,
};

mod catalog;

use catalog::{
    build_catalog, record_created, revalidate_available, revalidate_direct_child, CatalogBuild,
    LocatorRecord,
};

pub(crate) const MAX_CATALOG_CANDIDATES: usize = 256;
pub(crate) const MANAGED_CATALOG_IPC_BYTES: usize = 512 * 1024;
pub(crate) const MAX_ENTRY_PROBLEMS: usize = 8;
pub(crate) const MAX_PROBLEM_TEXT_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CatalogStatus {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogDiagnostic {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManagedEntryStatus {
    Available,
    Invalid,
    Unsupported,
    Unreadable,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManagedEntryOrigin {
    Managed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProblemSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedSessionProblem {
    code: String,
    severity: ProblemSeverity,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManagedDirectionCoverage {
    TransmitOnly,
    ReceiveOnly,
    TransmitAndReceive,
    Unknown,
}

impl From<CatalogDirectionCoverage> for ManagedDirectionCoverage {
    fn from(value: CatalogDirectionCoverage) -> Self {
        match value {
            CatalogDirectionCoverage::TransmitOnly => Self::TransmitOnly,
            CatalogDirectionCoverage::ReceiveOnly => Self::ReceiveOnly,
            CatalogDirectionCoverage::TransmitAndReceive => Self::TransmitAndReceive,
            CatalogDirectionCoverage::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedObservationCounts {
    total: usize,
    local_decodes: usize,
    public_spots: usize,
    imported_spots: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedCatalogEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    locator_id: Option<String>,
    bundle_name: String,
    origin: ManagedEntryOrigin,
    origin_label: String,
    status: ManagedEntryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callsign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lifecycle: Option<SessionLifecycleV2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema_version: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<ExperimentMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    planned_repetitions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    direction_coverage: Option<ManagedDirectionCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    planned_cycle_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observation_counts: Option<ManagedObservationCounts>,
    bands: Vec<Band>,
    antenna_labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    antenna_count: Option<usize>,
    same_session_id_count: usize,
    problems: Vec<ManagedSessionProblem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedSessionCatalog {
    status: CatalogStatus,
    entries: Vec<ManagedCatalogEntry>,
    diagnostics: Vec<CatalogDiagnostic>,
}

impl ManagedSessionCatalog {
    fn complete(entries: Vec<ManagedCatalogEntry>) -> Self {
        Self {
            status: CatalogStatus::Complete,
            entries,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedLocationContext {
    locator_id: String,
    bundle_name: String,
    origin: ManagedEntryOrigin,
    origin_label: String,
}

#[derive(Default)]
pub(crate) struct ManagedSessionsState(Mutex<HashMap<String, LocatorRecord>>);

impl ManagedSessionsState {
    fn replace_catalog(
        &self,
        CatalogBuild {
            mut catalog,
            records,
        }: CatalogBuild,
    ) -> Result<ManagedSessionCatalog, SessionErrorPayload> {
        let mut locators = HashMap::new();
        for (entry, record) in catalog.entries.iter_mut().zip(records) {
            if record.bundle_name.is_empty() {
                continue;
            }
            let locator_id = Uuid::new_v4().to_string();
            entry.locator_id = Some(locator_id.clone());
            locators.insert(locator_id, record);
        }
        fit_catalog_to_ipc(&mut catalog, &mut locators)?;
        *self.0.lock().map_err(|_| state_error())? = locators;
        Ok(catalog)
    }

    fn resolve(&self, locator_id: &str) -> Result<LocatorRecord, SessionErrorPayload> {
        self.0
            .lock()
            .map_err(|_| state_error())?
            .get(locator_id)
            .cloned()
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::Selection,
                    "Refresh Saved sessions and choose the entry again.",
                    "the managed locator is unknown or stale",
                )
            })
    }

    pub(crate) fn register_created(
        &self,
        root: &Path,
        path: &Path,
    ) -> Result<ManagedLocationContext, SessionErrorPayload> {
        let record = record_created(root, path)?;
        let locator_id = Uuid::new_v4().to_string();
        let bundle_name = record.bundle_name.clone();
        self.0
            .lock()
            .map_err(|_| state_error())?
            .insert(locator_id.clone(), record);
        Ok(ManagedLocationContext {
            locator_id,
            bundle_name,
            origin: ManagedEntryOrigin::Managed,
            origin_label: "Saved by AntennaBench".into(),
        })
    }
}

#[tauri::command]
pub(crate) fn list_managed_sessions(
    app: AppHandle,
    state: State<'_, ManagedSessionsState>,
) -> Result<ManagedSessionCatalog, SessionErrorPayload> {
    let root = managed_sessions_dir(&resolved_app_data_dir(&app)?);
    list_managed_sessions_for(state.inner(), &root)
}

#[tauri::command]
pub(crate) async fn open_managed_session(
    app: AppHandle,
    managed_state: State<'_, ManagedSessionsState>,
    active_state: State<'_, ActiveSessionState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    locator_id: String,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    let root = managed_sessions_dir(&resolved_app_data_dir(&app)?);
    let outcome = open_managed_session_for(
        managed_state.inner(),
        active_state.inner(),
        &root,
        &locator_id,
    )?;
    finish_open_side_effects(controller_state.inner(), wsjtx_state.inner());
    Ok(outcome)
}

#[tauri::command]
pub(crate) fn reveal_managed_sessions_directory(app: AppHandle) -> Result<(), SessionErrorPayload> {
    let app_data = resolved_app_data_dir(&app)?;
    let root = prepare_managed_sessions_dir(&app_data)?;
    reveal_with(&SystemRevealPort, RevealTarget::Directory(root))
}

#[tauri::command]
pub(crate) fn reveal_managed_session(
    app: AppHandle,
    state: State<'_, ManagedSessionsState>,
    locator_id: String,
) -> Result<(), SessionErrorPayload> {
    let root = managed_sessions_dir(&resolved_app_data_dir(&app)?);
    let record = state.resolve(&locator_id)?;
    let path = revalidate_direct_child(&root, &record)?;
    reveal_with(&SystemRevealPort, RevealTarget::Entry(path))
}

fn list_managed_sessions_for(
    state: &ManagedSessionsState,
    root: &Path,
) -> Result<ManagedSessionCatalog, SessionErrorPayload> {
    state.replace_catalog(build_catalog(root)?)
}

fn open_managed_session_for(
    managed_state: &ManagedSessionsState,
    active_state: &ActiveSessionState,
    root: &Path,
    locator_id: &str,
) -> Result<OpenSessionOutcome, SessionErrorPayload> {
    let record = managed_state.resolve(locator_id)?;
    let path = revalidate_available(root, &record)?;
    open_session_at_path_verified(active_state, path.clone(), |opened_path| {
        let verified_path = revalidate_available(root, &record)?;
        if verified_path == opened_path {
            Ok(())
        } else {
            Err(SessionErrorPayload::new(
                SessionErrorKind::Verification,
                "The saved session changed while it was opening.",
                "the managed source no longer resolves to the opened direct child",
            ))
        }
    })
    .map_err(|error| redact_managed_path(error, root))
}

fn redact_managed_path(mut error: SessionErrorPayload, root: &Path) -> SessionErrorPayload {
    if let Some(root) = root.to_str() {
        error.detail = error.detail.replace(root, "Saved sessions");
    }
    error
}

fn fit_catalog_to_ipc(
    catalog: &mut ManagedSessionCatalog,
    locators: &mut HashMap<String, LocatorRecord>,
) -> Result<(), SessionErrorPayload> {
    fit_catalog_to_ipc_with_limit(catalog, locators, MANAGED_CATALOG_IPC_BYTES)
}

fn fit_catalog_to_ipc_with_limit(
    catalog: &mut ManagedSessionCatalog,
    locators: &mut HashMap<String, LocatorRecord>,
    limit: usize,
) -> Result<(), SessionErrorPayload> {
    let mut bytes = serialized_len(catalog)?;
    if bytes <= limit {
        return Ok(());
    }
    catalog.status = CatalogStatus::Incomplete;
    catalog.diagnostics.push(CatalogDiagnostic {
        code: "resource.desktop.managed_catalog_ipc_bytes".into(),
        message: "The managed session catalog was shortened to fit the desktop IPC limit.".into(),
        limit: Some(limit as u64),
        observed: Some(bytes as u64),
    });
    while bytes > limit {
        let Some(removed) = catalog.entries.pop() else {
            return Err(SessionErrorPayload::resource(
                SessionErrorKind::Resource,
                "resource.desktop.managed_catalog_ipc_bytes",
                "managed_session_catalog",
                limit as u64,
                Some(bytes as u64),
                "bytes",
            ));
        };
        if let Some(locator_id) = removed.locator_id {
            locators.remove(&locator_id);
        }
        bytes = serialized_len(catalog)?;
    }
    Ok(())
}

fn serialized_len(value: &impl Serialize) -> Result<usize, SessionErrorPayload> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| {
            SessionErrorPayload::report_pipeline(format!(
                "managed catalog IPC serialization failed: {error}"
            ))
        })
}

fn is_supported_bundle_name(name: &str) -> bool {
    use antennabench_core::v2::{V1_BUNDLE_SUFFIX, V2_BUNDLE_SUFFIX};
    name.ends_with(V2_BUNDLE_SUFFIX) || name.ends_with(V1_BUNDLE_SUFFIX)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RevealTarget {
    Directory(PathBuf),
    Entry(PathBuf),
}

trait RevealPort {
    fn reveal(&self, target: &RevealTarget) -> std::io::Result<()>;
}

struct SystemRevealPort;

impl RevealPort for SystemRevealPort {
    fn reveal(&self, target: &RevealTarget) -> std::io::Result<()> {
        let status = reveal_command(target).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "the platform reveal command exited with {status}"
            )))
        }
    }
}

#[cfg(target_os = "macos")]
fn reveal_command(target: &RevealTarget) -> Command {
    let mut command = Command::new("/usr/bin/open");
    match target {
        RevealTarget::Directory(path) => {
            command.arg(path);
        }
        RevealTarget::Entry(path) => {
            command.arg("-R").arg(path);
        }
    }
    command
}

#[cfg(target_os = "windows")]
fn reveal_command(target: &RevealTarget) -> Command {
    let mut command = Command::new("explorer.exe");
    match target {
        RevealTarget::Directory(path) => {
            command.arg(path);
        }
        RevealTarget::Entry(path) => {
            command.arg(format!("/select,{}", path.display()));
        }
    }
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn reveal_command(target: &RevealTarget) -> Command {
    let mut command = Command::new("xdg-open");
    match target {
        RevealTarget::Directory(path) => command.arg(path),
        RevealTarget::Entry(path) => command.arg(path.parent().unwrap_or(path)),
    };
    command
}

fn reveal_with(port: &impl RevealPort, target: RevealTarget) -> Result<(), SessionErrorPayload> {
    port.reveal(&target).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The managed session location could not be revealed.",
            error.to_string(),
        )
    })
}

fn state_error() -> SessionErrorPayload {
    SessionErrorPayload::report_pipeline("managed session locator state is unavailable")
}

#[cfg(test)]
mod tests;
