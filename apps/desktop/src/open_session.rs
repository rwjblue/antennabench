use std::{
    error::Error as StdError,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Condvar, Mutex},
    time::UNIX_EPOCH,
};

use antennabench_analysis::AnalysisError;
use antennabench_core::{
    normalize_bundle,
    v2::{
        reduce_operator_events_v2, AdapterDisposition, AdapterInput, AdapterRecordV2,
        CorrectableOperatorEventPayloadV2, EventCorrectionActionV2, OperatorEventPayloadV2,
        SessionLifecycleV2, V1_BUNDLE_SUFFIX, V2_BUNDLE_SUFFIX,
    },
    v3::{
        project_wspr_run_v3, reduce_operator_events_v3, BundleV3Contents,
        CorrectableOperatorEventPayloadV3, EventCorrectionActionV3, OperatorEventPayloadV3,
    },
    v5::WsprReadinessBasisV5,
    v6::{DiagnosticOperationV6, DiagnosticPhaseV6, EvidenceEffectV6},
    Band, BundleContents, BundleValidationError, BundleValidationReport, SCHEMA_VERSION_V2,
    SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};
use antennabench_report::{
    build_report_with_snapshot_and_activity, render_compact_summary_html, render_standalone_html,
    render_standalone_html_with_operational_history, render_standalone_html_with_options,
    ControllerEvidenceHandling, ReportAcquisitionWorkflowStatus, ReportAdapterEvidence,
    ReportAntennaControlAttempt, ReportCompleteness, ReportError, ReportEventCorrection,
    ReportEventCorrectionAction, ReportImportedEvidence, ReportLifecycleEvent,
    ReportLifecycleEventKind, ReportOperatorEvent, ReportOperatorEventKind,
    ReportProviderCompleteness, ReportSnapshotContext, ReportWsprAttribution, ReportWsprCycle,
    ReportWsprReadinessBasis, StandaloneHtmlOptions,
};
use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError, LivePersistenceError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;
use uuid::Uuid;

use crate::antenna_control::AntennaControllerState;
use crate::wsjtx_session::WsjtxSessionState;

const SESSION_SUMMARY_IPC_BYTES: u64 = 64 * 1024;
const REPORT_DOCUMENT_IPC_BYTES: u64 = 16 * 1024 * 1024;

mod commands;
mod diagnostics;
mod errors;
mod projection;
mod state;

#[cfg(test)]
pub(crate) use commands::open_session_at_path;
pub(crate) use commands::{
    active_session_report, cancel_report_export, check_ipc_payload, confirm_report_export,
    export_active_session_report, export_session_bundle_at_path, finish_open_side_effects,
    open_session_at_path_verified, refresh_active_session_report,
    validate_portable_session_at_path,
};
pub(crate) use errors::{
    copy_error_payload, storage_error_payload, OpenSessionOutcome, OpenedSession, SessionErrorKind,
    SessionErrorPayload,
};
pub(crate) use state::{
    activate_created_bundle, active_session_live_projection, active_session_source,
    reload_active_session, update_active_session_live_projection, with_foreground_operation,
    with_suspended_foreground_operation, with_waiting_foreground_operation, ActiveSessionState,
};

use commands::bundle_suffix;
use diagnostics::{
    legacy_diagnostics_presentation, present_bundle_diagnostics, BundleDiagnosticsPresentation,
};
#[cfg(test)]
use errors::ExportSessionOutcome;
use errors::{
    report_error_payload, ExportReportOutcome, ExportSessionError, OpenSessionError,
    OperationalHistoryHandling, ReportExportFormat, ReportPresentation,
};
use projection::{load_snapshot, open_bundle, prepare_presentation};
use state::{
    assign_presentation_id, ActiveSession, PendingReportExport, ReportDestinationIdentity,
};

#[cfg(test)]
use commands::{
    active_session_report_for, cancel_pending_report_export_for,
    confirm_pending_report_export_with, export_active_report_with_selection,
    export_active_report_with_selection_and_disclosure, export_active_session_with_selection,
    export_bundle, open_session_with_selection, refresh_active_session_report_for,
    suggested_compact_summary_name, suggested_report_name, ReportReplacePort,
    SystemReportReplacePort,
};
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct E2eExportedSnapshots {
    pub(crate) report_path: PathBuf,
    pub(crate) compact_summary_path: PathBuf,
    pub(crate) bundle_path: PathBuf,
    pub(crate) revision: u64,
    pub(crate) presentation_id: u64,
    pub(crate) report_html: String,
    pub(crate) compact_summary_html: String,
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
    let compact_summary_path = root.join("complete-workflow-compact-summary.html");
    let bundle_path = root.join(format!("complete-workflow-export{V2_BUNDLE_SUFFIX}"));
    let report_outcome = export_active_report_with_selection(
        state,
        ReportExportFormat::FullEvidenceHtml,
        ControllerEvidenceHandling::Complete,
        |_| Ok(Some(report_path.clone())),
    )
    .expect("standalone report export");
    assert!(matches!(
        report_outcome,
        ExportReportOutcome::Exported {
            revision: Some(exported),
            format: ReportExportFormat::FullEvidenceHtml,
            ..
        } if exported == revision
    ));
    let report_replacement = export_active_report_with_selection(
        state,
        ReportExportFormat::FullEvidenceHtml,
        ControllerEvidenceHandling::Complete,
        |_| Ok(Some(report_path.clone())),
    )
    .expect("existing report requests replacement confirmation");
    let ExportReportOutcome::ConfirmationRequired {
        pending_export_id, ..
    } = report_replacement
    else {
        panic!("existing report did not request replacement confirmation");
    };
    confirm_pending_report_export_with(state, &pending_export_id, &SystemReportReplacePort)
        .expect("replace an existing report through the desktop boundary");
    let compact_summary_outcome = export_active_report_with_selection(
        state,
        ReportExportFormat::CompactSummaryHtml,
        ControllerEvidenceHandling::Complete,
        |_| Ok(Some(compact_summary_path.clone())),
    )
    .expect("compact share summary export");
    assert!(matches!(
        compact_summary_outcome,
        ExportReportOutcome::Exported {
            revision: Some(exported),
            format: ReportExportFormat::CompactSummaryHtml,
            ..
        } if exported == revision
    ));
    let compact_replacement = export_active_report_with_selection(
        state,
        ReportExportFormat::CompactSummaryHtml,
        ControllerEvidenceHandling::Complete,
        |_| Ok(Some(compact_summary_path.clone())),
    )
    .expect("existing compact report requests replacement confirmation");
    let ExportReportOutcome::ConfirmationRequired {
        pending_export_id, ..
    } = compact_replacement
    else {
        panic!("existing compact report did not request replacement confirmation");
    };
    confirm_pending_report_export_with(state, &pending_export_id, &SystemReportReplacePort)
        .expect("replace an existing compact report through the desktop boundary");
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
    assert_eq!(
        std::fs::read_to_string(&compact_summary_path).expect("exported compact HTML"),
        presentation.compact_summary_html
    );
    E2eExportedSnapshots {
        report_path,
        compact_summary_path,
        bundle_path,
        revision,
        presentation_id: presentation.presentation_id,
        report_html: presentation.report_html,
        compact_summary_html: presentation.compact_summary_html,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path, sync::mpsc, thread, time::Duration};

    use antennabench_analysis::AnalysisError;
    use antennabench_report::ReportError;
    use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError};
    use tempfile::TempDir;

    use super::{
        active_session_report_for, cancel_pending_report_export_for, check_ipc_payload,
        confirm_pending_report_export_with, copy_error_payload,
        export_active_report_with_selection, export_active_report_with_selection_and_disclosure,
        export_active_session_with_selection, export_bundle, open_bundle,
        open_session_with_selection, refresh_active_session_report_for, report_error_payload,
        suggested_compact_summary_name, suggested_report_name, with_suspended_foreground_operation,
        with_waiting_foreground_operation, ActiveSession, ActiveSessionState,
        ControllerEvidenceHandling, ExportReportOutcome, ExportSessionOutcome, OpenSessionOutcome,
        OpenedSession, OperationalHistoryHandling, ReportCompleteness, ReportExportFormat,
        ReportPresentation, ReportReplacePort, SessionErrorKind, SessionErrorPayload,
        SessionLifecycleV2, SystemReportReplacePort, REPORT_DOCUMENT_IPC_BYTES, SCHEMA_VERSION_V5,
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

    fn opened_report_state() -> ActiveSessionState {
        let state = ActiveSessionState::default();
        open_session_with_selection(&state, || Ok(Some(canonical_fixture())))
            .expect("open report fixture");
        state
    }

    fn expect_pending_export_id(outcome: ExportReportOutcome) -> String {
        let ExportReportOutcome::ConfirmationRequired {
            pending_export_id, ..
        } = outcome
        else {
            panic!("expected replacement confirmation");
        };
        pending_export_id
    }

    struct FailingReportReplacePort;

    impl ReportReplacePort for FailingReportReplacePort {
        fn replace(
            &self,
            temporary: tempfile::NamedTempFile,
            _destination: &Path,
        ) -> Result<(), String> {
            temporary.close().map_err(|error| error.to_string())?;
            Err("injected atomic replacement failure".into())
        }
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
            "session-3e698e14-13ff-4ce4-b6bb-71e66734c6e4"
        );
        assert!(!opened.summary.callsign.is_empty());
        assert!(opened
            .presentation
            .as_ref()
            .unwrap()
            .report_html
            .starts_with("<!doctype html>"));
        let payload = serde_json::to_value(super::OpenSessionOutcome::Opened {
            session: Box::new(opened.summary.clone()),
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
        let presentation = active_session_report_for(&state).unwrap();
        let serialized = serde_json::to_value(&presentation).unwrap();
        assert!(serialized.get("reportHtml").is_some());
        assert!(serialized.get("compactSummaryHtml").is_none());
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
    fn waiting_and_suspended_foreground_admission_remain_single_flight() {
        let state = ActiveSessionState::default();
        let guard = state.begin_foreground().unwrap();
        thread::scope(|scope| {
            let (started_tx, started_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            let state_for_waiter = &state;
            scope.spawn(move || {
                started_tx.send(()).unwrap();
                with_waiting_foreground_operation(state_for_waiter, || {
                    done_tx.send(()).unwrap();
                    Ok(())
                })
                .unwrap();
            });
            started_rx.recv().unwrap();
            assert!(done_rx.recv_timeout(Duration::from_millis(25)).is_err());
            drop(guard);
            done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        });

        let outer = state.begin_foreground().unwrap();
        let admitted = with_suspended_foreground_operation(&state, || {
            with_waiting_foreground_operation(&state, || Ok("operator mutation")).unwrap()
        })
        .unwrap();
        assert_eq!(admitted, "operator mutation");
        assert!(state.begin_foreground().is_err());
        drop(outer);
        assert!(state.begin_foreground().is_ok());
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
    fn compact_and_full_html_suggested_names_are_unambiguous() {
        let bundle = Path::new("/tmp/field-day.session.wsprabundle");
        assert_eq!(suggested_report_name(bundle), "field-day-report.html");
        assert_eq!(
            suggested_compact_summary_name(bundle),
            "field-day-compact-summary.html"
        );
        let unknown = Path::new("/tmp/no-recognized-suffix");
        assert_eq!(suggested_report_name(unknown), "antennabench-report.html");
        assert_eq!(
            suggested_compact_summary_name(unknown),
            "antennabench-compact-summary.html"
        );
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
        let exported = export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(html_destination.clone())),
        )
        .unwrap();
        assert_eq!(
            exported,
            ExportReportOutcome::Exported {
                file_name: "snapshot.html".into(),
                revision: first.revision,
                format: ReportExportFormat::FullEvidenceHtml,
            }
        );
        assert_eq!(
            fs::read_to_string(&html_destination).unwrap(),
            first.report_html
        );
        fs::write(&html_destination, "prior full report bytes").unwrap();
        let replacement = export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(html_destination.clone())),
        )
        .expect("existing full report requests confirmation");
        let ExportReportOutcome::ConfirmationRequired {
            pending_export_id,
            file_name,
            ..
        } = replacement
        else {
            panic!("existing full report did not request confirmation");
        };
        assert_eq!(file_name, "snapshot.html");
        assert_eq!(
            fs::read_to_string(&html_destination).unwrap(),
            "prior full report bytes"
        );
        confirm_pending_report_export_with(&state, &pending_export_id, &SystemReportReplacePort)
            .expect("atomically replace full report");
        assert_eq!(
            fs::read_to_string(&html_destination).unwrap(),
            first.report_html
        );
        assert_eq!(
            confirm_pending_report_export_with(
                &state,
                &pending_export_id,
                &SystemReportReplacePort,
            )
            .unwrap_err()
            .kind,
            SessionErrorKind::Conflict,
            "a pending replacement is single-use",
        );

        let compact_destination = temp.path().join("snapshot-compact-summary.html");
        let compact_exported = export_active_report_with_selection(
            &state,
            ReportExportFormat::CompactSummaryHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(compact_destination.clone())),
        )
        .unwrap();
        assert_eq!(
            compact_exported,
            ExportReportOutcome::Exported {
                file_name: "snapshot-compact-summary.html".into(),
                revision: first.revision,
                format: ReportExportFormat::CompactSummaryHtml,
            }
        );
        assert_eq!(
            fs::read_to_string(&compact_destination).unwrap(),
            first.compact_summary_html
        );
        fs::write(&compact_destination, "prior compact report bytes").unwrap();
        let replacement = export_active_report_with_selection(
            &state,
            ReportExportFormat::CompactSummaryHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(compact_destination.clone())),
        )
        .expect("existing compact report requests confirmation");
        let ExportReportOutcome::ConfirmationRequired {
            pending_export_id, ..
        } = replacement
        else {
            panic!("existing compact report did not request confirmation");
        };
        confirm_pending_report_export_with(&state, &pending_export_id, &SystemReportReplacePort)
            .expect("atomically replace compact report");
        assert_eq!(
            fs::read_to_string(&compact_destination).unwrap(),
            first.compact_summary_html
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
    fn cancelling_report_replacement_preserves_existing_bytes() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("cancelled.html");
        fs::write(&destination, "original report bytes").unwrap();
        let state = opened_report_state();
        let pending_export_id = expect_pending_export_id(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::FullEvidenceHtml,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(destination.clone())),
            )
            .unwrap(),
        );

        assert_eq!(
            cancel_pending_report_export_for(&state, &pending_export_id).unwrap(),
            ExportReportOutcome::Cancelled
        );
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "original report bytes"
        );
        assert!(state.0.lock().unwrap().pending_report_export.is_none());
    }

    #[test]
    fn failed_atomic_report_replacement_preserves_original_and_cleans_temporary_file() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("replace-failure.html");
        fs::write(&destination, "original report bytes").unwrap();
        let state = opened_report_state();
        let pending_export_id = expect_pending_export_id(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::CompactSummaryHtml,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(destination.clone())),
            )
            .unwrap(),
        );

        let error = confirm_pending_report_export_with(
            &state,
            &pending_export_id,
            &FailingReportReplacePort,
        )
        .unwrap_err();

        assert_eq!(error.kind, SessionErrorKind::Filesystem);
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "original report bytes"
        );
        assert_eq!(
            fs::read_dir(temp.path()).unwrap().count(),
            1,
            "the sibling temporary file was cleaned up"
        );
    }

    #[test]
    fn changed_destination_and_presentation_invalidate_pending_report_replacements() {
        let temp = TempDir::new().unwrap();
        let destination = temp.path().join("stale.html");
        fs::write(&destination, "original report bytes").unwrap();
        let state = opened_report_state();
        let pending_export_id = expect_pending_export_id(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::FullEvidenceHtml,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(destination.clone())),
            )
            .unwrap(),
        );
        fs::write(&destination, "changed after confirmation request").unwrap();

        let error = confirm_pending_report_export_with(
            &state,
            &pending_export_id,
            &SystemReportReplacePort,
        )
        .unwrap_err();
        assert_eq!(error.kind, SessionErrorKind::Conflict);
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "changed after confirmation request"
        );

        let pending_export_id = expect_pending_export_id(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::FullEvidenceHtml,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(destination.clone())),
            )
            .unwrap(),
        );
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
            .presentation_id += 1;

        let error = confirm_pending_report_export_with(
            &state,
            &pending_export_id,
            &SystemReportReplacePort,
        )
        .unwrap_err();
        assert_eq!(error.kind, SessionErrorKind::Conflict);
        assert_eq!(
            fs::read_to_string(destination).unwrap(),
            "changed after confirmation request"
        );
    }

    #[test]
    fn report_replacement_rejects_non_regular_destinations_without_exposing_paths() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("directory.html");
        fs::create_dir(&directory).unwrap();
        let state = opened_report_state();

        for format in [
            ReportExportFormat::FullEvidenceHtml,
            ReportExportFormat::CompactSummaryHtml,
        ] {
            let error = export_active_report_with_selection(
                &state,
                format,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(directory.clone())),
            )
            .unwrap_err();
            assert_eq!(error.kind, SessionErrorKind::Destination);
            assert!(!error
                .detail
                .contains(temp.path().to_string_lossy().as_ref()));
        }

        #[cfg(unix)]
        {
            let target = temp.path().join("target.html");
            let link = temp.path().join("link.html");
            fs::write(&target, "existing").unwrap();
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let error = export_active_report_with_selection(
                &state,
                ReportExportFormat::FullEvidenceHtml,
                ControllerEvidenceHandling::Complete,
                |_| Ok(Some(link.clone())),
            )
            .unwrap_err();
            assert_eq!(error.kind, SessionErrorKind::Destination);
            assert_eq!(fs::read_to_string(target).unwrap(), "existing");
            assert!(!error
                .detail
                .contains(temp.path().to_string_lossy().as_ref()));
        }
    }

    #[test]
    fn existing_report_confirmation_serializes_only_a_bounded_name_and_opaque_identity() {
        let temp = TempDir::new().unwrap();
        let long_name = format!("{}.html", "report-name".repeat(18));
        let destination = temp.path().join(long_name);
        fs::write(&destination, "existing").unwrap();
        let state = opened_report_state();

        let outcome = export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(destination)),
        )
        .unwrap();
        let json = serde_json::to_value(&outcome).unwrap();

        assert_eq!(json["status"], "confirmation_required");
        assert!(json["pendingExportId"].as_str().unwrap().len() <= 64);
        assert!(json["fileName"].as_str().unwrap().chars().count() <= 161);
        assert!(!json
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref()));
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
        assert_eq!(
            session.session_id,
            "session-3e698e14-13ff-4ce4-b6bb-71e66734c6e4"
        );

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
    fn full_report_export_selects_controller_detail_variant_only_when_applicable() {
        let temp = TempDir::new().expect("create isolated export directory");
        let source = temp.path().join("active.session.antennabundle");
        let state = ActiveSessionState::default();
        let presentation = ReportPresentation {
            presentation_id: 7,
            session_id: "session-controller-export".into(),
            revision: Some(4),
            lifecycle: Some(SessionLifecycleV2::Ended),
            completeness: ReportCompleteness::FullDetail,
            has_controller_evidence: true,
            operational_history: super::legacy_diagnostics_presentation(SCHEMA_VERSION_V5),
            report_html: "<p>complete sensitive controller details</p>".into(),
            compact_summary_html: "<p>compact</p>".into(),
            controller_omitted_report_html: Some(
                "<p>Omitted at export — retained in the session bundle</p>".into(),
            ),
            operational_history_report_html: "<p>redacted operational history</p>".into(),
            operational_history_controller_omitted_report_html: Some(
                "<p>both redacted and omitted</p>".into(),
            ),
        };
        let summary = OpenedSession {
            bundle_name: "active.session.antennabundle".into(),
            session_id: presentation.session_id.clone(),
            callsign: "N1TEST".into(),
            grid: "FN42".into(),
            antenna_count: 2,
            slot_count: 2,
            observation_count: 0,
            schema_version: SCHEMA_VERSION_V5,
            revision: presentation.revision,
            lifecycle: presentation.lifecycle,
            report_available: true,
            operational_history: presentation.operational_history.clone(),
        };
        state.0.lock().unwrap().active = Some(ActiveSession {
            source,
            live_projection: None,
            summary,
            presentation: Some(presentation),
        });

        let omitted_path = temp.path().join("omitted.html");
        export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::OmittedAtExport,
            |_| Ok(Some(omitted_path.clone())),
        )
        .expect("export omitted controller details");
        assert_eq!(
            fs::read_to_string(&omitted_path).unwrap(),
            "<p>Omitted at export — retained in the session bundle</p>"
        );

        let complete_path = temp.path().join("complete.html");
        export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::Complete,
            |_| Ok(Some(complete_path.clone())),
        )
        .expect("export complete controller details");
        assert_eq!(
            fs::read_to_string(&complete_path).unwrap(),
            "<p>complete sensitive controller details</p>"
        );

        let included_path = temp.path().join("included-redacted-history.html");
        export_active_report_with_selection_and_disclosure(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::OmittedAtExport,
            OperationalHistoryHandling::IncludedRedacted,
            |_| Ok(Some(included_path.clone())),
        )
        .expect("explicitly export the redacted operational history");
        assert_eq!(
            fs::read_to_string(&included_path).unwrap(),
            "<p>both redacted and omitted</p>"
        );

        let mut desktop = state.0.lock().unwrap();
        let manual_presentation = desktop
            .active
            .as_mut()
            .unwrap()
            .presentation
            .as_mut()
            .unwrap();
        manual_presentation.has_controller_evidence = false;
        manual_presentation.controller_omitted_report_html = None;
        drop(desktop);
        let manual_path = temp.path().join("manual.html");
        export_active_report_with_selection(
            &state,
            ReportExportFormat::FullEvidenceHtml,
            ControllerEvidenceHandling::OmittedAtExport,
            |_| Ok(Some(manual_path.clone())),
        )
        .expect("manual-only report ignores an inapplicable omission request");
        assert_eq!(
            fs::read_to_string(&manual_path).unwrap(),
            "<p>complete sensitive controller details</p>"
        );
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
