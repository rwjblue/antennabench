use std::{
    fs, io,
    path::{Path, PathBuf},
};

use antennabench_core::{
    codes,
    v2::{
        AdapterDisposition, AdapterInput, BundleManifestV2, OperatorEventPayloadV2,
        V2_BUNDLE_SUFFIX,
    },
    SCHEMA_VERSION_V2,
};
use antennabench_storage::{BundleAttachment, BundleStore, BundleStoreError};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles")
}

#[test]
fn upgrades_every_checked_in_v1_fixture_without_changing_source_bytes() {
    let mut fixtures = fs::read_dir(fixtures_root())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".session.wsprabundle"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert!(!fixtures.is_empty());

    for source in fixtures {
        let before = snapshot_files(&source).unwrap();
        let v1 = BundleStore::new(&source).read().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join(format!(
            "upgraded-{}{}",
            source.file_stem().unwrap().to_string_lossy(),
            V2_BUNDLE_SUFFIX
        ));
        let upgraded = BundleStore::new(&source)
            .upgrade_v1_to_v2(&destination)
            .unwrap_or_else(|error| panic!("upgrade {}: {error:#}", source.display()));

        assert_eq!(snapshot_files(&source).unwrap(), before);
        let current = upgraded.read_current().unwrap();
        assert_eq!(current.bundle.manifest.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(current.bundle.station.callsign, v1.station.callsign);
        assert_eq!(current.bundle.schedule.slots, v1.schedule.slots);
        assert_eq!(current.bundle.observations.len(), v1.observations.len());
        assert_eq!(current.bundle.rig.len(), v1.rig.len());
        assert_eq!(current.bundle.propagation.len(), v1.propagation.len());
        assert!(current.session_state.is_some());
        assert!(
            current
                .adapter_records
                .iter()
                .any(|record| record.disposition == AdapterDisposition::Malformed)
                || !v1
                    .wsjtx
                    .iter()
                    .any(|record| record.message_type.contains("malformed"))
        );
        assert!(current.bundle.observations.iter().all(|observation| current
            .adapter_records
            .iter()
            .any(|adapter| {
                adapter
                    .normalized_records
                    .iter()
                    .any(|link| link.record_id == observation.observation_id)
            })));

        let source_lines = fs::read_to_string(source.join("wsjtx.jsonl"))
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let migrated_lines = current
            .adapter_records
            .iter()
            .filter(|record| record.record_id.starts_with("legacy-wsjtx-"))
            .map(|record| match &record.input {
                AdapterInput::Inline { data, .. } => data.trim_end_matches('\n').to_string(),
                AdapterInput::Attachment { .. } => panic!("v1 WSJT-X line should remain inline"),
            })
            .collect::<Vec<_>>();
        assert_eq!(migrated_lines, source_lines);
        let migrated_shapes = current
            .adapter_records
            .iter()
            .filter(|record| record.record_id.starts_with("legacy-wsjtx-"))
            .map(|record| (record.record_type.as_str(), record.disposition))
            .collect::<Vec<_>>();
        let legacy_shapes = v1
            .wsjtx
            .iter()
            .map(|record| {
                (
                    record.message_type.as_str(),
                    if record.message_type.contains("malformed") {
                        AdapterDisposition::Malformed
                    } else {
                        AdapterDisposition::Accepted
                    },
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(migrated_shapes, legacy_shapes);

        let copied = temp.path().join(format!("copy{V2_BUNDLE_SUFFIX}"));
        upgraded.copy_losslessly_to(&copied).unwrap();
        assert_eq!(
            snapshot_files(&destination).unwrap(),
            snapshot_files(&copied).unwrap()
        );
        BundleStore::new(copied).read_current().unwrap();
    }
}

#[test]
fn content_addressed_attachments_are_verified_on_read() {
    let source = fixtures_root().join("minimal-whole-station.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join(format!("attachment{V2_BUNDLE_SUFFIX}"));
    let store = BundleStore::new(source)
        .upgrade_v1_to_v2(&destination)
        .unwrap();
    let reference = store
        .write_attachment(
            b"exact compressed or binary evidence\0\xff",
            "application/octet-stream",
            None,
            Some("opaque".into()),
            Some("fixture.bin".into()),
        )
        .unwrap();
    assert_eq!(
        store.read_attachment(&reference).unwrap(),
        b"exact compressed or binary evidence\0\xff"
    );
    let path = destination
        .join("attachments/sha256")
        .join(&reference.sha256);
    fs::write(&path, b"tampered").unwrap();
    assert!(matches!(
        store.read_attachment(&reference),
        Err(BundleStoreError::AttachmentMismatch { .. })
    ));
}

#[test]
fn referenced_attachment_writes_reopen_and_copy_losslessly() {
    let source = fixtures_root().join("minimal-whole-station.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let baseline = temp.path().join(format!("baseline{V2_BUNDLE_SUFFIX}"));
    let baseline_store = BundleStore::new(source)
        .upgrade_v1_to_v2(&baseline)
        .unwrap();
    let mut bundle = baseline_store.read_v2().unwrap();
    let attachment = BundleAttachment::new(
        b"large exact source input".to_vec(),
        "application/octet-stream",
        None,
        Some("opaque".into()),
        Some("capture.bin".into()),
    );
    bundle.adapter_records[0].input = AdapterInput::Attachment {
        attachment: attachment.reference.clone(),
    };
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();

    let authored = temp.path().join(format!("authored{V2_BUNDLE_SUFFIX}"));
    let authored_store = BundleStore::new(&authored);
    authored_store
        .write_v2_with_attachments(&bundle, std::slice::from_ref(&attachment))
        .unwrap();
    authored_store.read_v2().unwrap();
    assert_eq!(
        authored_store
            .read_attachment(&attachment.reference)
            .unwrap(),
        attachment.bytes
    );

    let copied = temp.path().join(format!("authored-copy{V2_BUNDLE_SUFFIX}"));
    let copied_store = authored_store.copy_losslessly_to(&copied).unwrap();
    assert_eq!(
        snapshot_files(&authored).unwrap(),
        snapshot_files(&copied).unwrap()
    );
    assert_eq!(
        copied_store.read_attachment(&attachment.reference).unwrap(),
        attachment.bytes
    );
}

#[test]
fn unknown_schema_versions_fail_with_a_typed_error_for_copy() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("unknown.session.antennabundle");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("manifest.json"), r#"{"schema_version":99}"#).unwrap();
    let error = BundleStore::new(source)
        .copy_losslessly_to(temp.path().join("copy.session.antennabundle"))
        .unwrap_err();
    assert!(matches!(
        error,
        antennabench_storage::BundleCopyError::Source {
            source: BundleStoreError::UnsupportedSchemaVersion { actual: 99 }
        }
    ));
    assert!(matches!(
        BundleStore::new(temp.path().join("unknown.session.antennabundle")).read(),
        Err(BundleStoreError::UnsupportedSchemaVersion { actual: 99 })
    ));
}

#[test]
fn v2_manifest_uses_generic_adapter_and_checkpoint_files() {
    let source = fixtures_root().join("minimal-whole-station.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join(format!("shape{V2_BUNDLE_SUFFIX}"));
    BundleStore::new(source)
        .upgrade_v1_to_v2(&destination)
        .unwrap();
    let manifest: BundleManifestV2 =
        serde_json::from_slice(&fs::read(destination.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.files.adapter_records, "adapter-records.jsonl");
    assert_eq!(manifest.files.session_state, "session-state.json");
    assert!(!destination.join("wsjtx.jsonl").exists());
}

#[test]
fn v2_serialization_is_deterministic_and_duplicate_members_fail_closed() {
    let source = fixtures_root().join("wsjtx-import-hardening.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join(format!("first{V2_BUNDLE_SUFFIX}"));
    let second = temp.path().join(format!("second{V2_BUNDLE_SUFFIX}"));
    BundleStore::new(&source).upgrade_v1_to_v2(&first).unwrap();
    BundleStore::new(&source).upgrade_v1_to_v2(&second).unwrap();
    assert_eq!(
        snapshot_files(&first).unwrap(),
        snapshot_files(&second).unwrap()
    );

    let manifest_path = first.join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).unwrap().replacen(
        "  \"schema_version\": 2,",
        "  \"schema_version\": 2,\n  \"schema_version\": 2,",
        1,
    );
    fs::write(manifest_path, manifest).unwrap();
    let store = BundleStore::new(&first);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_none());
    assert!(inspection
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == codes::DUPLICATE_MEMBER));
    let copy = temp
        .path()
        .join(format!("ambiguous-copy{V2_BUNDLE_SUFFIX}"));
    assert!(matches!(
        store.copy_losslessly_to(&copy),
        Err(antennabench_storage::BundleCopyError::Source {
            source: BundleStoreError::AmbiguousManifest { .. }
        })
    ));
    assert!(!copy.exists());
}

#[test]
fn strict_v2_write_rejects_unknown_explicit_actual_antenna_labels() {
    let source = fixtures_root().join("analysis-rich-whole-station.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let baseline = temp.path().join(format!("baseline{V2_BUNDLE_SUFFIX}"));
    let baseline_store = BundleStore::new(source)
        .upgrade_v1_to_v2(&baseline)
        .unwrap();
    let mut bundle = baseline_store.read_v2().unwrap();
    bundle.events[0].payload = OperatorEventPayloadV2::AntennaStateConfirmed {
        antenna_label: "not-defined".into(),
        note: None,
    };
    for observation in bundle
        .observations
        .iter_mut()
        .filter(|observation| observation.slot_id.as_deref() == Some("slot-001"))
    {
        observation.slot_label = Some("not-defined".into());
    }
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();

    let authored = temp.path().join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    let error = BundleStore::new(&authored).write_v2(&bundle).unwrap_err();
    let BundleStoreError::Validation { source: validation } = error else {
        panic!("expected validation error, got {error:?}");
    };
    assert!(validation.report().diagnostics().iter().any(|diagnostic| {
        diagnostic.code == codes::UNKNOWN_ANTENNA_LABEL
            && diagnostic.message.contains("not-defined")
    }));
    assert!(!authored.exists());
}

#[test]
fn upgrade_rejects_a_destination_inside_the_source_before_writing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.session.wsprabundle");
    copy_directory(
        &fixtures_root().join("minimal-whole-station.session.wsprabundle"),
        &source,
    )
    .unwrap();
    let destination = source
        .join("attachments")
        .join(format!("nested{V2_BUNDLE_SUFFIX}"));
    let error = BundleStore::new(&source)
        .upgrade_v1_to_v2(&destination)
        .unwrap_err();
    assert!(matches!(
        error,
        antennabench_storage::BundleUpgradeError::DestinationInsideSource { .. }
    ));
    assert!(!destination.exists());
}

fn snapshot_files(root: &Path) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) -> io::Result<()> {
        for entry in fs::read_dir(current)? {
            let path = entry?.path();
            if path.is_dir() {
                visit(root, &path, output)?;
            } else {
                output.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path)?,
                ));
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(root, root, &mut output)?;
    output.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(output)
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination_path)?;
        } else {
            fs::copy(entry.path(), destination_path)?;
        }
    }
    Ok(())
}
