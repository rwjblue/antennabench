use super::*;

pub(in super::super) fn render_audit_appendix(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"audit-appendix\" class=\"panel question-section\" aria-labelledby=\"audit-title\"><h2 id=\"audit-title\">Audit appendix</h2><p class=\"muted\">Open only the supporting detail needed for review. Closed disclosures remain closed in default print output.</p>");
    if snapshot_has_detail(report) {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review committed snapshot, lifecycle, acquisition, and controller attempts</summary><div class=\"disclosure-body\">");
        render_snapshot(out, report);
        out.push_str("</div></details>");
    }
    out.push_str("<details class=\"audit-disclosure\"><summary>Review station, antenna, and planned schedule detail</summary><div class=\"disclosure-body\">");
    render_context(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review comparison blocks and data-quality timeline</summary><div class=\"disclosure-body\">");
    render_comparison_blocks(out, report);
    render_comparison_timeline(out, report);
    out.push_str("</div></details></section>");
}
