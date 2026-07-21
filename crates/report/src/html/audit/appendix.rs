use crate::{ReportError, SessionReport};

use super::*;
use crate::html::templates::{
    render_template, AuditAppendixEndTemplate, AuditAppendixStartTemplate,
    AuditComparisonDisclosureStartTemplate, AuditContextDisclosureStartTemplate,
    AuditDisclosureEndTemplate, AuditSnapshotDisclosureStartTemplate,
};

pub(in super::super) fn render_audit_appendix(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    controller_evidence: ControllerEvidenceHandling,
) -> Result<(), ReportError> {
    render_template(
        out,
        &AuditAppendixStartTemplate {
            controller_details_omitted: controller_evidence
                == ControllerEvidenceHandling::OmittedAtExport
                && !report.snapshot.antenna_control_attempts.is_empty(),
        },
    )?;
    if snapshot_has_detail(report) {
        render_template(out, &AuditSnapshotDisclosureStartTemplate)?;
        render_snapshot(out, report, controller_evidence)?;
        render_template(out, &AuditDisclosureEndTemplate)?;
    }
    render_reporter_activity_audit(out, report)?;
    render_template(out, &AuditContextDisclosureStartTemplate)?;
    render_context(out, report)?;
    render_template(out, &AuditDisclosureEndTemplate)?;
    render_template(out, &AuditComparisonDisclosureStartTemplate)?;
    render_comparison_blocks(out, report)?;
    render_comparison_timeline(out, report)?;
    render_template(out, &AuditDisclosureEndTemplate)?;
    render_template(out, &AuditAppendixEndTemplate)
}
