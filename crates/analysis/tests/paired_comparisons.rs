use std::path::PathBuf;

use antennabench_analysis::{
    summarize_bundle, ComparisonAvailability, ComparisonBlockEligibility, ComparisonOrder,
    PathDirection, SolarContextMissingReason, SolarEndpointRole, SolarLightState,
    SolarPositionResult,
};
use antennabench_core::{
    normalize_bundle, Band, BundleContents, ExperimentMode, ObservationKind, ObservationRecord,
    RecordSource,
};
use antennabench_storage::BundleStore;
use chrono::Duration;

#[test]
fn exposes_every_comparison_availability_state() {
    let mut single = bundle_with_layout(&["A"]);
    single.schedule.mode = ExperimentMode::SingleAntennaProfiling;
    assert_eq!(
        summary(single).comparison.availability,
        ComparisonAvailability::NotApplicable
    );

    let unsupported = bundle_with_layout(&["A", "A"]);
    assert_eq!(
        summary(unsupported).comparison.availability,
        ComparisonAvailability::UnsupportedComparisonShape
    );

    let no_blocks = bundle_with_layout(&["A", "A", "B"]);
    assert_eq!(
        summary(no_blocks).comparison.availability,
        ComparisonAvailability::NoEligibleBlocks
    );

    let mut unmatched = bundle_with_layout(&["A", "B"]);
    unmatched.observations = vec![
        tx_observation(&unmatched, "left", 0, "K1LEFT", Some(-20.0)),
        tx_observation(&unmatched, "right", 1, "K1RIGHT", Some(-18.0)),
    ];
    assert_eq!(
        summary(unmatched).comparison.availability,
        ComparisonAvailability::NoMatchedPaths
    );

    let mut paired = bundle_with_layout(&["A", "B"]);
    paired.observations = vec![
        tx_observation(&paired, "left", 0, "K1PAIR", Some(-20.0)),
        tx_observation(&paired, "right", 1, "K1PAIR", Some(-18.0)),
    ];
    assert_eq!(
        summary(paired).comparison.availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
}

#[test]
fn duplicate_schedule_sequence_numbers_cannot_produce_paired_evidence() {
    let mut bundle = bundle_with_layout(&["A", "B"]);
    bundle.schedule.slots[1].sequence_number = bundle.schedule.slots[0].sequence_number;
    bundle.observations = vec![
        tx_observation(&bundle, "left", 0, "K1PAIR", Some(-20.0)),
        tx_observation(&bundle, "right", 1, "K1PAIR", Some(-18.0)),
    ];

    let comparison = summary(bundle).comparison;

    assert_eq!(
        comparison.availability,
        ComparisonAvailability::NoEligibleBlocks
    );
    assert_eq!(comparison.diagnostics.eligible_block_count, 0);
    assert_eq!(comparison.diagnostics.invalid_block_count, 1);
    assert!(comparison.paired_rows.is_empty());
    assert!(comparison.blocks.iter().all(|block| {
        block.eligibility == ComparisonBlockEligibility::AmbiguousSequenceOrder
            && block.order.is_none()
    }));
}

#[test]
fn uses_fixed_orientation_non_overlapping_blocks_and_equal_path_weight() {
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

    let comparison = summary(bundle).comparison;

    assert_eq!(comparison.left_label.as_deref(), Some("A"));
    assert_eq!(comparison.right_label.as_deref(), Some("B"));
    assert_eq!(
        comparison.delta_orientation.as_ref().map(|orientation| (
            orientation.minuend_label.as_str(),
            orientation.subtrahend_label.as_str()
        )),
        Some(("B", "A"))
    );
    assert_eq!(
        comparison
            .blocks
            .iter()
            .map(|block| block.order)
            .collect::<Vec<_>>(),
        vec![
            Some(ComparisonOrder::LeftThenRight),
            Some(ComparisonOrder::RightThenLeft),
            Some(ComparisonOrder::LeftThenRight),
        ]
    );
    assert_eq!(comparison.paired_rows.len(), 4);
    assert_eq!(comparison.path_summaries.len(), 2);
    assert_eq!(
        comparison.strata[0].median_path_delta_right_minus_left_db,
        Some(4.0),
        "path medians (+12 and -4) must receive equal weight"
    );
    assert_eq!(comparison.strata[0].left_then_right_block_count, 2);
    assert_eq!(comparison.strata[0].right_then_left_block_count, 1);
    assert!(comparison
        .paired_rows
        .iter()
        .all(|row| row.elapsed_seconds > 0));
}

#[test]
fn keeps_missing_unmatched_ambiguous_duplicate_and_conflict_diagnostics_visible() {
    let mut bundle = bundle_with_layout(&["A", "B"]);
    let mut ambiguous = tx_observation(&bundle, "ambiguous", 0, "K0OTHER", Some(-10.0));
    ambiguous.reporter_call = Some("K0THIRD".to_string());
    ambiguous.heard_call = Some("K0OTHER".to_string());
    let exact_a = tx_observation(&bundle, "duplicate-a", 0, "K5DUP", Some(-12.0));
    let mut exact_b = exact_a.clone();
    exact_b.observation_id = "duplicate-b".to_string();
    let conflict_a = tx_observation(&bundle, "conflict-a", 0, "K6CONFLICT", Some(-15.0));
    let mut conflict_b = conflict_a.clone();
    conflict_b.observation_id = "conflict-b".to_string();
    conflict_b.snr_db = Some(-14.0);
    let mut guard_exclusion = tx_observation(&bundle, "guard-exclusion", 0, "K7GUARD", Some(-16.0));
    guard_exclusion.meta.timestamp = bundle.schedule.slots[0].starts_at + Duration::seconds(1);
    guard_exclusion.mode = Some("FT8".to_string());
    bundle.observations = vec![
        tx_observation(&bundle, "paired-left", 0, "K1PAIR", Some(-20.0)),
        tx_observation(&bundle, "paired-right", 1, "K1PAIR", Some(-18.0)),
        tx_observation(&bundle, "left-only", 0, "K2LEFT", Some(-22.0)),
        tx_observation(&bundle, "right-only", 1, "K3RIGHT", Some(-17.0)),
        tx_observation(&bundle, "missing-left", 0, "K4MISSING", None),
        tx_observation(&bundle, "finite-right", 1, "K4MISSING", Some(-19.0)),
        ambiguous,
        exact_a,
        exact_b,
        tx_observation(&bundle, "duplicate-right", 1, "K5DUP", Some(-11.0)),
        conflict_a,
        conflict_b,
        tx_observation(&bundle, "conflict-right", 1, "K6CONFLICT", Some(-13.0)),
        guard_exclusion,
    ];

    let summary = summary(bundle);
    assert_eq!(summary.exclusion_records.len(), 1);
    assert_eq!(
        summary.exclusion_records[0].observation_id,
        "guard-exclusion"
    );
    let comparison = summary.comparison;
    let diagnostics = comparison.diagnostics;

    assert_eq!(diagnostics.paired_row_count, 2);
    assert_eq!(diagnostics.unmatched_left_count, 1);
    assert_eq!(diagnostics.unmatched_right_count, 2);
    assert_eq!(diagnostics.missing_snr_left_count, 1);
    assert_eq!(diagnostics.ambiguous_path_count, 1);
    assert_eq!(diagnostics.exact_duplicate_count, 1);
    assert_eq!(diagnostics.conflicting_duplicate_group_count, 1);
    assert_eq!(comparison.overlap_rows.len(), 6);
    let excluded_only = comparison
        .strata
        .iter()
        .find(|stratum| stratum.stratum.mode.as_str() == "FT8")
        .expect("an exclusion-only valid stratum remains answerable as unavailable");
    assert_eq!(excluded_only.paired_row_count, 0);
    assert_eq!(excluded_only.excluded_observation_count, 1);
}

#[test]
fn separates_signal_modes_and_normalizes_case_and_surrounding_whitespace() {
    let mut bundle = bundle_with_layout(&["A", "B", "A", "B", "A", "B"]);
    let mut wspr_left = tx_observation(&bundle, "wspr-left", 0, "K1MODE", Some(-20.0));
    wspr_left.mode = Some("  wspr ".to_string());
    let mut wspr_right = tx_observation(&bundle, "wspr-right", 1, "K1MODE", Some(-18.0));
    wspr_right.mode = Some("WSPR".to_string());
    let mut cross_mode_left = tx_observation(&bundle, "cw-left-only", 0, "K1MODE", Some(-15.0));
    cross_mode_left.mode = Some("CW".to_string());
    let mut cw_left = tx_observation(&bundle, "cw-left", 2, "K1MODE", Some(-17.0));
    cw_left.mode = Some("cw".to_string());
    let mut cw_right = tx_observation(&bundle, "cw-right", 3, "K1MODE", Some(-16.0));
    cw_right.mode = Some(" CW ".to_string());
    let mut rtty_left = tx_observation(&bundle, "rtty-left", 4, "K1MODE", Some(-14.0));
    rtty_left.mode = Some("RTTY".to_string());
    let mut rtty_right = tx_observation(&bundle, "rtty-right", 5, "K1MODE", Some(-13.0));
    rtty_right.mode = Some("rtty".to_string());
    bundle.observations = vec![
        wspr_left,
        wspr_right,
        cross_mode_left,
        cw_left,
        cw_right,
        rtty_left,
        rtty_right,
    ];

    let forward = summary(bundle.clone()).comparison;
    bundle.observations.reverse();
    let reversed = summary(bundle).comparison;

    assert_eq!(forward, reversed);
    assert_eq!(forward.paired_rows.len(), 3);
    assert_eq!(forward.diagnostics.unmatched_left_count, 1);
    assert_eq!(
        forward
            .strata
            .iter()
            .map(|row| row.stratum.mode.as_str())
            .collect::<Vec<_>>(),
        vec!["CW", "RTTY", "WSPR"]
    );
    assert!(forward
        .paired_rows
        .iter()
        .all(|row| row.left_observation_id != "cw-left-only"));
}

#[test]
fn counts_missing_and_invalid_modes_without_pairing_them() {
    let mut bundle = bundle_with_layout(&["A", "B"]);
    let mut missing = tx_observation(&bundle, "missing-mode", 0, "K1MODE", Some(-20.0));
    missing.mode = None;
    let mut blank = tx_observation(&bundle, "blank-mode", 1, "K1MODE", Some(-18.0));
    blank.mode = Some(" \t ".to_string());
    let mut invalid = tx_observation(&bundle, "invalid-mode", 1, "K2MODE", Some(-17.0));
    invalid.mode = Some("CW\0RTTY".to_string());
    bundle.observations = vec![missing, blank, invalid];

    let comparison = summary(bundle).comparison;

    assert_eq!(
        comparison.availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert_eq!(comparison.diagnostics.missing_or_invalid_mode_count, 3);
    assert_eq!(comparison.diagnostics.missing_mode_count, 2);
    assert_eq!(comparison.diagnostics.malformed_mode_count, 1);
    assert_eq!(comparison.timeline_rows[0].missing_or_invalid_mode_count, 1);
    assert_eq!(comparison.timeline_rows[1].missing_or_invalid_mode_count, 2);
    assert!(comparison.paired_rows.is_empty());
    assert!(comparison.overlap_rows.is_empty());
}

#[test]
fn separates_band_direction_mode_kind_and_source_and_is_observation_order_independent() {
    let mut bundle = bundle_with_layout(&["A", "B", "A", "B"]);
    bundle.schedule.slots[2].band = Band::M40;
    bundle.schedule.slots[3].band = Band::M40;
    let mut rx_left = rx_observation(&bundle, "rx-left", 2, "W1RX", Some(-24.0));
    rx_left.observation_kind = ObservationKind::PublicReport;
    rx_left.meta.source = RecordSource::Wsprnet;
    let mut rx_right = rx_observation(&bundle, "rx-right", 3, "W1RX", Some(-22.0));
    rx_right.observation_kind = ObservationKind::PublicReport;
    rx_right.meta.source = RecordSource::Wsprnet;
    bundle.observations = vec![
        tx_observation(&bundle, "tx-left", 0, "K1TX", Some(-20.0)),
        tx_observation(&bundle, "tx-right", 1, "K1TX", Some(-21.0)),
        rx_left,
        rx_right,
    ];
    let forward = summary(bundle.clone()).comparison;
    bundle.observations.reverse();
    let reversed = summary(bundle).comparison;

    assert_eq!(forward, reversed);
    assert_eq!(forward.strata.len(), 2);
    assert_eq!(forward.strata[0].stratum.direction, PathDirection::Transmit);
    assert_eq!(forward.strata[0].stratum.band, Band::M20);
    assert_eq!(forward.strata[0].stratum.mode.as_str(), "WSPR");
    assert_eq!(
        forward.strata[0].stratum.observation_kind,
        ObservationKind::LocalDecode
    );
    assert_eq!(forward.strata[0].stratum.source, RecordSource::WsjtxLog);
    assert_eq!(forward.strata[1].stratum.direction, PathDirection::Receive);
    assert_eq!(forward.strata[1].stratum.band, Band::M40);
    assert_eq!(forward.strata[1].stratum.mode.as_str(), "WSPR");
    assert_eq!(
        forward.strata[1].stratum.observation_kind,
        ObservationKind::PublicReport
    );
    assert_eq!(forward.strata[1].stratum.source, RecordSource::Wsprnet);
    let serialized = serde_json::to_string(&forward).expect("comparison should serialize");
    for prohibited in [
        "winner",
        "better",
        "equivalent",
        "significant",
        "confidence",
    ] {
        assert!(!serialized.contains(prohibited));
    }
}

#[test]
fn preserves_fixed_order_time_drift_balanced_order_and_bidirectional_deltas() {
    let mut fixed = bundle_with_layout(&["A", "B", "A", "B", "A", "B"]);
    fixed.observations = (0..6)
        .map(|slot| {
            tx_observation(
                &fixed,
                &format!("drift-{slot}"),
                slot,
                "K1DRIFT",
                Some(-30.0 + slot as f32),
            )
        })
        .collect();
    let fixed_comparison = summary(fixed).comparison;
    assert!(fixed_comparison
        .paired_rows
        .iter()
        .all(|row| row.order == ComparisonOrder::LeftThenRight));
    assert!(fixed_comparison
        .paired_rows
        .windows(2)
        .all(|rows| rows[0].left_timestamp < rows[1].left_timestamp));

    let mut balanced = bundle_with_layout(&["A", "B", "B", "A"]);
    balanced.observations = vec![
        tx_observation(&balanced, "positive-left", 0, "K1POS", Some(-20.0)),
        tx_observation(&balanced, "positive-right", 1, "K1POS", Some(-18.0)),
        tx_observation(&balanced, "negative-right", 2, "K2NEG", Some(-22.0)),
        tx_observation(&balanced, "negative-left", 3, "K2NEG", Some(-19.0)),
        tx_observation(&balanced, "zero-left", 0, "K3ZERO", Some(-17.0)),
        tx_observation(&balanced, "zero-right", 1, "K3ZERO", Some(-17.0)),
    ];
    let balanced_comparison = summary(balanced).comparison;
    assert_eq!(
        balanced_comparison.diagnostics.left_then_right_block_count,
        1
    );
    assert_eq!(
        balanced_comparison.diagnostics.right_then_left_block_count,
        1
    );
    let deltas = balanced_comparison
        .paired_rows
        .iter()
        .map(|row| row.delta_right_minus_left_db)
        .collect::<Vec<_>>();
    assert!(deltas.iter().any(|delta| *delta < 0.0));
    assert!(deltas.contains(&0.0));
    assert!(deltas.iter().any(|delta| *delta > 0.0));
}

#[test]
fn derives_typed_deterministic_solar_context_without_inventing_locations() {
    let mut bundle = bundle_with_layout(&["A", "B"]);
    let mut left = tx_observation(&bundle, "solar-left", 0, "K1SOLAR", Some(-20.0));
    left.reporter_grid = None;
    let mut right = tx_observation(&bundle, "solar-right", 1, "K1SOLAR", Some(-18.0));
    right.reporter_grid = Some("not-a-grid".into());
    bundle.observations = vec![right, left];

    let forward = summary(bundle.clone()).solar_context;
    bundle.observations.reverse();
    let reordered = summary(bundle).solar_context;
    assert_eq!(forward, reordered);
    assert_eq!(forward.rows.len(), 1);
    let row = &forward.rows[0];
    assert_eq!(row.stratum.direction, PathDirection::Transmit);
    assert_eq!(row.stratum.mode.as_str(), "WSPR");
    assert_eq!(row.left.observation_id, "solar-left");
    assert_eq!(row.right.observation_id, "solar-right");
    assert_eq!(row.left.station.role, SolarEndpointRole::Station);
    assert_eq!(row.left.remote.role, SolarEndpointRole::Remote);
    assert!(matches!(
        row.left.station.result,
        SolarPositionResult::Available {
            light_state: SolarLightState::Daylight
                | SolarLightState::CivilTwilight
                | SolarLightState::NauticalTwilight
                | SolarLightState::AstronomicalTwilight
                | SolarLightState::Night,
            ..
        }
    ));
    assert_eq!(
        row.left.remote.result,
        SolarPositionResult::Missing {
            reason: SolarContextMissingReason::MissingGrid
        }
    );
    assert_eq!(
        row.right.remote.result,
        SolarPositionResult::Missing {
            reason: SolarContextMissingReason::InvalidGrid
        }
    );
    let serialized = serde_json::to_string(&forward).expect("solar context should serialize");
    for prohibited in ["winner", "better", "caused", "superior"] {
        assert!(!serialized.contains(prohibited));
    }
}

fn summary(bundle: BundleContents) -> antennabench_analysis::AnalysisSummary {
    summarize_bundle(&normalize_bundle(bundle)).expect("synthetic bundle should summarize")
}

fn bundle_with_layout(labels: &[&str]) -> BundleContents {
    let mut bundle = fixture_bundle();
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
    let mut observation = observation(bundle, id, slot_index, snr_db);
    observation.reporter_call = Some(remote.to_string());
    observation.heard_call = Some(bundle.station.callsign.clone());
    observation.reporter_grid = Some("EM12".to_string());
    observation.heard_grid = Some(bundle.station.grid.clone());
    observation
}

fn rx_observation(
    bundle: &BundleContents,
    id: &str,
    slot_index: usize,
    remote: &str,
    snr_db: Option<f32>,
) -> ObservationRecord {
    let mut observation = observation(bundle, id, slot_index, snr_db);
    observation.reporter_call = Some(bundle.station.callsign.clone());
    observation.heard_call = Some(remote.to_string());
    observation.reporter_grid = Some(bundle.station.grid.clone());
    observation.heard_grid = Some("EN52".to_string());
    observation
}

fn observation(
    bundle: &BundleContents,
    id: &str,
    slot_index: usize,
    snr_db: Option<f32>,
) -> ObservationRecord {
    let slot = &bundle.schedule.slots[slot_index];
    let mut observation = fixture_bundle().observations[0].clone();
    observation.observation_id = id.to_string();
    observation.meta.timestamp = slot.starts_at + Duration::seconds(30);
    observation.meta.source = RecordSource::WsjtxLog;
    observation.observation_kind = ObservationKind::LocalDecode;
    observation.band = slot.band;
    observation.snr_db = snr_db;
    observation.slot_id = None;
    observation.slot_label = None;
    observation.slot_confidence = None;
    observation
}

fn fixture_bundle() -> BundleContents {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    BundleStore::new(root)
        .read_normalized_validated()
        .expect("fixture bundle should be normalized and valid")
}
