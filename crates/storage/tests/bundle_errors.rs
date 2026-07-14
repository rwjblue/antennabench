use std::path::PathBuf;

use antennabench_core::{codes, normalize_bundle, BundleFileRole, BundleValidationProfile};
use antennabench_storage::{BundleStore, BundleStoreError};

#[test]
fn missing_manifest_returns_read_error_with_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("missing.session.wsprabundle");
    std::fs::create_dir_all(&bundle_path).unwrap();

    let error = BundleStore::new(&bundle_path).read().unwrap_err();

    match error {
        BundleStoreError::Read { path, .. } => {
            assert!(path.ends_with("manifest.json"));
        }
        other => panic!("expected read error, got {other:?}"),
    }
}

#[test]
fn invalid_jsonl_returns_typed_diagnostic_with_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("invalid-jsonl.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    std::fs::write(bundle_path.join("events.jsonl"), "{not valid json}\n").unwrap();

    let error = BundleStore::new(&bundle_path).read().unwrap_err();

    let BundleStoreError::Validation { source } = error else {
        panic!("expected validation error, got {error:?}");
    };
    let diagnostic = &source.report().diagnostics()[0];
    assert_eq!(diagnostic.code, codes::INVALID_JSON);
    assert_eq!(diagnostic.location.file, BundleFileRole::Events);
    assert_eq!(diagnostic.location.record_index, Some(0));
    assert_eq!(diagnostic.location.physical_line, Some(1));
}

#[test]
fn read_validated_returns_validation_error_for_parseable_invalid_bundle() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("invalid-bundle.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    std::fs::write(
        bundle_path.join("events.jsonl"),
        r#"{"meta":{"schema_version":1,"session_id":"session-invalid-jsonl","timestamp":"2026-07-09T20:00:00Z","source":"operator"},"event_id":"event-001","slot_id":"missing-slot","event_type":"switched","note":null}
"#,
    )
    .unwrap();

    let error = BundleStore::new(&bundle_path).read_validated().unwrap_err();

    match error {
        BundleStoreError::Validation { source } => {
            assert_eq!(source.diagnostic_count(), 1);
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn unknown_modeled_fields_are_visible_but_compatible_with_read_and_analysis() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("unknown-field.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    let station_path = bundle_path.join("station.json");
    let station = std::fs::read_to_string(&station_path).unwrap().replace(
        "  \"operator_notes\": null",
        "  \"operator_notes\": null,\n  \"future_field\": 42",
    );
    std::fs::write(&station_path, station).unwrap();

    let store = BundleStore::new(&bundle_path);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_some());
    assert_eq!(inspection.report().diagnostics().len(), 1);
    let diagnostic = &inspection.report().diagnostics()[0];
    assert_eq!(diagnostic.code, codes::UNKNOWN_FIELD);
    assert_eq!(diagnostic.location.file, BundleFileRole::Station);
    assert_eq!(
        diagnostic.location.field_path.as_deref(),
        Some("/future_field")
    );
    assert!(inspection
        .report()
        .allows(BundleValidationProfile::CompatibilityRead));
    assert!(inspection
        .report()
        .allows(BundleValidationProfile::Analysis));
    assert!(!inspection
        .report()
        .allows(BundleValidationProfile::StrictCreation));
    assert!(!inspection.report().allows(BundleValidationProfile::Upgrade));
    assert_eq!(store.read().unwrap().station.callsign, "N1RWJ");
    store.read_normalized_validated().unwrap();
    assert!(matches!(
        store.read_validated(),
        Err(BundleStoreError::Validation { .. })
    ));

    let destination = tempdir
        .path()
        .join("unknown-field-copy.session.wsprabundle");
    store.copy_losslessly_to(&destination).unwrap();
    assert_eq!(
        std::fs::read(&station_path).unwrap(),
        std::fs::read(destination.join("station.json")).unwrap()
    );
}

#[test]
fn duplicate_modeled_members_block_typed_projection_without_being_collapsed() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("duplicate-member.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    let station_path = bundle_path.join("station.json");
    let station = std::fs::read_to_string(&station_path).unwrap().replace(
        "  \"callsign\": \"N1RWJ\",",
        "  \"callsign\": \"FIRST\",\n  \"callsign\": \"SECOND\",",
    );
    std::fs::write(&station_path, station).unwrap();

    let store = BundleStore::new(&bundle_path);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_none());
    let duplicate = inspection
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::DUPLICATE_MEMBER)
        .expect("duplicate member diagnostic");
    assert_eq!(duplicate.location.file, BundleFileRole::Station);
    assert_eq!(duplicate.location.field_path.as_deref(), Some("/callsign"));
    for profile in [
        BundleValidationProfile::CompatibilityRead,
        BundleValidationProfile::Analysis,
        BundleValidationProfile::StrictCreation,
        BundleValidationProfile::Upgrade,
    ] {
        assert!(!inspection.report().allows(profile));
    }
    assert!(matches!(
        store.read(),
        Err(BundleStoreError::Validation { .. })
    ));
}

#[test]
fn read_normalized_validated_repairs_stale_alignment_annotations_before_validation() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let expected = BundleStore::new(&fixture).read_validated().unwrap();
    let mut stale = expected.clone();

    let missing_annotation = observation_mut(&mut stale, "obs-001");
    missing_annotation.slot_id = None;
    missing_annotation.slot_label = None;
    missing_annotation.slot_confidence = None;

    let stale_annotation = observation_mut(&mut stale, "obs-005");
    stale_annotation.slot_id = Some("slot-001".to_string());
    stale_annotation.slot_label = Some("A".to_string());
    stale_annotation.slot_confidence = Some(0.95);

    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("stale.session.wsprabundle");
    BundleStore::new(&bundle_path).write(&stale).unwrap();

    let error = BundleStore::new(&bundle_path).read_validated().unwrap_err();
    match error {
        BundleStoreError::Validation { .. } => {}
        other => panic!("expected validation error, got {other:?}"),
    }

    let repaired = BundleStore::new(&bundle_path)
        .read_normalized_validated()
        .unwrap();
    assert_eq!(repaired, normalize_bundle(stale));
    assert_eq!(repaired, expected);
}

fn observation_mut<'a>(
    bundle: &'a mut antennabench_core::BundleContents,
    observation_id: &str,
) -> &'a mut antennabench_core::ObservationRecord {
    bundle
        .observations
        .iter_mut()
        .find(|observation| observation.observation_id == observation_id)
        .unwrap_or_else(|| panic!("missing observation {observation_id}"))
}

fn write_minimal_bundle_files(bundle_path: &std::path::Path) {
    std::fs::write(
        bundle_path.join("manifest.json"),
        r#"{
  "schema_version": 1,
  "session_id": "session-invalid-jsonl",
  "created_at": "2026-07-09T19:58:00Z",
  "app_version": "0.1.0",
  "files": {
    "manifest": "manifest.json",
    "station": "station.json",
    "antennas": "antennas.json",
    "schedule": "schedule.json",
    "events": "events.jsonl",
    "observations": "observations.jsonl",
    "wsjtx": "wsjtx.jsonl",
    "rig": "rig.jsonl",
    "propagation": "propagation.jsonl",
    "analysis": "analysis.json",
    "attachments_dir": "attachments"
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("station.json"),
        r#"{
  "schema_version": 1,
  "session_id": "session-invalid-jsonl",
  "callsign": "N1RWJ",
  "grid": "FN42",
  "power_watts": 5.0,
  "operator_notes": null
}
"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("antennas.json"),
        r#"{
  "schema_version": 1,
  "session_id": "session-invalid-jsonl",
  "antennas": []
}
"#,
    )
    .unwrap();
    std::fs::write(
        bundle_path.join("schedule.json"),
        r#"{
  "schema_version": 1,
  "session_id": "session-invalid-jsonl",
  "mode": "whole_station_ab",
  "goal": "general_coverage",
  "slots": []
}
"#,
    )
    .unwrap();
    std::fs::write(bundle_path.join("observations.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("events.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("wsjtx.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("rig.jsonl"), "").unwrap();
    std::fs::write(bundle_path.join("propagation.jsonl"), "").unwrap();
    std::fs::write(
        bundle_path.join("analysis.json"),
        r#"{
  "schema_version": 1,
  "session_id": "session-invalid-jsonl",
  "generated_at": null,
  "status": "not_run",
  "notes": []
}
"#,
    )
    .unwrap();
}
