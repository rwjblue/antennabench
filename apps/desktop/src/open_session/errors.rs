use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportPresentation {
    pub(super) presentation_id: u64,
    pub(super) session_id: String,
    pub(super) revision: Option<u64>,
    pub(super) lifecycle: Option<SessionLifecycleV2>,
    pub(super) completeness: ReportCompleteness,
    pub(super) has_controller_evidence: bool,
    pub(crate) operational_history: BundleDiagnosticsPresentation,
    pub(super) report_html: String,
    #[serde(skip)]
    pub(super) compact_summary_html: String,
    #[serde(skip)]
    pub(super) controller_omitted_report_html: Option<String>,
    #[serde(skip)]
    pub(super) operational_history_report_html: String,
    #[serde(skip)]
    pub(super) operational_history_controller_omitted_report_html: Option<String>,
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
    pub(crate) operational_history: BundleDiagnosticsPresentation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum OpenSessionOutcome {
    Cancelled,
    Opened { session: Box<OpenedSession> },
}

#[cfg(test)]
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
        format: ReportExportFormat,
    },
}

/// The two derived HTML artifacts available from one committed report snapshot.
/// The session bundle uses its separate lossless export command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReportExportFormat {
    CompactSummaryHtml,
    #[default]
    FullEvidenceHtml,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OperationalHistoryHandling {
    #[default]
    Omitted,
    IncludedRedacted,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) operation: Option<Box<OperationErrorDetail>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_persistence: Option<Box<DiagnosticPersistenceDetail>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationErrorDetail {
    pub(crate) code: String,
    pub(crate) stage: String,
    pub(crate) observed: Option<u64>,
    pub(crate) limit: Option<u64>,
    pub(crate) unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticPersistenceDetail {
    pub(crate) status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason_code: Option<String>,
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
            operation: None,
            diagnostic_persistence: None,
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
        let mut payload = Self::new(
            kind,
            "The local operation was stopped by its resource policy.",
            format!(
                "code={code} stage={stage} limit={limit} observed={} unit={unit} complete=false",
                observed.map_or_else(|| "unknown".to_string(), |value| value.to_string())
            ),
        );
        payload.operation = Some(Box::new(OperationErrorDetail {
            code: code.into(),
            stage: stage.into(),
            observed,
            limit: Some(limit),
            unit: Some(unit.into()),
        }));
        payload
    }

    pub(crate) fn with_diagnostic_persisted(mut self, diagnostic_id: Option<String>) -> Self {
        self.diagnostic_persistence = Some(Box::new(DiagnosticPersistenceDetail {
            status: "persisted",
            diagnostic_id,
            reason_code: None,
        }));
        self
    }

    pub(crate) fn with_operation(mut self, code: &str, stage: &str) -> Self {
        self.operation = Some(Box::new(OperationErrorDetail {
            code: code.into(),
            stage: stage.into(),
            observed: None,
            limit: None,
            unit: None,
        }));
        self
    }

    pub(crate) fn with_diagnostic_not_persisted(mut self, reason_code: impl Into<String>) -> Self {
        self.diagnostic_persistence = Some(Box::new(DiagnosticPersistenceDetail {
            status: "not_persisted",
            diagnostic_id: None,
            reason_code: Some(reason_code.into()),
        }));
        self
    }

    pub(crate) fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }
}

#[derive(Debug, Error)]
pub(super) enum OpenSessionError {
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
pub(super) enum ExportSessionError {
    #[cfg(test)]
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
            #[cfg(test)]
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

pub(super) fn error_with_source(error: &dyn StdError) -> String {
    error
        .source()
        .map_or_else(|| error.to_string(), |source| format!("{error}: {source}"))
}

pub(crate) fn copy_error_payload(error: BundleCopyError) -> SessionErrorPayload {
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
        BundleStoreError::AmbiguousManifest { message } => SessionErrorPayload::new(
            SessionErrorKind::JsonParse,
            "The bundle manifest is invalid or ambiguous.",
            message,
        ),
        BundleStoreError::Validation { source } => SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The session bundle did not pass validation.",
            validation_error_detail(&source),
        ),
        BundleStoreError::UnsupportedSchemaVersion { actual } => SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "This session bundle schema is not supported by this AntennaBench version.",
            format!("unsupported schema version {actual}"),
        ),
        BundleStoreError::Resource(source) => {
            let diagnostic = source.diagnostic;
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
                &format!("{:?}", diagnostic.unit).to_ascii_lowercase(),
            )
        }
        error => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The session bundle could not be read.",
            error
                .source()
                .map_or_else(|| error.to_string(), |source| format!("{error}: {source}")),
        ),
    }
}

pub(super) fn report_error_payload(error: ReportError) -> SessionErrorPayload {
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

pub(super) fn validation_error_detail(source: &antennabench_core::BundleValidationError) -> String {
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
