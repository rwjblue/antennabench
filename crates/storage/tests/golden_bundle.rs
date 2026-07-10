use std::path::PathBuf;

use antennabench_core::{
    align_schedule_slots, apply_slot_assignments, AlignedSlotStatus, AnalysisStatus,
    ExperimentMode, ObservationKind, RecordSource, SessionGoal, SlotAlignmentPolicy,
};
use antennabench_storage::BundleStore;

const SESSION_ID: &str = "session-2026-07-09-n1rwj-20m";

#[test]
fn imports_exports_and_regenerates_minimal_whole_station_alignment() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");

    let imported = BundleStore::new(&fixture).read_validated().unwrap();
    let normalized_imported = BundleStore::new(&fixture)
        .read_normalized_validated()
        .unwrap();
    assert_eq!(normalized_imported, imported);

    assert_eq!(imported.manifest.schema_version, 1);
    assert_eq!(imported.manifest.session_id, SESSION_ID);
    assert_eq!(imported.station.callsign, "N1RWJ");
    assert_eq!(imported.station.grid, "FN42");
    assert_eq!(imported.antennas.antennas.len(), 2);
    assert_eq!(imported.schedule.mode, ExperimentMode::WholeStationAb);
    assert_eq!(imported.schedule.goal, SessionGoal::GeneralCoverage);
    assert_eq!(imported.schedule.slots.len(), 4);
    assert_eq!(imported.events.len(), 4);
    assert_eq!(imported.observations.len(), 5);
    assert_eq!(
        imported.observations[0].observation_kind,
        ObservationKind::LocalDecode
    );
    assert_eq!(imported.observations[1].meta.source, RecordSource::Wsprnet);
    assert_eq!(imported.wsjtx.len(), 1);
    assert_eq!(imported.rig.len(), 1);
    assert_eq!(imported.propagation.len(), 1);
    assert_eq!(imported.analysis.status, AnalysisStatus::NotRun);

    let alignment = align_schedule_slots(
        &imported.schedule,
        &imported.events,
        &imported.observations,
        SlotAlignmentPolicy::default(),
    );
    assert_eq!(alignment.slots[0].status, AlignedSlotStatus::Switched);
    assert_eq!(alignment.slots[1].status, AlignedSlotStatus::Bad);
    assert_eq!(alignment.slots[2].status, AlignedSlotStatus::Missed);
    assert_eq!(alignment.slots[3].status, AlignedSlotStatus::LateSwitch);

    let regenerated_observations =
        apply_slot_assignments(&imported.observations, &alignment.observation_assignments);
    assert_eq!(regenerated_observations, imported.observations);

    let tempdir = tempfile::tempdir().unwrap();
    let exported = tempdir.path().join("exported.session.wsprabundle");
    BundleStore::new(&exported).write(&imported).unwrap();

    let reimported = BundleStore::new(&exported).read().unwrap();

    assert_eq!(reimported, imported);
}

#[test]
fn imports_exports_and_regenerates_wsjtx_import_hardening_bundle() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/wsjtx-import-hardening.session.wsprabundle");

    let imported = BundleStore::new(&fixture).read_validated().unwrap();
    let normalized_imported = BundleStore::new(&fixture)
        .read_normalized_validated()
        .unwrap();
    assert_eq!(normalized_imported, imported);

    assert_eq!(imported.manifest.schema_version, 1);
    assert_eq!(
        imported.manifest.session_id,
        "session-wsjtx-import-hardening"
    );
    assert_eq!(imported.station.callsign, "N1RWJ");
    assert_eq!(imported.station.grid, "FN42");
    assert_eq!(imported.wsjtx.len(), 14);
    assert_eq!(imported.observations.len(), 3);
    assert!(imported
        .observations
        .iter()
        .all(|observation| observation.observation_kind == ObservationKind::LocalDecode));
    assert!(imported
        .observations
        .iter()
        .all(|observation| observation.meta.source == RecordSource::WsjtxLog));

    let decode_records = imported
        .wsjtx
        .iter()
        .filter(|record| record.message_type == "all_wspr_decode")
        .count();
    let malformed_records = imported
        .wsjtx
        .iter()
        .filter(|record| record.message_type == "all_wspr_malformed")
        .count();
    assert_eq!(decode_records, 3);
    assert_eq!(malformed_records, 11);

    let malformed_observation_ids: Vec<&str> = imported
        .observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.observation_id.as_str(),
                "edge-cases-obs-000004"
                    | "edge-cases-obs-000005"
                    | "edge-cases-obs-000006"
                    | "edge-cases-obs-000007"
                    | "edge-cases-obs-000008"
                    | "edge-cases-obs-000009"
                    | "edge-cases-obs-000010"
                    | "edge-cases-obs-000011"
                    | "edge-cases-obs-000012"
                    | "edge-cases-obs-000013"
                    | "edge-cases-obs-000014"
            )
        })
        .map(|observation| observation.observation_id.as_str())
        .collect();
    assert!(
        malformed_observation_ids.is_empty(),
        "malformed rows must not produce observations"
    );

    let tempdir = tempfile::tempdir().unwrap();
    let exported = tempdir
        .path()
        .join("wsjtx-import-hardening-exported.session.wsprabundle");
    BundleStore::new(&exported).write(&imported).unwrap();

    let reimported = BundleStore::new(&exported).read().unwrap();

    assert_eq!(reimported, imported);
    assert_eq!(reimported.wsjtx, imported.wsjtx);
    assert_eq!(reimported.observations, imported.observations);
}
