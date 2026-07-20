use std::fmt::Write as _;

use super::*;
use crate::{
    great_circle_position, natural_earth_coastline, station_coordinates_from_grid,
    AzimuthalEquidistantProjection, ReportCoverageMapGroup, ReportCoveragePanel,
    ReportCoveragePolarCell, ReportCoverageState, SquareRootPolarFrame, EARTH_ANTIPODE_DISTANCE_KM,
};
use antennabench_analysis::ComparisonSide;

const SECTOR_LABELS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

pub(in super::super) fn render_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Where active receivers heard each antenna</h2><p class=\"muted\">Each comparison group stays separate. Identity color means heard; neutral hatching means active on this band but did not report this antenna; plain land means no active-receiver evidence.</p>");
    if report.coverage_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no complete pair of band-qualified, located reporter-activity panels is available. The map is omitted rather than rendering missing census evidence as no reach.</p></section>");
        return;
    }
    render_shared_map_definitions(out, report);
    for (index, group) in report.coverage_maps.iter().enumerate() {
        render_full_group(out, report, group, index);
    }
    out.push_str("<p class=\"muted\">Cells merge reporters at four-character Maidenhead granularity; exact retained locator values drive polar placement. Unmapped reporters remain in activity totals and are counted below.</p></section>");
}

pub(in super::super) fn render_compact_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section compact-coverage\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Active-receiver coverage shape</h2><p class=\"muted\">Eight bearing sectors × four square-root-scaled distance rings. Identity color means heard; neutral hatching means active, not heard; plain cells mean no active receivers.</p>");
    if report.coverage_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified reporter-activity map is available. Missing census evidence is not shown as no reach.</p></section>");
        return;
    }
    for (index, group) in report.coverage_maps.iter().enumerate() {
        render_compact_group(out, group, index);
    }
    out.push_str("</section>");
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

fn render_compact_group(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCoverageMapGroup,
    index: usize,
) {
    write_html!(
        out,
        "<article class=\"coverage-group\"><h3>{}</h3><div class=\"coverage-panels\">",
        comparison_stratum(&group.stratum)
    );
    for (panel_index, panel) in group.panels.iter().enumerate() {
        let hatch = format!("compact-hatch-{index}-{panel_index}");
        write_html!(out, "<figure class=\"coverage-panel coverage-side-{}\"><figcaption><strong>{}</strong><span>{} heard / {} active · {} unmapped</span></figcaption><svg class=\"coverage-polar-cells\" viewBox=\"-108 -108 216 216\" role=\"img\" aria-label=\"Eight-sector by four-ring coverage cells for {}\"><defs>{}</defs>", side_class(panel.side), escape_html(&panel.antenna_label), panel.heard_reporter_count, panel.active_reporter_count, panel.unmapped_reporter_count, escape_html(&panel.antenna_label), hatch_pattern(&hatch));
        render_polar_cells(out, panel, &hatch);
        out.push_str("<circle class=\"coverage-station\" r=\"2.2\"/><text class=\"coverage-cardinal\" x=\"0\" y=\"-101\">N</text><text class=\"coverage-cardinal\" x=\"102\" y=\"3\">E</text><text class=\"coverage-cardinal\" x=\"0\" y=\"106\">S</text><text class=\"coverage-cardinal\" x=\"-102\" y=\"3\">W</text></svg></figure>");
    }
    out.push_str("</div>");
    render_polar_numbers(out, group);
    out.push_str("</article>");
}

fn render_polar_cells(out: &mut CheckedHtmlWriter<'_>, panel: &ReportCoveragePanel, hatch: &str) {
    let edges = ReportDistanceBin::GEOMETRY_OUTER_EDGES_KM;
    let frame = SquareRootPolarFrame::new(edges[3]).unwrap();
    let radii = edges.map(|edge| frame.radius(edge).unwrap() * 100.0);
    for sector in 0..8u8 {
        for ring in 0..4u8 {
            let cell = polar_cell(panel, sector, ring);
            let state = cell.map(|cell| cell.state);
            let fill = state.map_or_else(
                || "var(--coverage-land)".to_string(),
                |state| state_fill(state, panel.side, hatch),
            );
            let inner = if ring == 0 {
                0.0
            } else {
                radii[ring as usize - 1]
            };
            let outer = radii[ring as usize];
            let path = annular_sector_path(inner, outer, sector);
            let (heard, active) = cell
                .map(|cell| (cell.heard_reporter_count, cell.active_reporter_count))
                .unwrap_or_default();
            write_html!(out, "<path class=\"coverage-polar-cell\" d=\"{path}\" fill=\"{fill}\"><title>{}, {}: {} heard of {} active reporters</title></path>", SECTOR_LABELS[sector as usize], ring_label(ring), heard, active);
        }
    }
}

fn render_polar_numbers(out: &mut CheckedHtmlWriter<'_>, group: &ReportCoverageMapGroup) {
    let left = &group.panels[0];
    let right = &group.panels[1];
    write_html!(out, "<div class=\"table-wrap coverage-numbers\"><table><caption>Polar-cell numbers (accessible equivalent)</caption><thead><tr><th scope=\"col\">Sector</th><th scope=\"col\">Distance</th><th scope=\"col\">{}</th><th scope=\"col\">{}</th></tr></thead><tbody>", escape_html(&left.antenna_label), escape_html(&right.antenna_label));
    for sector in 0..8u8 {
        for ring in 0..4u8 {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                SECTOR_LABELS[sector as usize],
                ring_label(ring),
                polar_number(polar_cell(left, sector, ring)),
                polar_number(polar_cell(right, sector, ring))
            );
        }
    }
    out.push_str("</tbody></table></div>");
}

fn polar_number(cell: Option<&ReportCoveragePolarCell>) -> String {
    cell.map_or_else(
        || "No active receivers".to_string(),
        |cell| {
            format!(
                "{} heard / {} active",
                cell.heard_reporter_count, cell.active_reporter_count
            )
        },
    )
}

fn polar_cell(
    panel: &ReportCoveragePanel,
    sector: u8,
    ring: u8,
) -> Option<&ReportCoveragePolarCell> {
    panel
        .polar_cells
        .iter()
        .find(|cell| cell.bearing_sector == sector && cell.distance_ring == ring)
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
