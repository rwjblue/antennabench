use antennabench_analysis::{
    ComparisonAvailability, ComparisonBlock, ComparisonBlockEligibility, ComparisonDiagnostics,
    ComparisonOrder, ComparisonSide, ComparisonStratum, ObservedAntennaPath,
    ObservedAntennaPathProfile, ObservedPathLocation, PairedComparisonAnalysis, PathDirection,
    ReporterActivityAnalysis, ReporterActivityCoverage, ReporterActivityJointOutcome,
    ReporterActivityJointReceiver, ReporterActivityJointSummary, ReporterActivityPairedRate,
    SignalMode,
};
use antennabench_core::{AlignedSlotStatus, Band, ObservationKind, RecordSource};
use chrono::{TimeZone, Utc};

use super::project_coverage_overlap;

#[test]
fn separates_observed_and_common_opportunity_complementarity_and_counts_repeatability() {
    let stratum = stratum(PathDirection::Transmit, Band::M20);
    let comparison = comparison(
        vec![
            profile(
                &stratum,
                ComparisonSide::Left,
                "A",
                vec![
                    path("A-only-1", &[0], 1),
                    path("A-only-2", &[0], 1),
                    path("shared", &[0, 1], 10),
                ],
            ),
            profile(
                &stratum,
                ComparisonSide::Right,
                "B",
                vec![path("B-only", &[1], 1), path("shared", &[0, 1], 2)],
            ),
        ],
        vec![
            block(0, Band::M20, ComparisonOrder::LeftThenRight),
            block(1, Band::M20, ComparisonOrder::RightThenLeft),
        ],
    );
    let activity = activity(stratum.clone());
    let groups = project_coverage_overlap(&comparison, &activity);
    let group = &groups[0];
    let observed = group.observed.as_ref().unwrap();

    assert_eq!(
        (
            observed.left_only_unique_path_count,
            observed.shared_unique_path_count,
            observed.right_only_unique_path_count,
            observed.total_system_unique_path_count,
        ),
        (2, 1, 1, 4)
    );
    let left = observed.left.as_ref().unwrap();
    let right = observed.right.as_ref().unwrap();
    assert_eq!(
        (
            left.unique_endpoint_count,
            left.path_block_observation_count,
            left.observed_once_path_count,
            left.repeated_path_count,
        ),
        (3, 4, 2, 1)
    );
    assert_eq!(left.paths[2].observation_count, 10);
    assert_eq!(left.paths[2].left_then_right_block_count, 1);
    assert_eq!(left.paths[2].right_then_left_block_count, 1);
    assert_eq!(left.paths[0].left_then_right_block_count, 1);
    assert_eq!(left.paths[0].right_then_left_block_count, 0);
    assert_eq!(
        right
            .block_count_distribution
            .iter()
            .map(|row| (row.observed_block_count, row.unique_path_count))
            .collect::<Vec<_>>(),
        vec![(1, 1), (2, 1)]
    );

    let common = group.common_opportunity.as_ref().unwrap();
    assert_eq!(common.unique_common_active_receiver_count, 3);
    assert_eq!(common.receiver_block_opportunity_count, 6);
    assert_eq!(
        (
            common.heard_both_count,
            common.left_only_count,
            common.right_only_count,
            common.heard_neither_count,
        ),
        (2, 1, 2, 1)
    );
    assert_ne!(observed.left_only_unique_path_count, common.left_only_count);
    assert_eq!(common.order_summaries.len(), 2);
    let receiver = common
        .receiver_frequencies
        .iter()
        .find(|row| row.receiver == "R1")
        .unwrap();
    assert_eq!(
        (
            receiver.opportunity_count,
            receiver.left_detection_count,
            receiver.right_detection_count,
            receiver.left_then_right_opportunity_count,
            receiver.right_then_left_opportunity_count,
        ),
        (2, 1, 0, 1, 1)
    );
}

#[test]
fn strata_and_single_block_repeatability_remain_explicit() {
    let tx = stratum(PathDirection::Transmit, Band::M20);
    let rx = stratum(PathDirection::Receive, Band::M40);
    let comparison = comparison(
        vec![
            profile(
                &tx,
                ComparisonSide::Left,
                "A",
                vec![path("TX-A-only", &[0], 1)],
            ),
            profile(
                &tx,
                ComparisonSide::Right,
                "B",
                vec![path("TX-B-only", &[0], 1)],
            ),
            profile(
                &rx,
                ComparisonSide::Left,
                "A",
                vec![path("RX-shared-1", &[1], 1), path("RX-shared-2", &[1], 1)],
            ),
            profile(
                &rx,
                ComparisonSide::Right,
                "B",
                vec![path("RX-shared-1", &[1], 1), path("RX-shared-2", &[1], 1)],
            ),
        ],
        vec![
            block(0, Band::M20, ComparisonOrder::LeftThenRight),
            block(1, Band::M40, ComparisonOrder::RightThenLeft),
        ],
    );
    let groups = project_coverage_overlap(&comparison, &ReporterActivityAnalysis::default());

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].stratum, tx);
    assert_eq!(groups[1].stratum, rx);
    assert_eq!(
        observed_counts(&groups[0]),
        (1, 0, 1),
        "sparse disjoint paths remain highly complementary"
    );
    assert_eq!(
        observed_counts(&groups[1]),
        (0, 2, 0),
        "shared paths retain high overlap with no incremental reach"
    );
    for group in groups {
        let observed = group.observed.unwrap();
        assert_eq!(observed.eligible_block_count, 1);
        for repeatability in [&observed.left, &observed.right].into_iter().flatten() {
            assert_eq!(
                repeatability.observed_once_path_count,
                repeatability.unique_endpoint_count
            );
            assert_eq!(repeatability.repeated_path_count, 0);
        }
    }
}

fn observed_counts(group: &crate::ReportCoverageOverlapGroup) -> (usize, usize, usize) {
    let observed = group.observed.as_ref().unwrap();
    (
        observed.left_only_unique_path_count,
        observed.shared_unique_path_count,
        observed.right_only_unique_path_count,
    )
}

fn comparison(
    observed_path_profiles: Vec<ObservedAntennaPathProfile>,
    blocks: Vec<ComparisonBlock>,
) -> PairedComparisonAnalysis {
    PairedComparisonAnalysis {
        availability: ComparisonAvailability::NoMatchedPaths,
        left_label: Some("A".into()),
        right_label: Some("B".into()),
        delta_orientation: None,
        diagnostics: ComparisonDiagnostics::default(),
        blocks,
        overlap_rows: Vec::new(),
        timeline_rows: Vec::new(),
        paired_rows: Vec::new(),
        path_summaries: Vec::new(),
        strata: Vec::new(),
        observed_path_profiles,
    }
}

fn profile(
    stratum: &ComparisonStratum,
    side: ComparisonSide,
    antenna_label: &str,
    paths: Vec<ObservedAntennaPath>,
) -> ObservedAntennaPathProfile {
    ObservedAntennaPathProfile {
        stratum: stratum.clone(),
        side,
        antenna_label: antenna_label.into(),
        unique_path_count: paths.len(),
        located_path_count: 0,
        missing_location_path_count: paths.len(),
        inconsistent_location_path_count: 0,
        paths,
    }
}

fn path(
    remote_path: &str,
    block_indices: &[usize],
    observation_count: usize,
) -> ObservedAntennaPath {
    ObservedAntennaPath {
        remote_path: remote_path.into(),
        location: ObservedPathLocation::Missing,
        block_support_count: block_indices.len(),
        slot_support_count: block_indices.len(),
        observation_count,
        block_indices: block_indices.to_vec(),
        slot_ids: Vec::new(),
        observation_ids: Vec::new(),
        snr: None,
    }
}

fn block(index: usize, band: Band, order: ComparisonOrder) -> ComparisonBlock {
    ComparisonBlock {
        block_index: index,
        band,
        first_slot_id: format!("first-{index}"),
        first_sequence_number: index as u32 * 2,
        first_starts_at: Utc.timestamp_opt(index as i64 * 120, 0).unwrap(),
        first_label: Some("A".into()),
        first_status: AlignedSlotStatus::Switched,
        second_slot_id: Some(format!("second-{index}")),
        second_sequence_number: Some(index as u32 * 2 + 1),
        second_starts_at: Some(Utc.timestamp_opt(index as i64 * 120 + 60, 0).unwrap()),
        second_label: Some("B".into()),
        second_status: Some(AlignedSlotStatus::Switched),
        order: Some(order),
        eligibility: ComparisonBlockEligibility::Eligible,
    }
}

fn activity(stratum: ComparisonStratum) -> ReporterActivityAnalysis {
    let mut first = paired(stratum.clone(), 0, ComparisonOrder::LeftThenRight);
    first.receivers = vec![
        receiver("R1", ReporterActivityJointOutcome::LeftOnly),
        receiver("R2", ReporterActivityJointOutcome::HeardBoth),
        receiver("R3", ReporterActivityJointOutcome::RightOnly),
    ];
    first.left_only_count = 1;
    first.heard_both_count = 1;
    first.right_only_count = 1;
    first.left_heard_count = 2;
    first.right_heard_count = 2;
    let mut second = paired(stratum.clone(), 1, ComparisonOrder::RightThenLeft);
    second.receivers = vec![
        receiver("R1", ReporterActivityJointOutcome::HeardNeither),
        receiver("R2", ReporterActivityJointOutcome::HeardBoth),
        receiver("R3", ReporterActivityJointOutcome::RightOnly),
    ];
    second.heard_both_count = 1;
    second.right_only_count = 1;
    second.heard_neither_count = 1;
    second.left_heard_count = 1;
    second.right_heard_count = 2;
    ReporterActivityAnalysis {
        census_cycles: Vec::new(),
        cycle_rates: Vec::new(),
        paired_rates: vec![first, second],
        joint_summaries: vec![ReporterActivityJointSummary {
            stratum,
            coverage: ReporterActivityCoverage::Complete,
            eligible_block_count: 2,
            known_coverage_block_count: 2,
            left_then_right_block_count: 1,
            right_then_left_block_count: 1,
            unique_active_receiver_count: 3,
            receiver_block_opportunity_count: 6,
            heard_both_count: 2,
            left_only_count: 1,
            right_only_count: 2,
            heard_neither_count: 1,
            left_detection_rate: Some(0.5),
            right_detection_rate: Some(2.0 / 3.0),
        }],
    }
}

fn paired(
    stratum: ComparisonStratum,
    block_index: usize,
    order: ComparisonOrder,
) -> ReporterActivityPairedRate {
    ReporterActivityPairedRate {
        stratum,
        block_index,
        order,
        coverage: ReporterActivityCoverage::Complete,
        left_slot_id: format!("left-{block_index}"),
        right_slot_id: format!("right-{block_index}"),
        active_in_both_count: 3,
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

fn receiver(
    receiver: &str,
    outcome: ReporterActivityJointOutcome,
) -> ReporterActivityJointReceiver {
    ReporterActivityJointReceiver {
        receiver: receiver.into(),
        receiver_grid: None,
        outcome,
    }
}

fn stratum(direction: PathDirection, band: Band) -> ComparisonStratum {
    ComparisonStratum {
        direction,
        band,
        mode: SignalMode::normalize("WSPR").unwrap(),
        observation_kind: ObservationKind::PublicReport,
        source: RecordSource::Wsprnet,
    }
}
