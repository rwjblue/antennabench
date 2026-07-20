use std::collections::{BTreeMap, BTreeSet};

use antennabench_analysis::{
    ComparisonSide, ComparisonStratum, PathDirection, ReporterActivityAnalysis,
    ReporterActivityCoverage,
};

use crate::{
    great_circle_position, station_coordinates_from_grid, ReportCoverageCell,
    ReportCoverageMapGroup, ReportCoveragePanel, ReportCoveragePolarCell, ReportCoverageReporter,
    ReportCoverageState,
};

const POLAR_RING_EDGES_KM: [f64; 4] = [1_000.0, 3_000.0, 8_000.0, 20_015.0];

pub(crate) fn build_coverage_maps(
    station_grid: &str,
    activity: &ReporterActivityAnalysis,
) -> Vec<ReportCoverageMapGroup> {
    let station = station_coordinates_from_grid(station_grid);
    let mut strata = Vec::<ComparisonStratum>::new();
    for rate in &activity.cycle_rates {
        if rate.stratum.direction == PathDirection::Transmit && !strata.contains(&rate.stratum) {
            strata.push(rate.stratum.clone());
        }
    }
    strata
        .into_iter()
        .filter_map(|stratum| {
            let panels = [ComparisonSide::Left, ComparisonSide::Right]
                .into_iter()
                .filter_map(|side| build_panel(&stratum, side, station, activity))
                .collect::<Vec<_>>();
            (panels.len() == 2).then_some(ReportCoverageMapGroup { stratum, panels })
        })
        .collect()
}

fn build_panel(
    stratum: &ComparisonStratum,
    side: ComparisonSide,
    station: Option<crate::GeographicCoordinates>,
    activity: &ReporterActivityAnalysis,
) -> Option<ReportCoveragePanel> {
    let rates = activity
        .cycle_rates
        .iter()
        .filter(|rate| rate.stratum == *stratum && rate.side == Some(side))
        .collect::<Vec<_>>();
    if rates.is_empty()
        || rates
            .iter()
            .any(|rate| !rate.coverage.is_known() || rate.census_cycle_index.is_none())
    {
        return None;
    }
    let coverage = rates
        .iter()
        .map(|rate| rate.coverage)
        .fold(ReporterActivityCoverage::Complete, combine_coverage);
    let antenna_label = rates[0].antenna_label.clone();
    let mut reporters = BTreeMap::<String, (Option<String>, bool)>::new();
    for rate in rates {
        let heard = rate
            .heard_reporters
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let census = &activity.census_cycles[rate.census_cycle_index?];
        for reporter in &census.active_reporters {
            let entry = reporters
                .entry(reporter.reporter.clone())
                .or_insert_with(|| (reporter.reporter_grid.clone(), false));
            if entry.0.is_none() && reporter.reporter_grid.is_some() {
                entry.0.clone_from(&reporter.reporter_grid);
            }
            entry.1 |= heard.contains(reporter.reporter.as_str());
        }
    }

    let active_reporter_count = reporters.len();
    let heard_reporter_count = reporters.values().filter(|(_, heard)| *heard).count();
    let mut cells = BTreeMap::<String, (usize, usize)>::new();
    let mut mapped_reporters = Vec::new();
    let mut polar = BTreeMap::<(u8, u8), (usize, usize)>::new();
    for (reporter, (grid, heard)) in reporters {
        let Some(grid) = grid.filter(|grid| station_coordinates_from_grid(grid).is_some()) else {
            continue;
        };
        let maidenhead_4 = grid.trim().to_ascii_uppercase()[..4].to_string();
        let counts = cells.entry(maidenhead_4).or_default();
        counts.0 += 1;
        counts.1 += usize::from(heard);
        if let (Some(station), Some(destination)) = (station, station_coordinates_from_grid(&grid))
        {
            if let Some(position) = great_circle_position(station, destination) {
                let sector = ((position.initial_bearing_degrees + 22.5) / 45.0).floor() as u8 % 8;
                let ring = POLAR_RING_EDGES_KM
                    .iter()
                    .position(|edge| position.distance_km <= *edge)
                    .unwrap_or(3) as u8;
                let counts = polar.entry((sector, ring)).or_default();
                counts.0 += 1;
                counts.1 += usize::from(heard);
            }
        }
        mapped_reporters.push(ReportCoverageReporter {
            reporter,
            reporter_grid: grid,
            state: state(heard),
        });
    }
    let mapped_reporter_count = mapped_reporters.len();
    let unmapped_reporter_count = active_reporter_count - mapped_reporter_count;
    Some(ReportCoveragePanel {
        side,
        antenna_label,
        coverage,
        active_reporter_count,
        heard_reporter_count,
        mapped_reporter_count,
        unmapped_reporter_count,
        cells: cells
            .into_iter()
            .map(|(maidenhead_4, (active, heard))| ReportCoverageCell {
                maidenhead_4,
                state: state(heard > 0),
                active_reporter_count: active,
                heard_reporter_count: heard,
            })
            .collect(),
        polar_cells: polar
            .into_iter()
            .map(
                |((bearing_sector, distance_ring), (active, heard))| ReportCoveragePolarCell {
                    bearing_sector,
                    distance_ring,
                    state: state(heard > 0),
                    active_reporter_count: active,
                    heard_reporter_count: heard,
                },
            )
            .collect(),
        reporters: mapped_reporters,
    })
}

fn state(heard: bool) -> ReportCoverageState {
    if heard {
        ReportCoverageState::Heard
    } else {
        ReportCoverageState::ActiveNotHeard
    }
}

fn combine_coverage(
    left: ReporterActivityCoverage,
    right: ReporterActivityCoverage,
) -> ReporterActivityCoverage {
    match (left, right) {
        (ReporterActivityCoverage::Truncated, _) | (_, ReporterActivityCoverage::Truncated) => {
            ReporterActivityCoverage::Truncated
        }
        (ReporterActivityCoverage::Partial, _) | (_, ReporterActivityCoverage::Partial) => {
            ReporterActivityCoverage::Partial
        }
        (ReporterActivityCoverage::Unknown(reason), _)
        | (_, ReporterActivityCoverage::Unknown(reason)) => {
            ReporterActivityCoverage::Unknown(reason)
        }
        _ => ReporterActivityCoverage::Complete,
    }
}
