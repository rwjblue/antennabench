use super::*;
use crate::{
    ReportAntennaRepeatability, ReportCoverageOverlapGroup, ReportObservedComplementarity,
    ReportOpportunityComplementarity,
};

pub(in super::super) fn render_overlap_repeatability_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    render_section(out, report, true);
}

pub(in super::super) fn render_compact_overlap_repeatability_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    render_section(out, report, false);
}

fn render_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport, include_audit: bool) {
    out.push_str("<section id=\"coverage-overlap\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"coverage-overlap-title\"><h2 id=\"coverage-overlap-title\">Coverage overlap and repeatability</h2><p class=\"muted\">Observed complementarity counts unique paths that appeared in the recorded evidence. Opportunity-conditioned complementarity separately counts one-sided detections among receivers proven active during both cycles. Neither measures signal strength or becomes a composite antenna score.</p>");
    if report.coverage_overlap.is_empty() {
        out.push_str(
            "<p class=\"empty\">No overlap or repeatability evidence is available.</p></section>",
        );
        return;
    }
    for (index, group) in report.coverage_overlap.iter().enumerate() {
        render_group(out, group, index, include_audit);
    }
    out.push_str("<p class=\"muted\">Unique observed paths do not prove the other antenna could never reach those endpoints. Repeated block support is descriptive, not an inferential uncertainty statement, reliability probability, or recommendation to retain or remove an antenna.</p></section>");
}

fn render_group(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCoverageOverlapGroup,
    index: usize,
    include_audit: bool,
) {
    write_html!(out, "<article class=\"coverage-overlap-group\" aria-labelledby=\"coverage-overlap-group-{index}\"><h3 id=\"coverage-overlap-group-{index}\">{}</h3>", comparison_stratum(&group.stratum));
    match &group.observed {
        Some(observed) => render_observed(out, observed, include_audit),
        None => out.push_str("<p class=\"empty\">No observed-path overlap is available for this comparison group.</p>"),
    }
    match &group.common_opportunity {
        Some(common) => render_common(out, common, include_audit),
        None => out.push_str("<p class=\"empty\"><strong>Opportunity-conditioned complementarity unavailable:</strong> no common-active receiver census covers this comparison group. Observed-only paths do not substitute for that denominator.</p>"),
    }
    out.push_str("</article>");
}

fn render_observed(
    out: &mut CheckedHtmlWriter<'_>,
    observed: &ReportObservedComplementarity,
    include_audit: bool,
) {
    let left_label = observed
        .left
        .as_ref()
        .map_or("First antenna", |profile| &profile.antenna_label);
    let right_label = observed
        .right
        .as_ref()
        .map_or("Second antenna", |profile| &profile.antenna_label);
    write_html!(out, "<h4>Observed complementarity</h4><p>Using both antennas produced <strong>{}</strong> unique observed paths: {} appeared only on {}, {} appeared on both, and {} appeared only on {}. The incremental recorded contributions were {} beyond {} and {} beyond {}.</p>", observed.total_system_unique_path_count, observed.left_only_unique_path_count, escape_html(left_label), observed.shared_unique_path_count, observed.right_only_unique_path_count, escape_html(right_label), observed.incremental_left_path_count, escape_html(right_label), observed.incremental_right_path_count, escape_html(left_label));
    write_html!(out, "<div class=\"table-wrap\"><table><caption>Observed unique-path overlap</caption><thead><tr><th scope=\"col\">{} only</th><th scope=\"col\">Both</th><th scope=\"col\">{} only</th><th scope=\"col\">Two-antenna total</th></tr></thead><tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div>", escape_html(left_label), escape_html(right_label), observed.left_only_unique_path_count, observed.shared_unique_path_count, observed.right_only_unique_path_count, observed.total_system_unique_path_count);
    if observed.eligible_block_count < 2 {
        write_html!(out, "<p class=\"empty\"><strong>Repeatability limited:</strong> only {} eligible block{} was available. Multi-block repetition cannot be established from a single block.</p>", observed.eligible_block_count, plural_suffix(observed.eligible_block_count));
    }
    out.push_str("<div class=\"repeatability-grid\">");
    for profile in [&observed.left, &observed.right].into_iter().flatten() {
        render_repeatability(out, profile, include_audit);
    }
    out.push_str("</div>");
}

fn render_repeatability(
    out: &mut CheckedHtmlWriter<'_>,
    profile: &ReportAntennaRepeatability,
    include_audit: bool,
) {
    write_html!(out, "<section class=\"repeatability-card\"><h5>{}</h5><p>{} unique path{} contributed {} path-block observation{}: {} appeared in one block and {} appeared in multiple blocks.</p>", escape_html(&profile.antenna_label), profile.unique_endpoint_count, plural_suffix(profile.unique_endpoint_count), profile.path_block_observation_count, plural_suffix(profile.path_block_observation_count), profile.observed_once_path_count, profile.repeated_path_count);
    out.push_str("<div class=\"table-wrap\"><table><caption>Observed block-count distribution</caption><thead><tr><th scope=\"col\">Blocks observed</th><th scope=\"col\">Unique paths</th></tr></thead><tbody>");
    for row in &profile.block_count_distribution {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td></tr>",
            row.observed_block_count,
            row.unique_path_count
        );
    }
    out.push_str("</tbody></table></div>");
    if include_audit && !profile.paths.is_empty() {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review exact path/block support</summary><div class=\"table-wrap\"><table><caption>Per-path repeatability and antenna order</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Blocks</th><th scope=\"col\">Raw observations</th><th scope=\"col\">First → second</th><th scope=\"col\">Second → first</th></tr></thead><tbody>");
        for path in &profile.paths {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&path.remote_path),
                path.observed_block_count,
                path.observation_count,
                path.left_then_right_block_count,
                path.right_then_left_block_count
            );
        }
        out.push_str("</tbody></table></div></details>");
    }
    out.push_str("</section>");
}

fn render_common(
    out: &mut CheckedHtmlWriter<'_>,
    common: &ReportOpportunityComplementarity,
    include_audit: bool,
) {
    write_html!(out, "<h4>Opportunity-conditioned complementarity</h4><p class=\"muted\">Coverage: {} ({} of {} eligible blocks known). The denominator is {} receiver-block opportunities from {} unique receivers active during both cycles.</p>", coverage_text(common.coverage), common.known_coverage_block_count, common.eligible_block_count, common.receiver_block_opportunity_count, common.unique_common_active_receiver_count);
    if !common.coverage.is_known() {
        out.push_str("<p class=\"empty\">Common-opportunity outcomes are unavailable; missing activity evidence is not counted as no detection.</p>");
        return;
    }
    write_html!(out, "<div class=\"table-wrap\"><table><caption>Common-opportunity detection overlap</caption><thead><tr><th scope=\"col\">First only</th><th scope=\"col\">Both</th><th scope=\"col\">Second only</th><th scope=\"col\">Heard neither</th><th scope=\"col\">Opportunities</th></tr></thead><tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div>", common.left_only_count, common.heard_both_count, common.right_only_count, common.heard_neither_count, common.receiver_block_opportunity_count);
    if !common.order_summaries.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Common-opportunity outcomes by antenna order</caption><thead><tr><th scope=\"col\">Order</th><th scope=\"col\">Blocks</th><th scope=\"col\">Opportunities</th><th scope=\"col\">First only</th><th scope=\"col\">Both</th><th scope=\"col\">Second only</th><th scope=\"col\">Neither</th></tr></thead><tbody>");
        for row in &common.order_summaries {
            write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", order_label(row.order), row.block_count, row.receiver_block_opportunity_count, row.left_only_count, row.heard_both_count, row.right_only_count, row.heard_neither_count);
        }
        out.push_str("</tbody></table></div>");
    }
    if include_audit && !common.receiver_frequencies.is_empty() {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review per-receiver detection frequency</summary><div class=\"table-wrap\"><table><caption>Receiver detection across eligible blocks</caption><thead><tr><th scope=\"col\">Receiver</th><th scope=\"col\">Opportunities</th><th scope=\"col\">First detections</th><th scope=\"col\">Second detections</th><th scope=\"col\">First → second opportunities</th><th scope=\"col\">Second → first opportunities</th></tr></thead><tbody>");
        for row in &common.receiver_frequencies {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&row.receiver),
                row.opportunity_count,
                row.left_detection_count,
                row.right_detection_count,
                row.left_then_right_opportunity_count,
                row.right_then_left_opportunity_count
            );
        }
        out.push_str("</tbody></table></div></details>");
    }
}

fn order_label(order: antennabench_analysis::ComparisonOrder) -> &'static str {
    match order {
        antennabench_analysis::ComparisonOrder::LeftThenRight => "First → second",
        antennabench_analysis::ComparisonOrder::RightThenLeft => "Second → first",
    }
}
