use super::super::geometry::geometry_class;
use super::*;

pub(in super::super) fn render_same_path_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Same-path signal</h2>");
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
    out.push_str("<section id=\"reach-unique-paths\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Reach and unique paths</h2>");
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
    out.push_str("<p class=\"path-view-note\">Each blue dot is one unique remote path’s median across its matched pairs; the purple diamond is the median across those path medians. A 0 dB dot is retained as a true zero.</p>");
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
    write_html!(out, "<div class=\"path-strip\" aria-hidden=\"true\"><div class=\"path-strip-axis\"><span></span><span class=\"path-strip-axis-track\"><strong class=\"path-strip-side path-strip-side-negative\">{}</strong><span class=\"path-strip-axis-zero\">0 dB</span><strong class=\"path-strip-side path-strip-side-positive\">{}</strong></span><span></span></div>", negative_label, positive_label);
    for path in &row.path_median_deltas {
        let position = delta_position(path.median_delta_right_minus_left_db, max_abs);
        write_html!(out, "<div class=\"path-strip-row\"><span class=\"chart-label\">{}</span><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-dot geometry-left {}\"></span></span><span>{} dB</span></div>", escape_html(&path.remote_path), geometry_class(position), format_signed(path.median_delta_right_minus_left_db));
    }
    let median_position = delta_position(median, max_abs);
    write_html!(out, "<div class=\"path-strip-row\"><strong>Group median</strong><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-median geometry-left {}\"></span></span><strong>{} dB</strong></div></div>", geometry_class(median_position), format_signed(median));
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
