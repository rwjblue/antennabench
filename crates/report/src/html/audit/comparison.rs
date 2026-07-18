use super::*;

pub(in super::super) fn render_comparison_diagnostics(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    let diagnostics = report.comparison.diagnostics;
    let (left_label, right_label) = report_antenna_labels(report);
    out.push_str("<h3>Coverage and data-quality counts</h3><dl class=\"stat-grid\">");
    comparison_stat(out, "Blocks", diagnostics.block_count);
    comparison_stat(out, "Eligible blocks", diagnostics.eligible_block_count);
    comparison_stat(out, "Invalid blocks", diagnostics.invalid_block_count);
    comparison_stat(
        out,
        &format!("{left_label} then {right_label}"),
        diagnostics.left_then_right_block_count,
    );
    comparison_stat(
        out,
        &format!("{right_label} then {left_label}"),
        diagnostics.right_then_left_block_count,
    );
    comparison_stat(out, "Matched pairs", diagnostics.paired_row_count);
    comparison_stat(out, "Unique paths", diagnostics.unique_path_count);
    comparison_stat(
        out,
        &format!("Unmatched — {left_label}"),
        diagnostics.unmatched_left_count,
    );
    comparison_stat(
        out,
        &format!("Unmatched — {right_label}"),
        diagnostics.unmatched_right_count,
    );
    comparison_stat(
        out,
        &format!("Missing SNR — {left_label}"),
        diagnostics.missing_snr_left_count,
    );
    comparison_stat(
        out,
        &format!("Missing SNR — {right_label}"),
        diagnostics.missing_snr_right_count,
    );
    comparison_stat(
        out,
        "Missing or invalid mode",
        diagnostics.missing_or_invalid_mode_count,
    );
    comparison_stat(out, "Missing mode", diagnostics.missing_mode_count);
    comparison_stat(out, "Malformed mode", diagnostics.malformed_mode_count);
    comparison_stat(out, "Ambiguous paths", diagnostics.ambiguous_path_count);
    comparison_stat(
        out,
        "Exact duplicates collapsed",
        diagnostics.exact_duplicate_count,
    );
    comparison_stat(
        out,
        "Conflicting duplicate groups",
        diagnostics.conflicting_duplicate_group_count,
    );
    comparison_stat(
        out,
        "Alignment exclusions",
        diagnostics.excluded_observation_count,
    );
    out.push_str("</dl>");
}
pub(in super::super) fn comparison_stat(
    out: &mut CheckedHtmlWriter<'_>,
    label: &str,
    value: usize,
) {
    write_html!(
        out,
        "<div class=\"stat\"><dt>{}</dt><dd>{}</dd></div>",
        label,
        value
    );
}
pub(in super::super) fn render_overlap(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Path overlap and missingness</h3>");
    if report.comparison.overlap_rows.is_empty() {
        out.push_str("<p class=\"empty\">No path-level overlap rows are available.</p>");
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<div class=\"legend\"><span><i class=\"swatch left\"></i>{} usable</span><span><i class=\"swatch right\"></i>{} usable</span></div><div class=\"comparison-chart\" aria-hidden=\"true\">", left_label, right_label);
    for row in &report.comparison.overlap_rows {
        let total = (row.left_finite_count + row.right_finite_count).max(1) as f64;
        let left_width = row.left_finite_count as f64 / total * 100.0;
        let right_width = row.right_finite_count as f64 / total * 100.0;
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"bar-track\"><span class=\"bar left\" style=\"width:{left_width:.3}%\"></span><span class=\"bar right\" style=\"width:{right_width:.3}%\"></span></span><span>{} / {}</span></div>", escape_html(&row.remote_path), comparison_stratum(&row.stratum), row.left_finite_count, row.right_finite_count);
    }
    write_html!(out, "</div><div class=\"table-wrap\"><table><caption>Path overlap and missingness data</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Remote path</th><th scope=\"col\">{} usable</th><th scope=\"col\">{} usable</th><th scope=\"col\">Matched</th><th scope=\"col\">Unmatched — {}</th><th scope=\"col\">Unmatched — {}</th><th scope=\"col\">Missing SNR — {}</th><th scope=\"col\">Missing SNR — {}</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>", left_label, right_label, left_label, right_label, left_label, right_label);
    for row in &report.comparison.overlap_rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.left_finite_count, row.right_finite_count, row.paired_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_comparison_timeline(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Data-quality timeline</h3>");
    if report.comparison.timeline_rows.is_empty() {
        out.push_str("<p class=\"empty\">No comparison timeline rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"timeline\" aria-hidden=\"true\">");
    for row in &report.comparison.timeline_rows {
        let invalid = if row.block_eligible { "" } else { " invalid" };
        let issue = if row.excluded_observation_count > 0
            || row.missing_snr_count > 0
            || row.missing_or_invalid_mode_count > 0
            || row.ambiguous_path_count > 0
            || row.conflicting_duplicate_group_count > 0
        {
            " issue"
        } else {
            ""
        };
        write_html!(
            out,
            "<span class=\"timeline-slot{invalid}{issue}\"><strong>{}</strong><br>{}<br>{}</span>",
            row.sequence_number,
            escape_html(row.actual_label.as_deref().unwrap_or("—")),
            slot_status(row.status)
        );
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Data-quality timeline details</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Eligible</th><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Starts</th><th scope=\"col\">Band</th><th scope=\"col\">Actual label</th><th scope=\"col\">Side</th><th scope=\"col\">Status</th><th scope=\"col\">Total</th><th scope=\"col\">Usable</th><th scope=\"col\">Excluded</th><th scope=\"col\">Missing SNR</th><th scope=\"col\">Missing/invalid mode</th><th scope=\"col\">Ambiguous</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>");
    let (left_label, right_label) = report_antenna_labels(report);
    for row in &report.comparison.timeline_rows {
        timeline_table_row(out, row, &left_label, &right_label);
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_comparison_blocks(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Comparison block inventory</h3>");
    if report.comparison.blocks.is_empty() {
        out.push_str("<p class=\"empty\">No comparison block rows are available.</p>");
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    out.push_str("<div class=\"table-wrap\"><table><caption>Exact adjacent same-band block construction</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Band</th><th scope=\"col\">First slot</th><th scope=\"col\">First actual / status</th><th scope=\"col\">Second slot</th><th scope=\"col\">Second actual / status</th><th scope=\"col\">Order</th><th scope=\"col\">Eligibility</th></tr></thead><tbody>");
    for block in &report.comparison.blocks {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{} · #{} · {}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", block.block_index + 1, band(block.band), escape_html(&block.first_slot_id), block.first_sequence_number, timestamp(block.first_starts_at), escape_html(block.first_label.as_deref().unwrap_or("Not recorded")), slot_status(block.first_status), block.second_slot_id.as_ref().map(|id| format!("{} · #{} · {}", id, block.second_sequence_number.unwrap_or_default(), block.second_starts_at.map(timestamp).unwrap_or_else(|| "Not recorded".into()))).map(|value| escape_html(&value)).unwrap_or_else(|| "Not recorded".into()), escape_html(&format!("{} / {}", block.second_label.as_deref().unwrap_or("Not recorded"), block.second_status.map(slot_status).unwrap_or("Not recorded"))), block.order.map(|order| labeled_comparison_order(order, &left_label, &right_label)).unwrap_or_else(|| "Unavailable".into()), block_eligibility(block.eligibility));
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn timeline_table_row(
    out: &mut CheckedHtmlWriter<'_>,
    row: &ComparisonTimelineRow,
    left_label: &str,
    right_label: &str,
) {
    let side = match row.side {
        Some(ComparisonSide::Left) => left_label,
        Some(ComparisonSide::Right) => right_label,
        None => "Unavailable",
    };
    write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", row.block_index + 1, yes_no(row.block_eligible), row.sequence_number, escape_html(&row.slot_id), timestamp(row.starts_at), band(row.band), escape_html(row.actual_label.as_deref().unwrap_or("Not recorded")), side, slot_status(row.status), row.total_observation_count, row.usable_observation_count, row.excluded_observation_count, row.missing_snr_count, row.missing_or_invalid_mode_count, row.ambiguous_path_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
}
pub(in super::super) fn render_paired_differences(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Matched-pair difference distribution</h3>");
    let rows = &report.comparison.paired_rows;
    if rows.is_empty() {
        out.push_str(
            "<p class=\"empty\">No usable same-path matched differences are available.</p>",
        );
        return;
    }
    let max_abs = rows
        .iter()
        .map(|row| row.delta_right_minus_left_db.abs())
        .fold(1.0_f64, f64::max);
    out.push_str("<div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        let width = row.delta_right_minus_left_db.abs() / max_abs * 50.0;
        let left = if row.delta_right_minus_left_db < 0.0 {
            50.0 - width
        } else {
            50.0
        };
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"comparison-track\"><span class=\"comparison-zero\"></span><span class=\"comparison-delta\" style=\"left:{left:.3}%;width:{width:.3}%\"></span></span><span>{} dB</span></div>", escape_html(&row.remote_path), comparison_stratum(&row.stratum), format_signed(row.delta_right_minus_left_db));
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "</div><div class=\"table-wrap\"><table><caption>Matched-pair difference data</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">{} observation</th><th scope=\"col\">{} observation</th><th scope=\"col\">{} slot</th><th scope=\"col\">{} slot</th><th scope=\"col\">{} SNR</th><th scope=\"col\">{} SNR</th><th scope=\"col\">Signed delta</th><th scope=\"col\">Elapsed</th></tr></thead><tbody>", left_label, right_label, left_label, right_label, left_label, right_label);
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{} s</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, labeled_comparison_order(row.order, &left_label, &right_label), escape_html(&row.left_observation_id), escape_html(&row.right_observation_id), escape_html(&row.left_slot_id), escape_html(&row.right_slot_id), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), row.elapsed_seconds);
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_paired_snr_time(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Matched SNR over time</h3>");
    let rows = &report.comparison.paired_rows;
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">No matched SNR-over-time pairs are available.</p>");
        return;
    }
    let minimum = rows
        .iter()
        .flat_map(|row| [row.left_snr_db, row.right_snr_db])
        .fold(f64::INFINITY, f64::min);
    let maximum = rows
        .iter()
        .flat_map(|row| [row.left_snr_db, row.right_snr_db])
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (maximum - minimum).max(1.0);
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<div class=\"legend\"><span><i class=\"swatch left\"></i>{}</span><span><i class=\"swatch right\"></i>{}</span></div><div class=\"comparison-chart\" aria-hidden=\"true\">", left_label, right_label);
    for row in rows {
        let left = (row.left_snr_db - minimum) / span * 100.0;
        let right = (row.right_snr_db - minimum) / span * 100.0;
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"snr-pair\"><span class=\"snr-left\" style=\"left:{left:.3}%\"></span><span class=\"snr-right\" style=\"left:{right:.3}%\"></span></span><span>{} / {} dB</span></div>", timestamp(row.left_timestamp), escape_html(&row.remote_path), format_number(row.left_snr_db), format_number(row.right_snr_db));
    }
    write_html!(out, "</div><div class=\"table-wrap\"><table><caption>Matched SNR over time data</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">{} time</th><th scope=\"col\">{} time</th><th scope=\"col\">Elapsed</th><th scope=\"col\">{} SNR</th><th scope=\"col\">{} SNR</th></tr></thead><tbody>", left_label, right_label, left_label, right_label);
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} s</td><td>{} dB</td><td>{} dB</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, labeled_comparison_order(row.order, &left_label, &right_label), timestamp(row.left_timestamp), timestamp(row.right_timestamp), row.elapsed_seconds, format_number(row.left_snr_db), format_number(row.right_snr_db));
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_stratum_summaries(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Comparison-group descriptive summaries</h3>");
    if report.comparison.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison groups are available.</p>");
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<div class=\"table-wrap\"><table><caption>Comparison-group summary data</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Matched pairs</th><th scope=\"col\">Paths</th><th scope=\"col\">Blocks</th><th scope=\"col\">{} → {}</th><th scope=\"col\">{} → {}</th><th scope=\"col\">Unmatched — {} / {}</th><th scope=\"col\">Missing SNR — {} / {}</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th><th scope=\"col\">Observed range</th><th scope=\"col\">Median across paths</th></tr></thead><tbody>", left_label, right_label, right_label, left_label, left_label, right_label, left_label, right_label);
    for row in &report.comparison.strata {
        let range = row
            .minimum_delta_right_minus_left_db
            .zip(row.maximum_delta_right_minus_left_db)
            .map(|(minimum, maximum)| {
                format!(
                    "{} to {} dB",
                    format_signed(minimum),
                    format_signed(maximum)
                )
            })
            .unwrap_or_else(not_available);
        let median = row
            .median_path_delta_right_minus_left_db
            .map(|value| format!("{} dB", format_signed(value)))
            .unwrap_or_else(not_available);
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), row.paired_row_count, row.unique_path_count, row.contributing_block_count, row.left_then_right_block_count, row.right_then_left_block_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count, range, median);
    }
    out.push_str("</tbody></table></div>");
}
