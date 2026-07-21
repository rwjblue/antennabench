use super::super::geometry::geometry_class;
use super::*;
use antennabench_analysis::{
    ReporterActivityCoverage, ReporterActivityJointOutcome, ReporterActivityUnknownReason,
};

pub(in super::super) fn render_reporter_activity_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"reporter-activity\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reporter-activity-title\"><h2 id=\"reporter-activity-title\">Which antenna was heard more often by the same active receivers?</h2><p>This section asks a narrower question than raw path counts: when a receiver was confirmed active during both back-to-back antenna cycles, which antenna’s transmission did it report? Each receiver-block is one shared opportunity, so both antenna rates use the same listening population.</p><p class=\"muted\">If an active receiver reported neither transmission, both are recorded as non-detections for that opportunity. Receivers absent from the activity census are excluded as missing evidence, never counted as misses. The same receiver can contribute another descriptive opportunity in a later block; these are not independent inferential samples, and comparison groups remain separate.</p>");
    if report.reporter_activity.cycle_rates.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified reporter-activity analysis is available. Missing census evidence is not zero activity and does not mean that no station was listening.</p></section>");
        return;
    }

    let (left_label, right_label) = report_antenna_labels(report);
    if !report.reporter_activity.joint_summaries.is_empty() {
        for (index, row) in report.reporter_activity.joint_summaries.iter().enumerate() {
            write_html!(out, "<article class=\"antenna-card activity-group\" aria-labelledby=\"activity-group-{index}\"><h3 id=\"activity-group-{index}\">{}</h3><p class=\"muted activity-coverage\"><strong>{}</strong> · {} of {} eligible blocks have known coverage · {} unique active receivers</p>", comparison_stratum(&row.stratum), coverage_text(row.coverage), row.known_coverage_block_count, row.eligible_block_count, row.unique_active_receiver_count);
            if row.coverage.is_known()
                && row.receiver_block_opportunity_count > 0
                && row.left_detection_rate.is_some()
                && row.right_detection_rate.is_some()
            {
                render_detection_rates(out, row, &left_label, &right_label);
                render_outcome_strip(out, row, &left_label, &right_label);
            } else {
                out.push_str("<p class=\"empty\"><strong>Detection rates unavailable:</strong> activity coverage or a non-empty common-active denominator is unavailable. This is missing evidence, not a zero rate.</p>");
            }
            out.push_str("</article>");
        }
    }

    out.push_str("<details class=\"audit-disclosure\"><summary>Review exact group, order, block, cycle, and receiver detail</summary><div class=\"disclosure-body\">");
    render_joint_summary_table(out, report, &left_label, &right_label);
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
    out.push_str("</div></details>");
    out.push_str("<p class=\"muted\">An active station that did not report the session callsign is below-threshold evidence for that cycle. A station absent from the census remains no evidence at all. Partial or truncated census rows visibly qualify every affected rate.</p></section>");
}

fn render_detection_rates(
    out: &mut CheckedHtmlWriter<'_>,
    row: &antennabench_analysis::ReporterActivityJointSummary,
    left_label: &str,
    right_label: &str,
) {
    let denominator = row.receiver_block_opportunity_count;
    let left_heard = row.heard_both_count + row.left_only_count;
    let right_heard = row.heard_both_count + row.right_only_count;
    let left_rate = row.left_detection_rate.expect("known aggregate rate");
    let right_rate = row.right_detection_rate.expect("known aggregate rate");
    let (higher_label, higher_heard, higher_rate, lower_label, lower_heard, lower_rate) =
        if right_rate >= left_rate {
            (
                right_label,
                right_heard,
                right_rate,
                left_label,
                left_heard,
                left_rate,
            )
        } else {
            (
                left_label,
                left_heard,
                left_rate,
                right_label,
                right_heard,
                right_rate,
            )
        };
    if higher_rate == lower_rate {
        write_html!(out, "<p class=\"activity-takeaway\"><strong>In these {} shared opportunities, both antennas had the same detection rate:</strong> {} and {} were each reported {:.1}% of the time. Heard by both contributes to both antenna rates.</p>", denominator, escape_html(left_label), escape_html(right_label), higher_rate * 100.0);
    } else {
        write_html!(out, "<p class=\"activity-takeaway\"><strong>In these {} shared opportunities, {} was reported more often:</strong> {} of {} ({:.1}%) versus {} with {} of {} ({:.1}%), a {:.1} percentage-point difference. Heard by both contributes to both antenna rates.</p>", denominator, escape_html(higher_label), higher_heard, denominator, higher_rate * 100.0, escape_html(lower_label), lower_heard, denominator, lower_rate * 100.0, (higher_rate - lower_rate) * 100.0);
    }
    write_html!(out, "<div class=\"chart detection-rate-chart\" aria-label=\"Detection rates among the same active receiver opportunities\"><div class=\"chart-row detection-rate-row\"><span class=\"chart-label\"><strong>{}</strong><br><small>{} of {} opportunities</small></span><span class=\"bar-track detection-rate-track\" aria-hidden=\"true\"><i class=\"bar left geometry-width {}\"></i></span><strong>{:.1}%</strong></div><div class=\"chart-row detection-rate-row\"><span class=\"chart-label\"><strong>{}</strong><br><small>{} of {} opportunities</small></span><span class=\"bar-track detection-rate-track\" aria-hidden=\"true\"><i class=\"bar right geometry-width {}\"></i></span><strong>{:.1}%</strong></div><div class=\"detection-rate-axis\" aria-hidden=\"true\"><span>0%</span><span>50%</span><span>100%</span></div></div>", left_label, left_heard, denominator, geometry_class(left_rate * 100.0), left_rate * 100.0, right_label, right_heard, denominator, geometry_class(right_rate * 100.0), right_rate * 100.0);
}

fn render_outcome_strip(
    out: &mut CheckedHtmlWriter<'_>,
    row: &antennabench_analysis::ReporterActivityJointSummary,
    left_label: &str,
    right_label: &str,
) {
    let denominator = row.receiver_block_opportunity_count;
    let outcomes = [
        (row.left_only_count, "left", format!("{left_label} only")),
        (row.heard_both_count, "both", "Heard both".to_string()),
        (row.right_only_count, "right", format!("{right_label} only")),
        (
            row.heard_neither_count,
            "neither",
            "Heard neither".to_string(),
        ),
    ];
    write_html!(out, "<div class=\"detection-outcomes\"><p><strong>Exactly {} opportunities:</strong> four mutually exclusive outcomes</p><dl class=\"facts detection-outcome-counts\">", denominator);
    for (count, class, label) in &outcomes {
        write_html!(out, "<div class=\"fact\"><dt><i class=\"swatch outcome-{class}\" aria-hidden=\"true\"></i>{}</dt><dd>{count}</dd></div>", escape_html(label));
    }
    out.push_str("</dl><div class=\"reach-bar detection-outcome-strip\" aria-hidden=\"true\">");
    for (count, class, _) in &outcomes {
        if *count > 0 {
            let width = *count as f64 / denominator as f64 * 100.0;
            write_html!(
                out,
                "<span class=\"reach-seg outcome-{class} geometry-width {}\"></span>",
                geometry_class(width)
            );
        }
    }
    out.push_str("</div></div>");
}

fn render_joint_summary_table(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    left_label: &str,
    right_label: &str,
) {
    if report.reporter_activity.joint_summaries.is_empty() {
        return;
    }
    write_html!(out, "<div class=\"table-wrap\"><table><caption>Joint detection outcomes by separate comparison group</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Unique active receivers</th><th scope=\"col\">Eligible blocks / order</th><th scope=\"col\">Receiver-block opportunities</th><th scope=\"col\">Heard both</th><th scope=\"col\">{} only</th><th scope=\"col\">{} only</th><th scope=\"col\">Heard neither</th><th scope=\"col\">Detection rate — {} / {}</th><th scope=\"col\">Coverage</th></tr></thead><tbody>", left_label, right_label, left_label, right_label);
    for row in &report.reporter_activity.joint_summaries {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{} <span class=\"muted\">({} {}→{}; {} {}→{})</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}<br><span class=\"muted\">{} of {} blocks have known coverage</span></td></tr>", comparison_stratum(&row.stratum), row.unique_active_receiver_count, row.eligible_block_count, row.left_then_right_block_count, left_label, right_label, row.right_then_left_block_count, right_label, left_label, row.receiver_block_opportunity_count, row.heard_both_count, row.left_only_count, row.right_only_count, row.heard_neither_count, aggregate_rate_text(row.left_detection_rate), aggregate_rate_text(row.right_detection_rate), coverage_text(row.coverage), row.known_coverage_block_count, row.eligible_block_count);
    }
    out.push_str("</tbody></table></div>");
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
