use crate::{
    ReportAcquisitionWorkflowStatus, ReportError, ReportLifecycleEventKind, SessionReport,
};
use antennabench_core::v2::SessionLifecycleV2;

use super::{evidence_summary, ControllerEvidenceHandling};
use crate::html::{
    shared::{evidence_coverage, CheckedHtmlWriter},
    templates::{
        render_template, ContextTemplate, EvidenceOverviewEndTemplate,
        EvidenceOverviewStartTemplate, SnapshotTemplate,
    },
    view::{ContextView, SnapshotView},
};

pub(in super::super) fn render_snapshot(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    controller_evidence: ControllerEvidenceHandling,
) -> Result<(), ReportError> {
    if !snapshot_has_detail(report) {
        return Ok(());
    }
    render_template(
        out,
        &SnapshotTemplate {
            view: SnapshotView::new(report, controller_evidence),
        },
    )
}

pub(in super::super) fn snapshot_has_detail(report: &SessionReport) -> bool {
    let snapshot = &report.snapshot;
    snapshot.checkpoint_revision.is_some()
        || snapshot.lifecycle.is_some()
        || !snapshot.lifecycle_events.is_empty()
        || !snapshot.operator_events.is_empty()
        || !snapshot.wspr_cycles.is_empty()
        || !snapshot.antenna_control_attempts.is_empty()
        || snapshot.adapter_evidence.record_count > 0
        || snapshot.adapter_evidence.gap_count > 0
        || snapshot.adapter_evidence.workflow_status
            != ReportAcquisitionWorkflowStatus::NotConfigured
        || !snapshot.adapter_evidence.imports.is_empty()
}

pub(in super::super) fn lifecycle(value: SessionLifecycleV2) -> &'static str {
    match value {
        SessionLifecycleV2::Draft => "Draft",
        SessionLifecycleV2::Ready => "Ready",
        SessionLifecycleV2::Running => "Running / in progress",
        SessionLifecycleV2::Interrupted => "Interrupted / in progress",
        SessionLifecycleV2::Ended => "Ended / final",
        SessionLifecycleV2::Abandoned => "Abandoned / final",
    }
}

pub(in super::super) fn lifecycle_event(value: ReportLifecycleEventKind) -> &'static str {
    match value {
        ReportLifecycleEventKind::Started => "Started",
        ReportLifecycleEventKind::Interrupted => "Interrupted",
        ReportLifecycleEventKind::InterruptionDetected => "Interruption detected",
        ReportLifecycleEventKind::Resumed => "Resumed",
        ReportLifecycleEventKind::Ended => "Ended",
        ReportLifecycleEventKind::Abandoned => "Abandoned",
    }
}

pub(in super::super) fn render_context(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ContextTemplate {
            view: ContextView::new(report),
        },
    )
}

pub(in super::super) fn render_overall(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &EvidenceOverviewStartTemplate {
            coverage: evidence_coverage(report.evidence.evidence_quality),
        },
    )?;
    evidence_summary(out, &report.evidence.overall)?;
    render_template(out, &EvidenceOverviewEndTemplate)
}
