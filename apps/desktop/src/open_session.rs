use std::{
    error::Error as StdError,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_analysis::AnalysisError;
use antennabench_core::{
    normalize_bundle, project_wspr_run_v3, AdapterDisposition, AdapterInput, AdapterRecordV2, Band,
    BundleContents, BundleV3Contents, BundleValidationError, BundleValidationReport,
    OperatorEventPayloadV2, OperatorEventPayloadV3, SessionLifecycleV2, WsprReadinessBasisV5,
    SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, V1_BUNDLE_SUFFIX,
    V2_BUNDLE_SUFFIX,
};
use antennabench_report::{
    build_report_with_snapshot, render_standalone_html, ReportAdapterEvidence,
    ReportAntennaControlAttempt, ReportCompleteness, ReportError, ReportImportedEvidence,
    ReportLifecycleEvent, ReportLifecycleEventKind, ReportSnapshotContext, ReportWsprAttribution,
    ReportWsprCycle, ReportWsprReadinessBasis,
};
use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError, LivePersistenceError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

use crate::wsjtx_session::WsjtxSessionState;

const SESSION_SUMMARY_IPC_BYTES: u64 = 64 * 1024;
const REPORT_DOCUMENT_IPC_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct ActiveSessionState(Mutex<DesktopState>);

#[derive(Default)]
struct DesktopState {
    active: Option<ActiveSession>,
    export_source: Option<PathBuf>,
    foreground_busy: bool,
    next_presentation_id: u64,
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

pub(crate) fn reload_active_session(
    state: &ActiveSessionState,
    source: &Path,
) -> Result<OpenedSession, SessionErrorPayload> {
    let refreshed = open_bundle(source).map_err(SessionErrorPayload::from)?;
    let summary = refreshed.summary.clone();
    let mut desktop = state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    if desktop
        .active
        .as_ref()
        .is_some_and(|session| session.source != source)
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The active session changed while WSPR.live evidence was importing.",
            "the import remains committed to the originally selected session",
        ));
    }
    desktop.active = Some(refreshed);
    Ok(summary)
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
    presentation: Option<ReportPresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportPresentation {
    presentation_id: u64,
    session_id: String,
    revision: Option<u64>,
    lifecycle: Option<SessionLifecycleV2>,
    completeness: ReportCompleteness,
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
    pub(crate) schema_version: u16,
    pub(crate) revision: Option<u64>,
    pub(crate) lifecycle: Option<SessionLifecycleV2>,
    pub(crate) report_available: bool,
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
        revision: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ExportReportOutcome {
    Cancelled,
    Exported {
        #[serde(rename = "fileName")]
        file_name: String,
        revision: Option<u64>,
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
    Live(#[from] LivePersistenceError),
    #[error("the committed snapshot changed while its layered diagnostics were being read")]
    SnapshotChanged,
}

#[derive(Debug, Error)]
enum ExportSessionError {
    #[error("no active session is available to export")]
    NoActiveSession,
    #[error("selected destination is not a session bundle: {name}")]
    InvalidDestination { name: String },
    #[error(transparent)]
    Copy(#[from] BundleCopyError),
    #[error(transparent)]
    Live(#[from] LivePersistenceError),
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
            OpenSessionError::Live(error) => crate::conductor::live_error_payload(error),
            OpenSessionError::SnapshotChanged => Self::new(
                SessionErrorKind::StaleRevision,
                "The session changed while its coherent snapshot was being prepared.",
                "the prior presentation remains available; retry the refresh",
            ),
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
            ExportSessionError::Live(error) => crate::conductor::live_error_payload(error),
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

struct LoadedSnapshot {
    bundle: BundleContents,
    intended_cycle_count: usize,
    schema_version: u16,
    validation: BundleValidationReport,
    report_snapshot: ReportSnapshotContext,
    revision: Option<u64>,
    lifecycle: Option<SessionLifecycleV2>,
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

    let snapshot = load_snapshot(path, &bundle_name)?;
    Ok(build_active_session(
        path.to_path_buf(),
        bundle_name,
        snapshot,
    ))
}

fn load_snapshot(path: &Path, bundle_name: &str) -> Result<LoadedSnapshot, OpenSessionError> {
    let store = BundleStore::new(path);
    if bundle_name.ends_with(V2_BUNDLE_SUFFIX) {
        let schema_version = store.schema_version()?;
        let (current, revision, lifecycle, report_snapshot, intended_cycle_count) =
            match schema_version {
                SCHEMA_VERSION_V2 => {
                    let bundle = store.read_v2_checkpointed()?;
                    let revision = bundle.session_state.revision;
                    let lifecycle = bundle.session_state.lifecycle;
                    let report_snapshot = report_snapshot(&bundle);
                    let intended_cycle_count = bundle.schedule.slots.len();
                    (
                        bundle.into_current(),
                        revision,
                        lifecycle,
                        report_snapshot,
                        intended_cycle_count,
                    )
                }
                SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => {
                    let bundle = store.read_v3_checkpointed()?;
                    let revision = bundle.session_state.revision;
                    let lifecycle = bundle.session_state.lifecycle;
                    let report_snapshot = report_snapshot_v3(&bundle);
                    let intended_cycle_count = bundle
                        .schedule
                        .wspr_cycle_intents
                        .len()
                        .max(bundle.schedule.slots.len());
                    (
                        bundle.into_current(),
                        revision,
                        lifecycle,
                        report_snapshot,
                        intended_cycle_count,
                    )
                }
                actual => {
                    return Err(OpenSessionError::Storage(
                        BundleStoreError::UnsupportedSchemaVersion { actual },
                    ));
                }
            };
        let (inspected, validation) = store.inspect()?.into_current_parts();
        let inspected = inspected.ok_or_else(|| {
            OpenSessionError::Storage(BundleStoreError::Validation {
                source: BundleValidationError::from_report(validation.clone()),
            })
        })?;
        if inspected != current {
            return Err(OpenSessionError::SnapshotChanged);
        }
        let bundle = normalize_bundle(current.bundle);
        Ok(LoadedSnapshot {
            bundle,
            intended_cycle_count,
            schema_version,
            validation,
            report_snapshot,
            revision: Some(revision),
            lifecycle: Some(lifecycle),
        })
    } else {
        let (bundle, validation) = store.read_for_analysis()?;
        Ok(LoadedSnapshot {
            intended_cycle_count: bundle.schedule.slots.len(),
            schema_version: bundle.manifest.schema_version,
            bundle,
            validation,
            report_snapshot: ReportSnapshotContext::default(),
            revision: None,
            lifecycle: None,
        })
    }
}

fn report_snapshot(bundle: &antennabench_core::BundleV2Contents) -> ReportSnapshotContext {
    let adapter = report_adapter_evidence(&bundle.adapter_records);
    let lifecycle_events = bundle
        .events
        .iter()
        .filter_map(|event| {
            let (kind, detail) = match &event.payload {
                OperatorEventPayloadV2::SessionStarted { note } => {
                    (ReportLifecycleEventKind::Started, note.clone())
                }
                OperatorEventPayloadV2::SessionInterrupted { reason } => {
                    (ReportLifecycleEventKind::Interrupted, reason.clone())
                }
                OperatorEventPayloadV2::InterruptionDetected { reason } => (
                    ReportLifecycleEventKind::InterruptionDetected,
                    reason.clone(),
                ),
                OperatorEventPayloadV2::SessionResumed { note } => {
                    (ReportLifecycleEventKind::Resumed, note.clone())
                }
                OperatorEventPayloadV2::SessionEnded { reason } => {
                    (ReportLifecycleEventKind::Ended, reason.clone())
                }
                OperatorEventPayloadV2::SessionAbandoned { reason } => {
                    (ReportLifecycleEventKind::Abandoned, reason.clone())
                }
                _ => return None,
            };
            Some(ReportLifecycleEvent {
                kind,
                occurred_at: event.occurred_at,
                detail,
            })
        })
        .collect();
    ReportSnapshotContext {
        checkpoint_revision: Some(bundle.session_state.revision),
        lifecycle: Some(bundle.session_state.lifecycle),
        lifecycle_events,
        wspr_cycles: Vec::new(),
        antenna_control_attempts: Vec::new(),
        adapter_evidence: adapter,
    }
}

fn report_snapshot_v3(bundle: &BundleV3Contents) -> ReportSnapshotContext {
    let adapter = report_adapter_evidence(&bundle.adapter_records);
    let lifecycle_events = bundle
        .events
        .iter()
        .filter_map(|event| {
            let (kind, detail) = match &event.payload {
                OperatorEventPayloadV3::SessionStarted { note } => {
                    (ReportLifecycleEventKind::Started, note.clone())
                }
                OperatorEventPayloadV3::SessionInterrupted { reason } => {
                    (ReportLifecycleEventKind::Interrupted, reason.clone())
                }
                OperatorEventPayloadV3::InterruptionDetected { reason } => (
                    ReportLifecycleEventKind::InterruptionDetected,
                    reason.clone(),
                ),
                OperatorEventPayloadV3::SessionResumed { note } => {
                    (ReportLifecycleEventKind::Resumed, note.clone())
                }
                OperatorEventPayloadV3::SessionEnded { reason } => {
                    (ReportLifecycleEventKind::Ended, reason.clone())
                }
                OperatorEventPayloadV3::SessionAbandoned { reason } => {
                    (ReportLifecycleEventKind::Abandoned, reason.clone())
                }
                _ => return None,
            };
            Some(ReportLifecycleEvent {
                kind,
                occurred_at: event.occurred_at,
                detail,
            })
        })
        .collect();
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let wspr_cycles = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .map(|intent| {
            let observed = projection
                .cycles
                .iter()
                .find(|cycle| cycle.intent_id == intent.intent_id);
            ReportWsprCycle {
                intent_id: intent.intent_id.clone(),
                sequence_number: intent.sequence_number,
                band: intent.band,
                direction: intent.direction,
                planned_antenna: intent.antenna_label.clone(),
                actual_antenna: observed.map(|cycle| cycle.antenna_label.clone()),
                ready_at: observed.map(|cycle| cycle.ready_at),
                starts_at: observed.map(|cycle| cycle.window.starts_at),
                transmission_ends_at: observed.map(|cycle| cycle.window.transmission_ends_at),
                attribution: observed.map_or_else(
                    || {
                        if projection
                            .skipped_intent_ids
                            .iter()
                            .any(|intent_id| intent_id == &intent.intent_id)
                        {
                            ReportWsprAttribution::Skipped
                        } else {
                            ReportWsprAttribution::Pending
                        }
                    },
                    |cycle| {
                        if cycle.occupancy_fully_covers_transmission {
                            ReportWsprAttribution::Attributable
                        } else {
                            ReportWsprAttribution::UnknownAntennaOccupancy
                        }
                    },
                ),
                readiness_basis: bundle.events.iter().find_map(|event| {
                    if event.slot_id.as_deref() != Some(intent.intent_id.as_str()) {
                        return None;
                    }
                    let OperatorEventPayloadV3::WsprCycleArmed { readiness, .. } = &event.payload
                    else {
                        return None;
                    };
                    Some(match readiness {
                        Some(WsprReadinessBasisV5::CommandVerified { .. }) => {
                            ReportWsprReadinessBasis::CommandVerified
                        }
                        Some(WsprReadinessBasisV5::OperatorConfirmed) | None => {
                            ReportWsprReadinessBasis::OperatorConfirmed
                        }
                    })
                }),
            }
        })
        .collect();
    let antenna_control_attempts = bundle
        .rig
        .iter()
        .filter_map(|record| {
            let invocation = record.antenna_control.as_ref()?;
            Some(ReportAntennaControlAttempt {
                record_id: record.record_id.clone(),
                role: invocation.role,
                controller_profile_name: invocation.controller_profile_name.clone(),
                controller_profile_revision: invocation.controller_profile_revision.clone(),
                resolved_program: invocation.command.resolved_program.clone(),
                resolved_arguments: invocation.command.resolved_arguments.clone(),
                intent_id: invocation.context.intent_id.clone(),
                antenna: invocation.context.antenna.clone(),
                target: invocation.context.target.clone(),
                mode: invocation.context.mode,
                started_at: invocation.started_at,
                completed_at: invocation.completed_at,
                elapsed_milliseconds: invocation.elapsed_milliseconds,
                disposition: invocation.disposition.clone(),
                stdout: invocation.stdout.clone(),
                stderr: invocation.stderr.clone(),
            })
        })
        .collect();
    ReportSnapshotContext {
        checkpoint_revision: Some(bundle.session_state.revision),
        lifecycle: Some(bundle.session_state.lifecycle),
        lifecycle_events,
        wspr_cycles,
        antenna_control_attempts,
        adapter_evidence: adapter,
    }
}

fn report_adapter_evidence(records: &[AdapterRecordV2]) -> ReportAdapterEvidence {
    let mut adapter = ReportAdapterEvidence {
        record_count: records.len(),
        ..ReportAdapterEvidence::default()
    };
    for record in records {
        match record.disposition {
            AdapterDisposition::Accepted => adapter.accepted_count += 1,
            AdapterDisposition::Malformed => adapter.malformed_count += 1,
            AdapterDisposition::Unsupported => adapter.unsupported_count += 1,
            AdapterDisposition::Filtered => adapter.filtered_count += 1,
            AdapterDisposition::Duplicate => adapter.duplicate_count += 1,
            AdapterDisposition::Conflict => adapter.conflict_count += 1,
            AdapterDisposition::PartiallyNormalized => adapter.partially_normalized_count += 1,
        }
        if record.record_type == "acquisition_gap" {
            adapter.gap_count += 1;
        }
        if record.record_type == "wspr_live_import_summary" {
            if let AdapterInput::Inline { data, .. } = &record.input {
                if let Ok(import) = serde_json::from_str::<WsprLiveReportImport>(data) {
                    adapter.imports.push(import.into_report());
                }
            }
        }
    }
    adapter.evidence_complete = adapter.gap_count == 0
        && adapter
            .imports
            .iter()
            .all(|import| import.completeness_known);
    adapter
}

#[derive(Debug, Deserialize)]
struct WsprLiveReportImport {
    provider_id: String,
    source_id: String,
    captured_at: chrono::DateTime<chrono::Utc>,
    window_start: chrono::DateTime<chrono::Utc>,
    window_end: chrono::DateTime<chrono::Utc>,
    selected_bands: Vec<Band>,
    completeness: String,
    counts: WsprLiveReportCounts,
}

#[derive(Debug, Deserialize)]
struct WsprLiveReportCounts {
    total: usize,
    accepted: usize,
    malformed: usize,
    filtered: usize,
    unsupported: usize,
    duplicate: usize,
    conflict: usize,
    observations_created: usize,
}

impl WsprLiveReportImport {
    fn into_report(self) -> ReportImportedEvidence {
        ReportImportedEvidence {
            provider_id: self.provider_id,
            source_id: self.source_id,
            captured_at: self.captured_at,
            window_start: self.window_start,
            window_end: self.window_end,
            selected_bands: self.selected_bands,
            total_count: self.counts.total,
            accepted_count: self.counts.accepted,
            malformed_count: self.counts.malformed,
            filtered_count: self.counts.filtered,
            unsupported_count: self.counts.unsupported,
            duplicate_count: self.counts.duplicate,
            conflict_count: self.counts.conflict,
            observations_created: self.counts.observations_created,
            completeness_known: self.completeness == "known",
        }
    }
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

fn suggested_report_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| bundle_suffix(name).map(|suffix| name.trim_end_matches(suffix)))
        .map_or_else(
            || "antennabench-report.html".into(),
            |stem| format!("{stem}-report.html"),
        )
}

fn export_bundle(
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

fn bundle_suffix(name: &str) -> Option<&'static str> {
    [V2_BUNDLE_SUFFIX, V1_BUNDLE_SUFFIX]
        .into_iter()
        .find(|suffix| name.ends_with(suffix))
}

fn build_active_session(
    source: PathBuf,
    bundle_name: String,
    snapshot: LoadedSnapshot,
) -> ActiveSession {
    let presentation = prepare_presentation(&snapshot).ok();
    ActiveSession {
        source,
        summary: OpenedSession {
            bundle_name,
            session_id: snapshot.bundle.manifest.session_id.clone(),
            callsign: snapshot.bundle.station.callsign.clone(),
            grid: snapshot.bundle.station.grid.clone(),
            antenna_count: snapshot.bundle.antennas.antennas.len(),
            slot_count: snapshot.intended_cycle_count,
            observation_count: snapshot.bundle.observations.len(),
            schema_version: snapshot.schema_version,
            revision: snapshot.revision,
            lifecycle: snapshot.lifecycle,
            report_available: presentation.is_some(),
        },
        presentation,
    }
}

fn prepare_presentation(snapshot: &LoadedSnapshot) -> Result<ReportPresentation, ReportError> {
    let report = build_report_with_snapshot(
        &snapshot.bundle,
        &snapshot.validation,
        snapshot.report_snapshot.clone(),
    )?;
    let report_html = render_standalone_html(&report)?;
    Ok(ReportPresentation {
        presentation_id: 0,
        session_id: snapshot.bundle.manifest.session_id.clone(),
        revision: snapshot.revision,
        lifecycle: snapshot.lifecycle,
        completeness: report.completeness,
        report_html,
    })
}

pub(crate) fn activate_created_bundle(
    state: &ActiveSessionState,
    path: PathBuf,
) -> Result<OpenedSession, SessionErrorPayload> {
    let mut session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
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
    let mut session = open_bundle(&path).map_err(SessionErrorPayload::from)?;
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

fn assign_presentation_id(state: &mut DesktopState, session: &mut ActiveSession) {
    if let Some(presentation) = &mut session.presentation {
        state.next_presentation_id = state.next_presentation_id.saturating_add(1);
        presentation.presentation_id = state.next_presentation_id;
    }
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
    let (bundle_name, revision) =
        export_bundle(&source, &destination).map_err(SessionErrorPayload::from)?;

    Ok(ExportSessionOutcome::Exported {
        bundle_name,
        revision,
    })
}

fn export_active_report_with_selection<F>(
    state: &ActiveSessionState,
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
        .write_all(presentation.report_html.as_bytes())
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
    })
}

fn active_session_report_for(
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

fn refresh_active_session_report_for(
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
        wsjtx_state.stop_all("WSJT-X reception stopped because a different session was opened.");
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
) -> Result<ExportReportOutcome, SessionErrorPayload> {
    export_active_report_with_selection(state.inner(), |source| {
        let mut dialog = app
            .dialog()
            .file()
            .set_title("Export the coherent AntennaBench report snapshot")
            .set_file_name(suggested_report_name(source))
            .set_can_create_directories(true)
            .add_filter("Standalone HTML report", &["html"]);
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
) -> Result<ReportPresentation, SessionErrorPayload> {
    active_session_report_for(state.inner())
}

#[tauri::command]
pub(crate) fn refresh_active_session_report(
    state: State<'_, ActiveSessionState>,
) -> Result<ReportPresentation, SessionErrorPayload> {
    refresh_active_session_report_for(state.inner())
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct E2eExportedSnapshots {
    pub(crate) report_path: PathBuf,
    pub(crate) bundle_path: PathBuf,
    pub(crate) revision: u64,
    pub(crate) presentation_id: u64,
    pub(crate) report_html: String,
}

#[cfg(test)]
pub(crate) fn e2e_report_snapshot(state: &ActiveSessionState) -> (u64, u64, String) {
    let presentation = refresh_active_session_report_for(state).expect("coherent report refresh");
    (
        presentation.revision.expect("schema-v2 report revision"),
        presentation.presentation_id,
        presentation.report_html,
    )
}

#[cfg(test)]
pub(crate) fn export_e2e_snapshots(
    state: &ActiveSessionState,
    root: &Path,
) -> E2eExportedSnapshots {
    let presentation = refresh_active_session_report_for(state).expect("coherent report refresh");
    let revision = presentation.revision.expect("schema-v2 report revision");
    let report_path = root.join("complete-workflow-report.html");
    let bundle_path = root.join(format!("complete-workflow-export{V2_BUNDLE_SUFFIX}"));
    let report_outcome =
        export_active_report_with_selection(state, |_| Ok(Some(report_path.clone())))
            .expect("standalone report export");
    assert!(matches!(
        report_outcome,
        ExportReportOutcome::Exported {
            revision: Some(exported),
            ..
        } if exported == revision
    ));
    assert!(export_active_report_with_selection(state, |_| Ok(Some(report_path.clone()))).is_err());
    let bundle_outcome =
        export_active_session_with_selection(state, |_| Ok(Some(bundle_path.clone())))
            .expect("lossless checkpoint export");
    assert!(matches!(
        bundle_outcome,
        ExportSessionOutcome::Exported {
            revision: Some(exported),
            ..
        } if exported == revision
    ));
    assert!(
        export_active_session_with_selection(state, |_| Ok(Some(bundle_path.clone()))).is_err()
    );
    assert_eq!(
        std::fs::read_to_string(&report_path).expect("exported HTML"),
        presentation.report_html
    );
    E2eExportedSnapshots {
        report_path,
        bundle_path,
        revision,
        presentation_id: presentation.presentation_id,
        report_html: presentation.report_html,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path};

    use antennabench_analysis::AnalysisError;
    use antennabench_report::ReportError;
    use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError};
    use tempfile::TempDir;

    use super::{
        active_session_report_for, check_ipc_payload, copy_error_payload,
        export_active_report_with_selection, export_active_session_with_selection, export_bundle,
        open_bundle, open_session_with_selection, refresh_active_session_report_for,
        report_error_payload, ActiveSessionState, ExportReportOutcome, ExportSessionOutcome,
        OpenSessionOutcome, SessionErrorKind, SessionErrorPayload, REPORT_DOCUMENT_IPC_BYTES,
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
        assert!(opened
            .presentation
            .as_ref()
            .unwrap()
            .report_html
            .starts_with("<!doctype html>"));
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
        state
            .0
            .lock()
            .unwrap()
            .active
            .as_mut()
            .unwrap()
            .presentation
            .as_mut()
            .unwrap()
            .report_html = "x".repeat(REPORT_DOCUMENT_IPC_BYTES as usize + 1);
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

        let (bundle_name, revision) =
            export_bundle(&source, &destination).expect("export source bundle");
        let reopened = open_bundle(&destination).expect("reopen exported bundle");
        let mut expected = opened.summary;
        expected.bundle_name = bundle_name.clone();

        assert_eq!(bundle_name, "exported.session.wsprabundle");
        assert_eq!(revision, None);
        assert_eq!(reopened.summary, expected);
    }

    #[test]
    fn v2_report_refresh_and_exports_share_one_committed_revision() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("live.session.antennabundle");
        let upgraded = temp.path().join("upgraded.session.antennabundle");
        let store = BundleStore::new(canonical_fixture())
            .upgrade_v1_to_v2(&upgraded)
            .unwrap();
        let mut bundle = store.read_v2_checkpointed().unwrap();
        let mut normalized = bundle.clone().into_current().bundle;
        antennabench_core::annotate_bundle_observations(&mut normalized);
        for observation in &mut bundle.observations {
            let current = normalized
                .observations
                .iter()
                .find(|current| current.observation_id == observation.observation_id)
                .unwrap();
            observation.slot_id.clone_from(&current.slot_id);
            observation.slot_label.clone_from(&current.slot_label);
            observation.slot_confidence = current.slot_confidence;
        }
        BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
        BundleStore::new(&source).write_v2(&bundle).unwrap();
        let state = ActiveSessionState::default();
        open_session_with_selection(&state, || Ok(Some(source.clone()))).unwrap();

        let first = active_session_report_for(&state).unwrap();
        assert!(first.revision.is_some());
        assert!(first.report_html.contains("Committed session snapshot"));
        assert!(first.report_html.contains("Checkpoint revision"));
        let unchanged = refresh_active_session_report_for(&state).unwrap();
        assert_eq!(unchanged.presentation_id, first.presentation_id);

        let html_destination = temp.path().join("snapshot.html");
        let exported =
            export_active_report_with_selection(&state, |_| Ok(Some(html_destination.clone())))
                .unwrap();
        assert_eq!(
            exported,
            ExportReportOutcome::Exported {
                file_name: "snapshot.html".into(),
                revision: first.revision,
            }
        );
        assert_eq!(
            fs::read_to_string(&html_destination).unwrap(),
            first.report_html
        );
        assert_eq!(
            export_active_report_with_selection(&state, |_| Ok(Some(html_destination.clone())))
                .unwrap_err()
                .kind,
            SessionErrorKind::Destination
        );

        let bundle_destination = temp.path().join("snapshot.session.antennabundle");
        let outcome =
            export_active_session_with_selection(&state, |_| Ok(Some(bundle_destination.clone())))
                .unwrap();
        assert_eq!(
            outcome,
            ExportSessionOutcome::Exported {
                bundle_name: "snapshot.session.antennabundle".into(),
                revision: first.revision,
            }
        );
        assert_eq!(
            BundleStore::new(bundle_destination)
                .read_v2_checkpointed()
                .unwrap()
                .session_state
                .revision,
            first.revision.unwrap()
        );
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
                bundle_name: "resource-safe-copy.session.wsprabundle".into(),
                revision: None,
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
        assert!(source_report.report_html.starts_with("<!doctype html>"));

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
                bundle_name: "exported.session.wsprabundle".into(),
                revision: None,
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
        let reopened_report =
            active_session_report_for(&state).expect("view report after exported bundle reopen");
        assert_eq!(reopened_report.report_html, source_report.report_html);
        assert_eq!(reopened_report.revision, source_report.revision);
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
