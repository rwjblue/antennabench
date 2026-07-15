use std::path::PathBuf;

use antennabench_analysis::{
    summarize_bundle, ComparisonAvailability, EvidenceQuality, ObservationExclusionReason,
    ObservationKindCount,
};
use antennabench_core::{
    normalize_bundle, validate_bundle, AnalysisStatus, Band, ObservationKind, OperatorEventType,
};
use antennabench_report::{build_report, UsableObservationKindCounts};
use antennabench_storage::BundleStore;

const FIXTURE_NAME: &str = "canonical-sample-report.session.wsprabundle";
const SESSION_ID: &str = "session-canonical-sample-2026-03-14";

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
    assert_eq!(normalized.station.callsign, "N0CALL");
    assert_eq!(normalized.antennas.antennas.len(), 2);
    assert_eq!(normalized.schedule.slots.len(), 12);
    assert_eq!(normalized.events.len(), 14);
    assert_eq!(normalized.observations.len(), 26);
    assert_eq!(normalized.wsjtx.len(), 5);
    assert_eq!(normalized.rig.len(), 3);
    assert_eq!(normalized.propagation.len(), 2);
    assert_eq!(normalized.analysis.status, AnalysisStatus::NotRun);

    assert!(normalized
        .events
        .iter()
        .any(|event| event.event_type == OperatorEventType::MissedSlot));
    assert!(normalized
        .events
        .iter()
        .any(|event| event.event_type == OperatorEventType::BadSlot));
    assert!(normalized
        .observations
        .iter()
        .any(|observation| observation.observation_kind == ObservationKind::LocalDecode));
    assert!(normalized.observations.iter().any(|observation| {
        matches!(
            observation.observation_kind,
            ObservationKind::PublicReport | ObservationKind::ImportedSpot
        )
    }));
    assert!(normalized.observations.iter().any(|observation| {
        observation.frequency_hz.is_none()
            && observation.reporter_grid.is_none()
            && observation.snr_db.is_none()
    }));

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
    assert_eq!(summary.overall.observation_counts.total, 26);
    assert_eq!(summary.overall.observation_counts.usable, 19);
    assert_eq!(summary.overall.observation_counts.excluded, 7);
    assert_eq!(
        summary.overall.usable_observation_kinds,
        vec![
            ObservationKindCount {
                kind: ObservationKind::LocalDecode,
                count: 7,
            },
            ObservationKindCount {
                kind: ObservationKind::PublicReport,
                count: 8,
            },
            ObservationKindCount {
                kind: ObservationKind::ImportedSpot,
                count: 4,
            },
        ]
    );
    assert_eq!(summary.antennas.len(), 2);
    assert!(summary
        .antennas
        .iter()
        .all(|antenna| antenna.evidence_quality == EvidenceQuality::Moderate));
    assert_eq!(
        summary.comparison.availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert!(summary.comparison.paired_rows.is_empty());
    assert_eq!(
        summary
            .bands
            .iter()
            .find(|band| band.band == Band::M20)
            .expect("20m summary should exist")
            .evidence
            .observation_counts
            .usable,
        9
    );
    assert_eq!(
        summary
            .bands
            .iter()
            .find(|band| band.band == Band::M40)
            .expect("40m summary should exist")
            .evidence
            .observation_counts
            .usable,
        10
    );
    for reason in [
        ObservationExclusionReason::GuardTime,
        ObservationExclusionReason::NearBoundary,
        ObservationExclusionReason::BeforeObservedSwitch,
        ObservationExclusionReason::MissedSlot,
        ObservationExclusionReason::BadSlot,
        ObservationExclusionReason::BandMismatch,
        ObservationExclusionReason::OutsideSchedule,
    ] {
        assert_eq!(
            summary
                .overall
                .exclusions
                .iter()
                .find(|exclusion| exclusion.reason == reason)
                .map(|exclusion| exclusion.count),
            Some(1)
        );
    }

    let report = build_report(&reopened).expect("canonical sample should build report data");
    assert_eq!(report.context.session_id, SESSION_ID);
    assert_eq!(report.context.bands, vec![Band::M40, Band::M20]);
    assert_eq!(report.context.schedule.slot_count, 12);
    assert_eq!(report.evidence.evidence_quality, EvidenceQuality::Moderate);
    assert_eq!(
        report.evidence.overall.usable_observation_kinds,
        UsableObservationKindCounts {
            local_decode: 7,
            public_report: 8,
            imported_spot: 4,
        }
    );
    assert_eq!(report.chart_data.antenna_snr.len(), 2);
    assert_eq!(report.chart_data.band_evidence_counts.len(), 2);
    assert_eq!(report.chart_data.slot_evidence_counts.len(), 12);
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
