use std::path::PathBuf;

use antennabench_core::{
    align_schedule_slots, apply_slot_assignments, validate_bundle, AlignedSlotStatus,
    AnalysisStatus, ExperimentMode, ObservationKind, RecordSource, SessionGoal, SlotAlignmentPolicy,
};
use antennabench_storage::BundleStore;

const SESSION_ID: &str = "session-2026-07-09-n1rwj-20m";

#[test]
fn imports_exports_and_regenerates_minimal_whole_station_alignment() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");

    let imported = BundleStore::new(&fixture).read().unwrap();
    validate_bundle(&imported).unwrap();

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
