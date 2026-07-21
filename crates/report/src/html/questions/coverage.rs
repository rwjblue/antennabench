use std::fmt::Write as _;

use super::*;
use crate::{
    great_circle_position, natural_earth_coastline, station_coordinates_from_grid,
    AzimuthalEquidistantProjection, ReportCommonOpportunityCell, ReportCommonOpportunityMapGroup,
    ReportCommonOpportunityPolarCell, ReportCoverageMapGroup, ReportCoveragePanel,
    ReportCoverageState, EARTH_ANTIPODE_DISTANCE_KM,
};
use antennabench_analysis::ComparisonSide;

const SECTOR_LABELS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

pub(in super::super) fn render_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Common-opportunity detection by distance and bearing</h2><p>These maps reuse the common-active listening opportunities from the preceding comparison and group them by receiver location. Each antenna has a separate map; every supported cell shows its detection rate and heard/opportunity count.</p><p class=\"muted\">Rows retain the four fixed distance categories and columns retain the eight bearing sectors. The rows are categorical—not a proportional geographic radius—and this controlled comparison remains separate from the all-path observed distance and direction profile.</p>");
    if report.common_opportunity_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified common-active receiver population is available. Missing census evidence is not shown as no detection.</p></section>");
        return;
    }
    render_rate_legend(out, report);
    for (index, group) in report.common_opportunity_maps.iter().enumerate() {
        render_common_group(out, report, group, index, true);
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
    out.push_str("<section id=\"coverage-map\" class=\"panel question-section compact-coverage\" tabindex=\"-1\" aria-labelledby=\"coverage-map-title\"><h2 id=\"coverage-map-title\">Common-opportunity detection by distance and bearing</h2><p>This uses the same shared listening opportunities as the section above, now grouped by each active receiver’s location. Each antenna gets its own detection-rate map so its heard count is read against the same eligible denominator in every cell.</p><p class=\"muted\">Rows retain the four fixed distance categories and columns retain the eight bearing sectors. The rows are categorical—not a proportional geographic radius—and this controlled comparison remains separate from the all-path observed footprint.</p>");
    if report.common_opportunity_maps.is_empty() {
        out.push_str("<p class=\"empty\"><strong>Coverage unknown:</strong> no band-qualified common-active receiver population is available. Missing census evidence is not shown as no detection.</p></section>");
        return;
    }
    render_rate_legend(out, report);
    for (index, group) in report.common_opportunity_maps.iter().enumerate() {
        render_common_group(out, report, group, index, false);
    }
    out.push_str("</section>");
}

fn render_rate_legend(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let (left, right) = report_antenna_labels(report);
    write_html!(out, "<div class=\"notice common-opportunity-rate-legend\" aria-label=\"Per-antenna detection-rate map legend\"><strong>How to read the two maps</strong><p>Darker <span class=\"left-rate-word\">blue</span> means a higher {} detection rate; darker <span class=\"right-rate-word\">orange</span> means a higher {} detection rate. Every supported cell prints its percentage and heard/opportunity count.</p><small>Dashed outline: low support (&lt;5 opportunities) · hatched: zero opportunities · dotted: unavailable. Missing evidence is never zero.</small></div>", left, right);
}

fn render_common_group(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    group: &ReportCommonOpportunityMapGroup,
    index: usize,
    include_audit: bool,
) {
    write_html!(out, "<article class=\"coverage-group common-opportunity-group\" aria-labelledby=\"common-opportunity-{index}\"><h3 id=\"common-opportunity-{index}\">{}</h3><p class=\"muted\">{} unique common-active receivers; {} receiver-block opportunities; {} / {} opportunities located. Coverage: {} ({} of {} eligible blocks known).</p>", comparison_stratum(&group.stratum), group.unique_common_active_receiver_count, group.receiver_block_opportunity_count, group.located_receiver_block_opportunity_count, group.receiver_block_opportunity_count, coverage_text(group.coverage), group.known_coverage_block_count, group.eligible_block_count);
    if !group.coverage.is_known() {
        write_html!(out, "<p class=\"empty\"><strong>Common-opportunity geography unavailable:</strong> {}. Missing activity evidence is not treated as no detection.</p></article>", coverage_text(group.coverage));
        return;
    }
    render_common_findings(out, report, group);
    render_common_rate_maps(out, report, group, index);
    write_html!(out, "<details class=\"audit-disclosure polar-data-disclosure\"><summary>Show exact distance and bearing data</summary><div class=\"disclosure-body\">");
    render_common_polar_numbers(out, group);
    render_common_marginals(out, group);
    out.push_str("</div></details>");
    if include_audit && !group.blocks.is_empty() {
        render_common_block_audit(out, group);
    }
    out.push_str("</article>");
}

fn render_common_findings(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    group: &ReportCommonOpportunityMapGroup,
) {
    let most_material = group
        .polar_cells
        .iter()
        .filter_map(|cell| {
            let left = cell.facts.left_detection_rate?;
            let right = cell.facts.right_detection_rate?;
            (cell.facts.receiver_block_opportunity_count > 0)
                .then_some((cell, (right - left) * 100.0))
        })
        .max_by(|(left_cell, left_delta), (right_cell, right_delta)| {
            left_delta
                .abs()
                .total_cmp(&right_delta.abs())
                .then_with(|| {
                    left_cell
                        .facts
                        .receiver_block_opportunity_count
                        .cmp(&right_cell.facts.receiver_block_opportunity_count)
                })
        });
    if let Some((cell, difference)) = most_material {
        let (left_antenna, right_antenna) = report_antenna_labels(report);
        let (higher_label, lower_label) = if difference >= 0.0 {
            (right_antenna, left_antenna)
        } else {
            (left_antenna, right_antenna)
        };
        write_html!(out, "<p><strong>Most pronounced recorded cell:</strong> {} / {} had a {} percentage-point difference ({} higher than {}; {} common opportunities{}). This is session-scoped common-opportunity evidence, not a radiation pattern or universal ranking.</p>", azimuth_sector_label(cell.bearing_sector), cell.distance_bin.label(), format_number(difference.abs()), higher_label, lower_label, cell.facts.receiver_block_opportunity_count, if cell.facts.receiver_block_opportunity_count < 5 { "; low support" } else { "" });
    }
}

fn render_common_rate_maps(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    group: &ReportCommonOpportunityMapGroup,
    index: usize,
) {
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<div class=\"common-opportunity-rate-maps\" aria-label=\"Separate antenna detection-rate maps; {} located opportunities and {} with location unavailable\">", group.located_receiver_block_opportunity_count, group.location_unavailable_receiver_block_opportunity_count);
    render_common_rate_map(out, group, index, ComparisonSide::Left, &left_label);
    render_common_rate_map(out, group, index, ComparisonSide::Right, &right_label);
    out.push_str("</div>");
}

fn render_common_rate_map(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
    group_index: usize,
    side: ComparisonSide,
    antenna_label: &str,
) {
    let side_class = match side {
        ComparisonSide::Left => "left",
        ComparisonSide::Right => "right",
    };
    write_html!(out, "<figure class=\"coverage-panel common-opportunity-rate-panel {side_class}\"><figcaption><strong>{}</strong><span>Detection rate among common-active opportunities</span></figcaption><div class=\"common-opportunity-rate-scroll\"><table class=\"common-opportunity-rate-map\"><caption class=\"visually-hidden\">{} detection rate by distance and bearing for comparison group {}</caption><thead><tr><th scope=\"col\">Distance</th>", escape_html(antenna_label), escape_html(antenna_label), group_index + 1);
    for sector in SECTOR_LABELS {
        write_html!(out, "<th scope=\"col\">{sector}</th>");
    }
    out.push_str("</tr></thead><tbody>");
    for ring in 0..4u8 {
        write_html!(out, "<tr><th scope=\"row\">{}</th>", ring_label(ring));
        for sector in 0..8u8 {
            let cell = common_polar_cell(group, sector, ring).map(|value| &value.facts);
            let presentation = rate_cell_presentation(
                cell,
                side,
                SECTOR_LABELS[sector as usize],
                ring_label(ring),
                antenna_label,
                group.coverage,
            );
            write_html!(out, "<td class=\"common-opportunity-rate-cell {side_class} {}\" tabindex=\"0\" aria-label=\"{}\" title=\"{}\"><strong>{}</strong><small>{}</small></td>", presentation.class, escape_html(&presentation.label), escape_html(&presentation.label), presentation.rate_text, presentation.count_text);
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div></figure>");
}

struct RateCellPresentation {
    class: String,
    label: String,
    rate_text: String,
    count_text: String,
}

fn rate_cell_presentation(
    cell: Option<&ReportCommonOpportunityCell<ReportDistanceBin>>,
    side: ComparisonSide,
    sector: &str,
    distance: &str,
    antenna_label: &str,
    group_coverage: antennabench_analysis::ReporterActivityCoverage,
) -> RateCellPresentation {
    let Some(cell) = cell else {
        let label = format!(
            "{antenna_label}, {sector}, {distance}: unavailable; no located common-opportunity cell"
        );
        return RateCellPresentation {
            class: "rate-unavailable".to_string(),
            label,
            rate_text: "—".to_string(),
            count_text: "no cell".to_string(),
        };
    };
    let opportunities = cell.receiver_block_opportunity_count;
    if opportunities == 0 {
        let label = format!(
            "{antenna_label}, {sector}, {distance}: zero common opportunities; Rate unavailable; not zero detection; {}",
            coverage_text(cell.coverage)
        );
        return RateCellPresentation {
            class: "zero-opportunities".to_string(),
            label,
            rate_text: "n/a".to_string(),
            count_text: "0 opp".to_string(),
        };
    }
    let (rate, heard) = match side {
        ComparisonSide::Left => (cell.left_detection_rate, cell.left_heard_count),
        ComparisonSide::Right => (cell.right_detection_rate, cell.right_heard_count),
    };
    let Some(rate) = rate else {
        let label = format!(
            "{antenna_label}, {sector}, {distance}: Rate unavailable with {opportunities} opportunities; not zero detection; {}",
            coverage_text(group_coverage)
        );
        return RateCellPresentation {
            class: "rate-unavailable".to_string(),
            label,
            rate_text: "n/a".to_string(),
            count_text: format!("{opportunities} opp"),
        };
    };
    let tone = if rate == 0.0 {
        "rate-zero"
    } else if rate <= 0.1 {
        "rate-low"
    } else if rate <= 0.25 {
        "rate-medium"
    } else if rate <= 0.5 {
        "rate-high"
    } else {
        "rate-very-high"
    };
    let support = if opportunities < 5 {
        " low-support"
    } else {
        ""
    };
    let qualification = if opportunities < 5 {
        format!(
            "Low support: {opportunities} opportunities; {}",
            coverage_text(cell.coverage)
        )
    } else {
        format!(
            "{opportunities} common opportunities; {}",
            coverage_text(cell.coverage)
        )
    };
    let label = format!("{antenna_label}, {sector}, {distance}: {heard} heard of {opportunities}, {:.1}% detection; {qualification}", rate * 100.0);
    RateCellPresentation {
        class: format!("{tone}{support}"),
        label,
        rate_text: format!("{:.0}%", rate * 100.0),
        count_text: format!("{heard}/{opportunities}"),
    }
}

fn render_common_polar_numbers(
    out: &mut CheckedHtmlWriter<'_>,
    group: &ReportCommonOpportunityMapGroup,
) {
    out.push_str("<div class=\"table-wrap coverage-numbers\"><table><caption>Common-opportunity distance and bearing cells</caption><thead><tr><th scope=\"col\">Sector</th><th scope=\"col\">Distance</th><th scope=\"col\">Unique receivers</th><th scope=\"col\">Opportunities</th><th scope=\"col\">Both</th><th scope=\"col\">First only</th><th scope=\"col\">Second only</th><th scope=\"col\">Neither</th><th scope=\"col\">Detection rate — first / second</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
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
    out.push_str("<details class=\"audit-disclosure\"><summary>Review per-block geographic outcome audit</summary><div class=\"table-wrap\"><table><caption>Common-opportunity geographic blocks</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Order / slots</th><th scope=\"col\">Coverage</th><th scope=\"col\">Common active</th><th scope=\"col\">Located / unavailable</th><th scope=\"col\">Populated distance × bearing cells</th></tr></thead><tbody>");
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
