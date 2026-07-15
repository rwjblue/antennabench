use antennabench_core::BundleContents;
use chrono::{DateTime, Datelike, Timelike, Utc};

use crate::{
    AnalysisBudget, AnalysisError, AnalysisResourceStage, PairedComparisonAnalysis,
    PairedObservationRow, SolarContextAlgorithm, SolarContextAnalysis, SolarContextMissingReason,
    SolarContextRow, SolarCoordinates, SolarEndpointContext, SolarEndpointRole, SolarLightState,
    SolarObservationContext, SolarPositionResult,
};

const ALGORITHM_ID: &str = "noaa-gml-fractional-year";
const ALGORITHM_VERSION: u16 = 1;
const COORDINATE_METHOD: &str = "maidenhead-cell-center-v1";

pub(crate) fn derive_solar_context(
    bundle: &BundleContents,
    comparison: &PairedComparisonAnalysis,
    budget: &AnalysisBudget<'_>,
) -> Result<SolarContextAnalysis, AnalysisError> {
    budget.collection(
        AnalysisResourceStage::Compare,
        "solar_context_rows",
        comparison.paired_rows.len(),
    )?;
    let mut rows = Vec::with_capacity(comparison.paired_rows.len());
    for (index, paired) in comparison.paired_rows.iter().enumerate() {
        budget.checkpoint(
            AnalysisResourceStage::Compare,
            "derive_solar_context",
            index,
        )?;
        rows.push(derive_row(bundle, paired));
    }
    Ok(SolarContextAnalysis {
        algorithm: SolarContextAlgorithm {
            algorithm_id: ALGORITHM_ID.into(),
            algorithm_version: ALGORITHM_VERSION,
            coordinate_method: COORDINATE_METHOD.into(),
        },
        rows,
    })
}

fn derive_row(bundle: &BundleContents, row: &PairedObservationRow) -> SolarContextRow {
    SolarContextRow {
        stratum: row.stratum.clone(),
        block_index: row.block_index,
        order: row.order,
        remote_path: row.remote_path.clone(),
        left: observation_context(
            bundle,
            &row.remote_path,
            &row.left_observation_id,
            row.left_timestamp,
            row.left_remote_grid.as_deref(),
        ),
        right: observation_context(
            bundle,
            &row.remote_path,
            &row.right_observation_id,
            row.right_timestamp,
            row.right_remote_grid.as_deref(),
        ),
    }
}

fn observation_context(
    bundle: &BundleContents,
    remote_path: &str,
    observation_id: &str,
    timestamp: DateTime<Utc>,
    remote_grid: Option<&str>,
) -> SolarObservationContext {
    SolarObservationContext {
        observation_id: observation_id.into(),
        timestamp,
        station: endpoint_context(
            SolarEndpointRole::Station,
            &bundle.station.callsign,
            Some(&bundle.station.grid),
            timestamp,
        ),
        remote: endpoint_context(
            SolarEndpointRole::Remote,
            remote_path,
            remote_grid,
            timestamp,
        ),
    }
}

fn endpoint_context(
    role: SolarEndpointRole,
    endpoint_id: &str,
    grid: Option<&str>,
    timestamp: DateTime<Utc>,
) -> SolarEndpointContext {
    let normalized_grid = grid.map(str::trim).filter(|grid| !grid.is_empty());
    let result = match normalized_grid {
        None => SolarPositionResult::Missing {
            reason: SolarContextMissingReason::MissingGrid,
        },
        Some(grid) => match maidenhead_cell_center(grid) {
            Some(coordinates) => {
                let elevation_degrees = solar_elevation(timestamp, coordinates)
                    .expect("validated Maidenhead coordinates are finite and in range");
                let light_state = classify_light(elevation_degrees);
                SolarPositionResult::Available {
                    coordinates,
                    elevation_degrees,
                    light_state,
                    gray_line: matches!(
                        light_state,
                        SolarLightState::CivilTwilight
                            | SolarLightState::NauticalTwilight
                            | SolarLightState::AstronomicalTwilight
                    ),
                }
            }
            None => SolarPositionResult::Missing {
                reason: SolarContextMissingReason::InvalidGrid,
            },
        },
    };
    SolarEndpointContext {
        role,
        endpoint_id: endpoint_id.into(),
        grid: normalized_grid.map(str::to_ascii_uppercase),
        result,
    }
}

fn classify_light(elevation_degrees: f64) -> SolarLightState {
    if elevation_degrees >= 0.0 {
        SolarLightState::Daylight
    } else if elevation_degrees >= -6.0 {
        SolarLightState::CivilTwilight
    } else if elevation_degrees >= -12.0 {
        SolarLightState::NauticalTwilight
    } else if elevation_degrees >= -18.0 {
        SolarLightState::AstronomicalTwilight
    } else {
        SolarLightState::Night
    }
}

fn solar_elevation(timestamp: DateTime<Utc>, coordinates: SolarCoordinates) -> Option<f64> {
    if !coordinates.latitude_degrees.is_finite()
        || !(-90.0..=90.0).contains(&coordinates.latitude_degrees)
        || !coordinates.longitude_degrees.is_finite()
        || !(-180.0..=180.0).contains(&coordinates.longitude_degrees)
    {
        return None;
    }
    let days = f64::from(timestamp.ordinal());
    let hour = f64::from(timestamp.hour())
        + f64::from(timestamp.minute()) / 60.0
        + f64::from(timestamp.second()) / 3_600.0
        + f64::from(timestamp.nanosecond()) / 3_600_000_000_000.0;
    let year = timestamp.year();
    let year_days =
        if year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0) {
            366.0
        } else {
            365.0
        };
    let gamma = std::f64::consts::TAU / year_days * (days - 1.0 + (hour - 12.0) / 24.0);
    let equation_of_time = 229.18
        * (0.000_075 + 0.001_868 * gamma.cos()
            - 0.032_077 * gamma.sin()
            - 0.014_615 * (2.0 * gamma).cos()
            - 0.040_849 * (2.0 * gamma).sin());
    let declination = 0.006_918 - 0.399_912 * gamma.cos() + 0.070_257 * gamma.sin()
        - 0.006_758 * (2.0 * gamma).cos()
        + 0.000_907 * (2.0 * gamma).sin()
        - 0.002_697 * (3.0 * gamma).cos()
        + 0.001_48 * (3.0 * gamma).sin();
    let true_solar_minutes =
        (hour * 60.0 + equation_of_time + 4.0 * coordinates.longitude_degrees).rem_euclid(1_440.0);
    let hour_angle = (true_solar_minutes / 4.0 - 180.0).to_radians();
    let latitude = coordinates.latitude_degrees.to_radians();
    let cosine_zenith = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .clamp(-1.0, 1.0);
    Some(90.0 - cosine_zenith.acos().to_degrees())
}

fn maidenhead_cell_center(grid: &str) -> Option<SolarCoordinates> {
    let grid = grid.trim().as_bytes();
    if !matches!(grid.len(), 4 | 6 | 8) {
        return None;
    }
    let field_lon = ascii_index(grid[0], b'A', b'R')?;
    let field_lat = ascii_index(grid[1], b'A', b'R')?;
    let square_lon = ascii_index(grid[2], b'0', b'9')?;
    let square_lat = ascii_index(grid[3], b'0', b'9')?;
    let mut longitude = -180.0 + f64::from(field_lon) * 20.0 + f64::from(square_lon) * 2.0;
    let mut latitude = -90.0 + f64::from(field_lat) * 10.0 + f64::from(square_lat);
    let (width, height) = match grid.len() {
        4 => (2.0, 1.0),
        6 => {
            longitude += f64::from(ascii_index(grid[4], b'A', b'X')?) * 5.0 / 60.0;
            latitude += f64::from(ascii_index(grid[5], b'A', b'X')?) * 2.5 / 60.0;
            (5.0 / 60.0, 2.5 / 60.0)
        }
        8 => {
            longitude += f64::from(ascii_index(grid[4], b'A', b'X')?) * 5.0 / 60.0;
            latitude += f64::from(ascii_index(grid[5], b'A', b'X')?) * 2.5 / 60.0;
            longitude += f64::from(ascii_index(grid[6], b'0', b'9')?) / 120.0;
            latitude += f64::from(ascii_index(grid[7], b'0', b'9')?) / 240.0;
            (1.0 / 120.0, 1.0 / 240.0)
        }
        _ => unreachable!(),
    };
    Some(SolarCoordinates {
        latitude_degrees: latitude + height / 2.0,
        longitude_degrees: longitude + width / 2.0,
    })
}

fn ascii_index(value: u8, minimum: u8, maximum: u8) -> Option<u8> {
    let value = value.to_ascii_uppercase();
    (minimum..=maximum)
        .contains(&value)
        .then_some(value - minimum)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn matches_noaa_gml_uncorrected_fixture_within_documented_bound() {
        // NOAA GML Solar Geometry Calculator: 40 N, 105 W, 2024-06-20 12:00 UTC
        // reports 3.93770 degrees uncorrected elevation. This lower-order NOAA
        // formula is required to remain within 0.5 degree of that independent result.
        let timestamp = Utc.with_ymd_and_hms(2024, 6, 20, 12, 0, 0).unwrap();
        let actual = solar_elevation(
            timestamp,
            SolarCoordinates {
                latitude_degrees: 40.0,
                longitude_degrees: -105.0,
            },
        )
        .unwrap();
        assert!((actual - 3.93770).abs() <= 0.5, "actual={actual}");
    }

    #[test]
    fn classifies_exact_twilight_thresholds() {
        assert_eq!(classify_light(0.0), SolarLightState::Daylight);
        assert_eq!(classify_light(-6.0), SolarLightState::CivilTwilight);
        assert_eq!(classify_light(-12.0), SolarLightState::NauticalTwilight);
        assert_eq!(classify_light(-18.0), SolarLightState::AstronomicalTwilight);
        assert_eq!(classify_light(-18.000_001), SolarLightState::Night);
    }

    #[test]
    fn resolves_maidenhead_cell_centers_and_rejects_adversarial_values() {
        assert_eq!(
            maidenhead_cell_center("FN31"),
            Some(SolarCoordinates {
                latitude_degrees: 41.5,
                longitude_degrees: -73.0,
            })
        );
        assert!(maidenhead_cell_center("FN31pr").is_some());
        assert!(maidenhead_cell_center("FN31pr42").is_some());
        for invalid in ["", "FN", "SN31", "FN3X", "FN31zz", "FN31pr4X", "💥"] {
            assert_eq!(maidenhead_cell_center(invalid), None, "grid={invalid}");
        }
    }

    #[test]
    fn handles_utc_rollover_and_polar_day_and_night() {
        let north = SolarCoordinates {
            latitude_degrees: 89.0,
            longitude_degrees: 0.0,
        };
        let summer = Utc.with_ymd_and_hms(2024, 6, 21, 23, 59, 59).unwrap();
        let winter = Utc.with_ymd_and_hms(2024, 12, 21, 0, 0, 1).unwrap();
        assert!(solar_elevation(summer, north).unwrap() > 0.0);
        assert!(solar_elevation(winter, north).unwrap() < -18.0);
        assert!(
            solar_elevation(summer + chrono::Duration::seconds(2), north)
                .unwrap()
                .is_finite()
        );
    }

    #[test]
    fn rejects_non_finite_and_out_of_range_coordinates() {
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        for coordinates in [
            SolarCoordinates {
                latitude_degrees: f64::NAN,
                longitude_degrees: 0.0,
            },
            SolarCoordinates {
                latitude_degrees: 91.0,
                longitude_degrees: 0.0,
            },
            SolarCoordinates {
                latitude_degrees: 0.0,
                longitude_degrees: 181.0,
            },
        ] {
            assert_eq!(solar_elevation(timestamp, coordinates), None);
        }
    }
}
