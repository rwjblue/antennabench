use super::*;
use antennabench_analysis::{
    ReporterActivityCoverage, ReporterActivityJointOutcome, ReporterActivityUnknownReason,
};

pub(in super::super) fn render_reporter_activity_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"reporter-activity\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reporter-activity-title\"><h2 id=\"reporter-activity-title\">Detection among receivers active in both cycles</h2><p class=\"muted\">Estimand: detection outcome, conditional on a receiver being active during both transmit cycles of an eligible block. Counts are descriptive; groups and blocks remain separate, and repeated opportunities from one receiver are not independent observations.</p>");
    if report.reporter_activity.cycle_rates.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified reporter-activity analysis is available. Missing census evidence is not zero activity and does not mean that no station was listening.</p></section>");
        return;
    }

    let (left_label, right_label) = report_antenna_labels(report);
    if !report.reporter_activity.joint_summaries.is_empty() {
        write_html!(out, "<div class=\"table-wrap\"><table><caption>Joint detection outcomes by separate comparison group</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Unique active receivers</th><th scope=\"col\">Eligible blocks / order</th><th scope=\"col\">Receiver-block opportunities</th><th scope=\"col\">Heard both</th><th scope=\"col\">{} only</th><th scope=\"col\">{} only</th><th scope=\"col\">Heard neither</th><th scope=\"col\">Detection rate — {} / {}</th><th scope=\"col\">Coverage</th></tr></thead><tbody>", left_label, right_label, left_label, right_label);
        for row in &report.reporter_activity.joint_summaries {
            write_html!(out, "<tr><td>{}</td><td>{}</td><td>{} <span class=\"muted\">({} {}→{}; {} {}→{})</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}<br><span class=\"muted\">{} of {} blocks have known coverage</span></td></tr>", comparison_stratum(&row.stratum), row.unique_active_receiver_count, row.eligible_block_count, row.left_then_right_block_count, left_label, right_label, row.right_then_left_block_count, right_label, left_label, row.receiver_block_opportunity_count, row.heard_both_count, row.left_only_count, row.right_only_count, row.heard_neither_count, aggregate_rate_text(row.left_detection_rate), aggregate_rate_text(row.right_detection_rate), coverage_text(row.coverage), row.known_coverage_block_count, row.eligible_block_count);
        }
        out.push_str("</tbody></table></div><p class=\"muted\">The four outcome columns partition receiver-block opportunities exactly. Unique receivers are counted once per comparison group; receiver-block opportunities count the same receiver again when active in another eligible block.</p>");
    }

    out.push_str("<details class=\"audit-disclosure\"><summary>Review per-block joint outcomes and per-cycle context</summary>");
    if !report.reporter_activity.paired_rates.is_empty() {
        write_html!(out, "<div class=\"table-wrap\"><table><caption>Per-block joint detection outcome audit</caption><thead><tr><th scope=\"col\">Comparison group / block</th><th scope=\"col\">Order / slots</th><th scope=\"col\">Active in both</th><th scope=\"col\">Both</th><th scope=\"col\">{} only</th><th scope=\"col\">{} only</th><th scope=\"col\">Neither</th><th scope=\"col\">Detection rate — {} / {}</th><th scope=\"col\">Coverage</th></tr></thead><tbody>", left_label, right_label, left_label, right_label);
        for row in &report.reporter_activity.paired_rates {
            write_html!(out, "<tr><td>{}<br><span class=\"muted\">Block {}</span></td><td>{}<br><span class=\"muted\">{} / {}</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), row.block_index + 1, labeled_comparison_order(row.order, &left_label, &right_label), escape_html(&row.left_slot_id), escape_html(&row.right_slot_id), row.active_in_both_count, row.heard_both_count, row.left_only_count, row.right_only_count, row.heard_neither_count, rate_text(row.left_heard_count, row.active_in_both_count, row.left_hearing_rate, row.coverage), rate_text(row.right_heard_count, row.active_in_both_count, row.right_hearing_rate, row.coverage), coverage_text(row.coverage));
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Hearing-rate-given-active by separate comparison group and cycle</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Cycle / antenna</th><th scope=\"col\">Heard / active</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    for row in &report.reporter_activity.cycle_rates {
        let result = rate_text(
            row.heard_reporter_count,
            row.active_reporter_count,
            row.hearing_rate,
            row.coverage,
        );
        write_html!(out, "<tr><td>{}</td><td>{}<br><span class=\"muted\">{} · {}</span></td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.antenna_label), timestamp(row.cycle_starts_at), escape_html(&row.slot_id), result, coverage_text(row.coverage));
    }
    out.push_str("</tbody></table></div>");
    if report
        .reporter_activity
        .paired_rates
        .iter()
        .any(|row| !row.receivers.is_empty())
    {
        out.push_str("<div class=\"table-wrap\"><table><caption>Receiver-level joint outcome audit</caption><thead><tr><th scope=\"col\">Comparison group / block</th><th scope=\"col\">Receiver</th><th scope=\"col\">Locator</th><th scope=\"col\">Outcome</th></tr></thead><tbody>");
        for row in &report.reporter_activity.paired_rates {
            for receiver in &row.receivers {
                write_html!(out, "<tr><td>{}<br><span class=\"muted\">Block {}</span></td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), row.block_index + 1, escape_html(&receiver.receiver), receiver.receiver_grid.as_deref().map(escape_html).unwrap_or_else(|| "Not recorded".to_string()), joint_outcome_text(receiver.outcome, &left_label, &right_label));
            }
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</details>");
    out.push_str("<p class=\"muted\">An active station that did not report the session callsign is below-threshold evidence for that cycle. A station absent from the census remains no evidence at all. Partial or truncated census rows visibly qualify every affected rate.</p></section>");
}

fn aggregate_rate_text(rate: Option<f64>) -> String {
    rate.map_or_else(
        || "Not defined".to_string(),
        |rate| format!("{:.1}%", rate * 100.0),
    )
}

fn joint_outcome_text(
    outcome: ReporterActivityJointOutcome,
    left_label: &str,
    right_label: &str,
) -> String {
    match outcome {
        ReporterActivityJointOutcome::HeardBoth => "Heard both".to_string(),
        ReporterActivityJointOutcome::LeftOnly => format!("Heard {left_label} only"),
        ReporterActivityJointOutcome::RightOnly => format!("Heard {right_label} only"),
        ReporterActivityJointOutcome::HeardNeither => "Heard neither".to_string(),
    }
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
        ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::UnsupportedSignalMode) => {
            "Coverage unknown — the live receiver census measures WSPR activity only"
        }
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
