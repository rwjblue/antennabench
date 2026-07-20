use super::*;

pub(in crate::html) fn render_reporter_activity_audit(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.reporter_activity.is_empty() {
        return;
    }
    out.push_str("<details class=\"audit-disclosure\"><summary>Review reporter-activity census provenance</summary><div class=\"disclosure-body\"><p class=\"muted\">Derived rates use only accepted, band-qualified census rows. Summary and row record IDs point back to the durable adapter evidence; bandless legacy rows are not used.</p>");
    out.push_str("<div class=\"table-wrap\"><table><caption>Census coverage used by derived rates</caption><thead><tr><th scope=\"col\">Cycle</th><th scope=\"col\">Band</th><th scope=\"col\">Coverage</th><th scope=\"col\">Active reporters</th><th scope=\"col\">Summary record IDs</th></tr></thead><tbody>");
    for cycle in &report.reporter_activity.census_cycles {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            timestamp(cycle.cycle_time),
            band(cycle.band),
            super::super::questions::coverage_text(cycle.coverage),
            cycle.active_reporters.len(),
            escape_html(&cycle.summary_record_ids.join(", "))
        );
    }
    out.push_str("</tbody></table></div>");
    let reporter_count = report
        .reporter_activity
        .census_cycles
        .iter()
        .map(|cycle| cycle.active_reporters.len())
        .sum::<usize>();
    if reporter_count > 0 {
        out.push_str("<div class=\"table-wrap\"><table><caption>Accepted active-reporter census rows</caption><thead><tr><th scope=\"col\">Cycle</th><th scope=\"col\">Band</th><th scope=\"col\">Reporter</th><th scope=\"col\">Grid</th><th scope=\"col\">Census record ID</th></tr></thead><tbody>");
        for cycle in &report.reporter_activity.census_cycles {
            for reporter in &cycle.active_reporters {
                write_html!(
                    out,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    timestamp(cycle.cycle_time),
                    band(cycle.band),
                    escape_html(&reporter.reporter),
                    super::location::optional_text(reporter.reporter_grid.as_deref()),
                    escape_html(&reporter.census_record_id)
                );
            }
        }
        out.push_str("</tbody></table></div>");
    } else if report.completeness == crate::ReportCompleteness::BoundedOverview {
        out.push_str("<p class=\"notice\">Per-reporter census provenance is omitted from this bounded overview; aggregate coverage and rates remain unsampled.</p>");
    }
    out.push_str("</div></details>");
}
