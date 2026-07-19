use antennabench_core::v6::{
    DiagnosticFactValueV6, DiagnosticTargetV6, DiagnosticsStateV6, OperationalDiagnosticV6,
    RuntimeContextV6, DIAGNOSTIC_MAX_RECORDS, DIAGNOSTIC_STREAM_MAX_BYTES,
};
use serde::Serialize;

use super::*;

const PRESENTED_CONTEXT_LIMIT: usize = 16;
const PRESENTED_DIAGNOSTIC_LIMIT: usize = 32;
const SUPPORT_SUMMARY_MAX_BYTES: usize = 16 * 1024;
const PRESENTATION_MAX_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticHistoryState {
    Complete,
    LegacyUnknown,
    RetentionCapped,
    PersistenceGap,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeContextPresentation {
    pub(crate) context_id: String,
    pub(crate) creator: bool,
    pub(crate) first_recorded_at: String,
    pub(crate) app_version: Option<String>,
    pub(crate) source_commit: Option<String>,
    pub(crate) source_state: String,
    pub(crate) build_channel: String,
    pub(crate) release_tag: Option<String>,
    pub(crate) target_triple: Option<String>,
    pub(crate) build_architecture: Option<String>,
    pub(crate) os_family: Option<String>,
    pub(crate) os_version: Option<String>,
    pub(crate) runtime_architecture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationalDiagnosticPresentation {
    pub(crate) diagnostic_id: String,
    pub(crate) runtime_context_id: String,
    pub(crate) occurred_at: String,
    pub(crate) operation: String,
    pub(crate) phase: String,
    pub(crate) code: String,
    pub(crate) summary: String,
    pub(crate) outcome: String,
    pub(crate) severity: String,
    pub(crate) revision_before: Option<u64>,
    pub(crate) revision_after: Option<u64>,
    pub(crate) evidence_effect: String,
    pub(crate) retry_disposition: String,
    pub(crate) retry_guidance_code: String,
    pub(crate) targets: Vec<String>,
    pub(crate) causes: Vec<String>,
    pub(crate) detail_truncated: bool,
    #[serde(skip)]
    support_causes: Vec<SafeSupportCause>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BundleDiagnosticsPresentation {
    pub(crate) history_state: DiagnosticHistoryState,
    pub(crate) retained_count: usize,
    pub(crate) retention_omitted_count: u64,
    pub(crate) presentation_omitted_count: usize,
    pub(crate) context_omitted_count: usize,
    pub(crate) record_limit: usize,
    pub(crate) byte_limit: usize,
    pub(crate) reason_code: Option<String>,
    pub(crate) contexts: Vec<RuntimeContextPresentation>,
    pub(crate) diagnostics: Vec<OperationalDiagnosticPresentation>,
    pub(crate) support_summary: String,
    pub(crate) support_summary_max_bytes: usize,
    pub(crate) support_summary_omitted_count: usize,
}

#[derive(Clone, Serialize)]
struct SafeSupportSummary {
    schema: &'static str,
    privacy: SafeSupportPrivacy,
    history: SafeSupportHistory,
    contexts: Vec<RuntimeContextPresentation>,
    diagnostics: Vec<SafeSupportDiagnostic>,
}

#[derive(Clone, Serialize)]
struct SafeSupportPrivacy {
    redacted_by_default: bool,
    omitted: &'static [&'static str],
}

#[derive(Clone, Serialize)]
struct SafeSupportHistory {
    state: DiagnosticHistoryState,
    retained_count: usize,
    retention_omitted_count: u64,
    presentation_omitted_count: usize,
    context_presentation_omitted_count: usize,
    support_summary_omitted_count: usize,
    support_summary_context_omitted_count: usize,
    reason_code: Option<String>,
    diagnostic_record_limit: usize,
    diagnostic_byte_limit: usize,
}

#[derive(Clone, Serialize)]
struct SafeSupportDiagnostic {
    occurred_at: String,
    runtime_context_id: String,
    operation: String,
    phase: String,
    code: String,
    outcome: String,
    revision_before: Option<u64>,
    revision_after: Option<u64>,
    evidence_effect: String,
    retry_disposition: String,
    retry_guidance_code: String,
    acquisition_windows: Vec<String>,
    causes: Vec<SafeSupportCause>,
    detail_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SafeSupportCause {
    code: String,
    phase: String,
    facts: Vec<String>,
}

struct PresentationHistory {
    state: DiagnosticHistoryState,
    retained_count: usize,
    retention_omitted_count: u64,
    presentation_omitted_count: usize,
    context_omitted_count: usize,
    reason_code: Option<String>,
}

pub(super) fn legacy_diagnostics_presentation(
    schema_version: u16,
) -> BundleDiagnosticsPresentation {
    finish_presentation(
        Vec::new(),
        Vec::new(),
        PresentationHistory {
            state: DiagnosticHistoryState::LegacyUnknown,
            retained_count: 0,
            retention_omitted_count: 0,
            presentation_omitted_count: 0,
            context_omitted_count: 0,
            reason_code: Some(format!(
                "bundle.schema_v{schema_version}_history_unavailable"
            )),
        },
    )
}

pub(super) fn present_bundle_diagnostics(
    schema_version: u16,
    bundle: &BundleV3Contents,
) -> BundleDiagnosticsPresentation {
    if schema_version < SCHEMA_VERSION_V6 {
        return legacy_diagnostics_presentation(schema_version);
    }

    let creator_id = bundle.manifest.creator_runtime_context_id.as_deref();
    let context_omitted_count = bundle
        .runtime_contexts
        .len()
        .saturating_sub(PRESENTED_CONTEXT_LIMIT);
    let contexts = select_contexts(&bundle.runtime_contexts, creator_id)
        .into_iter()
        .map(|context| present_context(context, creator_id == Some(context.context_id.as_str())))
        .collect();
    let presentation_omitted_count = bundle
        .diagnostics
        .len()
        .saturating_sub(PRESENTED_DIAGNOSTIC_LIMIT);
    let selected = bundle
        .diagnostics
        .iter()
        .skip(presentation_omitted_count)
        .collect::<Vec<_>>();
    let diagnostics = selected
        .iter()
        .map(|diagnostic| present_diagnostic(diagnostic))
        .collect();
    let status = bundle.session_state.diagnostics_status.as_ref();
    let history_state = match status.map(|status| status.state) {
        Some(DiagnosticsStateV6::Complete) => DiagnosticHistoryState::Complete,
        Some(DiagnosticsStateV6::Saturated) => DiagnosticHistoryState::RetentionCapped,
        Some(DiagnosticsStateV6::Gap) => DiagnosticHistoryState::PersistenceGap,
        None => DiagnosticHistoryState::Unavailable,
    };
    finish_presentation(
        contexts,
        diagnostics,
        PresentationHistory {
            state: history_state,
            retained_count: bundle.diagnostics.len(),
            retention_omitted_count: status.map_or(0, |status| status.omitted_count),
            presentation_omitted_count,
            context_omitted_count,
            reason_code: status.and_then(|status| status.reason_code.clone()),
        },
    )
}

fn select_contexts<'a>(
    contexts: &'a [RuntimeContextV6],
    creator_id: Option<&str>,
) -> Vec<&'a RuntimeContextV6> {
    if contexts.len() <= PRESENTED_CONTEXT_LIMIT {
        return contexts.iter().collect();
    }
    let mut selected = contexts
        .iter()
        .skip(contexts.len() - PRESENTED_CONTEXT_LIMIT)
        .collect::<Vec<_>>();
    if let Some(creator) = contexts
        .iter()
        .find(|context| Some(context.context_id.as_str()) == creator_id)
    {
        if !selected
            .iter()
            .any(|context| context.context_id == creator.context_id)
        {
            selected.remove(0);
            selected.insert(0, creator);
        }
    }
    selected
}

fn present_context(context: &RuntimeContextV6, creator: bool) -> RuntimeContextPresentation {
    RuntimeContextPresentation {
        context_id: context.context_id.clone(),
        creator,
        first_recorded_at: timestamp(context.first_recorded_at),
        app_version: context.build.app_version.clone(),
        source_commit: context.build.source_commit.clone(),
        source_state: machine(&context.build.source_state),
        build_channel: machine(&context.build.build_channel),
        release_tag: context.build.release_tag.clone(),
        target_triple: context.build.target_triple.clone(),
        build_architecture: context.build.build_architecture.clone(),
        os_family: context.platform.os_family.clone(),
        os_version: context.platform.os_version.clone(),
        runtime_architecture: context.platform.runtime_architecture.clone(),
    }
}

fn present_diagnostic(diagnostic: &OperationalDiagnosticV6) -> OperationalDiagnosticPresentation {
    OperationalDiagnosticPresentation {
        diagnostic_id: diagnostic.diagnostic_id.clone(),
        runtime_context_id: diagnostic.runtime_context_id.clone(),
        occurred_at: timestamp(diagnostic.occurred_at),
        operation: machine(&diagnostic.operation),
        phase: machine(&diagnostic.phase),
        code: diagnostic.code.clone(),
        summary: diagnostic.summary.clone(),
        outcome: machine(&diagnostic.outcome),
        severity: machine(&diagnostic.severity),
        revision_before: diagnostic.revision_before,
        revision_after: diagnostic.revision_after,
        evidence_effect: machine(&diagnostic.evidence_effect),
        retry_disposition: machine(&diagnostic.retry.disposition),
        retry_guidance_code: diagnostic.retry.guidance_code.clone(),
        targets: diagnostic.targets.iter().map(present_target).collect(),
        causes: diagnostic.causes.iter().map(present_cause).collect(),
        detail_truncated: machine(&diagnostic.detail_status.state) == "truncated",
        support_causes: diagnostic.causes.iter().map(safe_support_cause).collect(),
    }
}

fn present_target(target: &DiagnosticTargetV6) -> String {
    match target {
        DiagnosticTargetV6::Adapter { id } => format!("adapter: {id}"),
        DiagnosticTargetV6::Source { id } => format!("source: {id}"),
        DiagnosticTargetV6::Mutation { id } => format!("mutation: {id}"),
        DiagnosticTargetV6::Record { id } => format!("record: {id}"),
        DiagnosticTargetV6::Slot { id } => format!("slot: {id}"),
        DiagnosticTargetV6::Intent { id } => format!("intent: {id}"),
        DiagnosticTargetV6::AcquisitionWindow { start, end } => {
            format!("window: {} to {}", timestamp(*start), timestamp(*end))
        }
    }
}

fn present_cause(cause: &antennabench_core::v6::DiagnosticCauseV6) -> String {
    let facts = cause
        .facts
        .iter()
        .map(|fact| format!("{}={}", fact.name, present_fact_value(&fact.value)))
        .collect::<Vec<_>>();
    if facts.is_empty() {
        format!("{} ({})", cause.code, machine(&cause.phase))
    } else {
        format!(
            "{} ({}) · {}",
            cause.code,
            machine(&cause.phase),
            facts.join(", ")
        )
    }
}

fn present_fact_value(value: &DiagnosticFactValueV6) -> String {
    match value {
        DiagnosticFactValueV6::Bool(value) => value.to_string(),
        DiagnosticFactValueV6::I64(value) => value.to_string(),
        DiagnosticFactValueV6::U64(value) => value.to_string(),
        DiagnosticFactValueV6::Timestamp(value) => timestamp(*value),
        DiagnosticFactValueV6::Enum(value) | DiagnosticFactValueV6::Identifier(value) => {
            value.clone()
        }
    }
}

fn finish_presentation(
    mut contexts: Vec<RuntimeContextPresentation>,
    mut diagnostics: Vec<OperationalDiagnosticPresentation>,
    mut history: PresentationHistory,
) -> BundleDiagnosticsPresentation {
    loop {
        let (support_summary, support_summary_omitted_count) =
            build_support_summary(&contexts, &diagnostics, &history);
        let presentation = BundleDiagnosticsPresentation {
            history_state: history.state,
            retained_count: history.retained_count,
            retention_omitted_count: history.retention_omitted_count,
            presentation_omitted_count: history.presentation_omitted_count,
            context_omitted_count: history.context_omitted_count,
            record_limit: DIAGNOSTIC_MAX_RECORDS,
            byte_limit: DIAGNOSTIC_STREAM_MAX_BYTES,
            reason_code: history.reason_code.clone(),
            contexts: contexts.clone(),
            diagnostics: diagnostics.clone(),
            support_summary,
            support_summary_max_bytes: SUPPORT_SUMMARY_MAX_BYTES,
            support_summary_omitted_count,
        };
        let encoded_bytes = serde_json::to_vec(&presentation)
            .expect("diagnostics presentation contains only serializable values")
            .len();
        if encoded_bytes <= PRESENTATION_MAX_BYTES {
            return presentation;
        }
        if !diagnostics.is_empty() {
            diagnostics.remove(0);
            history.presentation_omitted_count += 1;
            continue;
        }
        if contexts.len() > 1 {
            contexts.pop();
            history.context_omitted_count += 1;
            continue;
        }
        return presentation;
    }
}

fn build_support_summary(
    contexts: &[RuntimeContextPresentation],
    diagnostics: &[OperationalDiagnosticPresentation],
    history: &PresentationHistory,
) -> (String, usize) {
    let safe_diagnostics = diagnostics
        .iter()
        .map(safe_support_diagnostic)
        .collect::<Vec<_>>();
    let mut summary = SafeSupportSummary {
        schema: "antennabench_support_summary.v1",
        privacy: SafeSupportPrivacy {
            redacted_by_default: true,
            omitted: &[
                "station identity and grid",
                "bundle filename and local paths",
                "target and arbitrary identifier values",
                "controller output, attachments, and evidence rows",
            ],
        },
        history: SafeSupportHistory {
            state: history.state,
            retained_count: history.retained_count,
            retention_omitted_count: history.retention_omitted_count,
            presentation_omitted_count: history.presentation_omitted_count,
            context_presentation_omitted_count: history.context_omitted_count,
            support_summary_omitted_count: 0,
            support_summary_context_omitted_count: 0,
            reason_code: history.reason_code.clone(),
            diagnostic_record_limit: DIAGNOSTIC_MAX_RECORDS,
            diagnostic_byte_limit: DIAGNOSTIC_STREAM_MAX_BYTES,
        },
        contexts: contexts.to_vec(),
        diagnostics: safe_diagnostics,
    };
    let original_count = summary.diagnostics.len();
    let original_context_count = summary.contexts.len();
    loop {
        summary.history.support_summary_omitted_count = original_count - summary.diagnostics.len();
        summary.history.support_summary_context_omitted_count =
            original_context_count - summary.contexts.len();
        let encoded = serde_json::to_string_pretty(&summary)
            .expect("support summary contains only serializable presentation values");
        if encoded.len() <= SUPPORT_SUMMARY_MAX_BYTES {
            return (encoded, original_count - summary.diagnostics.len());
        }
        if !summary.diagnostics.is_empty() {
            summary.diagnostics.remove(0);
            continue;
        }
        if summary.contexts.len() > 1 {
            summary.contexts.pop();
            continue;
        }
        return (
            "{\n  \"schema\": \"antennabench_support_summary.v1\",\n  \"history\": {\"state\": \"unavailable\"},\n  \"privacy\": {\"redacted_by_default\": true}\n}".into(),
            original_count,
        );
    }
}

fn safe_support_diagnostic(
    diagnostic: &OperationalDiagnosticPresentation,
) -> SafeSupportDiagnostic {
    let acquisition_windows = diagnostic
        .targets
        .iter()
        .filter(|target| target.starts_with("window: "))
        .cloned()
        .collect();
    SafeSupportDiagnostic {
        occurred_at: diagnostic.occurred_at.clone(),
        runtime_context_id: diagnostic.runtime_context_id.clone(),
        operation: diagnostic.operation.clone(),
        phase: diagnostic.phase.clone(),
        code: diagnostic.code.clone(),
        outcome: diagnostic.outcome.clone(),
        revision_before: diagnostic.revision_before,
        revision_after: diagnostic.revision_after,
        evidence_effect: diagnostic.evidence_effect.clone(),
        retry_disposition: diagnostic.retry_disposition.clone(),
        retry_guidance_code: diagnostic.retry_guidance_code.clone(),
        acquisition_windows,
        causes: diagnostic.support_causes.clone(),
        detail_truncated: diagnostic.detail_truncated,
    }
}

fn safe_support_cause(cause: &antennabench_core::v6::DiagnosticCauseV6) -> SafeSupportCause {
    let facts = cause
        .facts
        .iter()
        .filter_map(|fact| match &fact.value {
            DiagnosticFactValueV6::Identifier(_) => None,
            value => Some(format!("{}={}", fact.name, present_fact_value(value))),
        })
        .collect();
    SafeSupportCause {
        code: cause.code.clone(),
        phase: machine(&cause.phase),
        facts,
    }
}

fn machine(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn timestamp(value: chrono::DateTime<chrono::Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_support_summary_is_bounded_redacted_and_honest() {
        let view = legacy_diagnostics_presentation(5);
        assert_eq!(view.history_state, DiagnosticHistoryState::LegacyUnknown);
        assert!(view.support_summary.len() <= SUPPORT_SUMMARY_MAX_BYTES);
        assert!(view.support_summary.contains("redacted_by_default"));
        assert!(view.support_summary.contains("legacy_unknown"));
        assert!(!view.support_summary.contains("No failures"));
    }

    #[test]
    fn presentation_drops_oldest_rows_until_the_ipc_reserve_is_respected() {
        let contexts = (0..PRESENTED_CONTEXT_LIMIT)
            .map(|index| RuntimeContextPresentation {
                context_id: format!("ctx_{index}_{}", "c".repeat(512)),
                creator: index == 0,
                first_recorded_at: "2026-07-19T12:00:00Z".into(),
                app_version: Some("v".repeat(512)),
                source_commit: Some("s".repeat(512)),
                source_state: "clean".into(),
                build_channel: "development".into(),
                release_tag: None,
                target_triple: Some("t".repeat(512)),
                build_architecture: Some("aarch64".into()),
                os_family: Some("macos".into()),
                os_version: Some("15.6".into()),
                runtime_architecture: Some("aarch64".into()),
            })
            .collect();
        let diagnostics = (0..PRESENTED_DIAGNOSTIC_LIMIT)
            .map(|index| OperationalDiagnosticPresentation {
                diagnostic_id: format!("diag-{index}"),
                runtime_context_id: "ctx-1".into(),
                occurred_at: "2026-07-19T12:01:00Z".into(),
                operation: "wspr_live_acquisition".into(),
                phase: "preflight".into(),
                code: "resource.jsonl_line_bytes".into(),
                summary: "x".repeat(8 * 1024),
                outcome: "failed".into(),
                severity: "error".into(),
                revision_before: Some(1),
                revision_after: Some(1),
                evidence_effect: "none_committed".into(),
                retry_disposition: "requires_input_change".into(),
                retry_guidance_code: "wspr_live.reduce_input".into(),
                targets: vec!["window: start to end".into()],
                causes: vec!["observed_bytes=301337, limit_bytes=262144".into()],
                detail_truncated: false,
                support_causes: Vec::new(),
            })
            .collect();
        let view = finish_presentation(
            contexts,
            diagnostics,
            PresentationHistory {
                state: DiagnosticHistoryState::Complete,
                retained_count: PRESENTED_DIAGNOSTIC_LIMIT,
                retention_omitted_count: 0,
                presentation_omitted_count: 0,
                context_omitted_count: 0,
                reason_code: None,
            },
        );
        assert!(serde_json::to_vec(&view).unwrap().len() <= PRESENTATION_MAX_BYTES);
        assert!(view.presentation_omitted_count > 0);
        assert!(view.support_summary.len() <= SUPPORT_SUMMARY_MAX_BYTES);
    }
}
