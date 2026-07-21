use super::super::geometry::geometry_class;
use super::*;

pub(in super::super) fn render_distance_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_observed_profile_intro(out, report);
    render_goal_distance_focus(out, report);
    render_all_path_profiles(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review exact observed-path profile rows</summary><div class=\"disclosure-body\">");
    render_observed_profile_audit(out, report);
    out.push_str("</div></details>");
    if !is_single_antenna_lens(report) {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review shared-path distance and direction context</summary><div class=\"disclosure-body\"><p class=\"muted\">This separate view answers where finite-SNR differences occurred among paths decoded on both antennas.</p>");
        render_observed_path_context(out, report);
        out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review exact paired-row distance and azimuth detail</summary><div class=\"disclosure-body\">");
        render_location_views(out, report)?;
        out.push_str("</div></details>");
    }
    out.push_str("<details class=\"audit-disclosure\"><summary>Review derived solar context</summary><div class=\"disclosure-body\">");
    render_solar_context(out, report)?;
    out.push_str("</div></details></section>");
    Ok(())
}

pub(in super::super) fn render_compact_observed_footprint_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    out.push_str("<section id=\"observed-footprint\" class=\"panel question-section observed-footprint\" tabindex=\"-1\" aria-labelledby=\"observed-footprint-title\"><h2 id=\"observed-footprint-title\">Observed footprint</h2>");
    if is_single_antenna_lens(report) {
        out.push_str("<p class=\"notice\">These are the unique usable paths recorded for the profiled antenna. They describe collected evidence, not a radiation pattern or unobserved coverage.</p>");
    } else {
        out.push_str("<p class=\"notice\">This is the uncontrolled set of unique usable paths observed for either antenna. Unlike common-active receiver detection, it does not prove the same receivers were listening in both cycles; observed-only paths are not controlled non-detections.</p>");
    }
    render_goal_distance_focus(out, report);
    let (available, unavailable): (Vec<_>, Vec<_>) =
        report.overview.strata.iter().partition(|stratum| {
            stratum.observed_profile.left.is_some()
                || stratum.observed_profile.right.is_some()
                || stratum.reach.left_only_unique_path_count > 0
                || stratum.reach.both_unique_path_count > 0
                || stratum.reach.right_only_unique_path_count > 0
        });
    if available.is_empty() {
        out.push_str("<p class=\"empty\">No usable observed-path footprint is available. Missing evidence is not rendered as zero reach.</p></section>");
        return Ok(());
    }
    for (index, stratum) in available.into_iter().enumerate() {
        write_html!(out, "<article class=\"antenna-card footprint-group\" aria-labelledby=\"footprint-group-{index}\"><h3 id=\"footprint-group-{index}\">{}</h3>", comparison_stratum(&stratum.stratum));
        render_footprint_overlap(out, report, stratum)?;
        let profile = &stratum.observed_profile;
        out.push_str("<details class=\"audit-disclosure footprint-profile-disclosure\"><summary>Review observed distance and direction profile</summary><div class=\"disclosure-body\"><p class=\"muted\">These all-path counts are retained for geographic context, but stay secondary to the controlled common-active detection maps above.</p>");
        render_profile_bar_chart(
            out,
            "Observed unique paths by distance",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |value| &value.distance_bins,
            |cell| cell.category.label(),
        );
        render_profile_bar_chart(
            out,
            "Observed unique paths by direction",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |value| &value.azimuth_sectors,
            |cell| fixed_azimuth_sector_label(cell.category),
        );
        if profile.composition_location_unavailable_count > 0 {
            write_html!(out, "<p class=\"muted\">{} unique path{} could not enter distance composition because location was missing, inconsistent, or differed between antenna records.</p>", profile.composition_location_unavailable_count, plural_suffix(profile.composition_location_unavailable_count));
        }
        render_profile_totals(out, profile.left.as_ref(), profile.right.as_ref());
        render_profile_distribution(
            out,
            "Exact unique-path distance counts and observation support",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |value| &value.distance_bins,
            |cell| cell.category.label(),
        );
        render_profile_distribution(
            out,
            "Exact unique-path direction counts and observation support",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |value| &value.azimuth_sectors,
            |cell| fixed_azimuth_sector_label(cell.category),
        );
        out.push_str("</div></details></article>");
    }
    if !unavailable.is_empty() {
        write_html!(out, "<p class=\"empty collapsed-empty-strata\">No usable observed footprint in {} of {} comparison groups: {}. Missing path or location evidence is not rendered as zero.</p>", unavailable.len(), report.overview.strata.len(), comparison_strata_list(&unavailable));
    }
    render_compact_repeatability_disclosure(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review exact unique observed-path rows</summary><div class=\"disclosure-body\">");
    render_observed_profile_audit(out, report);
    out.push_str("</div></details></section>");
    Ok(())
}

fn render_footprint_overlap(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    stratum: &ReportOverviewStratum,
) -> Result<(), ReportError> {
    let (left_label, right_label) = report_antenna_labels(report);
    let reach = &stratum.reach;
    let left_total = reach.left_only_unique_path_count + reach.both_unique_path_count;
    let right_total = reach.right_only_unique_path_count + reach.both_unique_path_count;
    write_html!(out, "<div class=\"reach-strip footprint-overlap\"><div class=\"reach-cells footprint-overlap-counts\"><span><strong>{}</strong><small>{} only</small></span><span><strong>{}</strong><small>Heard by both</small></span><span><strong>{}</strong><small>{} only</small></span></div>", reach.left_only_unique_path_count, left_label, reach.both_unique_path_count, reach.right_only_unique_path_count, right_label);
    render_reach_bar(out, reach, "reach-bar")?;
    write_html!(
        out,
        "<p><strong>{}</strong>: {} unique paths · <strong>{}</strong>: {} unique paths</p></div>",
        left_label,
        left_total,
        right_label,
        right_total
    );
    Ok(())
}

fn render_profile_bar_chart<T: Copy>(
    out: &mut CheckedHtmlWriter<'_>,
    heading: &str,
    left: Option<&ReportObservedAntennaProfile>,
    right: Option<&ReportObservedAntennaProfile>,
    cells: impl Fn(&ReportObservedAntennaProfile) -> &[ReportObservedProfileCell<T>],
    label: impl Fn(&ReportObservedProfileCell<T>) -> &'static str,
) {
    let left_label = left.map_or("First antenna", |profile| profile.antenna_label.as_str());
    let right_label = right.map_or("Second antenna", |profile| profile.antenna_label.as_str());
    let row_count = left
        .map(|profile| cells(profile).len())
        .or_else(|| right.map(|profile| cells(profile).len()))
        .unwrap_or_default();
    let maximum = (0..row_count)
        .flat_map(|index| {
            [
                left.and_then(|profile| cells(profile).get(index))
                    .map_or(0, |cell| cell.unique_path_count),
                right
                    .and_then(|profile| cells(profile).get(index))
                    .map_or(0, |cell| cell.unique_path_count),
            ]
        })
        .max()
        .unwrap_or(0);
    write_html!(out, "<section class=\"footprint-profile\"><h4>{}</h4><p class=\"muted\">Paired bars share one scale; counts are unique observed paths, not repeated observations.</p><div class=\"chart footprint-profile-chart\">", escape_html(heading));
    for index in 0..row_count {
        let left_cell = left.and_then(|profile| cells(profile).get(index));
        let right_cell = right.and_then(|profile| cells(profile).get(index));
        let category = left_cell
            .or(right_cell)
            .expect("observed profile category exists");
        let left_count = left_cell.map_or(0, |cell| cell.unique_path_count);
        let right_count = right_cell.map_or(0, |cell| cell.unique_path_count);
        let scale = maximum.max(1) as f64;
        write_html!(out, "<div class=\"chart-row footprint-profile-row\"><span class=\"chart-label\"><strong>{}</strong><br>{}</span><span class=\"bar-track footprint-bar-track\"><i class=\"bar left geometry-width {}\"></i></span><span>{}</span></div><div class=\"chart-row footprint-profile-row\"><span class=\"chart-label\"><strong>{}</strong><br>{}</span><span class=\"bar-track footprint-bar-track\"><i class=\"bar right geometry-width {}\"></i></span><span>{}</span></div>", label(category), escape_html(left_label), geometry_class(left_count as f64 / scale * 100.0), left_count, label(category), escape_html(right_label), geometry_class(right_count as f64 / scale * 100.0), right_count);
    }
    out.push_str("</div></section>");
}

fn render_observed_profile_intro(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if is_single_antenna_lens(report) {
        out.push_str("<section id=\"distance-direction\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"distance-direction-title\"><h2 id=\"distance-direction-title\">Observed distance and direction profile</h2><p class=\"notice\">Evidence basis: all usable paths recorded for the profiled antenna, kept within each direction, band, mode, evidence-kind, and source group. This describes collected paths, not a radiation pattern, propagation model, or claim about unobserved distances and directions.</p>");
    } else {
        out.push_str("<section id=\"distance-direction\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"distance-direction-title\"><h2 id=\"distance-direction-title\">Observed distance and direction profile</h2><p class=\"notice\">Evidence basis: all usable paths observed for each antenna in eligible blocks, including paths observed on only one antenna. Receiver/transmitter availability may have changed between antenna periods; this describes collected paths and is not a controlled detection comparison. This is not a radiation pattern, propagation model, or causal conclusion about observed or unobserved distances and directions, and it does not rank antennas universally.</p>");
    }
}

fn render_goal_distance_focus(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let Some(lens) = &report.overview.goal_lens else {
        return;
    };
    if lens.emphasized_distance_bins.is_empty() {
        return;
    }
    write_html!(out, "<p class=\"goal-distance-focus\"><strong>Predeclared goal focus:</strong> {}. Counts for all four fixed categories remain alongside this focus.</p>", lens.emphasized_distance_bins.iter().map(|bin| bin.label()).collect::<Vec<_>>().join("; "));
}

fn render_all_path_profiles(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let available = report
        .overview
        .strata
        .iter()
        .filter(|stratum| {
            stratum.observed_profile.left.is_some() || stratum.observed_profile.right.is_some()
        })
        .collect::<Vec<_>>();
    if available.is_empty() {
        out.push_str("<p class=\"empty\">No usable observed paths have location evidence for an antenna profile.</p>");
        return;
    }
    out.push_str("<p class=\"muted\">Each remote path contributes once per antenna and exact comparison group. Observation support remains visible, but repeated observations from one endpoint do not inflate the footprint count. Near / local distance is a practical proxy only; propagation mode was not measured.</p>");
    for (index, stratum) in available.into_iter().enumerate() {
        let profile = &stratum.observed_profile;
        write_html!(out, "<section aria-labelledby=\"observed-profile-{index}\"><h3 id=\"observed-profile-{index}\">{}</h3>", comparison_stratum(&stratum.stratum));
        if let (Some(left), Some(right)) = (&profile.left, &profile.right) {
            if let (Some(left_bin), Some(right_bin)) = (
                strict_dominant_distance(left),
                strict_dominant_distance(right),
            ) {
                if left_bin != right_bin {
                    write_html!(out, "<p><strong>Observed profile:</strong> {}’s observed paths were concentrated in {}, while {}’s observed paths were concentrated in {} in this run.</p>", escape_html(&left.antenna_label), left_bin.label(), escape_html(&right.antenna_label), right_bin.label());
                }
            }
        }
        render_profile_totals(out, profile.left.as_ref(), profile.right.as_ref());
        render_profile_distribution(
            out,
            "Side-by-side observed distance distribution",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |profile| &profile.distance_bins,
            |cell| cell.category.label(),
        );
        render_profile_distribution(
            out,
            "Side-by-side observed azimuth distribution",
            profile.left.as_ref(),
            profile.right.as_ref(),
            |profile| &profile.azimuth_sectors,
            |cell| fixed_azimuth_sector_label(cell.category),
        );
        let (left_label, right_label) = report_antenna_labels(report);
        write_html!(out, "<div class=\"table-wrap\"><table><caption>Observed-path composition within each distance category</caption><thead><tr><th scope=\"col\">Distance</th><th scope=\"col\">{} only</th><th scope=\"col\">Shared</th><th scope=\"col\">{} only</th></tr></thead><tbody>", left_label, right_label);
        for cell in &profile.distance_composition {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                cell.category.label(),
                cell.left_only_unique_path_count,
                cell.shared_unique_path_count,
                cell.right_only_unique_path_count
            );
        }
        out.push_str("</tbody></table></div>");
        if profile.composition_location_unavailable_count > 0 {
            write_html!(out, "<p class=\"muted\">{} path{} could not enter distance composition because location was missing, inconsistent, or differed between antenna records.</p>", profile.composition_location_unavailable_count, plural_suffix(profile.composition_location_unavailable_count));
        }
        out.push_str("</section>");
    }
}

fn render_profile_totals(
    out: &mut CheckedHtmlWriter<'_>,
    left: Option<&ReportObservedAntennaProfile>,
    right: Option<&ReportObservedAntennaProfile>,
) {
    out.push_str("<div class=\"table-wrap\"><table><caption>Unique observed paths and location availability</caption><thead><tr><th scope=\"col\">Antenna</th><th scope=\"col\">Unique paths</th><th scope=\"col\">Located</th><th scope=\"col\">Missing location</th><th scope=\"col\">Inconsistent location</th></tr></thead><tbody>");
    for profile in [left, right].into_iter().flatten() {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&profile.antenna_label),
            profile.unique_path_count,
            profile.located_path_count,
            profile.missing_location_path_count,
            profile.inconsistent_location_path_count
        );
    }
    out.push_str("</tbody></table></div>");
}

fn render_profile_distribution<T: Copy>(
    out: &mut CheckedHtmlWriter<'_>,
    caption: &str,
    left: Option<&ReportObservedAntennaProfile>,
    right: Option<&ReportObservedAntennaProfile>,
    cells: impl Fn(&ReportObservedAntennaProfile) -> &[ReportObservedProfileCell<T>],
    label: impl Fn(&ReportObservedProfileCell<T>) -> &'static str,
) {
    let left_label = left.map_or("Left", |profile| profile.antenna_label.as_str());
    let right_label = right.map_or("Right", |profile| profile.antenna_label.as_str());
    write_html!(out, "<div class=\"table-wrap\"><table><caption>{}</caption><thead><tr><th scope=\"col\">Category</th><th scope=\"col\">{}</th><th scope=\"col\">{}</th></tr></thead><tbody>", escape_html(caption), escape_html(left_label), escape_html(right_label));
    let row_count = left
        .map(|profile| cells(profile).len())
        .or_else(|| right.map(|profile| cells(profile).len()))
        .unwrap_or_default();
    for index in 0..row_count {
        let left_cell = left.and_then(|profile| cells(profile).get(index));
        let right_cell = right.and_then(|profile| cells(profile).get(index));
        let category = left_cell
            .or(right_cell)
            .expect("profile distribution row exists");
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            label(category),
            observed_profile_cell_text(left_cell),
            observed_profile_cell_text(right_cell)
        );
    }
    out.push_str("</tbody></table></div>");
}

fn observed_profile_cell_text<T>(cell: Option<&ReportObservedProfileCell<T>>) -> String {
    cell.map_or_else(
        || "0 paths / 0 observations".to_string(),
        |cell| {
            format!(
                "{} path{} / {} observation{}",
                cell.unique_path_count,
                plural_suffix(cell.unique_path_count),
                cell.observation_count,
                plural_suffix(cell.observation_count)
            )
        },
    )
}

fn strict_dominant_distance(profile: &ReportObservedAntennaProfile) -> Option<ReportDistanceBin> {
    let mut ranked = profile.distance_bins.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .unique_path_count
            .cmp(&left.unique_path_count)
            .then_with(|| left.category.index().cmp(&right.category.index()))
    });
    let first = ranked.first()?;
    (first.unique_path_count > 0
        && ranked
            .get(1)
            .is_none_or(|second| first.unique_path_count > second.unique_path_count))
    .then_some(first.category)
}

fn render_observed_profile_audit(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.comparison.observed_path_profiles.is_empty() {
        out.push_str("<p class=\"empty\">Exact observed-path rows were omitted by the bounded report profile.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Exact unique observed-path records</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Antenna / path</th><th scope=\"col\">Location</th><th scope=\"col\">Block / slot support</th><th scope=\"col\">Observations</th><th scope=\"col\">Observed SNR summary</th></tr></thead><tbody>");
    for profile in &report.comparison.observed_path_profiles {
        for path in &profile.paths {
            let location = match &path.location {
                antennabench_analysis::ObservedPathLocation::Available {
                    remote_grid,
                    distance_km,
                    initial_bearing_degrees,
                } => format!(
                    "{} · {:.0} km · {:.0}°",
                    escape_html(remote_grid),
                    distance_km,
                    initial_bearing_degrees
                ),
                antennabench_analysis::ObservedPathLocation::Missing => "Missing".to_string(),
                antennabench_analysis::ObservedPathLocation::Inconsistent => {
                    "Inconsistent".to_string()
                }
            };
            let snr = path.snr.map_or_else(
                || "No finite SNR".to_string(),
                |snr| {
                    format!(
                        "{} sample{} · median {} dB · range {} to {} dB",
                        snr.sample_count,
                        plural_suffix(snr.sample_count),
                        format_number(snr.median_db),
                        format_number(snr.min_db),
                        format_number(snr.max_db)
                    )
                },
            );
            write_html!(out, "<tr><td>{}</td><td>{}<br><span class=\"muted\">{}</span></td><td>{}</td><td>{} / {}<br><span class=\"muted\">blocks {} · slots {}</span></td><td>{}<br><span class=\"muted\">{}</span></td><td>{}</td></tr>", comparison_stratum(&profile.stratum), escape_html(&profile.antenna_label), escape_html(&path.remote_path), location, path.block_support_count, path.slot_support_count, escape_html(&path.block_indices.iter().map(|index| (index + 1).to_string()).collect::<Vec<_>>().join(", ")), escape_html(&path.slot_ids.join(", ")), path.observation_count, escape_html(&path.observation_ids.join(", ")), snr);
        }
    }
    out.push_str("</tbody></table></div>");
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
