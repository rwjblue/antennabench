use super::*;

pub(in super::super) fn render_audit_appendix(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    controller_evidence: ControllerEvidenceHandling,
) {
    out.push_str("<section id=\"audit-appendix\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"audit-title\"><h2 id=\"audit-title\">Audit appendix</h2><p class=\"muted\">Open only the supporting detail needed for review. Closed disclosures remain closed in default print output.</p>");
    if controller_evidence == ControllerEvidenceHandling::OmittedAtExport
        && !report.snapshot.antenna_control_attempts.is_empty()
    {
        out.push_str("<p class=\"notice controller-details-omission\"><strong>Controller command details omitted at export:</strong> The exporter explicitly chose to omit controller programs, arguments, targets, and output from this report. The lossless session bundle retains the complete controller evidence.</p>");
    }
    if snapshot_has_detail(report) {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review committed snapshot, lifecycle, acquisition, and controller attempts</summary><div class=\"disclosure-body\">");
        render_snapshot(out, report, controller_evidence);
        out.push_str("</div></details>");
    }
    render_reporter_activity_audit(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review station, antenna, and planned schedule detail</summary><div class=\"disclosure-body\">");
    render_context(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review comparison blocks and data-quality timeline</summary><div class=\"disclosure-body\">");
    render_comparison_blocks(out, report);
    render_comparison_timeline(out, report);
    out.push_str("</div></details></section>");
}
