use std::collections::{BTreeMap, BTreeSet};

use antennabench_analysis::{
    ReporterActivityAnalysis, ReporterActivityCoverage, ReporterActivityJointOutcome,
};

use crate::{
    great_circle_position, station_coordinates_from_grid, ReportAzimuthSector,
    ReportCommonOpportunityBlock, ReportCommonOpportunityCell, ReportCommonOpportunityMapGroup,
    ReportCommonOpportunityPolarCell, ReportDistanceBin,
};

pub(crate) fn build_common_opportunity_maps(
    station_grid: &str,
    activity: &ReporterActivityAnalysis,
) -> Vec<ReportCommonOpportunityMapGroup> {
    let station = station_coordinates_from_grid(station_grid);
    activity
        .joint_summaries
        .iter()
        .map(|summary| {
            let mut rows = activity
                .paired_rates
                .iter()
                .filter(|row| row.stratum == summary.stratum)
                .collect::<Vec<_>>();
            rows.sort_by_key(|row| row.block_index);
            let mut receiver_grids = BTreeMap::<String, Vec<Option<String>>>::new();
            for row in &rows {
                for receiver in &row.receivers {
                    receiver_grids
                        .entry(receiver.receiver.clone())
                        .or_default()
                        .push(receiver.receiver_grid.clone());
                }
            }
            let locations = receiver_grids
                .into_iter()
                .map(|(receiver, grids)| {
                    let location = resolve_location(station, &grids);
                    (receiver, location)
                })
                .collect::<BTreeMap<_, _>>();
            let mut distance = ReportDistanceBin::ALL.map(|_| CellAccumulator::default());
            let mut azimuth = ReportAzimuthSector::ALL.map(|_| CellAccumulator::default());
            let mut polar = BTreeMap::<(usize, usize), CellAccumulator>::new();
            let mut blocks = Vec::new();
            let mut located_opportunities = 0;

            for row in rows {
                let mut block_polar = BTreeMap::<(usize, usize), CellAccumulator>::new();
                let mut block_located = 0;
                for receiver in &row.receivers {
                    let Some(location) = locations
                        .get(&receiver.receiver)
                        .and_then(|location| *location)
                    else {
                        continue;
                    };
                    located_opportunities += 1;
                    block_located += 1;
                    distance[location.distance.index()].add(
                        &receiver.receiver,
                        receiver.outcome,
                        row.coverage,
                    );
                    azimuth[location.azimuth_index].add(
                        &receiver.receiver,
                        receiver.outcome,
                        row.coverage,
                    );
                    polar
                        .entry((location.azimuth_index, location.distance.index()))
                        .or_default()
                        .add(&receiver.receiver, receiver.outcome, row.coverage);
                    block_polar
                        .entry((location.azimuth_index, location.distance.index()))
                        .or_default()
                        .add(&receiver.receiver, receiver.outcome, row.coverage);
                }
                blocks.push(ReportCommonOpportunityBlock {
                    block_index: row.block_index,
                    order: row.order,
                    left_slot_id: row.left_slot_id.clone(),
                    right_slot_id: row.right_slot_id.clone(),
                    coverage: row.coverage,
                    common_active_receiver_count: row.active_in_both_count,
                    located_receiver_count: block_located,
                    location_unavailable_receiver_count: row
                        .active_in_both_count
                        .saturating_sub(block_located),
                    polar_cells: finish_polar_cells(block_polar, row.coverage),
                });
            }

            let located_unique = locations
                .values()
                .filter(|location| location.is_some())
                .count();
            ReportCommonOpportunityMapGroup {
                stratum: summary.stratum.clone(),
                coverage: summary.coverage,
                eligible_block_count: summary.eligible_block_count,
                known_coverage_block_count: summary.known_coverage_block_count,
                unique_common_active_receiver_count: summary.unique_active_receiver_count,
                receiver_block_opportunity_count: summary.receiver_block_opportunity_count,
                located_unique_receiver_count: located_unique,
                located_receiver_block_opportunity_count: located_opportunities,
                location_unavailable_unique_receiver_count: summary
                    .unique_active_receiver_count
                    .saturating_sub(located_unique),
                location_unavailable_receiver_block_opportunity_count: summary
                    .receiver_block_opportunity_count
                    .saturating_sub(located_opportunities),
                distance_cells: ReportDistanceBin::ALL
                    .into_iter()
                    .zip(distance)
                    .map(|(category, cell)| cell.finish(category, summary.coverage))
                    .collect(),
                azimuth_cells: ReportAzimuthSector::ALL
                    .into_iter()
                    .zip(azimuth)
                    .map(|(category, cell)| cell.finish(category, summary.coverage))
                    .collect(),
                polar_cells: finish_polar_cells(polar, summary.coverage),
                blocks,
            }
        })
        .collect()
}

pub(crate) fn overview_row_count(groups: &[ReportCommonOpportunityMapGroup]) -> usize {
    groups
        .iter()
        .map(|group| {
            1 + group.distance_cells.len() + group.azimuth_cells.len() + group.polar_cells.len()
        })
        .sum()
}

#[derive(Clone, Copy)]
struct ResolvedLocation {
    distance: ReportDistanceBin,
    azimuth_index: usize,
}

fn resolve_location(
    station: Option<crate::GeographicCoordinates>,
    grids: &[Option<String>],
) -> Option<ResolvedLocation> {
    let station = station?;
    let normalized = grids
        .iter()
        .map(|grid| {
            let grid = grid.as_deref()?.trim().to_ascii_uppercase();
            station_coordinates_from_grid(&grid).map(|_| grid)
        })
        .collect::<Option<BTreeSet<_>>>()?;
    if normalized.len() != 1 {
        return None;
    }
    let destination = station_coordinates_from_grid(normalized.first()?)?;
    let position = great_circle_position(station, destination)?;
    Some(ResolvedLocation {
        distance: ReportDistanceBin::classify(position.distance_km)?,
        azimuth_index: ((position.initial_bearing_degrees + 22.5) / 45.0).floor() as usize % 8,
    })
}

#[derive(Default)]
struct CellAccumulator {
    receivers: BTreeSet<String>,
    coverage: Option<ReporterActivityCoverage>,
    opportunities: usize,
    both: usize,
    left_only: usize,
    right_only: usize,
    neither: usize,
}

impl CellAccumulator {
    fn add(
        &mut self,
        receiver: &str,
        outcome: ReporterActivityJointOutcome,
        coverage: ReporterActivityCoverage,
    ) {
        self.receivers.insert(receiver.to_string());
        self.coverage = Some(
            self.coverage
                .map_or(coverage, |current| combine_coverage(current, coverage)),
        );
        self.opportunities += 1;
        match outcome {
            ReporterActivityJointOutcome::HeardBoth => self.both += 1,
            ReporterActivityJointOutcome::LeftOnly => self.left_only += 1,
            ReporterActivityJointOutcome::RightOnly => self.right_only += 1,
            ReporterActivityJointOutcome::HeardNeither => self.neither += 1,
        }
    }

    fn finish<T>(
        self,
        category: T,
        fallback_coverage: ReporterActivityCoverage,
    ) -> ReportCommonOpportunityCell<T> {
        let left_heard = self.both + self.left_only;
        let right_heard = self.both + self.right_only;
        ReportCommonOpportunityCell {
            category,
            coverage: self.coverage.unwrap_or(fallback_coverage),
            unique_common_active_receiver_count: self.receivers.len(),
            receiver_block_opportunity_count: self.opportunities,
            heard_both_count: self.both,
            left_only_count: self.left_only,
            right_only_count: self.right_only,
            heard_neither_count: self.neither,
            left_heard_count: left_heard,
            right_heard_count: right_heard,
            left_detection_rate: rate(left_heard, self.opportunities),
            right_detection_rate: rate(right_heard, self.opportunities),
        }
    }
}

fn finish_polar_cells(
    cells: BTreeMap<(usize, usize), CellAccumulator>,
    fallback_coverage: ReporterActivityCoverage,
) -> Vec<ReportCommonOpportunityPolarCell> {
    cells
        .into_iter()
        .map(|((azimuth, distance), cell)| {
            let distance_bin = ReportDistanceBin::ALL[distance];
            ReportCommonOpportunityPolarCell {
                bearing_sector: ReportAzimuthSector::ALL[azimuth],
                distance_bin,
                facts: cell.finish(distance_bin, fallback_coverage),
            }
        })
        .collect()
}

fn rate(heard: usize, opportunities: usize) -> Option<f64> {
    (opportunities > 0).then_some(heard as f64 / opportunities as f64)
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

#[cfg(test)]
mod tests;
