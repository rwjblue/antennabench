use super::*;

pub(in super::super) fn render_eligibility(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.eligibility_exclusions.is_empty() {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"eligibility-title\"><h2 id=\"eligibility-title\">Evidence eligibility disclosures</h2><p class=\"notice\">Affected evidence is excluded only from calculations that require it. Unrelated valid evidence remains included.</p><div class=\"table-wrap\"><table><caption>Validation-driven exclusions</caption><thead><tr><th scope=\"col\">Reason code</th><th scope=\"col\">Kind</th><th scope=\"col\">Scope</th><th scope=\"col\">Count</th></tr></thead><tbody>");
    for exclusion in &report.eligibility_exclusions {
        write_html!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&exclusion.code),
            eligibility_category(exclusion.category),
            eligibility_scope(exclusion.scope),
            exclusion.count
        );
    }
    out.push_str("</tbody></table></div></section>");
}
pub(in super::super) fn render_exclusion_records(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.exclusion_records.is_empty() {
        out.push_str("<p class=\"empty\">Record-level exclusion detail is unavailable or omitted by the bounded overview.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Every excluded observation retained by the report projection</caption><thead><tr><th scope=\"col\">Observation</th><th scope=\"col\">Reason</th><th scope=\"col\">Time</th><th scope=\"col\">Band</th><th scope=\"col\">Kind / source</th><th scope=\"col\">Mode</th><th scope=\"col\">Slot / label</th><th scope=\"col\">Confidence</th></tr></thead><tbody>");
    for record in &report.exclusion_records {
        write_html!(out, "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>", escape_html(&record.observation_id), exclusion_reason(record.reason), timestamp(record.timestamp), band(record.band), observation_kind(record.observation_kind), record_source(record.source), escape_html(record.mode.as_deref().unwrap_or("Not recorded")), escape_html(record.slot_id.as_deref().unwrap_or("Not assigned")), escape_html(record.assigned_label.as_deref().unwrap_or("Not assigned")), format_number(f64::from(record.assignment_confidence)));
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn eligibility_category(value: EligibilityExclusionCategory) -> &'static str {
    match value {
        EligibilityExclusionCategory::Missing => "Missing",
        EligibilityExclusionCategory::Malformed => "Malformed",
        EligibilityExclusionCategory::Contradictory => "Contradictory",
        EligibilityExclusionCategory::Unsupported => "Unsupported",
        EligibilityExclusionCategory::Duplicate => "Duplicate",
        EligibilityExclusionCategory::DeliberatelyExcluded => "Deliberately excluded",
    }
}
pub(in super::super) fn eligibility_scope(value: EligibilityScope) -> &'static str {
    match value {
        EligibilityScope::Field => "Field",
        EligibilityScope::Observation => "Observation",
        EligibilityScope::Slot => "Slot",
        EligibilityScope::ComparisonStratum => "Comparison stratum",
        EligibilityScope::ComparisonBlock => "Comparison block",
    }
}
pub(in super::super) fn render_notices(out: &mut CheckedHtmlWriter<'_>, notices: &[ReportNotice]) {
    if notices.is_empty() {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"notices-title\"><h2 id=\"notices-title\">Data notices</h2>");
    for notice in notices {
        write_html!(out, "<p class=\"notice\">{}</p>", notice_text(notice));
    }
    out.push_str("</section>");
}
