use std::path::Path;

use antennabench_core::{
    v2::MutationMember,
    v6::{
        DiagnosticCauseV6, DiagnosticDetailStateV6, DiagnosticDetailStatusV6, DiagnosticFactV6,
        DiagnosticFactValueV6, DiagnosticOperationV6, DiagnosticOutcomeV6, DiagnosticPhaseV6,
        DiagnosticRetryV6, DiagnosticSeverityV6, DiagnosticTargetV6, EvidenceEffectV6,
        OperationalDiagnosticV6, RetryDispositionV6, OPERATIONAL_DIAGNOSTIC_SCHEMA_V1,
    },
    SCHEMA_VERSION_V6,
};
use antennabench_storage::{BundleStore, LiveDiagnosticMutationV6, LiveSessionV3};

use crate::open_session::{SessionErrorKind, SessionErrorPayload};

pub(crate) fn persist_failure(
    source: &Path,
    operation: DiagnosticOperationV6,
    phase: DiagnosticPhaseV6,
    fallback_code: &str,
    evidence_effect: EvidenceEffectV6,
    targets: Vec<DiagnosticTargetV6>,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    if payload.diagnostic_persistence.is_some() {
        return payload;
    }
    let store = BundleStore::new(source);
    let Ok(mut writer) = crate::build_context::open_v3_writer(&store) else {
        return payload.with_diagnostic_not_persisted("diagnostic.writer_unavailable");
    };
    persist_failure_with_writer(
        &mut writer,
        operation,
        phase,
        fallback_code,
        evidence_effect,
        targets,
        payload,
    )
}

pub(crate) fn persist_failure_with_writer(
    writer: &mut LiveSessionV3,
    operation: DiagnosticOperationV6,
    phase: DiagnosticPhaseV6,
    fallback_code: &str,
    evidence_effect: EvidenceEffectV6,
    targets: Vec<DiagnosticTargetV6>,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    if payload.diagnostic_persistence.is_some() {
        return payload;
    }
    if writer.snapshot().manifest.schema_version != SCHEMA_VERSION_V6 {
        return payload;
    }
    let revision = writer.checkpoint().revision;
    let operation_detail = payload.operation.as_ref();
    let code = operation_detail.map_or_else(|| fallback_code.into(), |detail| detail.code.clone());
    let attempt_id = writer.allocate_id("diagnostic-attempt");
    let retry = retry_for(payload.kind);
    let mut facts = Vec::new();
    if let Some(detail) = operation_detail {
        facts.push(DiagnosticFactV6 {
            name: "stage".into(),
            value: DiagnosticFactValueV6::Enum(detail.stage.clone()),
        });
        if let Some(observed) = detail.observed {
            facts.push(DiagnosticFactV6 {
                name: "observed".into(),
                value: DiagnosticFactValueV6::U64(observed),
            });
        }
        if let Some(limit) = detail.limit {
            facts.push(DiagnosticFactV6 {
                name: "limit".into(),
                value: DiagnosticFactValueV6::U64(limit),
            });
        }
    }
    let diagnostic = OperationalDiagnosticV6 {
        schema: OPERATIONAL_DIAGNOSTIC_SCHEMA_V1.into(),
        diagnostic_id: writer.allocate_id("diagnostic"),
        correlation_id: writer.allocate_id("diagnostic-operation"),
        attempt_id,
        mutation: MutationMember {
            mutation_id: "pending-diagnostic".into(),
            member_index: 0,
            member_count: 1,
        },
        runtime_context_id: String::new(),
        occurred_at: chrono::Utc::now(),
        operation,
        phase,
        code: code.clone(),
        summary: summary_for(operation).into(),
        outcome: DiagnosticOutcomeV6::Failed,
        severity: DiagnosticSeverityV6::Error,
        revision_before: Some(revision),
        revision_after: Some(revision),
        diagnostic_revision: revision,
        evidence_effect,
        retry,
        targets,
        causes: vec![DiagnosticCauseV6 { code, phase, facts }],
        detail_status: DiagnosticDetailStatusV6 {
            state: DiagnosticDetailStateV6::Complete,
            omitted_fact_count: 0,
        },
    };
    let mutation_id = writer.allocate_id("diagnostic-mutation");
    let result = writer.append_diagnostic(LiveDiagnosticMutationV6 {
        expected_revision: revision,
        mutation_id,
        diagnostic,
    });
    match result {
        Ok(receipt) => payload.with_diagnostic_persisted(receipt.diagnostic_id),
        Err(_) => payload.with_diagnostic_not_persisted("diagnostic.persistence_failed"),
    }
}

fn retry_for(kind: SessionErrorKind) -> DiagnosticRetryV6 {
    let (disposition, guidance_code) = match kind {
        SessionErrorKind::StaleRevision | SessionErrorKind::Conflict => (
            RetryDispositionV6::RequiresStateChange,
            "refresh_state_then_retry",
        ),
        SessionErrorKind::Validation | SessionErrorKind::Resource => (
            RetryDispositionV6::RequiresInputChange,
            "change_input_profile_or_code",
        ),
        SessionErrorKind::Unsupported => {
            (RetryDispositionV6::NotRetryable, "operation_not_supported")
        }
        _ => (RetryDispositionV6::Retryable, "retry_when_condition_clears"),
    };
    DiagnosticRetryV6 {
        disposition,
        guidance_code: guidance_code.into(),
    }
}

fn summary_for(operation: DiagnosticOperationV6) -> &'static str {
    match operation {
        DiagnosticOperationV6::SessionMutation => "A session mutation did not commit.",
        DiagnosticOperationV6::CheckpointRecovery => "Checkpoint recovery did not complete.",
        DiagnosticOperationV6::WsprLiveAcquisition => "WSPR.live acquisition did not complete.",
        DiagnosticOperationV6::AdapterFileImport => "A file import did not complete.",
        DiagnosticOperationV6::WsjtxIntake => "WSJT-X intake did not complete.",
        DiagnosticOperationV6::AntennaControllerAttach => "Controller attachment did not complete.",
        DiagnosticOperationV6::AntennaControllerSwitch => "Controller switching did not complete.",
        DiagnosticOperationV6::AntennaControllerVerify => {
            "Controller verification did not complete."
        }
        DiagnosticOperationV6::ReportRender => "Report rendering did not complete.",
        DiagnosticOperationV6::ReportExport => "Report export did not complete.",
        DiagnosticOperationV6::BundleExport => "Bundle export did not complete.",
    }
}

pub(crate) fn persist_wsjtx_start_failure(
    source: &Path,
    payload: SessionErrorPayload,
) -> SessionErrorPayload {
    persist_failure(
        source,
        DiagnosticOperationV6::WsjtxIntake,
        DiagnosticPhaseV6::Admission,
        "wsjtx.intake_start_failed",
        EvidenceEffectV6::NoneCommitted,
        vec![DiagnosticTargetV6::Source {
            id: "wsjtx-udp".into(),
        }],
        payload,
    )
}

pub(crate) fn persist_wsjtx_gap(source: &Path, code: &str, receiver_ended: bool) {
    let message = if receiver_ended {
        "WSJT-X reception stopped after recording an acquisition gap."
    } else {
        "WSJT-X intake stopped after recording an acquisition gap."
    };
    let payload = SessionErrorPayload::new(
        SessionErrorKind::Resource,
        message,
        "the bounded acquisition gap remains in adapter evidence",
    )
    .with_operation(code, "acquire");
    let _ = persist_failure(
        source,
        DiagnosticOperationV6::WsjtxIntake,
        DiagnosticPhaseV6::Acquire,
        "wsjtx.acquisition_gap",
        EvidenceEffectV6::EarlierEvidenceRetained,
        vec![DiagnosticTargetV6::Source {
            id: "wsjtx-udp".into(),
        }],
        payload,
    );
}
