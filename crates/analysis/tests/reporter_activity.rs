use std::{collections::BTreeMap, path::PathBuf};

use antennabench_analysis::{
    summarize_bundle_with_activity, ComparisonOrder, PathDirection, ReporterActivityCoverage,
    ReporterActivityJointOutcome, ReporterActivityUnknownReason,
};
use antennabench_core::{
    normalize_bundle,
    v2::{
        AdapterDisposition, AdapterInput, AdapterReasonId, AdapterRecordV2, MutationMember,
        Provenance, RecordMetaV2,
    },
    Band, BundleContents, ObservationKind, ObservationRecord, RecordSource,
};
use antennabench_storage::BundleStore;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;

#[test]
fn computes_field_shape_cycle_and_paired_active_set_rates() {
    let (bundle, records, directions) = field_shape(false, true);
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .expect("activity fixture should analyze")
        .reporter_activity;

    assert_eq!(activity.cycle_rates.len(), 3);
    assert_eq!(activity.cycle_rates[0].antenna_label, "A");
    assert_eq!(activity.cycle_rates[0].active_reporter_count, 10);
    assert_eq!(activity.cycle_rates[0].heard_reporter_count, 6);
    assert_eq!(activity.cycle_rates[0].hearing_rate, Some(0.6));
    assert_eq!(activity.cycle_rates[1].antenna_label, "B");
    assert_eq!(activity.cycle_rates[1].heard_reporter_count, 2);
    assert_eq!(activity.cycle_rates[1].hearing_rate, Some(0.2));
    assert_eq!(
        activity.cycle_rates[2].antenna_label, "B",
        "the third same-antenna control cycle remains independently visible"
    );
    assert_eq!(activity.cycle_rates[2].heard_reporter_count, 7);
    assert_eq!(activity.cycle_rates[2].hearing_rate, Some(0.7));

    assert_eq!(activity.paired_rates.len(), 1);
    let paired = &activity.paired_rates[0];
    assert_eq!(paired.active_in_both_count, 8);
    assert_eq!(paired.left_heard_count, 6);
    assert_eq!(paired.right_heard_count, 2);
    assert_eq!(paired.left_hearing_rate, Some(0.75));
    assert_eq!(paired.right_hearing_rate, Some(0.25));
    assert_eq!(paired.heard_both_count, 2);
    assert_eq!(paired.left_only_count, 4);
    assert_eq!(paired.right_only_count, 0);
    assert_eq!(paired.heard_neither_count, 2);
    assert_eq!(
        paired.active_in_both_count,
        paired.heard_both_count
            + paired.left_only_count
            + paired.right_only_count
            + paired.heard_neither_count
    );
    assert!(paired
        .receivers
        .iter()
        .all(|receiver| receiver.receiver_grid.as_deref() == Some("EM12")));
    assert!(paired
        .receivers
        .iter()
        .all(|receiver| receiver.receiver != "K020ACT" && receiver.receiver != "K021ACT"));
}

#[test]
fn partitions_all_four_joint_outcomes_and_aggregates_repeated_receivers() {
    let (bundle, records, directions) = joint_shape();
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .expect("joint outcome fixture should analyze")
        .reporter_activity;

    assert_eq!(activity.paired_rates.len(), 2);
    for row in &activity.paired_rates {
        assert_eq!(row.active_in_both_count, 4);
        assert_eq!(row.heard_both_count, 1);
        assert_eq!(row.left_only_count, 1);
        assert_eq!(row.right_only_count, 1);
        assert_eq!(row.heard_neither_count, 1);
        assert_eq!(row.receivers.len(), 4);
        assert_eq!(
            row.active_in_both_count,
            row.heard_both_count
                + row.left_only_count
                + row.right_only_count
                + row.heard_neither_count
        );
        assert_eq!(row.left_hearing_rate, Some(0.5));
        assert_eq!(row.right_hearing_rate, Some(0.5));
    }
    assert_eq!(
        activity.paired_rates[0].order,
        ComparisonOrder::LeftThenRight
    );
    assert_eq!(
        activity.paired_rates[1].order,
        ComparisonOrder::RightThenLeft
    );
    assert_eq!(
        activity.paired_rates[0]
            .receivers
            .iter()
            .map(|row| row.outcome)
            .collect::<Vec<_>>(),
        vec![
            ReporterActivityJointOutcome::HeardBoth,
            ReporterActivityJointOutcome::LeftOnly,
            ReporterActivityJointOutcome::RightOnly,
            ReporterActivityJointOutcome::HeardNeither,
        ]
    );

    let summary = &activity.joint_summaries[0];
    assert_eq!(summary.eligible_block_count, 2);
    assert_eq!(summary.known_coverage_block_count, 2);
    assert_eq!(summary.left_then_right_block_count, 1);
    assert_eq!(summary.right_then_left_block_count, 1);
    assert_eq!(summary.unique_active_receiver_count, 4);
    assert_eq!(summary.receiver_block_opportunity_count, 8);
    assert_eq!(summary.heard_both_count, 2);
    assert_eq!(summary.left_only_count, 2);
    assert_eq!(summary.right_only_count, 2);
    assert_eq!(summary.heard_neither_count, 2);
}

#[test]
fn joint_analysis_is_independent_of_evidence_input_order() {
    let (bundle, records, directions) = joint_shape();
    let expected = summarize_bundle_with_activity(&bundle, &records, &directions)
        .unwrap()
        .reporter_activity;
    let mut reversed_bundle = bundle.clone();
    reversed_bundle.observations.reverse();
    let mut reversed_records = records;
    reversed_records.reverse();
    let actual = summarize_bundle_with_activity(&reversed_bundle, &reversed_records, &directions)
        .unwrap()
        .reporter_activity;

    assert_eq!(actual.paired_rates, expected.paired_rates);
    assert_eq!(actual.joint_summaries, expected.joint_summaries);
}

#[test]
fn receive_direction_is_explicitly_unsupported_for_joint_activity() {
    let (mut bundle, records, _) = joint_shape();
    for observation in &mut bundle.observations {
        let remote = observation.reporter_call.take().unwrap();
        observation.reporter_call = Some(bundle.station.callsign.clone());
        observation.heard_call = Some(remote);
        observation.reporter_grid = Some(bundle.station.grid.clone());
        observation.heard_grid = Some("EM12".into());
    }
    let directions = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| (slot.slot_id.clone(), PathDirection::Receive))
        .collect::<BTreeMap<_, _>>();
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .unwrap()
        .reporter_activity;

    assert!(!activity.paired_rates.is_empty());
    assert!(activity.paired_rates.iter().all(|row| {
        row.coverage
            == ReporterActivityCoverage::Unknown(
                ReporterActivityUnknownReason::UnsupportedReceiveDirection,
            )
            && row.receivers.is_empty()
            && row.active_in_both_count == 0
    }));
}

#[test]
fn non_wspr_activity_is_explicitly_outside_the_census_boundary() {
    let (mut bundle, records, directions) = joint_shape();
    for observation in &mut bundle.observations {
        observation.mode = Some("FT8".into());
    }
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .unwrap()
        .reporter_activity;

    assert!(!activity.paired_rates.is_empty());
    assert!(activity.paired_rates.iter().all(|row| {
        row.coverage
            == ReporterActivityCoverage::Unknown(
                ReporterActivityUnknownReason::UnsupportedSignalMode,
            )
            && row.receivers.is_empty()
    }));
}

#[test]
fn useful_one_sided_detection_survives_without_a_finite_matched_path() {
    let (mut bundle, records, directions) = joint_shape();
    bundle.observations.retain(|observation| {
        let slot = observation.slot_id.as_deref().unwrap();
        let receiver = observation.reporter_call.as_deref().unwrap();
        (slot == "joint-slot-1" && receiver == "K001ACT")
            || (slot == "joint-slot-2" && receiver == "K002ACT")
            || (slot == "joint-slot-3" && receiver == "K002ACT")
            || (slot == "joint-slot-4" && receiver == "K001ACT")
    });
    let summary = summarize_bundle_with_activity(&bundle, &records, &directions).unwrap();

    assert!(summary.comparison.paired_rows.is_empty());
    assert_eq!(summary.reporter_activity.paired_rates.len(), 2);
    assert!(summary.reporter_activity.paired_rates.iter().all(|row| {
        row.left_only_count == 1
            && row.right_only_count == 1
            && row.heard_both_count == 0
            && row.heard_neither_count == 2
    }));
}

#[test]
fn partial_coverage_preserves_the_exact_joint_partition() {
    let (bundle, mut records, directions) = joint_shape();
    records[0].disposition = AdapterDisposition::PartiallyNormalized;
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .unwrap()
        .reporter_activity;

    assert!(activity.paired_rates.iter().all(|row| {
        row.coverage == ReporterActivityCoverage::Partial
            && row.active_in_both_count
                == row.heard_both_count
                    + row.left_only_count
                    + row.right_only_count
                    + row.heard_neither_count
    }));
}

#[test]
fn bandless_or_absent_census_evidence_stays_coverage_unknown() {
    let (bundle, records, directions) = field_shape(false, false);
    assert!(records
        .iter()
        .any(|record| record.record_id == "legacy-bandless-row"));
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .expect("legacy activity fixture should analyze")
        .reporter_activity;

    assert!(activity.census_cycles.is_empty());
    assert!(activity.cycle_rates.iter().all(|row| {
        row.coverage
            == ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::NoCensusCoverage)
            && row.hearing_rate.is_none()
            && row.active_reporter_count == 0
    }));
    assert!(activity.paired_rates.iter().all(|row| {
        row.coverage
            == ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::NoCensusCoverage)
            && row.left_hearing_rate.is_none()
            && row.right_hearing_rate.is_none()
    }));
}

#[test]
fn truncation_caveat_propagates_to_every_derived_rate() {
    let (bundle, records, directions) = field_shape(true, true);
    let activity = summarize_bundle_with_activity(&bundle, &records, &directions)
        .expect("truncated activity fixture should analyze")
        .reporter_activity;

    assert!(activity
        .cycle_rates
        .iter()
        .all(|row| row.coverage == ReporterActivityCoverage::Truncated));
    assert!(activity
        .paired_rates
        .iter()
        .all(|row| row.coverage == ReporterActivityCoverage::Truncated));
}

fn field_shape(
    truncated: bool,
    include_summary: bool,
) -> (
    BundleContents,
    Vec<AdapterRecordV2>,
    BTreeMap<String, PathDirection>,
) {
    let mut bundle = fixture_bundle();
    bundle.events.clear();
    bundle.observations.clear();
    let template = bundle.schedule.slots[0].clone();
    bundle.schedule.slots = ["A", "B", "B"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let mut slot = template.clone();
            slot.slot_id = format!("activity-slot-{}", index + 1);
            slot.sequence_number = (index + 1) as u32;
            slot.starts_at = template.starts_at + Duration::minutes((index * 2) as i64);
            slot.antenna_label = label.to_string();
            slot
        })
        .collect();
    for (slot, heard_count) in [6, 2, 7].into_iter().enumerate() {
        for reporter in 0..heard_count {
            bundle
                .observations
                .push(tx_observation(&bundle, slot, &format!("K{reporter:03}ACT")));
        }
    }
    let bundle = normalize_bundle(bundle);
    let directions = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| (slot.slot_id.clone(), PathDirection::Transmit))
        .collect::<BTreeMap<_, _>>();
    let first_canonical = bundle.schedule.slots[0].starts_at - Duration::seconds(1);
    let last_canonical = bundle.schedule.slots[2].starts_at - Duration::seconds(1);
    let mut records = Vec::new();
    if include_summary {
        records.push(adapter_record(
            "activity-summary",
            "wspr_live_activity_census_summary",
            if truncated {
                AdapterDisposition::PartiallyNormalized
            } else {
                AdapterDisposition::Accepted
            },
            json!({
                "window_start": first_canonical,
                "window_end": last_canonical + Duration::minutes(2),
                "selected_bands": [Band::M20],
                "truncated": truncated,
                "counts": { "malformed": 0 }
            }),
        ));
        for (slot, reporters) in [(0, 0..10), (1, 0..8), (1, 20..22), (2, 0..10)] {
            let cycle_time = bundle.schedule.slots[slot].starts_at - Duration::seconds(1);
            for reporter in reporters {
                records.push(adapter_record(
                    &format!("census-{slot}-{reporter}"),
                    "wspr_live_activity_census",
                    AdapterDisposition::Accepted,
                    json!({
                        "cycle_time": cycle_time,
                        "band": Band::M20,
                        "reporter": format!("K{reporter:03}ACT"),
                        "reporter_grid": "EM12"
                    }),
                ));
            }
        }
    }
    records.push(adapter_record(
        "legacy-bandless-row",
        "wspr_live_activity_census",
        AdapterDisposition::Accepted,
        json!({
            "cycle_time": first_canonical,
            "reporter": "K999OLD",
            "reporter_grid": "FN42"
        }),
    ));
    (bundle, records, directions)
}

fn joint_shape() -> (
    BundleContents,
    Vec<AdapterRecordV2>,
    BTreeMap<String, PathDirection>,
) {
    let mut bundle = fixture_bundle();
    bundle.events.clear();
    bundle.observations.clear();
    let template = bundle.schedule.slots[0].clone();
    bundle.schedule.slots = ["A", "B", "B", "A"]
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let mut slot = template.clone();
            slot.slot_id = format!("joint-slot-{}", index + 1);
            slot.sequence_number = (index + 1) as u32;
            slot.starts_at = template.starts_at + Duration::minutes((index * 2) as i64);
            slot.antenna_label = label.to_string();
            slot
        })
        .collect();
    for (slot, reporters) in [
        (0, vec!["K000ACT", "K001ACT"]),
        (1, vec!["K000ACT", "K002ACT"]),
        (2, vec!["K000ACT", "K002ACT"]),
        (3, vec!["K000ACT", "K001ACT"]),
    ] {
        for reporter in reporters {
            bundle
                .observations
                .push(tx_observation(&bundle, slot, reporter));
        }
    }
    let bundle = normalize_bundle(bundle);
    let directions = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| (slot.slot_id.clone(), PathDirection::Transmit))
        .collect::<BTreeMap<_, _>>();
    let first_canonical = bundle.schedule.slots[0].starts_at - Duration::seconds(1);
    let last_canonical = bundle.schedule.slots[3].starts_at - Duration::seconds(1);
    let mut records = vec![adapter_record(
        "joint-summary",
        "wspr_live_activity_census_summary",
        AdapterDisposition::Accepted,
        json!({
            "window_start": first_canonical,
            "window_end": last_canonical + Duration::minutes(2),
            "selected_bands": [Band::M20],
            "truncated": false,
            "counts": { "malformed": 0 }
        }),
    )];
    for (slot, reporter) in (0..4).flat_map(|slot| (0..4).map(move |reporter| (slot, reporter))) {
        records.push(adapter_record(
            &format!("joint-census-{slot}-{reporter}"),
            "wspr_live_activity_census",
            AdapterDisposition::Accepted,
            json!({
                "cycle_time": bundle.schedule.slots[slot].starts_at - Duration::seconds(1),
                "band": Band::M20,
                "reporter": format!("K{reporter:03}ACT"),
                "reporter_grid": "EM12"
            }),
        ));
    }
    records.push(adapter_record(
        "joint-census-right-only-extra",
        "wspr_live_activity_census",
        AdapterDisposition::Accepted,
        json!({
            "cycle_time": bundle.schedule.slots[1].starts_at - Duration::seconds(1),
            "band": Band::M20,
            "reporter": "K999EXTRA",
            "reporter_grid": "FN42"
        }),
    ));
    (bundle, records, directions)
}

fn tx_observation(bundle: &BundleContents, slot_index: usize, reporter: &str) -> ObservationRecord {
    let slot = &bundle.schedule.slots[slot_index];
    let mut observation = fixture_bundle().observations[0].clone();
    observation.observation_id = format!("{}-{reporter}", slot.slot_id);
    observation.meta.timestamp = slot.starts_at + Duration::seconds(30);
    observation.meta.source = RecordSource::WsjtxLog;
    observation.observation_kind = ObservationKind::LocalDecode;
    observation.band = slot.band;
    observation.mode = Some("WSPR".into());
    observation.reporter_call = Some(reporter.into());
    observation.heard_call = Some(bundle.station.callsign.clone());
    observation.reporter_grid = Some("EM12".into());
    observation.heard_grid = Some(bundle.station.grid.clone());
    observation.snr_db = Some(-20.0);
    observation.slot_id = None;
    observation.slot_label = None;
    observation.slot_confidence = None;
    observation
}

fn adapter_record(
    record_id: &str,
    record_type: &str,
    disposition: AdapterDisposition,
    data: serde_json::Value,
) -> AdapterRecordV2 {
    AdapterRecordV2 {
        meta: RecordMetaV2 {
            schema_version: 2,
            session_id: "activity-test".into(),
            recorded_at: DateTime::<Utc>::UNIX_EPOCH,
            provenance: Provenance::from_legacy(RecordSource::WsprLive, "activity-test"),
            mutation: MutationMember {
                mutation_id: record_id.into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        record_id: record_id.into(),
        source_time: None,
        record_type: record_type.into(),
        disposition,
        reason: AdapterReasonId::new("wspr-live.activity-census").unwrap(),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: serde_json::to_string(&data).unwrap(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: Some("synthetic-activity.json".into()),
        },
    }
}

fn fixture_bundle() -> BundleContents {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    BundleStore::new(root)
        .read_normalized_validated()
        .expect("fixture bundle should be normalized and valid")
}
