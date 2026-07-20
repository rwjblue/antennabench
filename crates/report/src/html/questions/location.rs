use super::*;

pub(in super::super) fn render_distance_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"distance-direction\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"distance-direction-title\"><h2 id=\"distance-direction-title\">Observed distance and direction profile</h2><p class=\"notice\">Evidence basis: located paired paths observed in this session. This is not a radiation pattern, propagation model, or causal conclusion about antenna performance in observed or unobserved directions and distances.</p>");
    render_observed_path_context(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review exact paired-row distance and azimuth detail</summary><div class=\"disclosure-body\">");
    render_location_views(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review derived solar context</summary><div class=\"disclosure-body\">");
    render_solar_context(out, report);
    out.push_str("</div></details></section>");
}
pub(in super::super) fn render_observed_path_context(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No observed paired paths are available for distance or azimuth context. This is not a near-zero path delta.</p>");
        return;
    }
    let available = report
        .overview
        .strata
        .iter()
        .enumerate()
        .filter(|(_, stratum)| located_path_count(&stratum.location_context) > 0)
        .collect::<Vec<_>>();
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|stratum| located_path_count(&stratum.location_context) == 0)
        .collect::<Vec<_>>();
    if available.is_empty() {
        let missing = unavailable
            .iter()
            .map(|row| row.location_context.missing_location_path_count)
            .sum::<usize>();
        let inconsistent = unavailable
            .iter()
            .map(|row| row.location_context.inconsistent_location_path_count)
            .sum::<usize>();
        write_html!(out, "<p class=\"empty\">No observed matched paths are available for distance or azimuth context across {} ({}). Location unavailable remains separate ({} missing, {} inconsistent). This is not a near-zero path delta.</p>", comparison_groups_label(unavailable.len()), comparison_strata_list(&unavailable), missing, inconsistent);
        return;
    }
    out.push_str("<p class=\"muted\">Each located paired path contributes once to one fixed distance bin and one fixed 45° compass sector. The supporting paired-row count stays visible; repeated rows from one endpoint do not increase a cell’s path count.</p>");
    for (index, stratum) in available {
        let context = &stratum.location_context;
        write_html!(
            out,
            "<section aria-labelledby=\"path-context-{index}\"><h3 id=\"path-context-{index}\">{}</h3>",
            comparison_stratum(&stratum.stratum)
        );
        write_html!(out, "<p class=\"muted\">{} located matched path{}; {} location unavailable ({} missing, {} inconsistent). Exact per-antenna values remain in the matched-pair audit table.</p>", located_path_count(context), plural_suffix(located_path_count(context)), context.missing_location_path_count + context.inconsistent_location_path_count, context.missing_location_path_count, context.inconsistent_location_path_count);
        render_location_context_cells(
            out,
            "Observed distance",
            "Fixed distance bins for observed paired paths",
            &context.distance_bins,
            distance_bin_label,
        );
        render_location_context_cells(
            out,
            "Observed azimuth",
            "Fixed 45° azimuth sectors for observed paired paths",
            &context.azimuth_sectors,
            fixed_azimuth_sector_label,
        );
        render_location_path_audit(out, &context.paths);
        out.push_str("</section>");
    }
    if !unavailable.is_empty() {
        let missing = unavailable
            .iter()
            .map(|row| row.location_context.missing_location_path_count)
            .sum::<usize>();
        let inconsistent = unavailable
            .iter()
            .map(|row| row.location_context.inconsistent_location_path_count)
            .sum::<usize>();
        write_html!(out, "<p class=\"empty collapsed-empty-strata\">No located matched paths in {} of {} comparison groups: {}. Location unavailable remains separate ({} missing, {} inconsistent).</p>", unavailable.len(), report.overview.strata.len(), comparison_strata_list(&unavailable), missing, inconsistent);
    }
}
pub(in super::super) fn render_location_context_cells<T: Copy>(
    out: &mut CheckedHtmlWriter<'_>,
    heading: &str,
    caption: &str,
    cells: &[ReportOverviewLocationCell<T>],
    label: impl Fn(T) -> &'static str,
) {
    write_html!(
        out,
        "<h4>{}</h4><div class=\"location-context\" aria-hidden=\"true\">",
        heading
    );
    for cell in cells {
        let class = if cell.unique_located_path_count == 0 {
            " empty-cell"
        } else {
            ""
        };
        write_html!(out, "<div class=\"location-context-cell{}\"><strong>{}</strong><span>{}</span><small>{}</small></div>", class, label(cell.category), location_cell_delta(cell), location_cell_evidence(cell));
    }
    out.push_str("</div><div class=\"table-wrap\"><table class=\"location-context-table\">");
    write_html!(out, "<caption>{}</caption><thead><tr><th scope=\"col\">Bin or sector</th><th scope=\"col\">Unique located paths</th><th scope=\"col\">Supporting matched pairs</th><th scope=\"col\">Median path delta</th><th scope=\"col\">Evidence state</th></tr></thead><tbody>", caption);
    for cell in cells {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            label(cell.category),
            cell.unique_located_path_count,
            cell.paired_row_count,
            location_cell_delta(cell),
            location_cell_evidence(cell)
        );
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn located_path_count(
    context: &crate::ReportOverviewLocationContext,
) -> usize {
    context
        .paths
        .iter()
        .filter(|path| path.availability == ReportPathLocationAvailability::Available)
        .count()
}
pub(in super::super) fn location_cell_delta<T>(cell: &ReportOverviewLocationCell<T>) -> String {
    match cell.median_path_delta_right_minus_left_db {
        Some(delta) if delta.abs() < 0.5 => format!("{} dB (near-zero)", format_signed(delta)),
        Some(delta) => format!("{} dB", format_signed(delta)),
        None => "No observed paired paths".into(),
    }
}
pub(in super::super) fn location_cell_evidence<T>(cell: &ReportOverviewLocationCell<T>) -> String {
    match cell.unique_located_path_count {
        0 => "No observed paired paths".into(),
        1 | 2 => format!(
            "Sparse evidence: {} path(s), {} row(s)",
            cell.unique_located_path_count, cell.paired_row_count
        ),
        _ => format!(
            "{} path(s), {} row(s)",
            cell.unique_located_path_count, cell.paired_row_count
        ),
    }
}
pub(in super::super) fn render_location_path_audit(
    out: &mut CheckedHtmlWriter<'_>,
    paths: &[crate::ReportOverviewLocationPath],
) {
    out.push_str("<details class=\"audit-disclosure\"><summary>Review matched-path location aggregate audit</summary><div class=\"disclosure-body\"><div class=\"table-wrap\"><table><caption>One location-status record per matched path; raw per-antenna values remain below in the matched-pair audit.</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Matched pairs</th><th scope=\"col\">Median path delta</th><th scope=\"col\">Location status</th><th scope=\"col\">Distance</th><th scope=\"col\">Azimuth</th></tr></thead><tbody>");
    for path in paths {
        let status = match path.availability {
            ReportPathLocationAvailability::Available => "Available",
            ReportPathLocationAvailability::Missing => "Missing",
            ReportPathLocationAvailability::Inconsistent => "Inconsistent",
        };
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&path.remote_path),
            path.paired_row_count,
            format_signed(path.median_delta_right_minus_left_db),
            status,
            optional_measure_f64(path.distance_km, "km"),
            optional_measure_f64(path.azimuth_degrees, "°")
        );
    }
    out.push_str("</tbody></table></div></div></details>");
}
pub(in super::super) fn distance_bin_label(bin: ReportDistanceBin) -> &'static str {
    bin.label()
}
pub(in super::super) fn fixed_azimuth_sector_label(sector: ReportAzimuthSector) -> &'static str {
    match sector {
        ReportAzimuthSector::North => "N (337.5°–22.5°)",
        ReportAzimuthSector::NorthEast => "NE (22.5°–67.5°)",
        ReportAzimuthSector::East => "E (67.5°–112.5°)",
        ReportAzimuthSector::SouthEast => "SE (112.5°–157.5°)",
        ReportAzimuthSector::South => "S (157.5°–202.5°)",
        ReportAzimuthSector::SouthWest => "SW (202.5°–247.5°)",
        ReportAzimuthSector::West => "W (247.5°–292.5°)",
        ReportAzimuthSector::NorthWest => "NW (292.5°–337.5°)",
    }
}
