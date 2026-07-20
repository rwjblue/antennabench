use std::fmt::Write as _;

use super::*;
use crate::{
    great_circle_position, natural_earth_coastline, station_coordinates_from_grid,
    AzimuthalEquidistantProjection, ReportCommonOpportunityCell, ReportCommonOpportunityMapGroup,
    ReportCommonOpportunityPolarCell, ReportCoverageMapGroup, ReportCoveragePanel,
    ReportCoverageState, SquareRootPolarFrame, EARTH_ANTIPODE_DISTANCE_KM,
};
use antennabench_analysis::ComparisonSide;

const SECTOR_LABELS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

pub(in super::super) fn render_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Common-opportunity detection by distance and bearing</h2><p class=\"muted\">Evidence basis: detection outcome, conditional on the receiver being active during both antenna cycles. This common listening-opportunity denominator is separate from the all-path observed distance and direction profile. Colors divide receiver-block opportunities into first-antenna only, both, second-antenna only, and heard neither.</p>");
    if report.common_opportunity_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified common-active receiver population is available. Missing census evidence is not shown as no detection.</p></section>");
        return;
    }
    render_outcome_legend(out, report);
    for (index, group) in report.common_opportunity_maps.iter().enumerate() {
        render_common_group(out, group, index, true);
    }
    if !report.coverage_maps.is_empty() {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review independent per-cycle activity panels</summary><p class=\"muted\">These legacy panels use each antenna cycle's own active-receiver population. They are context only and do not replace the common-opportunity comparison above.</p>");
        render_shared_map_definitions(out, report);
        for (index, group) in report.coverage_maps.iter().enumerate() {
            render_full_group(out, report, group, index);
        }
        out.push_str("</details>");
    }
    out.push_str("<p class=\"muted\">Receiver locations use retained Maidenhead locators and station-centered great-circle geometry. Missing or inconsistent locators remain in explicit unavailable counts. These recorded opportunities do not establish gain, significance, a radiation pattern, NVIS propagation, or a universal DX advantage.</p></section>");
}

pub(in super::super) fn render_compact_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section compact-coverage\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Common-opportunity detection shape</h2><p class=\"muted\">Eight bearing sectors × four distance categories, conditioned on receivers active during both cycles. This is not the all-path observed profile.</p>");
    if report.common_opportunity_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified common-active receiver population is available. Missing census evidence is not shown as no detection.</p></section>");
        return;
    }
    render_outcome_legend(out, report);
    for (index, group) in report.common_opportunity_maps.iter().enumerate() {
        render_common_group(out, group, index, false);
    }
    out.push_str("</section>");
}

fn render_outcome_legend(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let (left, right) = report_antenna_labels(report);
    write_html!(out, "<p class=\"common-opportunity-legend\"><span><i class=\"outcome-left\"></i>{} only</span><span><i class=\"outcome-both\"></i>Both</span><span><i class=\"outcome-right\"></i>{} only</span><span><i class=\"outcome-neither\"></i>Heard neither</span></p>", escape_html(&left), escape_html(&right));
}

fn render_common_group(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
    index: usize,
    include_audit: bool,
) {
    write_html!(out, "<article class=\"coverage-group common-opportunity-group\" aria-labelledby=\"common-opportunity-{index}\"><h3 id=\"common-opportunity-{index}\">{}</h3><p class=\"muted\">{} unique common-active receivers; {} receiver-block opportunities; {} / {} opportunities located. Coverage: {} ({} of {} eligible blocks known).</p>", comparison_stratum(&group.stratum), group.unique_common_active_receiver_count, group.receiver_block_opportunity_count, group.located_receiver_block_opportunity_count, group.receiver_block_opportunity_count, coverage_text(group.coverage), group.known_coverage_block_count, group.eligible_block_count);
    if !group.coverage.is_known() {
        write_html!(out, "<p class=\"empty\"><strong>Common-opportunity geography unavailable:</strong> {}. Missing activity evidence is not treated as no detection.</p></article>", coverage_text(group.coverage));
        return;
    }
    render_common_findings(out, group);
    render_common_polar(out, group, index);
    render_common_polar_numbers(out, group);
    render_common_marginals(out, group);
    if include_audit && !group.blocks.is_empty() {
        render_common_block_audit(out, group);
    }
    out.push_str("</article>");
}

fn render_common_findings(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
) {
    let differences = group
        .distance_cells
        .iter()
        .filter(|cell| {
            cell.receiver_block_opportunity_count > 0
                && cell.left_heard_count != cell.right_heard_count
        })
        .map(|cell| {
            format!(
                "{}: {} versus {} detections in {} recorded common opportunities",
                cell.category.label(),
                cell.left_heard_count,
                cell.right_heard_count,
                cell.receiver_block_opportunity_count
            )
        })
        .collect::<Vec<_>>();
    if !differences.is_empty() {
        write_html!(out, "<p><strong>Recorded per-bin difference:</strong> {}. These are session-scoped detection counts, not a universal antenna ranking.</p>", escape_html(&differences.join("; ")));
    }
}

fn render_common_polar(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
    index: usize,
) {
    write_html!(out, "<figure class=\"coverage-panel common-opportunity-polar\"><figcaption><strong>Station-centered common opportunities</strong><span>{} located · {} location unavailable</span></figcaption><svg class=\"coverage-polar-cells\" viewBox=\"-108 -108 216 216\" role=\"img\" aria-label=\"Common-opportunity outcomes by bearing and distance\"><defs>", group.located_receiver_block_opportunity_count, group.location_unavailable_receiver_block_opportunity_count);
    for sector in 0..8u8 {
        for ring in 0..4u8 {
            if let Some(cell) = common_polar_cell(group, sector, ring) {
                outcome_gradient(
                    out,
                    &format!("common-gradient-{index}-{sector}-{ring}"),
                    &cell.facts,
                );
            }
        }
    }
    out.push_str("</defs>");
    let edges = ReportDistanceBin::GEOMETRY_OUTER_EDGES_KM;
    let frame = SquareRootPolarFrame::new(edges[3]).unwrap();
    let radii = edges.map(|edge| frame.radius(edge).unwrap() * 100.0);
    for sector in 0..8u8 {
        for ring in 0..4u8 {
            let cell = common_polar_cell(group, sector, ring);
            let fill = cell.map_or_else(
                || "var(--coverage-land)".to_string(),
                |_| format!("url(#common-gradient-{index}-{sector}-{ring})"),
            );
            let inner = if ring == 0 {
                0.0
            } else {
                radii[ring as usize - 1]
            };
            let path = annular_sector_path(inner, radii[ring as usize], sector);
            let facts = cell.map(|cell| &cell.facts);
            write_html!(out, "<path class=\"coverage-polar-cell\" d=\"{path}\" fill=\"{fill}\"><title>{}, {}: {} opportunities; {} both, {} first only, {} second only, {} neither</title></path>", SECTOR_LABELS[sector as usize], ring_label(ring), facts.map_or(0, |cell| cell.receiver_block_opportunity_count), facts.map_or(0, |cell| cell.heard_both_count), facts.map_or(0, |cell| cell.left_only_count), facts.map_or(0, |cell| cell.right_only_count), facts.map_or(0, |cell| cell.heard_neither_count));
        }
    }
    out.push_str("<circle class=\"coverage-station\" r=\"2.2\"/><text class=\"coverage-cardinal\" x=\"0\" y=\"-101\">N</text><text class=\"coverage-cardinal\" x=\"102\" y=\"3\">E</text><text class=\"coverage-cardinal\" x=\"0\" y=\"106\">S</text><text class=\"coverage-cardinal\" x=\"-102\" y=\"3\">W</text></svg></figure>");
}

fn outcome_gradient(
    out: &mut CheckedHtmlWriter<'_>,
    id: &str,
    cell: &ReportCommonOpportunityCell<ReportDistanceBin>,
) {
    let total = cell.receiver_block_opportunity_count as f64;
    if total == 0.0 {
        return;
    }
    write_html!(
        out,
        "<linearGradient id=\"{id}\" x1=\"0\" x2=\"1\" y1=\"0\" y2=\"0\">"
    );
    let mut offset = 0.0;
    for (count, color) in [
        (cell.left_only_count, "var(--antenna-left)"),
        (cell.heard_both_count, "var(--coverage-both)"),
        (cell.right_only_count, "var(--antenna-right)"),
        (cell.heard_neither_count, "var(--coverage-neither)"),
    ] {
        if count == 0 {
            continue;
        }
        let next = offset + count as f64 / total * 100.0;
        write_html!(out, "<stop offset=\"{offset:.3}%\" stop-color=\"{color}\"/><stop offset=\"{next:.3}%\" stop-color=\"{color}\"/>");
        offset = next;
    }
    out.push_str("</linearGradient>");
}

fn render_common_polar_numbers(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
) {
    out.push_str("<div class=\"table-wrap coverage-numbers\"><table><caption>Common-opportunity polar cells (accessible equivalent)</caption><thead><tr><th scope=\"col\">Sector</th><th scope=\"col\">Distance</th><th scope=\"col\">Unique receivers</th><th scope=\"col\">Opportunities</th><th scope=\"col\">Both</th><th scope=\"col\">First only</th><th scope=\"col\">Second only</th><th scope=\"col\">Neither</th><th scope=\"col\">Detection rate — first / second</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    for sector in 0..8u8 {
        for ring in 0..4u8 {
            let cell = common_polar_cell(group, sector, ring).map(|cell| &cell.facts);
            write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>", SECTOR_LABELS[sector as usize], ring_label(ring), cell.map_or(0, |cell| cell.unique_common_active_receiver_count), cell.map_or(0, |cell| cell.receiver_block_opportunity_count), cell.map_or(0, |cell| cell.heard_both_count), cell.map_or(0, |cell| cell.left_only_count), cell.map_or(0, |cell| cell.right_only_count), cell.map_or(0, |cell| cell.heard_neither_count), cell.and_then(|cell| cell.left_detection_rate).map_or_else(|| "Not available".to_string(), percent), cell.and_then(|cell| cell.right_detection_rate).map_or_else(|| "Not available".to_string(), percent), cell.map_or(coverage_text(group.coverage), |cell| coverage_text(cell.coverage)));
        }
    }
    out.push_str("</tbody></table></div>");
}

fn render_common_marginals(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
) {
    out.push_str("<div class=\"table-wrap coverage-numbers\"><table><caption>Common-opportunity outcomes by distance category</caption><thead><tr><th scope=\"col\">Distance</th><th scope=\"col\">Unique receivers</th><th scope=\"col\">Opportunities</th><th scope=\"col\">Both</th><th scope=\"col\">First only</th><th scope=\"col\">Second only</th><th scope=\"col\">Neither</th><th scope=\"col\">First heard / second heard</th></tr></thead><tbody>");
    for cell in &group.distance_cells {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td></tr>", cell.category.label(), cell.unique_common_active_receiver_count, cell.receiver_block_opportunity_count, cell.heard_both_count, cell.left_only_count, cell.right_only_count, cell.heard_neither_count, cell.left_heard_count, cell.right_heard_count);
    }
    out.push_str("</tbody></table></div><div class=\"table-wrap coverage-numbers\"><table><caption>Common-opportunity outcomes by azimuth sector</caption><thead><tr><th scope=\"col\">Sector</th><th scope=\"col\">Unique receivers</th><th scope=\"col\">Opportunities</th><th scope=\"col\">Both</th><th scope=\"col\">First only</th><th scope=\"col\">Second only</th><th scope=\"col\">Neither</th><th scope=\"col\">First heard / second heard</th></tr></thead><tbody>");
    for cell in &group.azimuth_cells {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td></tr>", azimuth_sector_label(cell.category), cell.unique_common_active_receiver_count, cell.receiver_block_opportunity_count, cell.heard_both_count, cell.left_only_count, cell.right_only_count, cell.heard_neither_count, cell.left_heard_count, cell.right_heard_count);
    }
    write_html!(out, "</tbody></table></div><p class=\"muted\">Location unavailable: {} unique receivers / {} receiver-block opportunities. These remain in the overall common-active denominator.</p>", group.location_unavailable_unique_receiver_count, group.location_unavailable_receiver_block_opportunity_count);
}

fn render_common_block_audit(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
) {
    out.push_str("<details class=\"audit-disclosure\"><summary>Review per-block geographic outcome audit</summary><div class=\"table-wrap\"><table><caption>Common-opportunity geographic blocks</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Order / slots</th><th scope=\"col\">Coverage</th><th scope=\"col\">Common active</th><th scope=\"col\">Located / unavailable</th><th scope=\"col\">Populated polar cells</th></tr></thead><tbody>");
    for block in &group.blocks {
        write_html!(out, "<tr><td>{}</td><td>{}<br><span class=\"muted\">{} / {}</span></td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>", block.block_index + 1, comparison_order(block.order), escape_html(&block.left_slot_id), escape_html(&block.right_slot_id), coverage_text(block.coverage), block.common_active_receiver_count, block.located_receiver_count, block.location_unavailable_receiver_count, block.polar_cells.len());
    }
    out.push_str("</tbody></table></div></details>");
}

fn common_polar_cell(
    group: &ReportCommonOpportunityMapGroup,
    sector: u8,
    ring: u8,
) -> Option<&ReportCommonOpportunityPolarCell> {
    group.polar_cells.iter().find(|cell| {
        cell.bearing_sector == ReportAzimuthSector::ALL[sector as usize]
            && cell.distance_bin == ReportDistanceBin::ALL[ring as usize]
    })
}

fn percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn comparison_order(order: antennabench_analysis::ComparisonOrder) -> &'static str {
    match order {
        antennabench_analysis::ComparisonOrder::LeftThenRight => "First → second",
        antennabench_analysis::ComparisonOrder::RightThenLeft => "Second → first",
    }
}

fn azimuth_sector_label(sector: ReportAzimuthSector) -> &'static str {
    SECTOR_LABELS[ReportAzimuthSector::ALL
        .iter()
        .position(|candidate| *candidate == sector)
        .unwrap()]
}

fn render_shared_map_definitions(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let world = equirectangular_coastline_path();
    let polar = station_coordinates_from_grid(&report.context.station.grid)
        .and_then(AzimuthalEquidistantProjection::new)
        .map(polar_coastline_path)
        .unwrap_or_default();
    write_html!(out, "<svg class=\"coverage-defs\" width=\"0\" height=\"0\" aria-hidden=\"true\"><defs><path id=\"coverage-world-coast\" d=\"{}\"/><path id=\"coverage-polar-coast\" d=\"{}\"/></defs></svg>", world, polar);
}

fn render_full_group(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    group: &ReportCoverageMapGroup,
    index: usize,
) {
    write_html!(out, "<article class=\"coverage-group\" aria-labelledby=\"coverage-group-{index}\"><h3 id=\"coverage-group-{index}\">{}</h3>", comparison_stratum(&group.stratum));
    write_html!(out, "<div class=\"coverage-toggle\"><input class=\"coverage-choice\" type=\"radio\" name=\"coverage-view-{index}\" id=\"coverage-grid-{index}\" checked><input class=\"coverage-choice\" type=\"radio\" name=\"coverage-view-{index}\" id=\"coverage-polar-{index}\"><div class=\"coverage-tabs\" role=\"group\" aria-label=\"Coverage map view\"><label for=\"coverage-grid-{index}\">Grid squares</label><label for=\"coverage-polar-{index}\">Bearing and distance</label></div><div class=\"coverage-view coverage-grid-view\">");
    render_panel_pair(out, group, index, MapView::Grid, report);
    out.push_str("</div><div class=\"coverage-view coverage-polar-view\">");
    render_panel_pair(out, group, index, MapView::Polar, report);
    out.push_str("</div></div>");
    render_panel_numbers(out, group);
    out.push_str("</article>");
}

fn render_panel_pair(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCoverageMapGroup,
    group_index: usize,
    view: MapView,
    report: &SessionReport,
) {
    out.push_str("<div class=\"coverage-panels\">");
    for (panel_index, panel) in group.panels.iter().enumerate() {
        write_html!(out, "<figure class=\"coverage-panel coverage-side-{}\"><figcaption><strong>{}</strong><span>{} heard / {} active · {} unmapped</span></figcaption>", side_class(panel.side), escape_html(&panel.antenna_label), panel.heard_reporter_count, panel.active_reporter_count, panel.unmapped_reporter_count);
        match view {
            MapView::Grid => render_world_svg(out, panel, group_index, panel_index),
            MapView::Polar => render_polar_svg(
                out,
                panel,
                group_index,
                panel_index,
                &report.context.station.grid,
            ),
        }
        out.push_str("</figure>");
    }
    out.push_str("</div>");
}

fn render_world_svg(
    out: &mut CheckedHtmlWriter<'_>,
    panel: &ReportCoveragePanel,
    group_index: usize,
    panel_index: usize,
) {
    let hatch = format!("coverage-hatch-grid-{group_index}-{panel_index}");
    write_html!(out, "<svg class=\"coverage-world\" viewBox=\"0 0 360 180\" role=\"img\" aria-label=\"Four-character Maidenhead receiver coverage for {}\"><defs>{}</defs><rect class=\"coverage-ocean\" width=\"360\" height=\"180\"/><use href=\"#coverage-world-coast\" class=\"coverage-land\"/>", escape_html(&panel.antenna_label), hatch_pattern(&hatch));
    for cell in &panel.cells {
        let Some(center) = station_coordinates_from_grid(&cell.maidenhead_4) else {
            continue;
        };
        let x = center.longitude_degrees + 179.0;
        let y = 89.5 - center.latitude_degrees;
        let fill = state_fill(cell.state, panel.side, &hatch);
        write_html!(out, "<rect class=\"coverage-cell\" x=\"{x:.3}\" y=\"{y:.3}\" width=\"2\" height=\"1\" fill=\"{fill}\"><title>{}: {} heard of {} active reporters</title></rect>", escape_html(&cell.maidenhead_4), cell.heard_reporter_count, cell.active_reporter_count);
    }
    out.push_str("</svg>");
}

fn render_polar_svg(
    out: &mut CheckedHtmlWriter<'_>,
    panel: &ReportCoveragePanel,
    group_index: usize,
    panel_index: usize,
    station_grid: &str,
) {
    let hatch = format!("coverage-hatch-polar-{group_index}-{panel_index}");
    write_html!(out, "<svg class=\"coverage-polar\" viewBox=\"-108 -108 216 216\" role=\"img\" aria-label=\"Station-centered bearing and distance coverage for {}\"><defs>{}<clipPath id=\"coverage-clip-{group_index}-{panel_index}\"><circle r=\"100\"/></clipPath></defs><circle class=\"coverage-ocean\" r=\"100\"/><g clip-path=\"url(#coverage-clip-{group_index}-{panel_index})\"><use href=\"#coverage-polar-coast\" class=\"coverage-polar-coast\"/>", escape_html(&panel.antenna_label), hatch_pattern(&hatch));
    for distance in [5_000.0, 10_000.0, 15_000.0, 20_000.0] {
        let radius = distance / EARTH_ANTIPODE_DISTANCE_KM * 100.0;
        write_html!(out, "<circle class=\"coverage-ring\" r=\"{radius:.3}\"/>");
    }
    if let Some(station) = station_coordinates_from_grid(station_grid) {
        for reporter in &panel.reporters {
            let Some(destination) = station_coordinates_from_grid(&reporter.reporter_grid) else {
                continue;
            };
            let Some(position) = great_circle_position(station, destination) else {
                continue;
            };
            let radius = position.distance_km / EARTH_ANTIPODE_DISTANCE_KM * 100.0;
            let bearing = position.initial_bearing_degrees.to_radians();
            let x = radius * bearing.sin();
            let y = -radius * bearing.cos();
            let fill = state_fill(reporter.state, panel.side, &hatch);
            write_html!(out, "<circle class=\"coverage-dot\" cx=\"{x:.3}\" cy=\"{y:.3}\" r=\"1.6\" fill=\"{fill}\"><title>{} at {}: {}; {:.0} km, {:.0}°</title></circle>", escape_html(&reporter.reporter), escape_html(&reporter.reporter_grid), state_label(reporter.state), position.distance_km, position.initial_bearing_degrees);
        }
    }
    out.push_str("</g><circle class=\"coverage-station\" r=\"2.2\"><title>Station</title></circle><text class=\"coverage-cardinal\" x=\"0\" y=\"-101\">N</text><text class=\"coverage-cardinal\" x=\"102\" y=\"3\">E</text><text class=\"coverage-cardinal\" x=\"0\" y=\"106\">S</text><text class=\"coverage-cardinal\" x=\"-102\" y=\"3\">W</text></svg>");
}

fn render_panel_numbers(out: &mut CheckedHtmlWriter<'_>, group: &ReportCoverageMapGroup) {
    out.push_str("<div class=\"table-wrap coverage-numbers\"><table><caption>Coverage-map numbers (accessible equivalent)</caption><thead><tr><th scope=\"col\">Antenna</th><th scope=\"col\">Heard</th><th scope=\"col\">Active, not heard</th><th scope=\"col\">Mapped / unmapped</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    for panel in &group.panels {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>",
            escape_html(&panel.antenna_label),
            panel.heard_reporter_count,
            panel.active_reporter_count - panel.heard_reporter_count,
            panel.mapped_reporter_count,
            panel.unmapped_reporter_count,
            coverage_text(panel.coverage)
        );
    }
    out.push_str("</tbody></table></div>");
}

fn annular_sector_path(inner: f64, outer: f64, sector: u8) -> String {
    let start = -22.5 + f64::from(sector) * 45.0;
    let end = start + 45.0;
    let outer_start = polar_xy(outer, start);
    let outer_end = polar_xy(outer, end);
    if inner == 0.0 {
        format!(
            "M0 0 L{:.3} {:.3} A{outer:.3} {outer:.3} 0 0 1 {:.3} {:.3} Z",
            outer_start.0, outer_start.1, outer_end.0, outer_end.1
        )
    } else {
        let inner_start = polar_xy(inner, start);
        let inner_end = polar_xy(inner, end);
        format!("M{:.3} {:.3} L{:.3} {:.3} A{outer:.3} {outer:.3} 0 0 1 {:.3} {:.3} L{:.3} {:.3} A{inner:.3} {inner:.3} 0 0 0 {:.3} {:.3} Z", inner_start.0, inner_start.1, outer_start.0, outer_start.1, outer_end.0, outer_end.1, inner_end.0, inner_end.1, inner_start.0, inner_start.1)
    }
}

fn polar_xy(radius: f64, bearing_degrees: f64) -> (f64, f64) {
    let bearing = bearing_degrees.to_radians();
    (radius * bearing.sin(), -radius * bearing.cos())
}

fn equirectangular_coastline_path() -> String {
    let mut path = String::new();
    for coastline in natural_earth_coastline() {
        for (index, point) in coastline.points.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            let _ = write!(
                path,
                "{command}{:.1} {:.1}",
                point.longitude_degrees + 180.0,
                90.0 - point.latitude_degrees
            );
        }
        path.push('Z');
    }
    path
}

fn polar_coastline_path(projection: AzimuthalEquidistantProjection) -> String {
    let mut path = String::new();
    for coastline in projection.project_coastline() {
        for (index, point) in coastline.points.iter().enumerate() {
            let command = if index == 0 { 'M' } else { 'L' };
            let _ = write!(
                path,
                "{command}{:.2} {:.2}",
                point.x_km / EARTH_ANTIPODE_DISTANCE_KM * 100.0,
                -point.y_km / EARTH_ANTIPODE_DISTANCE_KM * 100.0
            );
        }
    }
    path
}

fn hatch_pattern(id: &str) -> String {
    format!("<pattern id=\"{id}\" width=\"6\" height=\"6\" patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><rect width=\"6\" height=\"6\" fill=\"var(--coverage-active-base)\"/><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"6\" stroke=\"var(--coverage-hatch)\" stroke-width=\"2\"/></pattern>")
}

fn state_fill(state: ReportCoverageState, side: ComparisonSide, hatch: &str) -> String {
    match state {
        ReportCoverageState::Heard => match side {
            ComparisonSide::Left => "var(--antenna-left)".to_string(),
            ComparisonSide::Right => "var(--antenna-right)".to_string(),
        },
        ReportCoverageState::ActiveNotHeard => format!("url(#{hatch})"),
    }
}

fn side_class(side: ComparisonSide) -> &'static str {
    match side {
        ComparisonSide::Left => "left",
        ComparisonSide::Right => "right",
    }
}

fn state_label(state: ReportCoverageState) -> &'static str {
    match state {
        ReportCoverageState::Heard => "heard",
        ReportCoverageState::ActiveNotHeard => "active, not heard",
    }
}

fn ring_label(ring: u8) -> &'static str {
    ReportDistanceBin::ALL
        .get(ring as usize)
        .copied()
        .unwrap_or(ReportDistanceBin::Km3000AndAbove)
        .label()
}

#[derive(Clone, Copy)]
enum MapView {
    Grid,
    Polar,
}
