use std::{
    error::Error as StdError,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
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
    Band, BundleContents, BundleValidationError, BundleValidationReport, SCHEMA_VERSION_V2,
    SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
};
use antennabench_report::{
    build_report_with_snapshot, render_compact_summary_html, render_standalone_html,
    ReportAdapterEvidence, ReportAntennaControlAttempt, ReportCompleteness, ReportError,
    ReportEventCorrection, ReportEventCorrectionAction, ReportImportedEvidence,
    ReportLifecycleEvent, ReportLifecycleEventKind, ReportOperatorEvent, ReportOperatorEventKind,
    ReportSnapshotContext, ReportWsprAttribution, ReportWsprCycle, ReportWsprReadinessBasis,
};
use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError, LivePersistenceError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

use crate::antenna_control::AntennaControllerState;
use crate::wsjtx_session::WsjtxSessionState;

const SESSION_SUMMARY_IPC_BYTES: u64 = 64 * 1024;
const REPORT_DOCUMENT_IPC_BYTES: u64 = 16 * 1024 * 1024;

mod commands;
mod errors;
mod projection;
mod state;

pub(crate) use commands::{
    active_session_report, check_ipc_payload, export_active_session, export_active_session_report,
    open_session_bundle, refresh_active_session_report,
};
pub(crate) use errors::{
    storage_error_payload, OpenedSession, SessionErrorKind, SessionErrorPayload,
};
pub(crate) use state::{
    activate_created_bundle, active_session_source, reload_active_session,
    with_foreground_operation, ActiveSessionState,
};

use commands::bundle_suffix;
use errors::{
    report_error_payload, ExportReportOutcome, ExportSessionError, ExportSessionOutcome,
    OpenSessionError, OpenSessionOutcome, ReportExportFormat, ReportPresentation,
};
use projection::{load_snapshot, open_bundle, prepare_presentation};
use state::{assign_presentation_id, ActiveSession};

#[cfg(test)]
use commands::{
    active_session_report_for, export_active_report_with_selection,
    export_active_session_with_selection, export_bundle, open_session_with_selection,
    refresh_active_session_report_for, suggested_compact_summary_name, suggested_report_name,
};
#[cfg(test)]
use errors::copy_error_payload;

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
    let report_outcome =
        export_active_report_with_selection(state, ReportExportFormat::FullEvidenceHtml, |_| {
            Ok(Some(report_path.clone()))
        })
        .expect("standalone report export");
    assert!(matches!(
        report_outcome,
        ExportReportOutcome::Exported {
            revision: Some(exported),
            format: ReportExportFormat::FullEvidenceHtml,
            ..
        } if exported == revision
    ));
    assert!(export_active_report_with_selection(
        state,
        ReportExportFormat::FullEvidenceHtml,
        |_| Ok(Some(report_path.clone())),
    )
    .is_err());
    let compact_summary_outcome =
        export_active_report_with_selection(state, ReportExportFormat::CompactSummaryHtml, |_| {
            Ok(Some(compact_summary_path.clone()))
        })
        .expect("compact share summary export");
    assert!(matches!(
        compact_summary_outcome,
        ExportReportOutcome::Exported {
            revision: Some(exported),
            format: ReportExportFormat::CompactSummaryHtml,
            ..
        } if exported == revision
    ));
    assert!(export_active_report_with_selection(
        state,
        ReportExportFormat::CompactSummaryHtml,
        |_| Ok(Some(compact_summary_path.clone())),
    )
    .is_err());
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
    use std::{fs, io, path::Path};

    use antennabench_analysis::AnalysisError;
    use antennabench_report::ReportError;
    use antennabench_storage::{BundleCopyError, BundleStore, BundleStoreError};
    use tempfile::TempDir;

    use super::{
        active_session_report_for, check_ipc_payload, copy_error_payload,
        export_active_report_with_selection, export_active_session_with_selection, export_bundle,
        open_bundle, open_session_with_selection, refresh_active_session_report_for,
        report_error_payload, suggested_compact_summary_name, suggested_report_name,
        ActiveSessionState, ExportReportOutcome, ExportSessionOutcome, OpenSessionOutcome,
        ReportExportFormat, SessionErrorKind, SessionErrorPayload, REPORT_DOCUMENT_IPC_BYTES,
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
        assert_eq!(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::FullEvidenceHtml,
                |_| Ok(Some(html_destination.clone())),
            )
            .unwrap_err()
            .kind,
            SessionErrorKind::Destination
        );

        let compact_destination = temp.path().join("snapshot-compact-summary.html");
        let compact_exported = export_active_report_with_selection(
            &state,
            ReportExportFormat::CompactSummaryHtml,
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
        assert_eq!(
            export_active_report_with_selection(
                &state,
                ReportExportFormat::CompactSummaryHtml,
                |_| Ok(Some(compact_destination.clone())),
            )
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
