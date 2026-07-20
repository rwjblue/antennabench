use std::collections::{BTreeMap, BTreeSet};

use antennabench_analysis::{ComparisonSide, PairedComparisonAnalysis};

use crate::{
    ReportAzimuthSector, ReportDistanceBin, ReportObservedAntennaProfile,
    ReportObservedDistanceComposition, ReportObservedProfileCell, ReportOverviewObservedProfile,
};

pub(crate) fn project_observed_profile(
    stratum: &antennabench_analysis::ComparisonStratum,
    comparison: &PairedComparisonAnalysis,
) -> ReportOverviewObservedProfile {
    let left = comparison
        .observed_path_profiles
        .iter()
        .find(|profile| profile.stratum == *stratum && profile.side == ComparisonSide::Left);
    let right = comparison
        .observed_path_profiles
        .iter()
        .find(|profile| profile.stratum == *stratum && profile.side == ComparisonSide::Right);
    let mut composition =
        ReportDistanceBin::ALL.map(|category| ReportObservedDistanceComposition {
            category,
            left_only_unique_path_count: 0,
            shared_unique_path_count: 0,
            right_only_unique_path_count: 0,
        });
    let left_paths = left
        .into_iter()
        .flat_map(|profile| &profile.paths)
        .map(|path| (path.remote_path.as_str(), observed_path_distance_bin(path)))
        .collect::<BTreeMap<_, _>>();
    let right_paths = right
        .into_iter()
        .flat_map(|profile| &profile.paths)
        .map(|path| (path.remote_path.as_str(), observed_path_distance_bin(path)))
        .collect::<BTreeMap<_, _>>();
    let identities = left_paths
        .keys()
        .chain(right_paths.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut unavailable = 0;
    for identity in identities {
        match (left_paths.get(identity), right_paths.get(identity)) {
            (Some(Some(left)), None) => composition[left.index()].left_only_unique_path_count += 1,
            (None, Some(Some(right))) => {
                composition[right.index()].right_only_unique_path_count += 1;
            }
            (Some(Some(left)), Some(Some(right))) if left == right => {
                composition[left.index()].shared_unique_path_count += 1;
            }
            _ => unavailable += 1,
        }
    }
    ReportOverviewObservedProfile {
        left: left.map(project_observed_antenna_profile),
        right: right.map(project_observed_antenna_profile),
        distance_composition: composition.into_iter().collect(),
        composition_location_unavailable_count: unavailable,
    }
}

fn project_observed_antenna_profile(
    profile: &antennabench_analysis::ObservedAntennaPathProfile,
) -> ReportObservedAntennaProfile {
    let mut distance_bins = ReportDistanceBin::ALL.map(|category| ReportObservedProfileCell {
        category,
        unique_path_count: 0,
        observation_count: 0,
    });
    let mut azimuth_sectors = ReportAzimuthSector::ALL.map(|category| ReportObservedProfileCell {
        category,
        unique_path_count: 0,
        observation_count: 0,
    });
    for path in &profile.paths {
        let antennabench_analysis::ObservedPathLocation::Available {
            distance_km,
            initial_bearing_degrees,
            ..
        } = path.location
        else {
            continue;
        };
        let distance = ReportDistanceBin::classify(distance_km)
            .expect("an available observed path has a valid distance");
        let distance_cell = &mut distance_bins[distance.index()];
        distance_cell.unique_path_count += 1;
        distance_cell.observation_count += path.observation_count;
        let sector_index = azimuth_sector_index(initial_bearing_degrees);
        let sector_cell = &mut azimuth_sectors[sector_index];
        sector_cell.unique_path_count += 1;
        sector_cell.observation_count += path.observation_count;
    }
    ReportObservedAntennaProfile {
        side: profile.side,
        antenna_label: profile.antenna_label.clone(),
        unique_path_count: profile.unique_path_count,
        located_path_count: profile.located_path_count,
        missing_location_path_count: profile.missing_location_path_count,
        inconsistent_location_path_count: profile.inconsistent_location_path_count,
        distance_bins: distance_bins.into_iter().collect(),
        azimuth_sectors: azimuth_sectors.into_iter().collect(),
    }
}

fn observed_path_distance_bin(
    path: &antennabench_analysis::ObservedAntennaPath,
) -> Option<ReportDistanceBin> {
    match path.location {
        antennabench_analysis::ObservedPathLocation::Available { distance_km, .. } => {
            ReportDistanceBin::classify(distance_km)
        }
        antennabench_analysis::ObservedPathLocation::Missing
        | antennabench_analysis::ObservedPathLocation::Inconsistent => None,
    }
}

fn azimuth_sector_index(azimuth_degrees: f64) -> usize {
    ((azimuth_degrees + 22.5) / 45.0).floor() as usize % 8
}
