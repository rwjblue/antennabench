use std::path::PathBuf;

use antennabench_analysis::{ComparisonAvailability, PathDirection};
use antennabench_core::{
    normalize_bundle, validate_bundle_report, Band, BundleContents, ExperimentMode,
    ObservationRecord, RecordSource,
};
use antennabench_report::{
    build_report, build_report_with_resources, ReportAzimuthSector, ReportCancellationToken,
    ReportDistanceBin, ReportError, ReportOverviewLifecycleState, ReportOverviewLimitation,
    ReportOverviewPathDelta, ReportPathLocationAvailability, ReportResourceLimits, SessionReport,
};
use antennabench_storage::BundleStore;
use chrono::Duration;

#[test]
fn inconclusive_overview_is_explicit_serializable_and_has_no_invented_comparison() {
    let report = build_report(&inconclusive_bundle()).expect("inconclusive report should build");
    let overview = &report.overview;

    assert_eq!(overview.scope.session_id, report.context.session_id);
    assert_eq!(overview.scope.station, report.context.station);
    assert_eq!(overview.scope.goal, Some(report.context.goal));
    assert_eq!(
        overview.scope.experiment_mode,
        Some(report.context.experiment_mode)
    );
    assert_eq!(overview.scope.bands, report.context.bands);
    assert_eq!(
        overview.scope.antenna_labels,
        vec!["Vertical", "Inverted V"]
    );
    assert_eq!(
        overview.lifecycle.state,
        ReportOverviewLifecycleState::NotRecorded
    );
    assert_eq!(
        overview.comparison_availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert!(!overview.strata.is_empty());
    assert!(overview
        .strata
        .iter()
        .all(|row| matches!(row.path_delta, ReportOverviewPathDelta::Unavailable)));
    assert!(overview
        .limitations
        .contains(&ReportOverviewLimitation::NoMatchedPaths));

    let serialized = serde_json::to_value(&report).expect("overview should serialize");
    assert_eq!(
        serialized["overview"]["comparison_availability"],
        "no_matched_paths"
    );
    let decoded: SessionReport =
        serde_json::from_value(serialized).expect("overview should deserialize");
    assert_eq!(decoded, report);
}

#[test]
fn paired_overview_preserves_each_stratum_and_uses_path_medians() {
    let report = build_report(&paired_bundle()).expect("paired report should build");
    let overview = &report.overview;

    assert_eq!(
        overview.comparison_availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
    assert_eq!(
        overview.scope.delta_orientation,
        report.comparison.delta_orientation
    );
    assert_eq!(
        overview.scope.observed_directions,
        vec![PathDirection::Transmit]
    );
    assert!(overview.limitations.is_empty());
    assert_eq!(overview.strata.len(), 1);

    let row = &overview.strata[0];
    assert_eq!(row.stratum.direction, PathDirection::Transmit);
    assert_eq!(row.stratum.band, Band::M20);
    assert_eq!(row.stratum.mode.as_str(), "WSPR");
    assert_eq!(row.paired_row_count, 4);
    assert_eq!(row.unique_path_count, 2);
    assert_eq!(row.contributing_block_count, 3);
    assert_eq!(row.left_then_right_block_count, 2);
    assert_eq!(row.right_then_left_block_count, 1);
    assert_eq!(
        row.path_delta,
        ReportOverviewPathDelta::Available {
            minimum_delta_right_minus_left_db: -4.0,
            median_path_delta_right_minus_left_db: 4.0,
            maximum_delta_right_minus_left_db: 12.0,
        },
        "the +12 dB prolific path and -4 dB sparse path have equal path weight"
    );
    assert_eq!(
        row.path_median_deltas,
        vec![
            antennabench_report::ReportOverviewPathMedianDelta {
                remote_path: "K1PROLIFIC".into(),
                paired_row_count: 3,
                median_delta_right_minus_left_db: 12.0,
            },
            antennabench_report::ReportOverviewPathMedianDelta {
                remote_path: "K2SPARSE".into(),
                paired_row_count: 1,
                median_delta_right_minus_left_db: -4.0,
            },
        ]
    );
    assert_eq!(row.reach.left_only_unique_path_count, 0);
    assert_eq!(row.reach.both_unique_path_count, 2);
    assert_eq!(row.reach.right_only_unique_path_count, 0);
}

#[test]
fn overview_reach_keeps_unmatched_paths_and_missing_snr_distinct_from_zero_delta() {
    let mut bundle = bundle_with_layout(&["A", "B"]);
    bundle.observations = vec![
        tx_observation(&bundle, "zero-left", 0, "K1ZERO", Some(-20.0)),
        tx_observation(&bundle, "zero-right", 1, "K1ZERO", Some(-20.0)),
        tx_observation(&bundle, "left-only", 0, "K1LEFT", Some(-20.0)),
        tx_observation(&bundle, "right-only", 1, "K1RIGHT", Some(-20.0)),
        tx_observation(&bundle, "missing-left", 0, "K1MISSING", None),
        tx_observation(&bundle, "finite-right", 1, "K1MISSING", Some(-20.0)),
    ];
    let report = build_report(&normalize_bundle(bundle)).unwrap();
    let row = &report.overview.strata[0];

    assert_eq!(row.path_median_deltas.len(), 1);
    assert_eq!(row.path_median_deltas[0].remote_path, "K1ZERO");
    assert_eq!(
        row.path_median_deltas[0].median_delta_right_minus_left_db,
        0.0
    );
    assert_eq!(row.reach.left_only_unique_path_count, 1);
    assert_eq!(row.reach.both_unique_path_count, 1);
    assert_eq!(row.reach.right_only_unique_path_count, 2);
    assert_eq!(row.missing_snr_left_count, 1);
}

#[test]
fn overview_models_single_antenna_and_unavailable_comparison_shapes() {
    let mut single = bundle_with_layout(&["A"]);
    single.schedule.mode = ExperimentMode::SingleAntennaProfiling;
    assert_overview_unavailable(
        single,
        ComparisonAvailability::NotApplicable,
        ReportOverviewLimitation::ComparisonNotApplicable,
    );

    assert_overview_unavailable(
        bundle_with_layout(&["A", "A"]),
        ComparisonAvailability::UnsupportedComparisonShape,
        ReportOverviewLimitation::UnsupportedComparisonShape,
    );

    assert_overview_unavailable(
        bundle_with_layout(&["A", "A", "B"]),
        ComparisonAvailability::NoEligibleBlocks,
        ReportOverviewLimitation::NoEligibleBlocks,
    );

    let mut unmatched = bundle_with_layout(&["A", "B"]);
    unmatched.observations = vec![
        tx_observation(&unmatched, "left", 0, "K1LEFT", Some(-20.0)),
        tx_observation(&unmatched, "right", 1, "K1RIGHT", Some(-18.0)),
    ];
    assert_overview_unavailable(
        unmatched,
        ComparisonAvailability::NoMatchedPaths,
        ReportOverviewLimitation::NoMatchedPaths,
    );
}

#[test]
fn overview_is_independent_of_observation_order_and_keeps_missing_snr_typed() {
    let mut forward = paired_bundle();
    forward.observations.push(tx_observation(
        &forward,
        "missing-left",
        0,
        "K1MISSING",
        None,
    ));
    forward.observations.push(tx_observation(
        &forward,
        "finite-right",
        1,
        "K1MISSING",
        Some(-19.0),
    ));
    let reordered = {
        let mut reordered = forward.clone();
        reordered.observations.reverse();
        reordered
    };

    let forward = build_report(&normalize_bundle(forward)).expect("forward report should build");
    let reordered =
        build_report(&normalize_bundle(reordered)).expect("reordered report should build");

    assert_eq!(forward.overview, reordered.overview);
    assert!(forward
        .overview
        .limitations
        .contains(&ReportOverviewLimitation::MissingSnr {
            left_count: 1,
            right_count: 0,
        }));
    assert!(forward
        .overview
        .strata
        .iter()
        .all(|row| !matches!(row.path_delta, ReportOverviewPathDelta::Unavailable)));
}

#[test]
fn projects_fixed_distance_and_azimuth_context_once_per_paired_path() {
    let bundle = location_context_bundle();
    let report = build_report(&bundle).expect("location context should build");
    let context = &report.overview.strata[0].location_context;

    assert_eq!(
        context
            .distance_bins
            .iter()
            .map(|cell| cell.category)
            .collect::<Vec<_>>(),
        ReportDistanceBin::ALL
    );
    assert_eq!(
        context
            .distance_bins
            .iter()
            .map(|cell| (cell.unique_located_path_count, cell.paired_row_count))
            .collect::<Vec<_>>(),
        vec![(1, 1), (7, 8), (2, 2), (1, 1)],
        "500, 1500, and 3000 km belong to their upper bins; prolific rows count once"
    );
    assert_eq!(
        context
            .azimuth_sectors
            .iter()
            .map(|cell| cell.category)
            .collect::<Vec<_>>(),
        ReportAzimuthSector::ALL
    );
    assert_eq!(
        context
            .azimuth_sectors
            .iter()
            .map(|cell| (cell.unique_located_path_count, cell.paired_row_count))
            .collect::<Vec<_>>(),
        vec![
            (3, 3),
            (2, 3),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
        ],
        "every valid 45° boundary has a stable upper-sector assignment and values just below 360° wrap to north"
    );
    assert_eq!(
        context.azimuth_sectors[0].median_path_delta_right_minus_left_db,
        Some(0.0),
        "a true zero delta remains a populated cell rather than no evidence"
    );
    assert_eq!(context.missing_location_path_count, 1);
    assert_eq!(context.inconsistent_location_path_count, 1);
    assert!(context.paths.iter().any(|path| {
        path.remote_path == "K1MISSING"
            && path.availability == ReportPathLocationAvailability::Missing
    }));

    let observed = &report.overview.strata[0].observed_profile;
    let left = observed.left.as_ref().unwrap();
    let right = observed.right.as_ref().unwrap();
    assert_eq!((left.unique_path_count, right.unique_path_count), (13, 13));
    assert_eq!(
        (left.located_path_count, right.located_path_count),
        (12, 12)
    );
    assert_eq!(
        left.distance_bins
            .iter()
            .map(|cell| (cell.unique_path_count, cell.observation_count))
            .collect::<Vec<_>>(),
        vec![(1, 1), (8, 9), (2, 2), (1, 1)]
    );
    assert_eq!(
        observed
            .distance_composition
            .iter()
            .map(|cell| (
                cell.left_only_unique_path_count,
                cell.shared_unique_path_count,
                cell.right_only_unique_path_count,
            ))
            .collect::<Vec<_>>(),
        vec![(0, 1, 0), (0, 8, 0), (0, 2, 0), (0, 1, 0)]
    );
    assert_eq!(observed.composition_location_unavailable_count, 1);
    assert!(context.paths.iter().any(|path| {
        path.remote_path == "K1INCONSISTENT"
            && path.availability == ReportPathLocationAvailability::Inconsistent
    }));

    let mut reordered = bundle;
    reordered.observations.reverse();
    assert_eq!(
        report,
        build_report(&reordered).expect("input ordering must not change location context")
    );
}

#[test]
fn all_path_profiles_survive_disjoint_populations_missing_snr_and_bad_locations() {
    let mut bundle = all_path_profile_bundle();
    let report = build_report(&bundle).expect("all-path profile should build");

    assert_eq!(
        report.overview.comparison_availability,
        ComparisonAvailability::NoMatchedPaths
    );
    let observed = &report.overview.strata[0].observed_profile;
    let left = observed.left.as_ref().unwrap();
    let right = observed.right.as_ref().unwrap();
    assert_eq!((left.unique_path_count, right.unique_path_count), (4, 1));
    assert_eq!((left.located_path_count, right.located_path_count), (2, 1));
    assert_eq!(left.missing_location_path_count, 1);
    assert_eq!(left.inconsistent_location_path_count, 1);
    assert_eq!(left.distance_bins[0].unique_path_count, 2);
    assert_eq!(left.distance_bins[0].observation_count, 3);
    assert_eq!(right.distance_bins[3].unique_path_count, 1);
    assert_eq!(right.distance_bins[3].observation_count, 4);
    let left_rows = &report.comparison.observed_path_profiles[0].paths;
    let near = left_rows
        .iter()
        .find(|path| path.remote_path == "K1NEAR")
        .unwrap();
    assert_eq!(near.block_support_count, 2);
    assert_eq!(near.slot_support_count, 2);
    assert_eq!(near.observation_count, 2);
    assert_eq!(near.snr.unwrap().sample_count, 2);
    assert!(left_rows
        .iter()
        .find(|path| path.remote_path == "K1NOSNR")
        .unwrap()
        .snr
        .is_none());
    assert!(matches!(
        left_rows
            .iter()
            .find(|path| path.remote_path == "K1INCONSISTENT")
            .unwrap()
            .location,
        antennabench_analysis::ObservedPathLocation::Inconsistent
    ));

    bundle.observations.reverse();
    assert_eq!(
        report,
        build_report(&bundle).expect("profile must be input-order independent")
    );
}

#[test]
fn all_path_distance_composition_separates_matched_and_unmatched_paths_in_one_bin() {
    let mut bundle = all_path_profile_bundle();
    bundle.observations.extend([
        located_tx_observation(
            &bundle,
            "near-shared-right-1",
            1,
            "K1NEAR",
            -19.0,
            200.0,
            10.0,
        ),
        located_tx_observation(
            &bundle,
            "near-shared-right-2",
            3,
            "K1NEAR",
            -17.0,
            200.0,
            10.0,
        ),
        located_tx_observation(&bundle, "near-left-only", 0, "K1AONLY", -23.0, 250.0, 15.0),
        located_tx_observation(&bundle, "near-right-only", 1, "K1BONLY", -24.0, 300.0, 20.0),
    ]);
    let report = build_report(&normalize_bundle(bundle)).expect("mixed path profile should build");

    assert_eq!(
        report.overview.comparison_availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
    let near = &report.overview.strata[0]
        .observed_profile
        .distance_composition[0];
    assert_eq!(near.left_only_unique_path_count, 2);
    assert_eq!(near.shared_unique_path_count, 1);
    assert_eq!(near.right_only_unique_path_count, 1);
}

#[test]
fn overview_rows_are_part_of_the_required_bounded_projection() {
    let bundle = paired_bundle();
    let validation = validate_bundle_report(&bundle);
    let limits = ReportResourceLimits::testing(0, 8 * 1024 * 1024, 16 * 1024 * 1024);

    let error = build_report_with_resources(
        &bundle,
        &validation,
        limits,
        &ReportCancellationToken::default(),
    )
    .expect_err("a retained overview stratum must be bounded");
    assert!(matches!(
        error,
        ReportError::Resource(ref error)
            if error.diagnostic.code == "resource.report.rows"
                && error.diagnostic.role == "required_overview_rows"
                && error.diagnostic.observed == Some(61)
    ));
}

fn assert_overview_unavailable(
    bundle: BundleContents,
    availability: ComparisonAvailability,
    limitation: ReportOverviewLimitation,
) {
    let report = build_report(&normalize_bundle(bundle)).expect("report should build");
    assert_eq!(report.overview.comparison_availability, availability);
    if availability == ComparisonAvailability::NoMatchedPaths {
        assert!(!report.overview.strata.is_empty());
        assert!(report
            .overview
            .strata
            .iter()
            .all(|row| matches!(row.path_delta, ReportOverviewPathDelta::Unavailable)));
    } else {
        assert!(report.overview.strata.is_empty());
    }
    assert!(report.overview.limitations.contains(&limitation));
}

fn paired_bundle() -> BundleContents {
    let mut bundle = bundle_with_layout(&["A", "B", "B", "A", "A", "B"]);
    bundle.observations = vec![
        tx_observation(&bundle, "p1-l1", 0, "K1PROLIFIC", Some(-30.0)),
        tx_observation(&bundle, "p1-r1", 1, "K1PROLIFIC", Some(-18.0)),
        tx_observation(&bundle, "p1-r2", 2, "K1PROLIFIC", Some(-17.0)),
        tx_observation(&bundle, "p1-l2", 3, "K1PROLIFIC", Some(-29.0)),
        tx_observation(&bundle, "p1-l3", 4, "K1PROLIFIC", Some(-28.0)),
        tx_observation(&bundle, "p1-r3", 5, "K1PROLIFIC", Some(-16.0)),
        tx_observation(&bundle, "p2-l", 4, "K2SPARSE", Some(-20.0)),
        tx_observation(&bundle, "p2-r", 5, "K2SPARSE", Some(-24.0)),
    ];
    normalize_bundle(bundle)
}

fn location_context_bundle() -> BundleContents {
    let block_count = 14;
    let labels = (0..block_count)
        .flat_map(|_| ["A", "B"])
        .collect::<Vec<_>>();
    let mut bundle = bundle_with_layout(&labels);
    let samples = [
        ("K1BOUND00", 499.999, 0.0, 0.0),
        ("K1BOUND01", 500.0, 22.5, 2.0),
        ("K1BOUND02", 1499.999, 67.5, 2.0),
        ("K1BOUND03", 1500.0, 112.5, 2.0),
        ("K1BOUND04", 2999.999, 157.5, 2.0),
        ("K1BOUND05", 3000.0, 202.5, 2.0),
        ("K1BOUND06", 500.0, 247.5, 2.0),
        ("K1BOUND07", 500.0, 292.5, 2.0),
        ("K1BOUND08", 500.0, 337.5, 2.0),
        ("K1BOUND09", 500.0, 359.999, 0.0),
        ("K1PROLIFIC", 500.0, 22.5, 0.0),
        ("K1PROLIFIC", 500.0, 22.5, 0.0),
    ];
    let mut observations = Vec::new();
    for (block, (remote, distance_km, azimuth_degrees, delta)) in samples.iter().enumerate() {
        observations.push(located_tx_observation(
            &bundle,
            &format!("left-{block}"),
            block * 2,
            remote,
            -20.0,
            *distance_km,
            *azimuth_degrees,
        ));
        observations.push(located_tx_observation(
            &bundle,
            &format!("right-{block}"),
            block * 2 + 1,
            remote,
            -20.0 + *delta,
            *distance_km,
            *azimuth_degrees,
        ));
    }
    let missing_block = samples.len();
    let mut missing_left = tx_observation(
        &bundle,
        "missing-left",
        missing_block * 2,
        "K1MISSING",
        Some(-20.0),
    );
    let mut missing_right = tx_observation(
        &bundle,
        "missing-right",
        missing_block * 2 + 1,
        "K1MISSING",
        Some(-18.0),
    );
    missing_left.distance_km = None;
    missing_left.azimuth_degrees = None;
    missing_right.distance_km = None;
    missing_right.azimuth_degrees = None;
    observations.extend([missing_left, missing_right]);

    let inconsistent_block = missing_block + 1;
    observations.push(located_tx_observation(
        &bundle,
        "inconsistent-left",
        inconsistent_block * 2,
        "K1INCONSISTENT",
        -20.0,
        500.0,
        22.5,
    ));
    observations.push(located_tx_observation(
        &bundle,
        "inconsistent-right",
        inconsistent_block * 2 + 1,
        "K1INCONSISTENT",
        -18.0,
        501.0,
        22.5,
    ));
    bundle.observations = observations;
    normalize_bundle(bundle)
}

fn all_path_profile_bundle() -> BundleContents {
    let mut bundle = bundle_with_layout(&["A", "B", "A", "B", "A", "B", "A", "B"]);
    let mut observations = vec![
        located_tx_observation(&bundle, "near-1", 0, "K1NEAR", -20.0, 200.0, 10.0),
        located_tx_observation(&bundle, "near-2", 2, "K1NEAR", -18.0, 200.0, 10.0),
        located_tx_observation(&bundle, "dx-1", 1, "K1DX", -25.0, 4_000.0, 220.0),
        located_tx_observation(&bundle, "dx-2", 3, "K1DX", -24.0, 4_000.0, 220.0),
        located_tx_observation(&bundle, "dx-3", 5, "K1DX", -23.0, 4_000.0, 220.0),
        located_tx_observation(&bundle, "dx-4", 7, "K1DX", -22.0, 4_000.0, 220.0),
        located_tx_observation(
            &bundle,
            "inconsistent-1",
            4,
            "K1INCONSISTENT",
            -21.0,
            400.0,
            30.0,
        ),
        located_tx_observation(
            &bundle,
            "inconsistent-2",
            6,
            "K1INCONSISTENT",
            -19.0,
            600.0,
            30.0,
        ),
    ];
    let mut missing = tx_observation(&bundle, "missing", 0, "K1MISSING", Some(-22.0));
    missing.reporter_grid = None;
    missing.distance_km = None;
    missing.azimuth_degrees = None;
    observations.push(missing);
    let mut no_snr = located_tx_observation(&bundle, "no-snr", 2, "K1NOSNR", -20.0, 300.0, 40.0);
    no_snr.snr_db = None;
    observations.push(no_snr);
    bundle.observations = observations;
    normalize_bundle(bundle)
}

fn bundle_with_layout(labels: &[&str]) -> BundleContents {
    let mut bundle = minimal_bundle();
    bundle.events.clear();
    bundle.observations.clear();
    let template = bundle.schedule.slots[0].clone();
    bundle.schedule.slots = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let mut slot = template.clone();
            slot.slot_id = format!("slot-{:03}", index + 1);
            slot.sequence_number = (index + 1) as u32;
            slot.starts_at = template.starts_at + Duration::minutes((index * 2) as i64);
            slot.antenna_label = (*label).to_string();
            slot
        })
        .collect();
    bundle
}

fn tx_observation(
    bundle: &BundleContents,
    id: &str,
    slot_index: usize,
    remote: &str,
    snr_db: Option<f32>,
) -> ObservationRecord {
    let slot = &bundle.schedule.slots[slot_index];
    let mut observation = minimal_bundle().observations[0].clone();
    observation.observation_id = id.to_string();
    observation.meta.timestamp = slot.starts_at + Duration::seconds(30);
    observation.meta.source = RecordSource::WsjtxLog;
    observation.band = slot.band;
    observation.snr_db = snr_db;
    observation.reporter_call = Some(remote.to_string());
    observation.heard_call = Some(bundle.station.callsign.clone());
    observation.reporter_grid = Some("EM12".to_string());
    observation.heard_grid = Some(bundle.station.grid.clone());
    observation.slot_id = None;
    observation.slot_label = None;
    observation.slot_confidence = None;
    observation
}

fn located_tx_observation(
    bundle: &BundleContents,
    id: &str,
    slot_index: usize,
    remote: &str,
    snr_db: f32,
    distance_km: f64,
    azimuth_degrees: f64,
) -> ObservationRecord {
    let mut observation = tx_observation(bundle, id, slot_index, remote, Some(snr_db));
    observation.distance_km = Some(distance_km);
    observation.azimuth_degrees = Some(azimuth_degrees);
    observation
}

fn inconclusive_bundle() -> BundleContents {
    fixture_bundle("inconclusive-sample-report.session.wsprabundle")
}

fn minimal_bundle() -> BundleContents {
    fixture_bundle("minimal-whole-station.session.wsprabundle")
}

fn fixture_bundle(name: &str) -> BundleContents {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles")
        .join(name);
    BundleStore::new(root)
        .read_normalized_validated()
        .expect("fixture bundle should be normalized and valid")
}
