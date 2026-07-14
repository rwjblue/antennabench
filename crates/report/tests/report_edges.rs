use std::path::PathBuf;

use antennabench_core::normalize_bundle;
use antennabench_report::{build_report, render_standalone_html, ReportNotice};
use antennabench_storage::BundleStore;

#[test]
fn discloses_stale_annotation_without_hiding_unrelated_evidence() {
    let mut bundle = minimal_fixture_bundle();
    bundle.observations[0].slot_label = Some("B".to_string());

    let report = build_report(&bundle).expect("stale annotation should be narrowly excluded");
    let html = render_standalone_html(&report);

    assert_eq!(report.eligibility_exclusions.len(), 1);
    assert!(html.contains("Evidence eligibility disclosures"));
    assert!(html.contains("bundle.semantic.alignment_annotation_mismatch"));
    assert!(html.contains("Contradictory"));
}

#[test]
fn discloses_non_finite_snr_as_an_observation_exclusion() {
    let mut bundle = minimal_fixture_bundle();
    let baseline = build_report(&bundle).expect("baseline report");
    bundle.observations[0].snr_db = Some(f32::NAN);

    let report = build_report(&bundle).expect("non-finite SNR should be narrowly excluded");
    let serialized = serde_json::to_value(&report).expect("report serializes");

    assert_eq!(
        report.evidence.overall.observation_counts.total,
        bundle.observations.len()
    );
    assert_eq!(
        report.evidence.overall.observation_counts.excluded,
        baseline.evidence.overall.observation_counts.excluded + 1
    );
    assert_eq!(
        serialized["eligibility_exclusions"][0]["code"],
        "bundle.semantic.non_finite_number"
    );
    assert_eq!(
        serialized["eligibility_exclusions"][0]["category"],
        "malformed"
    );
}

#[test]
fn represents_an_empty_schedule_and_time_range_without_panicking() {
    let mut bundle = minimal_fixture_bundle();
    bundle.schedule.slots.clear();
    bundle.events.clear();
    bundle.observations.clear();
    let bundle = normalize_bundle(bundle);

    let report = build_report(&bundle).expect("empty schedule should produce a report");

    assert_eq!(report.context.scheduled_time_range, None);
    assert!(report.context.bands.is_empty());
    assert_eq!(report.context.schedule.slot_count, 0);
    assert!(report.context.schedule.slots.is_empty());
    assert_eq!(report.evidence.antennas.len(), 2);
    assert!(report.evidence.antennas.iter().all(|antenna| {
        antenna.contributing_slot_count == 0
            && antenna.evidence.observation_counts.total == 0
            && antenna.evidence.snr.is_none()
    }));
    assert!(report.evidence.bands.is_empty());
    assert!(report.evidence.slots.is_empty());
    assert_eq!(report.chart_data.antenna_snr.len(), 2);
    assert!(report
        .chart_data
        .antenna_snr
        .iter()
        .all(|row| row.usable_observation_count == 0 && row.snr.is_none()));
    assert!(report.chart_data.band_evidence_counts.is_empty());
    assert!(report.chart_data.slot_evidence_counts.is_empty());
    assert_eq!(
        report.notices,
        vec![
            ReportNotice::NoScheduledSlots,
            ReportNotice::NoUsableObservations,
            ReportNotice::NoUsableSnrSamples,
        ]
    );
}

#[test]
fn represents_usable_observations_without_snr_as_unavailable() {
    let mut bundle = minimal_fixture_bundle();
    for observation in &mut bundle.observations {
        observation.snr_db = None;
    }

    let report = build_report(&bundle).expect("missing SNR should remain valid evidence");

    assert_eq!(report.evidence.overall.observation_counts.usable, 2);
    assert_eq!(report.evidence.overall.snr, None);
    assert_eq!(report.notices, vec![ReportNotice::NoUsableSnrSamples]);
    assert_eq!(report.chart_data.antenna_snr.len(), 2);
    assert!(report
        .chart_data
        .antenna_snr
        .iter()
        .all(|row| row.usable_observation_count == 1 && row.snr.is_none()));
}

#[test]
fn complete_report_is_independent_of_observation_input_order() {
    let mut bundle = analysis_rich_fixture_bundle();
    let forward = build_report(&bundle).expect("forward bundle should produce a report");

    bundle.observations.reverse();
    let reversed = build_report(&bundle).expect("reversed bundle should produce a report");

    assert_eq!(forward, reversed);
}

fn minimal_fixture_bundle() -> antennabench_core::BundleContents {
    fixture_bundle("minimal-whole-station.session.wsprabundle")
}

fn analysis_rich_fixture_bundle() -> antennabench_core::BundleContents {
    fixture_bundle("analysis-rich-whole-station.session.wsprabundle")
}

fn fixture_bundle(name: &str) -> antennabench_core::BundleContents {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles")
        .join(name);
    BundleStore::new(root)
        .read_normalized_validated()
        .expect("fixture bundle should be normalized and valid")
}
