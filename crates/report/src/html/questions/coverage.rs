use std::fmt::Write as _;

use super::*;
use crate::{
    great_circle_position,
    html::{
        templates::{render_template, CoverageTemplate},
        view::{
            CommonBlockView, CommonCellRowView, CommonCoverageGroupView, CommonMarginalRowView,
            CoverageView, LegacyCoverageGroupView, LegacyPanelView, PolarDotView, RateCellView,
            RateMapRowView, RateMapView, WorldCellView,
        },
    },
    natural_earth_coastline, station_coordinates_from_grid, AzimuthalEquidistantProjection,
    ReportCommonOpportunityCell, ReportCommonOpportunityMapGroup, ReportCommonOpportunityPolarCell,
    ReportCoverageMapGroup, ReportCoveragePanel, ReportCoverageState, EARTH_ANTIPODE_DISTANCE_KM,
};
use antennabench_analysis::ComparisonSide;

const SECTOR_LABELS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

pub(in super::super) fn render_coverage_map_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &CoverageTemplate {
            view: coverage_view(report, false),
        },
    )
}

fn coverage_view(report: &SessionReport, summary: bool) -> CoverageView {
    let AntennaLabels {
        left: left_label,
        right: right_label,
    } = antenna_labels(report);
    let legacy_groups = if summary {
        Vec::new()
    } else {
        report
            .coverage_maps
            .iter()
            .enumerate()
            .map(|(index, group)| legacy_group_view(report, group, index))
            .collect()
    };
    CoverageView {
        summary,
        no_groups: report.common_opportunity_maps.is_empty(),
        groups: report
            .common_opportunity_maps
            .iter()
            .enumerate()
            .map(|(index, group)| {
                common_group_view(group, index, !summary, &left_label, &right_label)
            })
            .collect(),
        left_label,
        right_label,
        world_coastline: equirectangular_coastline_path(),
        polar_coastline: station_coordinates_from_grid(&report.context.station.grid)
            .and_then(AzimuthalEquidistantProjection::new)
            .map(polar_coastline_path)
            .unwrap_or_default(),
        legacy_groups,
    }
}

fn common_group_view(
    group: &ReportCommonOpportunityMapGroup,
    index: usize,
    include_audit: bool,
    left_label: &str,
    right_label: &str,
) -> CommonCoverageGroupView {
    let rate_maps = [
        (ComparisonSide::Left, left_label),
        (ComparisonSide::Right, right_label),
    ]
    .into_iter()
    .map(|(side, label)| rate_map_view(group, index, side, label))
    .collect();
    let cells = (0..8u8)
        .flat_map(|sector| {
            (0..4u8).map(move |ring| {
                let cell = common_polar_cell(group, sector, ring).map(|cell| &cell.facts);
                CommonCellRowView {
                    sector: SECTOR_LABELS[sector as usize],
                    distance: ring_label(ring),
                    receivers: cell.map_or(0, |cell| cell.unique_common_active_receiver_count),
                    opportunities: cell.map_or(0, |cell| cell.receiver_block_opportunity_count),
                    both: cell.map_or(0, |cell| cell.heard_both_count),
                    left_only: cell.map_or(0, |cell| cell.left_only_count),
                    right_only: cell.map_or(0, |cell| cell.right_only_count),
                    neither: cell.map_or(0, |cell| cell.heard_neither_count),
                    left_rate: cell
                        .and_then(|cell| cell.left_detection_rate)
                        .map_or_else(|| "Not available".to_string(), percent),
                    right_rate: cell
                        .and_then(|cell| cell.right_detection_rate)
                        .map_or_else(|| "Not available".to_string(), percent),
                    coverage: cell.map_or(coverage_text(group.coverage), |cell| {
                        coverage_text(cell.coverage)
                    }),
                }
            })
        })
        .collect();
    CommonCoverageGroupView {
        index,
        label: comparison_group_label(&group.stratum),
        receivers: group.unique_common_active_receiver_count,
        opportunities: group.receiver_block_opportunity_count,
        located_opportunities: group.located_receiver_block_opportunity_count,
        coverage: coverage_text(group.coverage),
        known_blocks: group.known_coverage_block_count,
        eligible_blocks: group.eligible_block_count,
        coverage_known: group.coverage.is_known(),
        finding: common_finding(group, left_label, right_label),
        rate_maps,
        cells,
        distance_rows: group
            .distance_cells
            .iter()
            .map(|cell| CommonMarginalRowView {
                label: cell.category.label(),
                receivers: cell.unique_common_active_receiver_count,
                opportunities: cell.receiver_block_opportunity_count,
                both: cell.heard_both_count,
                left_only: cell.left_only_count,
                right_only: cell.right_only_count,
                neither: cell.heard_neither_count,
                left_heard: cell.left_heard_count,
                right_heard: cell.right_heard_count,
            })
            .collect(),
        azimuth_rows: group
            .azimuth_cells
            .iter()
            .map(|cell| CommonMarginalRowView {
                label: azimuth_sector_label(cell.category),
                receivers: cell.unique_common_active_receiver_count,
                opportunities: cell.receiver_block_opportunity_count,
                both: cell.heard_both_count,
                left_only: cell.left_only_count,
                right_only: cell.right_only_count,
                neither: cell.heard_neither_count,
                left_heard: cell.left_heard_count,
                right_heard: cell.right_heard_count,
            })
            .collect(),
        unavailable_receivers: group.location_unavailable_unique_receiver_count,
        unavailable_opportunities: group.location_unavailable_receiver_block_opportunity_count,
        blocks: group
            .blocks
            .iter()
            .map(|block| CommonBlockView {
                block: block.block_index + 1,
                order: comparison_order(block.order),
                left_slot: block.left_slot_id.clone(),
                right_slot: block.right_slot_id.clone(),
                coverage: coverage_text(block.coverage),
                active: block.common_active_receiver_count,
                located: block.located_receiver_count,
                unavailable: block.location_unavailable_receiver_count,
                populated_cells: block.polar_cells.len(),
            })
            .collect(),
        include_audit,
    }
}

fn common_finding(
    group: &ReportCommonOpportunityMapGroup,
    left_label: &str,
    right_label: &str,
) -> Option<String> {
    group
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
        })
        .map(|(cell, difference)| {
            let (higher_label, lower_label) = if difference >= 0.0 {
                (right_label, left_label)
            } else {
                (left_label, right_label)
            };
            format!(
                "{} / {} had a {} percentage-point difference ({higher_label} higher than {lower_label}; {} common opportunities{}).",
                azimuth_sector_label(cell.bearing_sector),
                cell.distance_bin.label(),
                format_number(difference.abs()),
                cell.facts.receiver_block_opportunity_count,
                if cell.facts.receiver_block_opportunity_count < 5 {
                    "; low support"
                } else {
                    ""
                }
            )
        })
}

fn rate_map_view(
    group: &ReportCommonOpportunityMapGroup,
    group_index: usize,
    side: ComparisonSide,
    antenna_label: &str,
) -> RateMapView {
    RateMapView {
        side: side_class(side),
        antenna: antenna_label.to_string(),
        group_number: group_index + 1,
        rows: (0..4u8)
            .map(|ring| RateMapRowView {
                distance: ring_label(ring),
                cells: (0..8u8)
                    .map(|sector| {
                        rate_cell_presentation(
                            common_polar_cell(group, sector, ring).map(|value| &value.facts),
                            side,
                            SECTOR_LABELS[sector as usize],
                            ring_label(ring),
                            antenna_label,
                            group.coverage,
                        )
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn rate_cell_presentation(
    cell: Option<&ReportCommonOpportunityCell<ReportDistanceBin>>,
    side: ComparisonSide,
    sector: &str,
    distance: &str,
    antenna_label: &str,
    group_coverage: antennabench_analysis::ReporterActivityCoverage,
) -> RateCellView {
    let Some(cell) = cell else {
        return RateCellView {
            class: "rate-unavailable".to_string(),
            label: format!(
                "{antenna_label}, {sector}, {distance}: unavailable; no located common-opportunity cell"
            ),
            rate: "—".to_string(),
            count: "no cell".to_string(),
        };
    };
    let opportunities = cell.receiver_block_opportunity_count;
    if opportunities == 0 {
        return RateCellView {
            class: "zero-opportunities".to_string(),
            label: format!(
                "{antenna_label}, {sector}, {distance}: zero common opportunities; Rate unavailable; not zero detection; {}",
                coverage_text(cell.coverage)
            ),
            rate: "n/a".to_string(),
            count: "0 opp".to_string(),
        };
    }
    let (rate, heard) = match side {
        ComparisonSide::Left => (cell.left_detection_rate, cell.left_heard_count),
        ComparisonSide::Right => (cell.right_detection_rate, cell.right_heard_count),
    };
    let Some(rate) = rate else {
        return RateCellView {
            class: "rate-unavailable".to_string(),
            label: format!(
                "{antenna_label}, {sector}, {distance}: Rate unavailable with {opportunities} opportunities; not zero detection; {}",
                coverage_text(group_coverage)
            ),
            rate: "n/a".to_string(),
            count: format!("{opportunities} opp"),
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
    let low_support = opportunities < 5;
    let qualification = if low_support {
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
    RateCellView {
        class: format!("{tone}{}", if low_support { " low-support" } else { "" }),
        label: format!(
            "{antenna_label}, {sector}, {distance}: {heard} heard of {opportunities}, {:.1}% detection; {qualification}",
            rate * 100.0
        ),
        rate: format!("{:.0}%", rate * 100.0),
        count: format!("{heard}/{opportunities}"),
    }
}

fn legacy_group_view(
    report: &SessionReport,
    group: &ReportCoverageMapGroup,
    index: usize,
) -> LegacyCoverageGroupView {
    LegacyCoverageGroupView {
        index,
        label: comparison_group_label(&group.stratum),
        panels: group
            .panels
            .iter()
            .enumerate()
            .map(|(panel_index, panel)| legacy_panel_view(report, panel, index, panel_index))
            .collect(),
    }
}

fn legacy_panel_view(
    report: &SessionReport,
    panel: &ReportCoveragePanel,
    group_index: usize,
    panel_index: usize,
) -> LegacyPanelView {
    let grid_hatch = format!("coverage-hatch-grid-{group_index}-{panel_index}");
    let polar_hatch = format!("coverage-hatch-polar-{group_index}-{panel_index}");
    let polar_clip = format!("coverage-clip-{group_index}-{panel_index}");
    let world_cells = panel
        .cells
        .iter()
        .filter_map(|cell| {
            let center = station_coordinates_from_grid(&cell.maidenhead_4)?;
            Some(WorldCellView {
                x: format!("{:.3}", center.longitude_degrees + 179.0),
                y: format!("{:.3}", 89.5 - center.latitude_degrees),
                fill: state_fill(cell.state, panel.side, &grid_hatch),
                title: format!(
                    "{}: {} heard of {} active reporters",
                    cell.maidenhead_4, cell.heard_reporter_count, cell.active_reporter_count
                ),
            })
        })
        .collect();
    let polar_dots = station_coordinates_from_grid(&report.context.station.grid)
        .map(|station| {
            panel
                .reporters
                .iter()
                .filter_map(|reporter| {
                    let destination = station_coordinates_from_grid(&reporter.reporter_grid)?;
                    let position = great_circle_position(station, destination)?;
                    let radius = position.distance_km / EARTH_ANTIPODE_DISTANCE_KM * 100.0;
                    let bearing = position.initial_bearing_degrees.to_radians();
                    Some(PolarDotView {
                        x: format!("{:.3}", radius * bearing.sin()),
                        y: format!("{:.3}", -radius * bearing.cos()),
                        fill: state_fill(reporter.state, panel.side, &polar_hatch),
                        title: format!(
                            "{} at {}: {}; {:.0} km, {:.0}°",
                            reporter.reporter,
                            reporter.reporter_grid,
                            state_label(reporter.state),
                            position.distance_km,
                            position.initial_bearing_degrees
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    LegacyPanelView {
        side: side_class(panel.side),
        antenna: panel.antenna_label.clone(),
        heard: panel.heard_reporter_count,
        active: panel.active_reporter_count,
        mapped: panel.mapped_reporter_count,
        unmapped: panel.unmapped_reporter_count,
        coverage: coverage_text(panel.coverage),
        grid_hatch,
        polar_hatch,
        polar_clip,
        world_cells,
        polar_rings: [5_000.0, 10_000.0, 15_000.0, 20_000.0]
            .into_iter()
            .map(|distance| format!("{:.3}", distance / EARTH_ANTIPODE_DISTANCE_KM * 100.0))
            .collect(),
        polar_dots,
    }
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
