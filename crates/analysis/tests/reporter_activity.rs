use std::{collections::BTreeMap, path::PathBuf};

use antennabench_analysis::{
    summarize_bundle_with_activity, PathDirection, ReporterActivityCoverage,
    ReporterActivityUnknownReason,
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
