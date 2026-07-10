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
fn invalid_jsonl_returns_parse_error_with_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let bundle_path = tempdir.path().join("invalid-jsonl.session.wsprabundle");
    std::fs::create_dir_all(bundle_path.join("attachments")).unwrap();
    write_minimal_bundle_files(&bundle_path);
    std::fs::write(bundle_path.join("events.jsonl"), "{not valid json}\n").unwrap();

    let error = BundleStore::new(&bundle_path).read().unwrap_err();

    match error {
        BundleStoreError::ParseJson { path, .. } => {
            assert!(path.ends_with("events.jsonl"));
        }
        other => panic!("expected parse JSON error, got {other:?}"),
    }
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
            assert_eq!(source.issues().len(), 1);
        }
        other => panic!("expected validation error, got {other:?}"),
    }
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
