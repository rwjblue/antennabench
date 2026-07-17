use std::path::PathBuf;

use antennabench_analysis::{ComparisonAvailability, PathDirection};
use antennabench_core::{
    normalize_bundle, validate_bundle_report, Band, BundleContents, ExperimentMode,
    ObservationRecord, RecordSource,
};
use antennabench_report::{
    build_report, build_report_with_resources, ReportCancellationToken, ReportError,
    ReportOverviewLifecycleState, ReportOverviewLimitation, ReportOverviewPathDelta,
    ReportResourceLimits, SessionReport,
};
use antennabench_storage::BundleStore;
use chrono::Duration;

#[test]
fn canonical_overview_is_explicit_serializable_and_has_no_invented_comparison() {
    let report = build_report(&canonical_bundle()).expect("canonical report should build");
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
                && error.diagnostic.observed == Some(3)
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

fn canonical_bundle() -> BundleContents {
    fixture_bundle("canonical-sample-report.session.wsprabundle")
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
