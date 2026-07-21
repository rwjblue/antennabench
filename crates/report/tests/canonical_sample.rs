use std::path::PathBuf;

use antennabench_analysis::{
    summarize_bundle, ComparisonAvailability, EvidenceQuality, ObservationKindCount,
};
use antennabench_core::{
    normalize_bundle, validate_bundle, AnalysisStatus, Band, ObservationKind, OperatorEventType,
    RecordSource,
};
use antennabench_report::{build_report, ReportOverviewPathDelta, UsableObservationKindCounts};
use antennabench_storage::BundleStore;

const FIXTURE_NAME: &str = "canonical-sample-report.session.wsprabundle";
const SESSION_ID: &str = "session-3e698e14-13ff-4ce4-b6bb-71e66734c6e4";

#[test]
fn canonical_sample_exercises_the_complete_report_input_pipeline() {
    let fixture = fixture_path();
    let imported = BundleStore::new(&fixture)
        .read()
        .expect("canonical sample should parse");
    let normalized = normalize_bundle(imported.clone());

    assert_eq!(
        normalized, imported,
        "fixture annotations should be current"
    );
    validate_bundle(&normalized).expect("normalized canonical sample should validate");

    assert_eq!(normalized.manifest.session_id, SESSION_ID);
    assert_eq!(normalized.manifest.schema_version, 1);
    assert_eq!(normalized.station.callsign, "N1RWJ");
    assert_eq!(normalized.station.grid, "FN41fr");
    assert!(normalized.station.operator_notes.is_none());
    assert_eq!(normalized.antennas.antennas.len(), 2);
    assert_eq!(normalized.antennas.antennas[0].label, "Attic EFHW");
    assert_eq!(normalized.antennas.antennas[1].label, "DX Commander");
    assert_eq!(normalized.schedule.slots.len(), 16);
    assert_eq!(normalized.events.len(), 18);
    assert_eq!(normalized.observations.len(), 1_025);
    assert!(normalized.wsjtx.is_empty());
    assert_eq!(normalized.rig.len(), 26);
    assert!(normalized.propagation.is_empty());
    assert_eq!(normalized.analysis.status, AnalysisStatus::NotRun);
    assert!(normalized.analysis.notes.is_empty());

    assert_eq!(
        normalized
            .events
            .iter()
            .filter(|event| event.event_type == OperatorEventType::Switched)
            .count(),
        16
    );
    assert!(normalized.observations.iter().all(|observation| {
        observation.meta.source == RecordSource::WsprLive
            && observation.observation_kind == ObservationKind::ImportedSpot
            && observation.raw.is_null()
    }));
    assert!(normalized.rig.iter().all(|record| record.raw.is_null()));

    let tempdir = tempfile::tempdir().expect("temporary export directory should be created");
    let exported = tempdir.path().join(FIXTURE_NAME);
    BundleStore::new(&exported)
        .write(&normalized)
        .expect("canonical sample should export");
    let reopened = BundleStore::new(&exported)
        .read_normalized_validated()
        .expect("exported canonical sample should reopen");
    assert_eq!(reopened, normalized);

    let summary = summarize_bundle(&reopened).expect("canonical sample should analyze");
    assert_eq!(summary.evidence_quality, EvidenceQuality::Moderate);
    assert_eq!(summary.overall.observation_counts.total, 1_025);
    assert_eq!(summary.overall.observation_counts.usable, 1_025);
    assert_eq!(summary.overall.observation_counts.excluded, 0);
    assert_eq!(
        summary.overall.usable_observation_kinds,
        vec![ObservationKindCount {
            kind: ObservationKind::ImportedSpot,
            count: 1_025,
        }]
    );
    assert_eq!(summary.antennas.len(), 2);
    assert!(summary
        .antennas
        .iter()
        .all(|antenna| antenna.evidence_quality == EvidenceQuality::Moderate));
    assert_eq!(
        summary.comparison.availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
    assert_eq!(summary.comparison.paired_rows.len(), 327);
    assert_eq!(summary.comparison.blocks.len(), 8);
    assert_eq!(
        summary
            .bands
            .iter()
            .find(|band| band.band == Band::M20)
            .expect("20m summary should exist")
            .evidence
            .observation_counts
            .usable,
        1_025
    );
    assert!(summary.overall.exclusions.is_empty());

    let report = build_report(&reopened).expect("canonical sample should build report data");
    assert_eq!(report.context.session_id, SESSION_ID);
    assert_eq!(report.context.bands, vec![Band::M20]);
    assert_eq!(report.context.schedule.slot_count, 16);
    assert_eq!(report.evidence.evidence_quality, EvidenceQuality::Moderate);
    assert_eq!(
        report.evidence.overall.usable_observation_kinds,
        UsableObservationKindCounts {
            local_decode: 0,
            public_report: 0,
            imported_spot: 1_025,
        }
    );
    assert_eq!(report.chart_data.antenna_snr.len(), 2);
    assert_eq!(report.chart_data.band_evidence_counts.len(), 1);
    assert_eq!(report.chart_data.slot_evidence_counts.len(), 16);
    assert_eq!(report.overview.strata.len(), 1);
    assert_eq!(report.overview.strata[0].unique_path_count, 83);
    assert_eq!(report.overview.strata[0].paired_row_count, 327);
    assert_eq!(report.overview.strata[0].contributing_block_count, 7);
    assert_eq!(
        report.overview.strata[0].path_delta,
        ReportOverviewPathDelta::Available {
            minimum_delta_right_minus_left_db: -18.0,
            median_path_delta_right_minus_left_db: 5.0,
            maximum_delta_right_minus_left_db: 25.0,
        }
    );
    assert_eq!(
        report.comparison.availability,
        summary.comparison.availability
    );
    assert_eq!(
        report.comparison.diagnostics,
        summary.comparison.diagnostics
    );
    assert_eq!(report.comparison.blocks, summary.comparison.blocks);
    assert_eq!(
        report.comparison.timeline_rows,
        summary.comparison.timeline_rows
    );
    assert_eq!(
        report.comparison.paired_rows,
        summary.comparison.paired_rows
    );
    assert_eq!(report.solar_context, summary.solar_context);
    assert!(report.notices.is_empty());
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles")
        .join(FIXTURE_NAME)
}
