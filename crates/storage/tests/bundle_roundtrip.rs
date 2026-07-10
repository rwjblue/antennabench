use antennabench_core::{
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents, BundleFiles,
    BundleManifest, ExperimentMode, ObservationKind, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, RecordMeta, RecordSource, Schedule, SessionGoal, Station,
};
use antennabench_storage::{BundleStore, BundleStoreError};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-2026-07-09-n1rwj-20m";

#[test]
fn writes_and_reads_bundle_contents() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let store = BundleStore::new(&path);

    store.write(&bundle).unwrap();
    let round_tripped = store.read().unwrap();

    assert_eq!(round_tripped, bundle);
    assert!(path.join("manifest.json").is_file());
    assert!(path.join("station.json").is_file());
    assert!(path.join("antennas.json").is_file());
    assert!(path.join("schedule.json").is_file());
    assert!(path.join("events.jsonl").is_file());
    assert!(path.join("observations.jsonl").is_file());
    assert!(path.join("attachments").is_dir());
}

#[test]
fn write_uses_fixed_manifest_bootstrap_path() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.manifest = "custom-manifest.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let store = BundleStore::new(&path);

    store.write(&bundle).unwrap();
    let round_tripped = store.read().unwrap();

    assert_eq!(round_tripped, bundle);
    assert!(path.join("manifest.json").is_file());
    assert!(!path.join("custom-manifest.json").exists());
}

#[test]
fn write_rejects_paths_outside_bundle() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.station = "../station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "../station.json");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
    assert!(!tempdir.path().join("station.json").exists());
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_absolute_bundle_file_paths() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.station = "/tmp/station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "/tmp/station.json");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_nested_bundle_file_paths_before_writing() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.station = "metadata/station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "metadata/station.json");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn read_rejects_manifest_paths_outside_bundle() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let mut bundle = sample_bundle();
    bundle.manifest.files.station = "../station.json".to_string();
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("manifest.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&bundle.manifest).unwrap()
        ),
    )
    .unwrap();

    let error = BundleStore::new(&path).read().unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "../station.json");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
}

#[test]
fn read_rejects_absolute_manifest_paths() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let mut bundle = sample_bundle();
    bundle.manifest.files.station = "/tmp/station.json".to_string();
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("manifest.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&bundle.manifest).unwrap()
        ),
    )
    .unwrap();

    let error = BundleStore::new(&path).read().unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "/tmp/station.json");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
}

#[test]
fn read_rejects_manifest_attachment_path_outside_bundle() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let mut bundle = sample_bundle();
    bundle.manifest.files.attachments_dir = "../attachments".to_string();
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("manifest.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&bundle.manifest).unwrap()
        ),
    )
    .unwrap();

    let error = BundleStore::new(&path).read().unwrap_err();

    match error {
        BundleStoreError::InvalidBundlePath { path } => {
            assert_eq!(path, "../attachments");
        }
        other => panic!("expected invalid bundle path, got {other:?}"),
    }
}

#[test]
fn read_rejects_missing_attachment_directory() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let store = BundleStore::new(&path);
    store.write(&bundle).unwrap();
    std::fs::remove_dir(path.join("attachments")).unwrap();

    let error = store.read().unwrap_err();

    match error {
        BundleStoreError::InvalidAttachmentsDirectory { path } => {
            assert!(path.ends_with("attachments"));
        }
        other => panic!("expected invalid attachments directory, got {other:?}"),
    }
}

#[test]
fn write_rejects_duplicate_file_paths_before_writing() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.antennas = "station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::DuplicateBundlePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected duplicate bundle path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_manifest_path_colliding_with_file_path() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.manifest = "station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::DuplicateBundlePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected duplicate bundle path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_attachment_path_colliding_with_file_path() {
    let mut bundle = sample_bundle();
    bundle.manifest.files.attachments_dir = "station.json".to_string();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::DuplicateBundlePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected duplicate bundle path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_attachment_file_before_writing() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("attachments"), "not a directory").unwrap();

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidAttachmentsDirectory { path } => {
            assert!(path.ends_with("attachments"));
        }
        other => panic!("expected invalid attachments directory, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[test]
fn write_rejects_existing_directory_at_file_path_before_writing() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    std::fs::create_dir_all(path.join("station.json")).unwrap();

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundleFilePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected invalid bundle file path, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn write_rejects_existing_symlink_at_file_path_before_writing() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let outside = tempdir.path().join("outside.txt");
    std::fs::write(&outside, "outside").unwrap();
    std::fs::create_dir_all(&path).unwrap();
    std::os::unix::fs::symlink(&outside, path.join("station.json")).unwrap();

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundleFilePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected invalid bundle file path, got {other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
    assert!(!path.join("manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn read_rejects_existing_symlink_at_file_path() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let outside = tempdir.path().join("outside.txt");
    let store = BundleStore::new(&path);
    store.write(&bundle).unwrap();
    std::fs::write(&outside, "outside").unwrap();
    std::fs::remove_file(path.join("station.json")).unwrap();
    std::os::unix::fs::symlink(&outside, path.join("station.json")).unwrap();

    let error = store.read().unwrap_err();

    match error {
        BundleStoreError::InvalidBundleFilePath { path } => {
            assert!(path.ends_with("station.json"));
        }
        other => panic!("expected invalid bundle file path, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn write_rejects_existing_symlink_at_attachments_path_before_writing() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let outside = tempdir.path().join("outside-attachments");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    std::os::unix::fs::symlink(&outside, path.join("attachments")).unwrap();

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidAttachmentsDirectory { path } => {
            assert!(path.ends_with("attachments"));
        }
        other => panic!("expected invalid attachments directory, got {other:?}"),
    }
    assert!(!path.join("manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn read_rejects_existing_symlink_at_attachments_path() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path().join("example.session.wsprabundle");
    let outside = tempdir.path().join("outside-attachments");
    let store = BundleStore::new(&path);
    store.write(&bundle).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::remove_dir(path.join("attachments")).unwrap();
    std::os::unix::fs::symlink(&outside, path.join("attachments")).unwrap();

    let error = store.read().unwrap_err();

    match error {
        BundleStoreError::InvalidAttachmentsDirectory { path } => {
            assert!(path.ends_with("attachments"));
        }
        other => panic!("expected invalid attachments directory, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn write_rejects_symlinked_bundle_root_before_writing() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let outside = tempdir.path().join("outside");
    let path = tempdir.path().join("example.session.wsprabundle");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();

    let error = BundleStore::new(&path).write(&bundle).unwrap_err();

    match error {
        BundleStoreError::InvalidBundleRoot { path } => {
            assert!(path.ends_with("example.session.wsprabundle"));
        }
        other => panic!("expected invalid bundle root, got {other:?}"),
    }
    assert!(!outside.join("manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn read_rejects_symlinked_bundle_root() {
    let bundle = sample_bundle();
    let tempdir = tempfile::tempdir().unwrap();
    let outside = tempdir.path().join("outside.session.wsprabundle");
    let path = tempdir.path().join("example.session.wsprabundle");
    BundleStore::new(&outside).write(&bundle).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();

    let error = BundleStore::new(&path).read().unwrap_err();

    match error {
        BundleStoreError::InvalidBundleRoot { path } => {
            assert!(path.ends_with("example.session.wsprabundle"));
        }
        other => panic!("expected invalid bundle root, got {other:?}"),
    }
}

fn sample_bundle() -> BundleContents {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 0).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 7, 9, 19, 58, 0).unwrap(),
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            antennas: vec![
                Antenna {
                    label: "A".to_string(),
                    facets: vec!["vertical".to_string()],
                    height_m: None,
                    radial_count: None,
                    radial_length_m: None,
                    orientation_degrees: None,
                    tuner: None,
                    feedline: None,
                    notes: None,
                },
                Antenna {
                    label: "B".to_string(),
                    facets: vec!["dipole".to_string()],
                    height_m: None,
                    radial_count: None,
                    radial_length_m: None,
                    orientation_degrees: None,
                    tuner: None,
                    feedline: None,
                    notes: None,
                },
            ],
        },
        schedule: Schedule {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: vec![
                PlannedSlot {
                    slot_id: "slot-001".to_string(),
                    sequence_number: 1,
                    starts_at,
                    duration_seconds: 120,
                    guard_seconds: 15,
                    band: Band::M20,
                    antenna_label: "A".to_string(),
                },
                PlannedSlot {
                    slot_id: "slot-002".to_string(),
                    sequence_number: 2,
                    starts_at: starts_at + chrono::Duration::seconds(135),
                    duration_seconds: 120,
                    guard_seconds: 15,
                    band: Band::M20,
                    antenna_label: "B".to_string(),
                },
            ],
        },
        events: vec![OperatorEvent {
            meta: RecordMeta {
                schema_version: 1,
                session_id: SESSION_ID.to_string(),
                timestamp: starts_at,
                source: RecordSource::Operator,
            },
            event_id: "event-001".to_string(),
            slot_id: Some("slot-001".to_string()),
            event_type: OperatorEventType::Switched,
            note: None,
        }],
        observations: vec![ObservationRecord {
            meta: RecordMeta {
                schema_version: 1,
                session_id: SESSION_ID.to_string(),
                timestamp: starts_at,
                source: RecordSource::WsjtxLog,
            },
            observation_id: "observation-001".to_string(),
            observation_kind: ObservationKind::LocalDecode,
            band: Band::M20,
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".to_string()),
            reporter_call: Some("N1RWJ".to_string()),
            heard_call: Some("K1ABC".to_string()),
            reporter_grid: Some("FN42".to_string()),
            heard_grid: Some("EM12".to_string()),
            distance_km: Some(2500.0),
            azimuth_degrees: Some(250.0),
            snr_db: Some(-18.0),
            drift_hz_per_minute: Some(0.0),
            power_watts: Some(5.0),
            slot_id: Some("slot-001".to_string()),
            slot_label: Some("A".to_string()),
            slot_confidence: Some(0.95),
            raw: json!({ "line": "example wsjtx decode" }),
        }],
        wsjtx: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: vec!["analysis engine not part of the first slice".to_string()],
        },
    }
}
