use super::*;
use antennabench_analysis::{ReporterActivityCoverage, ReporterActivityUnknownReason};

pub(in super::super) fn render_reporter_activity_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"reporter-activity\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reporter-activity-title\"><h2 id=\"reporter-activity-title\">Detection among receivers active in both cycles</h2><p class=\"muted\">Evidence basis and denominator: receivers proven by the band-qualified census to be active during both transmit cycles of one eligible block. Per-cycle context, groups, and blocks remain separate; this does not select a winner.</p>");
    if report.reporter_activity.cycle_rates.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified reporter-activity analysis is available. Missing census evidence is not zero activity and does not mean that no station was listening.</p></section>");
        return;
    }

    out.push_str("<div class=\"table-wrap\"><table><caption>Hearing-rate-given-active by separate comparison group and cycle</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Cycle / antenna</th><th scope=\"col\">Heard / active</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    for row in &report.reporter_activity.cycle_rates {
        let result = rate_text(
            row.heard_reporter_count,
            row.active_reporter_count,
            row.hearing_rate,
            row.coverage,
        );
        write_html!(
            out,
            "<tr><td>{}</td><td>{}<br><span class=\"muted\">{} · {}</span></td><td>{}</td><td>{}</td></tr>",
            comparison_stratum(&row.stratum),
            escape_html(&row.antenna_label),
            timestamp(row.cycle_starts_at),
            escape_html(&row.slot_id),
            result,
            coverage_text(row.coverage),
        );
    }
    out.push_str("</tbody></table></div>");

    if !report.reporter_activity.paired_rates.is_empty() {
        let (left_label, right_label) = report_antenna_labels(report);
        write_html!(out, "<div class=\"table-wrap\"><table><caption>Paired hearing rates restricted to reporters active in both cycles</caption><thead><tr><th scope=\"col\">Comparison group / block</th><th scope=\"col\">Active in both</th><th scope=\"col\">{}</th><th scope=\"col\">{}</th><th scope=\"col\">Coverage</th></tr></thead><tbody>", left_label, right_label);
        for row in &report.reporter_activity.paired_rates {
            write_html!(out, "<tr><td>{}<br><span class=\"muted\">Block {}</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), row.block_index + 1, row.active_in_both_count, rate_text(row.left_heard_count, row.active_in_both_count, row.left_hearing_rate, row.coverage), rate_text(row.right_heard_count, row.active_in_both_count, row.right_hearing_rate, row.coverage), coverage_text(row.coverage));
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("<p class=\"muted\">An active station that did not report the session callsign is below-threshold evidence for that cycle. A station absent from the census remains no evidence at all. Partial or truncated census rows visibly qualify every affected rate.</p></section>");
}

pub(in super::super) fn coverage_text(coverage: ReporterActivityCoverage) -> &'static str {
    match coverage {
        ReporterActivityCoverage::Complete => "Complete band-qualified census",
        ReporterActivityCoverage::Partial => {
            "Partial census — malformed rows may reduce the denominator"
        }
        ReporterActivityCoverage::Truncated => {
            "Truncated census — capture limit may reduce the denominator"
        }
        ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::NoCensusCoverage) => {
            "Coverage unknown — no band-qualified census covers this cycle"
        }
        ReporterActivityCoverage::Unknown(
            ReporterActivityUnknownReason::UnsupportedReceiveDirection,
        ) => "Coverage unknown — receiver census does not measure receive-direction paths",
    }
}

fn rate_text(
    heard: usize,
    active: usize,
    rate: Option<f64>,
    coverage: ReporterActivityCoverage,
) -> String {
    if !coverage.is_known() {
        return "Not available (coverage unknown; not zero)".to_string();
    }
    rate.map_or_else(
        || format!("{heard} / {active}; rate not defined with no active reporters"),
        |rate| format!("{heard} / {active} ({:.1}%)", rate * 100.0),
    )
}
