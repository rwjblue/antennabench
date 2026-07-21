use std::collections::BTreeSet;

use crate::SessionReport;
use antennabench_analysis::{
    ComparisonStratum, PairedObservationRow, SolarContextMissingReason, SolarEndpointRole,
    SolarLightState, SolarPositionResult,
};

use super::super::{geometry::geometry_class, presentation::*, shared::*};

fn location_available(row: &PairedObservationRow) -> bool {
    row_grid(row).is_some() && row_distance(row).is_some() && row_azimuth(row).is_some()
}

fn row_grid(row: &PairedObservationRow) -> Option<&str> {
    row.left_remote_grid
        .as_deref()
        .filter(|grid| !grid.is_empty())
        .or_else(|| {
            row.right_remote_grid
                .as_deref()
                .filter(|grid| !grid.is_empty())
        })
}

fn row_distance(row: &PairedObservationRow) -> Option<f64> {
    row.left_distance_km
        .filter(|value| value.is_finite() && *value >= 0.0)
        .or_else(|| {
            row.right_distance_km
                .filter(|value| value.is_finite() && *value >= 0.0)
        })
}

fn row_azimuth(row: &PairedObservationRow) -> Option<f64> {
    row.left_azimuth_degrees
        .filter(|value| value.is_finite())
        .or_else(|| row.right_azimuth_degrees.filter(|value| value.is_finite()))
        .map(|value| value.rem_euclid(360.0))
}

fn sector_index(azimuth: f64) -> usize {
    ((azimuth / 45.0).floor() as usize).min(7)
}

fn sector_label(index: usize) -> &'static str {
    match index {
        0 => "0°–<45°",
        1 => "45°–<90°",
        2 => "90°–<135°",
        3 => "135°–<180°",
        4 => "180°–<225°",
        5 => "225°–<270°",
        6 => "270°–<315°",
        _ => "315°–<360°",
    }
}

fn optional_text(value: Option<&str>) -> String {
    value.unwrap_or("Not available").to_string()
}

fn optional_measure(value: Option<f64>, unit: &str) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{} {unit}", format_number(value)))
        .unwrap_or_else(not_available)
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LocationRowView {
    pub(in crate::html) stratum: String,
    pub(in crate::html) remote_path: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) order: String,
    pub(in crate::html) left_snr: String,
    pub(in crate::html) right_snr: String,
    pub(in crate::html) delta: String,
    pub(in crate::html) left_grid: String,
    pub(in crate::html) right_grid: String,
    pub(in crate::html) left_distance: String,
    pub(in crate::html) right_distance: String,
    pub(in crate::html) left_azimuth: String,
    pub(in crate::html) right_azimuth: String,
    pub(in crate::html) sector: &'static str,
    pub(in crate::html) availability: &'static str,
    pub(in crate::html) left_time: String,
    pub(in crate::html) right_time: String,
    pub(in crate::html) distance: Option<String>,
    pub(in crate::html) distance_class: String,
    pub(in crate::html) azimuth: Option<String>,
    pub(in crate::html) azimuth_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LocationStratumView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) paired_rows: usize,
    pub(in crate::html) unique_paths: usize,
    pub(in crate::html) located_paths: usize,
    pub(in crate::html) unavailable_rows: usize,
    pub(in crate::html) populated_sector: String,
    pub(in crate::html) rows: Vec<LocationRowView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LocationViewsView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) strata: Vec<LocationStratumView>,
}

impl LocationViewsView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels {
            left: left_label,
            right: right_label,
        } = antenna_labels(report);
        let mut strata = Vec::<ComparisonStratum>::new();
        for row in &report.comparison.paired_rows {
            if !strata.contains(&row.stratum) {
                strata.push(row.stratum.clone());
            }
        }
        Self {
            left_label: left_label.clone(),
            right_label: right_label.clone(),
            strata: strata
                .iter()
                .enumerate()
                .map(|(index, group)| {
                    let source = report
                        .comparison
                        .paired_rows
                        .iter()
                        .filter(|row| row.stratum == *group)
                        .collect::<Vec<_>>();
                    let unique_paths = source
                        .iter()
                        .map(|row| row.remote_path.as_str())
                        .collect::<BTreeSet<_>>();
                    let located_paths = source
                        .iter()
                        .filter(|row| location_available(row))
                        .map(|row| row.remote_path.as_str())
                        .collect::<BTreeSet<_>>();
                    let unavailable_rows =
                        source.iter().filter(|row| !location_available(row)).count();
                    let mut sectors: [BTreeSet<&str>; 8] = std::array::from_fn(|_| BTreeSet::new());
                    for row in &source {
                        if location_available(row) {
                            if let Some(azimuth) = row_azimuth(row) {
                                sectors[sector_index(azimuth)].insert(row.remote_path.as_str());
                            }
                        }
                    }
                    let (best_sector, sector_count) = sectors
                        .iter()
                        .enumerate()
                        .max_by_key(|(index, paths)| (paths.len(), std::cmp::Reverse(*index)))
                        .map(|(index, paths)| (index, paths.len()))
                        .unwrap_or((0, 0));
                    let maximum = source
                        .iter()
                        .filter_map(|row| row_distance(row))
                        .fold(1.0_f64, f64::max);
                    LocationStratumView {
                        index,
                        label: comparison_group_label(group),
                        paired_rows: source.len(),
                        unique_paths: unique_paths.len(),
                        located_paths: located_paths.len(),
                        unavailable_rows,
                        populated_sector: format!(
                            "{}: {} of {} located paths",
                            sector_label(best_sector),
                            sector_count,
                            located_paths.len()
                        ),
                        rows: source
                            .into_iter()
                            .map(|row| {
                                let available = location_available(row);
                                let distance = row_distance(row).filter(|_| available);
                                let azimuth = row_azimuth(row).filter(|_| available);
                                LocationRowView {
                                    stratum: comparison_group_label(&row.stratum),
                                    remote_path: row.remote_path.clone(),
                                    block: row.block_index + 1,
                                    order: labeled_comparison_order(
                                        row.order,
                                        &left_label,
                                        &right_label,
                                    ),
                                    left_snr: format_number(row.left_snr_db),
                                    right_snr: format_number(row.right_snr_db),
                                    delta: format_signed(row.delta_right_minus_left_db),
                                    left_grid: optional_text(row.left_remote_grid.as_deref()),
                                    right_grid: optional_text(row.right_remote_grid.as_deref()),
                                    left_distance: optional_measure(row.left_distance_km, "km"),
                                    right_distance: optional_measure(row.right_distance_km, "km"),
                                    left_azimuth: optional_measure(row.left_azimuth_degrees, "°"),
                                    right_azimuth: optional_measure(row.right_azimuth_degrees, "°"),
                                    sector: azimuth
                                        .map(|value| sector_label(sector_index(value)))
                                        .unwrap_or("Location unavailable"),
                                    availability: if available {
                                        "Available"
                                    } else {
                                        "Location unavailable"
                                    },
                                    left_time: timestamp(row.left_timestamp),
                                    right_time: timestamp(row.right_timestamp),
                                    distance: distance.map(format_number),
                                    distance_class: distance
                                        .map(|value| geometry_class(value / maximum * 100.0))
                                        .unwrap_or_default(),
                                    azimuth: azimuth.map(format_number),
                                    azimuth_class: azimuth
                                        .map(|value| geometry_class(value / 360.0 * 100.0))
                                        .unwrap_or_default(),
                                }
                            })
                            .collect(),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SolarRowView {
    pub(in crate::html) stratum: String,
    pub(in crate::html) remote_path: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) antenna: String,
    pub(in crate::html) observation: String,
    pub(in crate::html) time: String,
    pub(in crate::html) endpoint_role: &'static str,
    pub(in crate::html) endpoint_id: String,
    pub(in crate::html) grid: String,
    pub(in crate::html) coordinates: String,
    pub(in crate::html) elevation: String,
    pub(in crate::html) state: &'static str,
    pub(in crate::html) gray_line: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SolarContextView {
    pub(in crate::html) algorithm_id: String,
    pub(in crate::html) algorithm_version: u16,
    pub(in crate::html) coordinate_method: String,
    pub(in crate::html) rows: Vec<SolarRowView>,
}

impl SolarContextView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels { left, right } = antenna_labels(report);
        let mut rows = Vec::new();
        for row in &report.solar_context.rows {
            for (side, observation) in [(&left, &row.left), (&right, &row.right)] {
                for endpoint in [&observation.station, &observation.remote] {
                    let role = match endpoint.role {
                        SolarEndpointRole::Station => "Station",
                        SolarEndpointRole::Remote => "Remote",
                    };
                    let (coordinates, elevation, state, gray_line) = match &endpoint.result {
                        SolarPositionResult::Available {
                            coordinates,
                            elevation_degrees,
                            light_state,
                            gray_line,
                        } => (
                            format!(
                                "{:.6}°, {:.6}°",
                                coordinates.latitude_degrees, coordinates.longitude_degrees
                            ),
                            format!("{:.3}°", elevation_degrees),
                            solar_state(*light_state),
                            yes_no(*gray_line),
                        ),
                        SolarPositionResult::Missing { reason } => (
                            "Unavailable".into(),
                            "Unavailable".into(),
                            match reason {
                                SolarContextMissingReason::MissingGrid => "Missing grid",
                                SolarContextMissingReason::InvalidGrid => "Invalid grid",
                            },
                            "Unavailable",
                        ),
                    };
                    rows.push(SolarRowView {
                        stratum: comparison_group_label(&row.stratum),
                        remote_path: row.remote_path.clone(),
                        block: row.block_index + 1,
                        antenna: side.clone(),
                        observation: observation.observation_id.clone(),
                        time: timestamp(observation.timestamp),
                        endpoint_role: role,
                        endpoint_id: endpoint.endpoint_id.clone(),
                        grid: optional_text(endpoint.grid.as_deref()),
                        coordinates,
                        elevation,
                        state,
                        gray_line,
                    });
                }
            }
        }
        Self {
            algorithm_id: report.solar_context.algorithm.algorithm_id.clone(),
            algorithm_version: report.solar_context.algorithm.algorithm_version,
            coordinate_method: report.solar_context.algorithm.coordinate_method.clone(),
            rows,
        }
    }
}

fn solar_state(state: SolarLightState) -> &'static str {
    match state {
        SolarLightState::Daylight => "Daylight",
        SolarLightState::CivilTwilight => "Civil twilight",
        SolarLightState::NauticalTwilight => "Nautical twilight",
        SolarLightState::AstronomicalTwilight => "Astronomical twilight",
        SolarLightState::Night => "Night",
    }
}
