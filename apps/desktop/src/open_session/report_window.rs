use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tauri::{Manager, WebviewUrl, WebviewWindow};

use super::*;

const REPORT_WINDOW_PREFIX: &str = "report-";
const REPORT_WINDOW_PAGE: &str = "report-window.html";
const MAX_REPORT_WINDOWS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReportDocumentKind {
    Summary,
    FullEvidence,
}

impl ReportDocumentKind {
    fn title(self) -> &'static str {
        match self {
            Self::Summary => "Summary",
            Self::FullEvidence => "Full evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReportWindowDocument {
    window_label: String,
    presentation_id: u64,
    session_id: String,
    bundle_name: String,
    revision: Option<u64>,
    lifecycle: Option<SessionLifecycleV2>,
    document_kind: ReportDocumentKind,
    html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum OpenReportWindowOutcome {
    Created {
        #[serde(rename = "windowLabel")]
        window_label: String,
        revision: Option<u64>,
        #[serde(rename = "documentKind")]
        document_kind: ReportDocumentKind,
    },
    Focused {
        #[serde(rename = "windowLabel")]
        window_label: String,
        revision: Option<u64>,
        #[serde(rename = "documentKind")]
        document_kind: ReportDocumentKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReportWindowPortError {
    Focus,
    Create,
}

trait ReportWindowPort {
    fn focus(&self, label: &str) -> Result<bool, ReportWindowPortError>;
    fn create(&self, document: &ReportWindowDocument) -> Result<(), ReportWindowPortError>;
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ReportWindowState(Arc<Mutex<HashMap<String, ReportWindowDocument>>>);

impl ReportWindowState {
    fn remove(&self, label: &str) {
        if let Ok(mut documents) = self.0.lock() {
            documents.remove(label);
        }
    }

    fn clear(&self) -> Vec<String> {
        let Ok(mut documents) = self.0.lock() else {
            return Vec::new();
        };
        let labels = documents.keys().cloned().collect();
        documents.clear();
        labels
    }
}

struct SystemReportWindowPort {
    app: AppHandle,
    state: ReportWindowState,
}

impl ReportWindowPort for SystemReportWindowPort {
    fn focus(&self, label: &str) -> Result<bool, ReportWindowPortError> {
        let Some(window) = self.app.get_webview_window(label) else {
            return Ok(false);
        };
        window
            .unminimize()
            .and_then(|_| window.show())
            .and_then(|_| window.set_focus())
            .map_err(|_| ReportWindowPortError::Focus)?;
        Ok(true)
    }

    fn create(&self, document: &ReportWindowDocument) -> Result<(), ReportWindowPortError> {
        let revision = document
            .revision
            .map_or_else(|| "legacy".to_string(), |value| value.to_string());
        let title = format!(
            "AntennaBench {} · revision {revision}",
            document.document_kind.title()
        );
        let window = tauri::WebviewWindowBuilder::new(
            &self.app,
            &document.window_label,
            WebviewUrl::App(REPORT_WINDOW_PAGE.into()),
        )
        .title(title)
        .inner_size(980.0, 780.0)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .build()
        .map_err(|_| ReportWindowPortError::Create)?;
        let state = self.state.clone();
        let label = document.window_label.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                state.remove(&label);
            }
        });
        Ok(())
    }
}

fn report_window_error(error: ReportWindowPortError) -> SessionErrorPayload {
    let detail = match error {
        ReportWindowPortError::Focus => "the matching immutable report window could not be focused",
        ReportWindowPortError::Create => "the restricted native report window could not be created",
    };
    SessionErrorPayload::new(
        SessionErrorKind::ReportPipeline,
        "The separate report window could not be opened.",
        detail,
    )
}

fn report_window_label(
    session_id: &str,
    revision: Option<u64>,
    document_kind: ReportDocumentKind,
) -> String {
    let revision = revision.map_or_else(|| "legacy".to_string(), |value| value.to_string());
    let identity = format!("{session_id}\n{revision}\n{}", document_kind.title());
    let digest = Sha256::digest(identity.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{REPORT_WINDOW_PREFIX}{digest}")
}

fn selected_document(
    active: &ActiveSessionState,
    displayed_presentation_id: u64,
    document_kind: ReportDocumentKind,
) -> Result<ReportWindowDocument, SessionErrorPayload> {
    let active = active
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("active session state is unavailable"))?;
    let session = active.active.as_ref().ok_or_else(|| {
        SessionErrorPayload::report_pipeline("no active session report is available")
    })?;
    let presentation = super::commands::presentation_for_id(session, displayed_presentation_id)
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The displayed report is no longer available.",
                "the requested presentation does not belong to the active session",
            )
        })?;
    let html = match document_kind {
        ReportDocumentKind::Summary => presentation.summary_html.clone(),
        ReportDocumentKind::FullEvidence => presentation.report_html.clone(),
    };
    let document = ReportWindowDocument {
        window_label: report_window_label(
            &presentation.session_id,
            presentation.revision,
            document_kind,
        ),
        presentation_id: presentation.presentation_id,
        session_id: presentation.session_id.clone(),
        bundle_name: session.summary.bundle_name.clone(),
        revision: presentation.revision,
        lifecycle: presentation.lifecycle,
        document_kind,
        html,
    };
    check_ipc_payload(
        &document,
        REPORT_DOCUMENT_IPC_BYTES,
        "report_window_document",
    )?;
    Ok(document)
}

fn open_report_window_with(
    active: &ActiveSessionState,
    windows: &ReportWindowState,
    caller_label: &str,
    displayed_presentation_id: u64,
    document_kind: ReportDocumentKind,
    port: &impl ReportWindowPort,
) -> Result<OpenReportWindowOutcome, SessionErrorPayload> {
    if caller_label != "main" {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "This window cannot open another report window.",
            "only the main AntennaBench shell owns report-window creation",
        ));
    }
    let document = selected_document(active, displayed_presentation_id, document_kind)?;
    let label = document.window_label.clone();
    let mut registry = windows
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("report window state is unavailable"))?;
    if registry.contains_key(&label) {
        match port.focus(&label).map_err(report_window_error)? {
            true => {
                return Ok(OpenReportWindowOutcome::Focused {
                    window_label: label,
                    revision: document.revision,
                    document_kind,
                });
            }
            false => {
                registry.remove(&label);
            }
        }
    }
    if registry.len() >= MAX_REPORT_WINDOWS {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Resource,
            "No more separate report windows can be opened.",
            format!(
                "close an existing report window before opening another; limit={MAX_REPORT_WINDOWS}"
            ),
        ));
    }
    registry.insert(label.clone(), document.clone());
    if let Err(error) = port.create(&document) {
        registry.remove(&label);
        return Err(report_window_error(error));
    }
    Ok(OpenReportWindowOutcome::Created {
        window_label: label,
        revision: document.revision,
        document_kind,
    })
}

fn report_window_document_for(
    windows: &ReportWindowState,
    caller_label: &str,
) -> Result<ReportWindowDocument, SessionErrorPayload> {
    if !caller_label.starts_with(REPORT_WINDOW_PREFIX) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "This window cannot read a report-window presentation.",
            "the caller is outside the restricted report-window label boundary",
        ));
    }
    windows
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("report window state is unavailable"))?
        .get(caller_label)
        .cloned()
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "This report window no longer has an immutable presentation.",
                "close the stale window and reopen the displayed report from the main shell",
            )
        })
}

#[tauri::command]
pub(crate) async fn open_report_window(
    window: WebviewWindow,
    app: AppHandle,
    active: State<'_, ActiveSessionState>,
    windows: State<'_, ReportWindowState>,
    displayed_presentation_id: u64,
    document_kind: ReportDocumentKind,
) -> Result<OpenReportWindowOutcome, SessionErrorPayload> {
    let port = SystemReportWindowPort {
        app,
        state: windows.inner().clone(),
    };
    open_report_window_with(
        active.inner(),
        windows.inner(),
        window.label(),
        displayed_presentation_id,
        document_kind,
        &port,
    )
}

#[tauri::command]
pub(crate) fn report_window_document(
    window: WebviewWindow,
    windows: State<'_, ReportWindowState>,
) -> Result<ReportWindowDocument, SessionErrorPayload> {
    report_window_document_for(windows.inner(), window.label())
}

pub(crate) fn close_report_windows(app: &AppHandle, windows: &ReportWindowState) {
    for label in windows.clear() {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Mutex};

    use super::*;

    #[derive(Default)]
    struct FakePort {
        created: Mutex<Vec<ReportWindowDocument>>,
        focused: Mutex<Vec<String>>,
        create_failure: bool,
    }

    impl ReportWindowPort for FakePort {
        fn focus(&self, label: &str) -> Result<bool, ReportWindowPortError> {
            self.focused.lock().unwrap().push(label.into());
            Ok(true)
        }

        fn create(&self, document: &ReportWindowDocument) -> Result<(), ReportWindowPortError> {
            if self.create_failure {
                return Err(ReportWindowPortError::Create);
            }
            self.created.lock().unwrap().push(document.clone());
            Ok(())
        }
    }

    fn opened_state() -> ActiveSessionState {
        let state = ActiveSessionState::default();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
        super::super::commands::open_session_with_selection(&state, || Ok(Some(fixture))).unwrap();
        state
    }

    #[test]
    fn immutable_identity_reuses_exact_documents_and_allows_modes_to_coexist() {
        let active = opened_state();
        let windows = ReportWindowState::default();
        let port = FakePort::default();
        let presentation_id = super::super::commands::active_session_report_for(&active)
            .unwrap()
            .presentation_id;

        let summary = open_report_window_with(
            &active,
            &windows,
            "main",
            presentation_id,
            ReportDocumentKind::Summary,
            &port,
        )
        .unwrap();
        let OpenReportWindowOutcome::Created {
            window_label: summary_label,
            ..
        } = summary
        else {
            panic!("first Summary window was not created");
        };
        let repeated = open_report_window_with(
            &active,
            &windows,
            "main",
            presentation_id,
            ReportDocumentKind::Summary,
            &port,
        )
        .unwrap();
        assert!(matches!(repeated, OpenReportWindowOutcome::Focused { .. }));
        let full = open_report_window_with(
            &active,
            &windows,
            "main",
            presentation_id,
            ReportDocumentKind::FullEvidence,
            &port,
        )
        .unwrap();
        let OpenReportWindowOutcome::Created {
            window_label: full_label,
            ..
        } = full
        else {
            panic!("Full evidence window was not created");
        };

        assert_ne!(summary_label, full_label);
        assert_eq!(port.created.lock().unwrap().len(), 2);
        assert_eq!(
            port.focused.lock().unwrap().as_slice(),
            std::slice::from_ref(&summary_label)
        );
        let original = report_window_document_for(&windows, &summary_label).unwrap();
        assert_eq!(original.document_kind, ReportDocumentKind::Summary);
        assert!(original.html.contains("Session Summary"));
        assert_eq!(original.window_label, summary_label);

        let newer_presentation_id = {
            let mut state = active.0.lock().unwrap();
            let session = state.active.as_mut().unwrap();
            let displayed = session.presentation.clone().unwrap();
            let mut newer = displayed.clone();
            newer.presentation_id = displayed.presentation_id + 1;
            newer.revision = Some(displayed.revision.unwrap_or_default() + 1);
            newer.summary_html = "<p>newer immutable Summary</p>".into();
            session.retained_presentation = Some(displayed);
            session.presentation = Some(newer.clone());
            newer.presentation_id
        };
        let newer = open_report_window_with(
            &active,
            &windows,
            "main",
            newer_presentation_id,
            ReportDocumentKind::Summary,
            &port,
        )
        .unwrap();
        let OpenReportWindowOutcome::Created {
            window_label: newer_label,
            ..
        } = newer
        else {
            panic!("newer Summary window was not created");
        };
        assert_ne!(summary_label, newer_label);
        assert_eq!(
            report_window_document_for(&windows, &summary_label).unwrap(),
            original,
            "a newer main presentation cannot mutate an open immutable reader"
        );
        assert_eq!(
            report_window_document_for(&windows, &newer_label)
                .unwrap()
                .html,
            "<p>newer immutable Summary</p>"
        );
        assert_eq!(
            report_window_document_for(&windows, "main")
                .unwrap_err()
                .kind,
            SessionErrorKind::Unsupported
        );
    }

    #[test]
    fn cleanup_and_failures_never_leave_reusable_registry_entries() {
        let active = opened_state();
        let windows = ReportWindowState::default();
        let presentation_id = super::super::commands::active_session_report_for(&active)
            .unwrap()
            .presentation_id;
        let failure = open_report_window_with(
            &active,
            &windows,
            "main",
            presentation_id,
            ReportDocumentKind::Summary,
            &FakePort {
                create_failure: true,
                ..FakePort::default()
            },
        )
        .unwrap_err();
        assert_eq!(failure.kind, SessionErrorKind::ReportPipeline);
        assert!(!failure.detail.contains("<html"));
        assert!(windows.clear().is_empty());

        let port = FakePort::default();
        let created = open_report_window_with(
            &active,
            &windows,
            "main",
            presentation_id,
            ReportDocumentKind::Summary,
            &port,
        )
        .unwrap();
        let OpenReportWindowOutcome::Created { window_label, .. } = created else {
            panic!("report window was not created");
        };
        windows.remove(&window_label);
        assert_eq!(
            report_window_document_for(&windows, &window_label)
                .unwrap_err()
                .kind,
            SessionErrorKind::Conflict
        );
        assert_eq!(
            open_report_window_with(
                &active,
                &windows,
                &window_label,
                presentation_id,
                ReportDocumentKind::Summary,
                &port,
            )
            .unwrap_err()
            .kind,
            SessionErrorKind::Unsupported
        );
    }

    #[test]
    fn restricted_capability_grants_only_the_immutable_document_command() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/report-reader.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!(["report-*"]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!(["allow-report-window-document"])
        );
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/main-shell.json")).unwrap();
        let main_permissions = main["permissions"].as_array().unwrap();
        assert!(main_permissions.contains(&serde_json::json!("allow-open-report-window")));
        assert!(!main_permissions.contains(&serde_json::json!("allow-report-window-document")));
        let csp: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(
            csp["app"]["security"]["capabilities"],
            serde_json::json!(["main-shell", "report-reader"])
        );
        let policy = csp["app"]["security"]["csp"].as_str().unwrap();
        assert!(policy.contains("connect-src 'none'"));
        assert!(policy.contains("frame-src 'self' blob:"));
    }
}
