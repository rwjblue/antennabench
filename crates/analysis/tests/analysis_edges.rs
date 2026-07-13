use std::path::PathBuf;

use antennabench_analysis::{summarize_bundle, AnalysisError, EvidenceQuality};
use antennabench_core::{normalize_bundle, Band, BundleContents, ObservationKind};
use antennabench_storage::BundleStore;

#[test]
fn reports_every_exclusion_reason_exactly_once() {
    let mut bundle = fixture_bundle();
    let template = bundle.observations[0].clone();
    bundle.observations.extend([
        observation_at(
            &template,
            "obs-guard",
            "2026-07-09T20:00:10Z",
            Band::M20,
            Some(-10.0),
        ),
        observation_at(
            &template,
            "obs-boundary",
            "2026-07-09T20:01:55Z",
            Band::M20,
            Some(-11.0),
        ),
        observation_at(
            &template,
            "obs-wrong-band",
            "2026-07-09T20:00:45Z",
            Band::M40,
            Some(-12.0),
        ),
        observation_at(
            &template,
            "obs-outside",
            "2026-07-09T19:59:45Z",
            Band::M20,
            Some(-13.0),
        ),
    ]);
    let bundle = normalize_bundle(bundle);

    let summary = summarize_bundle(&bundle).expect("normalized bundle should summarize");

    assert_eq!(summary.overall.observation_counts.total, 9);
    assert_eq!(summary.overall.observation_counts.usable, 2);
    assert_eq!(summary.overall.observation_counts.excluded, 7);
    insta::assert_json_snapshot!(summary.overall.exclusions, @r#"
    [
      {
        "reason": "guard_time",
        "count": 1
      },
      {
        "reason": "near_boundary",
        "count": 1
      },
      {
        "reason": "before_observed_switch",
        "count": 1
      },
      {
        "reason": "missed_slot",
        "count": 1
      },
      {
        "reason": "bad_slot",
        "count": 1
      },
      {
        "reason": "band_mismatch",
        "count": 1
      },
      {
        "reason": "outside_schedule",
        "count": 1
      }
    ]
    "#);
}

#[test]
fn accepts_planned_no_switch_interior_evidence_and_missing_snr() {
    let bundle = bundle_with_samples(&[
        ("planned-a", "2026-07-09T20:00:30Z", Some(-20.0)),
        ("planned-b", "2026-07-09T20:02:30Z", None),
    ]);

    let summary = summarize_bundle(&bundle).expect("normalized bundle should summarize");

    assert_eq!(summary.overall.observation_counts.total, 2);
    assert_eq!(summary.overall.observation_counts.usable, 2);
    assert_eq!(summary.overall.observation_counts.excluded, 0);
    insta::assert_json_snapshot!(summary.overall, @r#"
    {
      "observation_counts": {
        "total": 2,
        "usable": 2,
        "excluded": 0
      },
      "exclusions": [],
      "usable_observation_kinds": [
        {
          "kind": "local_decode",
          "count": 2
        }
      ],
      "snr": {
        "sample_count": 1,
        "min_db": -20.0,
        "median_db": -20.0,
        "mean_db": -20.0,
        "max_db": -20.0
      }
    }
    "#);
}

#[test]
fn computes_odd_and_even_snr_statistics_without_display_rounding() {
    let odd = bundle_with_samples(&[
        ("odd-1", "2026-07-09T20:00:20Z", Some(-20.0)),
        ("odd-2", "2026-07-09T20:00:30Z", Some(-10.0)),
        ("odd-missing", "2026-07-09T20:00:40Z", None),
        ("odd-3", "2026-07-09T20:00:50Z", Some(0.0)),
    ]);
    let odd_summary = summarize_bundle(&odd).expect("odd sample should summarize");

    insta::assert_json_snapshot!(odd_summary.overall.snr, @r#"
    {
      "sample_count": 3,
      "min_db": -20.0,
      "median_db": -10.0,
      "mean_db": -10.0,
      "max_db": 0.0
    }
    "#);

    let even = bundle_with_samples(&[
        ("even-1", "2026-07-09T20:00:20Z", Some(-20.0)),
        ("even-2", "2026-07-09T20:00:30Z", Some(-10.0)),
        ("even-missing", "2026-07-09T20:00:40Z", None),
        ("even-3", "2026-07-09T20:00:50Z", Some(0.0)),
        ("even-4", "2026-07-09T20:01:00Z", Some(10.0)),
    ]);
    let even_summary = summarize_bundle(&even).expect("even sample should summarize");

    insta::assert_json_snapshot!(even_summary.overall.snr, @r#"
    {
      "sample_count": 4,
      "min_db": -20.0,
      "median_db": -5.0,
      "mean_db": -5.0,
      "max_db": 10.0
    }
    "#);
}

#[test]
fn summary_is_independent_of_observation_input_order() {
    let mut bundle = bundle_with_samples(&[
        ("order-1", "2026-07-09T20:00:30Z", Some(-18.0)),
        ("order-2", "2026-07-09T20:02:30Z", Some(-11.0)),
        ("order-3", "2026-07-09T20:04:30Z", Some(-25.0)),
        ("order-4", "2026-07-09T20:06:30Z", Some(-7.0)),
    ]);
    let forward = summarize_bundle(&bundle).expect("forward bundle should summarize");

    bundle.observations.reverse();
    let reversed = summarize_bundle(&bundle).expect("reversed bundle should summarize");

    assert_eq!(forward, reversed);
}

#[test]
fn empty_schedule_is_insufficient_without_panicking() {
    let mut bundle = fixture_bundle();
    bundle.schedule.slots.clear();
    bundle.events.clear();
    bundle.observations.clear();
    let bundle = normalize_bundle(bundle);

    let summary = summarize_bundle(&bundle).expect("empty schedule should summarize");

    assert_eq!(summary.evidence_quality, EvidenceQuality::Insufficient);
    assert!(summary.slots.is_empty());
    assert_eq!(summary.overall.observation_counts.total, 0);
}

#[test]
fn usable_observation_kinds_have_fixed_order() {
    let mut bundle = bundle_with_samples(&[
        ("kind-imported", "2026-07-09T20:00:20Z", Some(-20.0)),
        ("kind-local", "2026-07-09T20:00:30Z", Some(-19.0)),
        ("kind-public", "2026-07-09T20:00:40Z", Some(-18.0)),
    ]);
    bundle.observations[0].observation_kind = ObservationKind::ImportedSpot;
    bundle.observations[1].observation_kind = ObservationKind::LocalDecode;
    bundle.observations[2].observation_kind = ObservationKind::PublicReport;

    let summary = summarize_bundle(&bundle).expect("observation kinds should summarize");

    assert_eq!(
        summary
            .overall
            .usable_observation_kinds
            .iter()
            .map(|entry| entry.kind)
            .collect::<Vec<_>>(),
        vec![
            ObservationKind::LocalDecode,
            ObservationKind::PublicReport,
            ObservationKind::ImportedSpot,
        ]
    );
}

#[test]
fn rejects_stale_annotations_instead_of_normalizing_them() {
    let mut bundle = fixture_bundle();
    bundle.observations[0].slot_label = Some("B".to_string());

    let error = summarize_bundle(&bundle).expect_err("stale annotation must fail validation");

    assert!(matches!(error, AnalysisError::InvalidBundle(_)));
}

#[test]
fn rejects_non_finite_snr_with_the_observation_id() {
    let mut bundle = fixture_bundle();
    bundle.observations[0].snr_db = Some(f32::NAN);

    let error = summarize_bundle(&bundle).expect_err("non-finite SNR must be rejected");

    assert!(matches!(
        error,
        AnalysisError::NonFiniteSnr { observation_id }
            if observation_id == "obs-001"
    ));
}

#[test]
fn applies_insufficient_weak_and_moderate_quality_thresholds() {
    let insufficient = bundle_with_samples(&[
        ("a-only", "2026-07-09T20:00:30Z", Some(-10.0)),
        ("b-only", "2026-07-09T20:02:30Z", Some(-11.0)),
    ]);
    assert_eq!(
        summarize_bundle(&insufficient)
            .expect("insufficient sample should summarize")
            .evidence_quality,
        EvidenceQuality::Insufficient
    );

    let weak = bundle_with_samples(&[
        ("weak-a-1", "2026-07-09T20:00:30Z", Some(-10.0)),
        ("weak-b-1", "2026-07-09T20:02:30Z", Some(-11.0)),
        ("weak-a-2", "2026-07-09T20:04:30Z", Some(-12.0)),
        ("weak-b-2", "2026-07-09T20:06:30Z", Some(-13.0)),
    ]);
    assert_eq!(
        summarize_bundle(&weak)
            .expect("weak sample should summarize")
            .evidence_quality,
        EvidenceQuality::Weak
    );

    let uneven = bundle_with_samples(&[
        ("uneven-a-1", "2026-07-09T20:00:30Z", Some(-10.0)),
        ("uneven-b-1", "2026-07-09T20:02:30Z", Some(-11.0)),
        ("uneven-a-2", "2026-07-09T20:04:30Z", Some(-12.0)),
    ]);
    let uneven_summary = summarize_bundle(&uneven).expect("uneven sample should summarize");
    assert_eq!(
        uneven_summary.evidence_quality,
        EvidenceQuality::Insufficient
    );
    assert_eq!(
        uneven_summary
            .antennas
            .iter()
            .map(|antenna| antenna.evidence_quality)
            .collect::<Vec<_>>(),
        vec![EvidenceQuality::Weak, EvidenceQuality::Insufficient]
    );

    let moderate = moderate_bundle();
    let summary = summarize_bundle(&moderate).expect("moderate sample should summarize");
    assert_eq!(summary.evidence_quality, EvidenceQuality::Moderate);
    assert!(summary
        .antennas
        .iter()
        .all(|antenna| antenna.evidence_quality == EvidenceQuality::Moderate));
}

fn bundle_with_samples(samples: &[(&str, &str, Option<f32>)]) -> BundleContents {
    let mut bundle = fixture_bundle();
    let template = bundle.observations[0].clone();
    bundle.events.clear();
    bundle.observations = samples
        .iter()
        .map(|(id, timestamp, snr)| observation_at(&template, id, timestamp, Band::M20, *snr))
        .collect();
    normalize_bundle(bundle)
}

fn moderate_bundle() -> BundleContents {
    let mut bundle = fixture_bundle();
    bundle.events.clear();

    let mut slot_5 = bundle.schedule.slots[0].clone();
    slot_5.slot_id = "slot-005".to_string();
    slot_5.sequence_number = 5;
    slot_5.starts_at = "2026-07-09T20:08:00Z".parse().expect("valid timestamp");
    let mut slot_6 = bundle.schedule.slots[1].clone();
    slot_6.slot_id = "slot-006".to_string();
    slot_6.sequence_number = 6;
    slot_6.starts_at = "2026-07-09T20:10:00Z".parse().expect("valid timestamp");
    bundle.schedule.slots.extend([slot_5, slot_6]);

    let template = bundle.observations[0].clone();
    bundle.observations = [
        ("moderate-a-1", "2026-07-09T20:00:20Z"),
        ("moderate-a-2", "2026-07-09T20:00:40Z"),
        ("moderate-b-1", "2026-07-09T20:02:20Z"),
        ("moderate-b-2", "2026-07-09T20:02:40Z"),
        ("moderate-a-3", "2026-07-09T20:04:30Z"),
        ("moderate-b-3", "2026-07-09T20:06:30Z"),
        ("moderate-a-4", "2026-07-09T20:08:20Z"),
        ("moderate-a-5", "2026-07-09T20:08:40Z"),
        ("moderate-b-4", "2026-07-09T20:10:20Z"),
        ("moderate-b-5", "2026-07-09T20:10:40Z"),
    ]
    .iter()
    .enumerate()
    .map(|(index, (id, timestamp))| {
        let mut observation = observation_at(
            &template,
            id,
            timestamp,
            Band::M20,
            Some(-20.0 + index as f32),
        );
        observation.observation_kind = if index % 2 == 0 {
            ObservationKind::LocalDecode
        } else {
            ObservationKind::PublicReport
        };
        observation
    })
    .collect();

    normalize_bundle(bundle)
}

fn observation_at(
    template: &antennabench_core::ObservationRecord,
    id: &str,
    timestamp: &str,
    band: Band,
    snr_db: Option<f32>,
) -> antennabench_core::ObservationRecord {
    let mut observation = template.clone();
    observation.observation_id = id.to_string();
    observation.meta.timestamp = timestamp.parse().expect("valid timestamp");
    observation.band = band;
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
