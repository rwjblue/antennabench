use antennabench_analysis::{
    ComparisonOrder, ComparisonStratum, PathDirection, ReporterActivityAnalysis,
    ReporterActivityCoverage, ReporterActivityJointOutcome, ReporterActivityJointReceiver,
    ReporterActivityJointSummary, ReporterActivityPairedRate, ReporterActivityUnknownReason,
    SignalMode,
};
use antennabench_core::{Band, ObservationKind, RecordSource};

use super::build_common_opportunity_maps;

#[test]
fn geographic_joint_outcomes_preserve_denominators_order_coverage_and_missing_locations() {
    let activity = repeated_activity();
    let maps = build_common_opportunity_maps("AA00", &activity);
    let group = &maps[0];

    assert_eq!(group.unique_common_active_receiver_count, 5);
    assert_eq!(group.receiver_block_opportunity_count, 10);
    assert_eq!(group.located_unique_receiver_count, 4);
    assert_eq!(group.located_receiver_block_opportunity_count, 8);
    assert_eq!(group.location_unavailable_unique_receiver_count, 1);
    assert_eq!(
        group.location_unavailable_receiver_block_opportunity_count,
        2
    );
    assert_eq!(group.coverage, ReporterActivityCoverage::Truncated);
    assert_eq!(
        group
            .distance_cells
            .iter()
            .map(|cell| (
                cell.unique_common_active_receiver_count,
                cell.receiver_block_opportunity_count,
                cell.heard_both_count,
                cell.left_only_count,
                cell.right_only_count,
                cell.heard_neither_count,
            ))
            .collect::<Vec<_>>(),
        vec![
            (2, 4, 2, 2, 0, 0),
            (1, 2, 0, 0, 0, 2),
            (0, 0, 0, 0, 0, 0),
            (1, 2, 0, 0, 2, 0)
        ]
    );
    assert_eq!(group.distance_cells[0].left_detection_rate, Some(1.0));
    assert_eq!(group.distance_cells[0].right_detection_rate, Some(0.5));
    assert_eq!(group.distance_cells[3].left_detection_rate, Some(0.0));
    assert_eq!(group.distance_cells[3].right_detection_rate, Some(1.0));
    assert_eq!(group.blocks.len(), 2);
    assert_eq!(group.blocks[0].order, ComparisonOrder::LeftThenRight);
    assert_eq!(group.blocks[1].order, ComparisonOrder::RightThenLeft);
    assert!(group
        .distance_cells
        .iter()
        .filter(|cell| cell.receiver_block_opportunity_count > 0)
        .all(|cell| cell.coverage == ReporterActivityCoverage::Truncated));

    let mut reordered = activity;
    reordered.paired_rates.reverse();
    for row in &mut reordered.paired_rates {
        row.receivers.reverse();
    }
    assert_eq!(maps, build_common_opportunity_maps("AA00", &reordered));
}

#[test]
fn unsupported_receive_direction_remains_an_explicit_empty_geographic_group() {
    let stratum = stratum(PathDirection::Receive);
    let coverage = ReporterActivityCoverage::Unknown(
        ReporterActivityUnknownReason::UnsupportedReceiveDirection,
    );
    let activity = ReporterActivityAnalysis {
        census_cycles: Vec::new(),
        cycle_rates: Vec::new(),
        paired_rates: vec![paired_rate(
            stratum.clone(),
            0,
            ComparisonOrder::LeftThenRight,
            coverage,
        )],
        joint_summaries: vec![ReporterActivityJointSummary {
            stratum,
            coverage,
            eligible_block_count: 1,
            known_coverage_block_count: 0,
            left_then_right_block_count: 1,
            right_then_left_block_count: 0,
            unique_active_receiver_count: 0,
            receiver_block_opportunity_count: 0,
            heard_both_count: 0,
            left_only_count: 0,
            right_only_count: 0,
            heard_neither_count: 0,
            left_detection_rate: None,
            right_detection_rate: None,
        }],
    };

    let maps = build_common_opportunity_maps("AA00", &activity);
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].coverage, coverage);
    assert!(maps[0].polar_cells.is_empty());
    assert_eq!(maps[0].known_coverage_block_count, 0);
}

fn repeated_activity() -> ReporterActivityAnalysis {
    let stratum = stratum(PathDirection::Transmit);
    let mut first = paired_rate(
        stratum.clone(),
        0,
        ComparisonOrder::LeftThenRight,
        ReporterActivityCoverage::Partial,
    );
    first.receivers = receivers(false);
    apply_counts(&mut first);
    let mut second = paired_rate(
        stratum.clone(),
        1,
        ComparisonOrder::RightThenLeft,
        ReporterActivityCoverage::Truncated,
    );
    second.receivers = receivers(true);
    apply_counts(&mut second);
    ReporterActivityAnalysis {
        census_cycles: Vec::new(),
        cycle_rates: Vec::new(),
        paired_rates: vec![first, second],
        joint_summaries: vec![ReporterActivityJointSummary {
            stratum,
            coverage: ReporterActivityCoverage::Truncated,
            eligible_block_count: 2,
            known_coverage_block_count: 2,
            left_then_right_block_count: 1,
            right_then_left_block_count: 1,
            unique_active_receiver_count: 5,
            receiver_block_opportunity_count: 10,
            heard_both_count: 2,
            left_only_count: 3,
            right_only_count: 3,
            heard_neither_count: 2,
            left_detection_rate: Some(0.5),
            right_detection_rate: Some(0.5),
        }],
    }
}

fn receivers(second_block: bool) -> Vec<ReporterActivityJointReceiver> {
    vec![
        receiver(
            "K1NEAR",
            Some("AA00aa"),
            ReporterActivityJointOutcome::LeftOnly,
        ),
        receiver(
            "K1BOTH",
            Some("AA00bb"),
            ReporterActivityJointOutcome::HeardBoth,
        ),
        receiver(
            "K1DX",
            Some("AJ00aa"),
            ReporterActivityJointOutcome::RightOnly,
        ),
        receiver(
            "K1NEITHER",
            Some("AB00aa"),
            ReporterActivityJointOutcome::HeardNeither,
        ),
        receiver(
            "K1MISSING",
            None,
            if second_block {
                ReporterActivityJointOutcome::RightOnly
            } else {
                ReporterActivityJointOutcome::LeftOnly
            },
        ),
    ]
}

fn receiver(
    receiver: &str,
    receiver_grid: Option<&str>,
    outcome: ReporterActivityJointOutcome,
) -> ReporterActivityJointReceiver {
    ReporterActivityJointReceiver {
        receiver: receiver.to_string(),
        receiver_grid: receiver_grid.map(str::to_string),
        outcome,
    }
}

fn paired_rate(
    stratum: ComparisonStratum,
    block_index: usize,
    order: ComparisonOrder,
    coverage: ReporterActivityCoverage,
) -> ReporterActivityPairedRate {
    ReporterActivityPairedRate {
        stratum,
        block_index,
        order,
        coverage,
        left_slot_id: format!("left-{block_index}"),
        right_slot_id: format!("right-{block_index}"),
        active_in_both_count: 0,
        left_heard_count: 0,
        right_heard_count: 0,
        left_hearing_rate: None,
        right_hearing_rate: None,
        heard_both_count: 0,
        left_only_count: 0,
        right_only_count: 0,
        heard_neither_count: 0,
        receivers: Vec::new(),
    }
}

fn apply_counts(row: &mut ReporterActivityPairedRate) {
    row.active_in_both_count = 5;
    row.heard_both_count = 1;
    row.left_only_count = 1 + usize::from(row.block_index == 0);
    row.right_only_count = 1 + usize::from(row.block_index == 1);
    row.left_heard_count = row.heard_both_count + row.left_only_count;
    row.right_heard_count = row.heard_both_count + row.right_only_count;
    row.left_hearing_rate = Some(row.left_heard_count as f64 / 5.0);
    row.right_hearing_rate = Some(row.right_heard_count as f64 / 5.0);
    row.heard_neither_count = 1;
}

fn stratum(direction: PathDirection) -> ComparisonStratum {
    ComparisonStratum {
        direction,
        band: Band::M20,
        mode: SignalMode::normalize("WSPR").unwrap(),
        observation_kind: ObservationKind::PublicReport,
        source: RecordSource::Wsprnet,
    }
}
