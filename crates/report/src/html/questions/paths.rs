use super::super::geometry::geometry_class;
use super::*;
use std::collections::BTreeMap;

pub(in super::super) fn render_same_path_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Shared-path signal</h2><p class=\"muted\">Evidence basis: finite-SNR reports for the same remote path in both cycles of an eligible block, kept within each separate comparison group.</p>");
    render_same_path_view(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review same-path signal detail</summary><div class=\"disclosure-body\">");
    render_comparison_diagnostics(out, report);
    render_paired_differences(out, report);
    render_paired_snr_time(out, report);
    render_stratum_summaries(out, report);
    out.push_str("</div></details></section>");
}
pub(in super::super) fn render_reach_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"reach-unique-paths\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Observed reach</h2><p class=\"muted\">Evidence basis: unique observed finite-SNR remote paths by antenna within each separate comparison group.</p>");
    render_reach_view(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review path overlap and missingness</summary><div class=\"disclosure-body\">");
    render_overlap(out, report);
    out.push_str("</div></details></section>");
}
pub(in super::super) fn render_same_path_view(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison group has same-path evidence available. This is not a zero-delta result.</p>");
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    let orientation = report.overview.scope.delta_orientation.as_ref();
    let available = report
        .overview
        .strata
        .iter()
        .filter(|row| !row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|row| row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    if available.is_empty() {
        let missing_left = unavailable
            .iter()
            .map(|row| row.missing_snr_left_count)
            .sum::<usize>();
        let missing_right = unavailable
            .iter()
            .map(|row| row.missing_snr_right_count)
            .sum::<usize>();
        write_html!(out, "<p class=\"empty\">No usable same-path signal reports are available across {} ({}). Missing SNR remains separate ({}: {}, {}: {}). This is not a 0 dB result.</p>", comparison_groups_label(unavailable.len()), comparison_strata_list(&unavailable), left_label, missing_left, right_label, missing_right);
        return;
    }
    if let Some(orientation) = orientation {
        write_html!(out, "<p class=\"orientation\"><strong>Signed values:</strong> Positive values mean {} was stronger; negative values mean {} was stronger. The vertical reference is zero.</p>", escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label));
    }
    out.push_str("<p class=\"path-view-note\">Each dot is one unique remote path’s median across its matched pairs. The purple marker is the group median and the purple span is the middle half. A 0 dB dot is retained as a true zero.</p>");
    for row in available {
        render_same_path_stratum(out, row, orientation);
    }
    if !unavailable.is_empty() {
        let missing_left = unavailable
            .iter()
            .map(|row| row.missing_snr_left_count)
            .sum::<usize>();
        let missing_right = unavailable
            .iter()
            .map(|row| row.missing_snr_right_count)
            .sum::<usize>();
        write_html!(out, "<p class=\"empty collapsed-empty-strata\">No usable same-path signal reports in {} of {} comparison groups: {}. Missing SNR remains separate ({}: {}, {}: {}).</p>", unavailable.len(), report.overview.strata.len(), comparison_strata_list(&unavailable), left_label, missing_left, right_label, missing_right);
    }
}
pub(in super::super) fn render_same_path_stratum(
    out: &mut CheckedHtmlWriter<'_>,
    row: &ReportOverviewStratum,
    orientation: Option<&antennabench_analysis::DeltaOrientation>,
) {
    write_html!(
        out,
        "<h3>{}</h3><p class=\"muted\">{} matched path{} · {} matched pair{} · {} block{}</p>",
        comparison_stratum(&row.stratum),
        row.path_median_deltas.len(),
        plural_suffix(row.path_median_deltas.len()),
        row.paired_row_count,
        plural_suffix(row.paired_row_count),
        row.contributing_block_count,
        plural_suffix(row.contributing_block_count)
    );
    if row.path_median_deltas.is_empty() {
        if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
            let (left_label, right_label) = orientation
                .map(orientation_antenna_labels)
                .unwrap_or_else(|| ("Left".into(), "Right".into()));
            write_html!(out, "<p class=\"empty\">No usable same-path signal report is available; missing SNR is retained separately ({}: {}, {}: {}). This is not a 0 dB result.</p>", left_label, row.missing_snr_left_count, right_label, row.missing_snr_right_count);
        } else {
            out.push_str("<p class=\"empty\">No usable same-path signal report is available for this comparison group. This is not a 0 dB result.</p>");
        }
        return;
    }
    let median = match row.path_delta {
        ReportOverviewPathDelta::Available {
            median_path_delta_right_minus_left_db,
            ..
        } => median_path_delta_right_minus_left_db,
        ReportOverviewPathDelta::Unavailable => return,
    };
    let max_abs = row
        .path_median_deltas
        .iter()
        .map(|path| path.median_delta_right_minus_left_db.abs())
        .chain(std::iter::once(median.abs()))
        .fold(1.0_f64, f64::max);
    let (negative_label, positive_label) = orientation
        .map(orientation_antenna_labels)
        .unwrap_or_else(|| ("Negative side".into(), "Positive side".into()));
    let mut values = row
        .path_median_deltas
        .iter()
        .map(|path| path.median_delta_right_minus_left_db)
        .collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let first_quartile = interpolated_quantile(&values, 0.25);
    let third_quartile = interpolated_quantile(&values, 0.75);
    let negative_count = values.iter().filter(|value| **value < 0.0).count();
    let tied_count = values.iter().filter(|value| **value == 0.0).count();
    let positive_count = values.iter().filter(|value| **value > 0.0).count();

    write_html!(out, "<div class=\"path-distribution\"><dl class=\"facts path-distribution-summary\"><div class=\"fact\"><dt>{} stronger</dt><dd>{}</dd></div><div class=\"fact\"><dt>Tied at 0 dB</dt><dd>{}</dd></div><div class=\"fact\"><dt>{} stronger</dt><dd>{}</dd></div><div class=\"fact\"><dt>Group median</dt><dd>{} dB</dd></div><div class=\"fact\"><dt>Middle half</dt><dd>{} to {} dB</dd></div></dl>", negative_label, negative_count, tied_count, positive_label, positive_count, format_signed(median), format_signed(first_quartile), format_signed(third_quartile));
    render_path_distribution_svg(
        out,
        &values,
        PathDistributionScale {
            maximum_absolute: max_abs,
            median,
            first_quartile,
            third_quartile,
        },
        &negative_label,
        &positive_label,
    );
    out.push_str("</div>");
    let orientation_text = orientation
        .map(|_| "signed".to_string())
        .unwrap_or_else(|| "right − left".to_string());
    out.push_str("<details class=\"audit-disclosure path-detail-disclosure\"><summary>Review exact remote paths and matched-pair counts<span class=\"disclosure-purpose\">See which paths contributed, how many matched pairs support each path median, and the exact delta behind each dot.</span></summary><div class=\"disclosure-body\">");
    write_html!(out, "<div class=\"table-wrap\"><table><caption>One path-median {} SNR delta per remote path for {}; the group median is {} dB.</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Matched pairs</th><th scope=\"col\">Median delta</th></tr></thead><tbody>", escape_html(&orientation_text), comparison_stratum(&row.stratum), format_signed(median));
    for path in &row.path_median_deltas {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{} dB</td></tr>",
            escape_html(&path.remote_path),
            path.paired_row_count,
            format_signed(path.median_delta_right_minus_left_db)
        );
    }
    out.push_str("</tbody></table></div></div></details>");
}

#[derive(Clone, Copy)]
struct PathDistributionScale {
    maximum_absolute: f64,
    median: f64,
    first_quartile: f64,
    third_quartile: f64,
}

fn render_path_distribution_svg(
    out: &mut CheckedHtmlWriter<'_>,
    values: &[f64],
    scale: PathDistributionScale,
    negative_label: &str,
    positive_label: &str,
) {
    const LEFT: f64 = 44.0;
    const WIDTH: f64 = 632.0;
    const BASELINE: f64 = 156.0;
    let x = |value: f64| LEFT + delta_position(value, scale.maximum_absolute) / 100.0 * WIDTH;
    let mut stack_sizes = BTreeMap::<i64, usize>::new();
    for value in values {
        *stack_sizes
            .entry((value * 1_000.0).round() as i64)
            .or_default() += 1;
    }
    let largest_stack = stack_sizes.values().copied().max().unwrap_or(1);
    let vertical_step = (104.0 / largest_stack as f64).min(11.0);
    let radius = (vertical_step * 0.38).clamp(2.2, 4.2);
    let mut stack_offsets = BTreeMap::<i64, usize>::new();

    write_html!(out, "<svg class=\"coverage-polar path-distribution-chart\" viewBox=\"0 0 720 205\" role=\"img\" aria-label=\"Distribution of {} signed path-median SNR differences. Negative values favor {}; positive values favor {}.\"><rect width=\"720\" height=\"205\" rx=\"8\" fill=\"#f5f7fb\"/>", values.len(), escape_html(negative_label), escape_html(positive_label));
    out.push_str("<line class=\"path-distribution-axis\" x1=\"44\" y1=\"160\" x2=\"676\" y2=\"160\" stroke=\"#5c667a\"/><line class=\"path-distribution-zero\" x1=\"360\" y1=\"38\" x2=\"360\" y2=\"166\" stroke=\"#172033\" stroke-width=\"1.5\"/>");
    write_html!(out, "<line class=\"path-distribution-iqr\" x1=\"{:.2}\" y1=\"28\" x2=\"{:.2}\" y2=\"28\" stroke=\"#6d4c9a\" stroke-width=\"7\" stroke-linecap=\"round\"/><line class=\"path-distribution-median\" x1=\"{:.2}\" y1=\"18\" x2=\"{:.2}\" y2=\"42\" stroke=\"#6d4c9a\" stroke-width=\"3\"/>", x(scale.first_quartile), x(scale.third_quartile), x(scale.median), x(scale.median));
    for value in values {
        let bucket = (*value * 1_000.0).round() as i64;
        let level = stack_offsets.entry(bucket).or_default();
        let y = BASELINE - (*level as f64 + 0.5) * vertical_step;
        *level += 1;
        let (class, fill) = if *value < 0.0 {
            ("path-dot-negative", "#315da8")
        } else if *value > 0.0 {
            ("path-dot-positive", "#b35c00")
        } else {
            ("path-dot-zero", "#5c667a")
        };
        write_html!(out, "<circle class=\"path-distribution-dot {class}\" cx=\"{:.2}\" cy=\"{y:.2}\" r=\"{radius:.2}\" fill=\"{fill}\" stroke=\"#fff\"/>", x(*value));
    }
    write_html!(out, "<text class=\"path-distribution-label path-distribution-label-negative\" x=\"44\" y=\"186\" fill=\"#5c667a\" font-size=\"12\" font-weight=\"700\">{} stronger</text><text class=\"path-distribution-label\" x=\"360\" y=\"186\" fill=\"#5c667a\" font-size=\"12\" font-weight=\"700\" text-anchor=\"middle\">0 dB</text><text class=\"path-distribution-label path-distribution-label-positive\" x=\"676\" y=\"186\" fill=\"#5c667a\" font-size=\"12\" font-weight=\"700\" text-anchor=\"end\">{} stronger</text></svg>", escape_html(negative_label), escape_html(positive_label));
}

fn interpolated_quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * weight
}
pub(in super::super) fn render_reach_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<p class=\"muted\">Counts are unique remote paths with usable signal reports within each comparison group. “Heard by both” supplies the path universe for same-path analysis; {}-only and {}-only paths remain visible.</p>", left_label, right_label);
    if report.overview.strata.is_empty() {
        out.push_str(
            "<p class=\"empty\">No comparison group has path-reach evidence available.</p>",
        );
        return;
    }
    let (available, unavailable): (Vec<_>, Vec<_>) =
        report.overview.strata.iter().partition(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                > 0
        });
    for row in available {
        let reach = &row.reach;
        let universe = reach.left_only_unique_path_count
            + reach.both_unique_path_count
            + reach.right_only_unique_path_count;
        write_html!(out, "<h3>{}</h3>", comparison_stratum(&row.stratum));
        write_html!(out, "<div class=\"reach-strip\" aria-hidden=\"true\"><div class=\"reach-cells\"><span><strong>{}</strong><small><span class=\"swatch left\"></span>{} only</small></span><span><strong>{}</strong><small><span class=\"swatch both\"></span>heard by both</small></span><span><strong>{}</strong><small><span class=\"swatch right\"></span>{} only</small></span></div>", reach.left_only_unique_path_count, left_label, reach.both_unique_path_count, reach.right_only_unique_path_count, right_label);
        render_reach_bar(out, reach, "reach-bar");
        out.push_str("</div>");
        write_html!(out, "<p class=\"muted reach-note\">Segment widths are proportional to unique-path counts. {} heard {} of {} unique path{}; {} heard {}.</p>", left_label, reach.left_only_unique_path_count + reach.both_unique_path_count, universe, plural_suffix(universe), right_label, reach.right_only_unique_path_count + reach.both_unique_path_count);
        write_html!(out, "<div class=\"table-wrap\"><table><caption>Unique remote-path reach counts for {}.</caption><thead><tr><th scope=\"col\">{} only</th><th scope=\"col\">Heard by both</th><th scope=\"col\">{} only</th><th scope=\"col\">Missing SNR — {}</th><th scope=\"col\">Missing SNR — {}</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div>", comparison_stratum(&row.stratum), left_label, right_label, left_label, right_label, reach.left_only_unique_path_count, reach.both_unique_path_count, reach.right_only_unique_path_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
    if !unavailable.is_empty() {
        let missing_left = unavailable
            .iter()
            .map(|row| row.missing_snr_left_count)
            .sum::<usize>();
        let missing_right = unavailable
            .iter()
            .map(|row| row.missing_snr_right_count)
            .sum::<usize>();
        write_html!(out, "<p class=\"empty collapsed-empty-strata\">No usable path-reach signal reports in {} of {} comparison groups: {}. Missing SNR remains separate ({}: {}, {}: {}).</p>", unavailable.len(), report.overview.strata.len(), comparison_strata_list(&unavailable), left_label, missing_left, right_label, missing_right);
    }
}
pub(in super::super) fn render_reach_bar(
    out: &mut CheckedHtmlWriter<'_>,
    reach: &ReportOverviewReach,
    class: &str,
) {
    write_html!(out, "<span class=\"{class}\" aria-hidden=\"true\">");
    let counts = [
        (reach.left_only_unique_path_count, "left"),
        (reach.both_unique_path_count, "both"),
        (reach.right_only_unique_path_count, "right"),
    ];
    let total = counts.iter().map(|(count, _)| count).sum::<usize>().max(1) as f64;
    for (count, segment) in counts {
        if count > 0 {
            let width = count as f64 / total * 100.0;
            write_html!(
                out,
                "<span class=\"reach-seg {segment} {}\"></span>",
                geometry_class(width)
            );
        }
    }
    out.push_str("</span>");
}
pub(in super::super) fn delta_position(value: f64, maximum_absolute: f64) -> f64 {
    (50.0 + value / maximum_absolute * 50.0).clamp(0.0, 100.0)
}
pub(in super::super) fn plural_suffix(value: usize) -> &'static str {
    if value == 1 {
        ""
    } else {
        "s"
    }
}
