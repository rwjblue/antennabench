use std::path::PathBuf;

use antennabench_core::{normalize_bundle, validate_bundle_report};
use antennabench_report::{
    build_report, build_report_with_resources, render_compact_summary_html,
    render_compact_summary_html_with_resources, render_standalone_html,
    render_standalone_html_with_resources, ReportCancellationToken, ReportCompleteness,
    ReportError, ReportNotice, ReportResourceLimits, REPORT_RESOURCE_LIMITS,
};
use antennabench_storage::BundleStore;

#[test]
fn report_rows_fall_back_to_a_complete_unsampled_overview() {
    let bundle = minimal_fixture_bundle();
    let validation = validate_bundle_report(&bundle);
    let full = build_report(&bundle).unwrap();
    let required_overview_rows = full.eligibility_exclusions.len()
        + full.overview.strata.len() * 41
        + full.comparison.path_summaries.len()
        + full.reporter_activity.census_cycles.len()
        + full.reporter_activity.cycle_rates.len()
        + full.reporter_activity.paired_rates.len()
        + full.reporter_activity.joint_summaries.len()
        + full
            .coverage_maps
            .iter()
            .flat_map(|group| &group.panels)
            .map(|panel| panel.cells.len() + panel.polar_cells.len())
            .sum::<usize>()
        + full.overview.timeline.len()
        + full.snapshot.operator_events.len()
        + full.overview.goal_lens.as_ref().map_or(0, |lens| {
            lens.priority.len() + lens.emphasized_distance_bins.len()
        });
    let overview = build_report_with_resources(
        &bundle,
        &validation,
        ReportResourceLimits::testing(
            required_overview_rows as u64,
            8 * 1024 * 1024,
            16 * 1024 * 1024,
        ),
        &ReportCancellationToken::default(),
    )
    .unwrap();

    assert_eq!(overview.completeness, ReportCompleteness::BoundedOverview);
    assert_eq!(overview.evidence.overall, full.evidence.overall);
    assert_eq!(overview.comparison.diagnostics, full.comparison.diagnostics);
    assert_eq!(overview.overview, full.overview);
    assert_eq!(overview.overview.timeline, full.overview.timeline);
    assert!(overview.context.schedule.slots.is_empty());
    assert!(overview.evidence.slots.is_empty());
    assert!(overview.comparison.paired_rows.is_empty());
    assert!(overview.exclusion_records.is_empty());
    assert!(overview.notices.iter().any(|notice| matches!(
        notice,
        ReportNotice::DetailOmitted { row_count, .. } if *row_count > 0
    )));
    let html = render_standalone_html(&overview).unwrap();
    assert!(html.contains("Bounded overview"));
    assert!(html.contains("no rows were sampled"));
}

#[test]
fn report_model_html_and_cancellation_boundaries_are_typed_and_never_partial() {
    let bundle = minimal_fixture_bundle();
    let validation = validate_bundle_report(&bundle);
    let full = build_report(&bundle).unwrap();
    let full_model_bytes = serde_json::to_vec(&full).unwrap().len() as u64;
    let fallback = build_report_with_resources(
        &bundle,
        &validation,
        ReportResourceLimits::testing(25_000, full_model_bytes - 1, 16 * 1024 * 1024),
        &ReportCancellationToken::default(),
    )
    .unwrap();
    assert_eq!(fallback.completeness, ReportCompleteness::BoundedOverview);

    let too_small = build_report_with_resources(
        &bundle,
        &validation,
        ReportResourceLimits::testing(25_000, 1, 16 * 1024 * 1024),
        &ReportCancellationToken::default(),
    )
    .unwrap_err();
    assert!(matches!(
        too_small,
        ReportError::Resource(ref error)
            if error.diagnostic.code == "resource.report.model_bytes"
    ));

    let html = render_standalone_html(&full).unwrap();
    let html_bytes = html.len() as u64;
    for limit in [html_bytes, html_bytes + 1] {
        let rendered = render_standalone_html_with_resources(
            &full,
            ReportResourceLimits::testing(25_000, 8 * 1024 * 1024, limit),
            &ReportCancellationToken::default(),
        )
        .unwrap();
        assert_eq!(rendered, html);
    }
    let over = render_standalone_html_with_resources(
        &full,
        ReportResourceLimits::testing(25_000, 8 * 1024 * 1024, html_bytes - 1),
        &ReportCancellationToken::default(),
    )
    .unwrap_err();
    assert!(matches!(
        over,
        ReportError::Resource(ref error)
            if error.diagnostic.code == "resource.report.html_bytes"
    ));

    let cancellation = ReportCancellationToken::default();
    cancellation.cancel();
    let cancelled =
        render_standalone_html_with_resources(&full, REPORT_RESOURCE_LIMITS, &cancellation)
            .unwrap_err();
    assert!(matches!(
        cancelled,
        ReportError::Resource(ref error)
            if error.diagnostic.code == "resource.operation.cancelled"
    ));
}

#[test]
fn compact_summary_has_the_same_typed_resource_and_cancellation_boundaries() {
    let report = build_report(&minimal_fixture_bundle()).unwrap();
    let html = render_compact_summary_html(&report).unwrap();
    assert!(html.contains("class=\"compact-summary compact-small\""));
    let bytes = html.len() as u64;
    assert_eq!(
        render_compact_summary_html_with_resources(
            &report,
            ReportResourceLimits::testing(25_000, 8 * 1024 * 1024, bytes),
            &ReportCancellationToken::default(),
        )
        .unwrap(),
        html
    );
    assert!(matches!(
        render_compact_summary_html_with_resources(
            &report,
            ReportResourceLimits::testing(25_000, 8 * 1024 * 1024, bytes - 1),
            &ReportCancellationToken::default(),
        ),
        Err(ReportError::Resource(ref error))
            if error.diagnostic.code == "resource.report.html_bytes"
    ));
    let cancellation = ReportCancellationToken::default();
    cancellation.cancel();
    assert!(matches!(
        render_compact_summary_html_with_resources(&report, REPORT_RESOURCE_LIMITS, &cancellation),
        Err(ReportError::Resource(ref error))
            if error.diagnostic.code == "resource.operation.cancelled"
    ));
}

#[test]
fn discloses_stale_annotation_without_hiding_unrelated_evidence() {
    let mut bundle = minimal_fixture_bundle();
    bundle.observations[0].slot_label = Some("B".to_string());

    let report = build_report(&bundle).expect("stale annotation should be narrowly excluded");
    let html = render_standalone_html(&report).unwrap();

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
