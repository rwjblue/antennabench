use super::*;

pub(in super::super) fn render_same_path_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Same-path signal</h2>");
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
    out.push_str("<section id=\"reach-unique-paths\" class=\"panel question-section\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Reach and unique paths</h2>");
    render_reach_view(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review path overlap and missingness</summary><div class=\"disclosure-body\">");
    render_overlap(out, report);
    out.push_str("</div></details></section>");
}
pub(in super::super) fn render_same_path_view(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    let orientation = report.overview.scope.delta_orientation.as_ref();
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison stratum has same-path evidence available. This is not a zero-delta result.</p>");
        return;
    }
    if let Some(orientation) = orientation {
        write_html!(out, "<p class=\"orientation\"><strong>Orientation:</strong> each value is <strong>{} − {}</strong> SNR in dB. Negative values are toward {}; positive values are toward {}. The vertical reference is zero.</p>", escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.minuend_label));
    }
    out.push_str("<p class=\"path-view-note\">Each blue dot is one unique remote path’s median across its paired rows; the purple diamond is the median across those path medians. A finite 0 dB dot is retained as a true zero, not missing evidence.</p>");
    for row in &report.overview.strata {
        render_same_path_stratum(out, row, orientation);
    }
}
pub(in super::super) fn render_same_path_stratum(
    out: &mut CheckedHtmlWriter<'_>,
    row: &ReportOverviewStratum,
    orientation: Option<&antennabench_analysis::DeltaOrientation>,
) {
    write_html!(out, "<h3>{}</h3><p class=\"muted\">{} paired path{} · {} paired row{} · {} contributing block{}</p>", comparison_stratum(&row.stratum), row.path_median_deltas.len(), plural_suffix(row.path_median_deltas.len()), row.paired_row_count, plural_suffix(row.paired_row_count), row.contributing_block_count, plural_suffix(row.contributing_block_count));
    if row.path_median_deltas.is_empty() {
        if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
            write_html!(out, "<p class=\"empty\">No finite same-path delta is available; missing SNR is retained separately (left: {}, right: {}). This is not a 0 dB result.</p>", row.missing_snr_left_count, row.missing_snr_right_count);
        } else {
            out.push_str("<p class=\"empty\">No finite same-path paired evidence is available for this stratum. This is not a 0 dB result.</p>");
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
    out.push_str("<div class=\"path-strip\" aria-hidden=\"true\">");
    for path in &row.path_median_deltas {
        let position = delta_position(path.median_delta_right_minus_left_db, max_abs);
        write_html!(out, "<div class=\"path-strip-row\"><span class=\"chart-label\">{}</span><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-dot\" style=\"left:{position:.3}%\"></span></span><span>{} dB</span></div>", escape_html(&path.remote_path), format_signed(path.median_delta_right_minus_left_db));
    }
    let median_position = delta_position(median, max_abs);
    write_html!(out, "<div class=\"path-strip-row\"><strong>Stratum median</strong><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-median\" style=\"left:{median_position:.3}%\"></span></span><strong>{} dB</strong></div></div>", format_signed(median));
    let orientation_text = orientation
        .map(|orientation| {
            format!(
                "{} − {}",
                orientation.minuend_label, orientation.subtrahend_label
            )
        })
        .unwrap_or_else(|| "right − left".to_string());
    write_html!(out, "<div class=\"table-wrap\"><table><caption>One path-median {} SNR delta per remote path for {}; the stratum median is {} dB.</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Paired rows</th><th scope=\"col\">Median delta</th></tr></thead><tbody>", escape_html(&orientation_text), comparison_stratum(&row.stratum), format_signed(median));
    for path in &row.path_median_deltas {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{} dB</td></tr>",
            escape_html(&path.remote_path),
            path.paired_row_count,
            format_signed(path.median_delta_right_minus_left_db)
        );
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_reach_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<p class=\"muted\">Counts are unique finite remote paths within each stratum. “Both” records finite observations for the path on both antennas and supplies the path universe for same-path analysis; left-only and right-only paths are operationally interesting, but are <strong>not</strong> zero-SNR measurements.</p>");
    if report.overview.strata.is_empty() {
        out.push_str(
            "<p class=\"empty\">No comparison stratum has path-reach evidence available.</p>",
        );
        return;
    }
    for row in &report.overview.strata {
        let reach = &row.reach;
        write_html!(out, "<h3>{}</h3>", comparison_stratum(&row.stratum));
        if reach.left_only_unique_path_count
            + reach.both_unique_path_count
            + reach.right_only_unique_path_count
            == 0
        {
            if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
                write_html!(out, "<p class=\"empty\">No finite path reach counts; missing SNR is retained separately (left: {}, right: {}).</p>", row.missing_snr_left_count, row.missing_snr_right_count);
            } else {
                out.push_str("<p class=\"empty\">No finite path-reach evidence is available for this stratum.</p>");
            }
            continue;
        }
        write_html!(out, "<div class=\"reach-strip\" aria-hidden=\"true\"><span><strong>{}</strong><small>left only</small></span><span><strong>{}</strong><small>both</small></span><span><strong>{}</strong><small>right only</small></span></div><div class=\"table-wrap\"><table><caption>Unique finite remote-path reach counts for {}. Unmatched paths are not zero-SNR measurements.</caption><thead><tr><th scope=\"col\">Left only</th><th scope=\"col\">Both</th><th scope=\"col\">Right only</th><th scope=\"col\">Missing SNR left</th><th scope=\"col\">Missing SNR right</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div>", reach.left_only_unique_path_count, reach.both_unique_path_count, reach.right_only_unique_path_count, comparison_stratum(&row.stratum), reach.left_only_unique_path_count, reach.both_unique_path_count, reach.right_only_unique_path_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
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
