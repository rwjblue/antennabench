use std::{fs, io, path::Path};

use antennabench_core::{codes, BundleValidationProfile};
use antennabench_storage::{BundleCopyError, BundleStore};
use tempfile::TempDir;

fn canonical_fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/inconclusive-sample-report.session.wsprabundle")
}

fn copy_fixture(temp: &TempDir) -> std::path::PathBuf {
    let target = temp.path().join("source.session.wsprabundle");
    copy_directory(&canonical_fixture(), &target).expect("copy canonical fixture");
    target
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

fn snapshot_files(root: &Path) -> io::Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
    fn visit(
        root: &Path,
        current: &Path,
        snapshot: &mut Vec<(std::path::PathBuf, Vec<u8>)>,
    ) -> io::Result<()> {
        for entry in fs::read_dir(current)? {
            let path = entry?.path();
            if path.is_dir() {
                visit(root, &path, snapshot)?;
            } else {
                snapshot.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path)?,
                ));
            }
        }
        Ok(())
    }

    let mut snapshot = Vec::new();
    visit(root, root, &mut snapshot)?;
    snapshot.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(snapshot)
}

#[test]
fn copies_root_bytes_nested_attachments_and_reopens_without_mutating_source() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let nested = source.join("attachments/captures/2026-03-14");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("waterfall.bin"), [0, 1, 2, 0xff, 0x80]).unwrap();
    fs::write(source.join("opaque-root.bin"), [0xde, 0xad, 0xbe, 0xef]).unwrap();
    fs::create_dir(source.join("opaque-root-dir")).unwrap();
    fs::write(source.join("opaque-root-dir/evidence.txt"), b"preserve me").unwrap();

    // Make persisted derived annotations stale. Normalized import repairs these
    // in memory, while lossless copy must preserve the original bytes.
    let observations = source.join("observations.jsonl");
    let original_observations = fs::read_to_string(&observations).unwrap();
    let stale_observations = original_observations.replacen(
        "\"slot_label\":\"Vertical\"",
        "\"slot_label\":\"stale persisted label\"",
        1,
    );
    assert_ne!(stale_observations, original_observations);
    fs::write(&observations, stale_observations).unwrap();

    let before = snapshot_files(&source).unwrap();
    let destination = temp.path().join("exported.session.wsprabundle");

    let exported = BundleStore::new(&source)
        .copy_losslessly_to(&destination)
        .expect("lossless copy succeeds");

    assert_eq!(snapshot_files(&source).unwrap(), before);
    assert_eq!(snapshot_files(&destination).unwrap(), before);
    assert_eq!(
        fs::read(destination.join("attachments/captures/2026-03-14/waterfall.bin")).unwrap(),
        [0, 1, 2, 0xff, 0x80]
    );
    exported
        .read_normalized_validated()
        .expect("export reopens through canonical import path");
}

#[test]
fn copies_ambiguous_modeled_json_byte_for_byte_without_typed_projection() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let station_path = source.join("station.json");
    let station = fs::read_to_string(&station_path).unwrap().replace(
        "  \"callsign\": \"N0CALL\",",
        "  \"callsign\": \"FIRST\",\n  \"callsign\": \"SECOND\",",
    );
    fs::write(&station_path, station).unwrap();
    assert!(BundleStore::new(&source).read().is_err());
    let before = snapshot_files(&source).unwrap();
    let destination = temp.path().join("preserved-ambiguous.session.wsprabundle");

    let exported = BundleStore::new(&source)
        .copy_losslessly_to(&destination)
        .expect("storage-safe copy does not require typed projection");

    assert_eq!(snapshot_files(&destination).unwrap(), before);
    assert!(exported.read().is_err());
}

#[test]
fn copies_structurally_blocked_bundle_without_sending_it_to_analysis() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let events_path = source.join("events.jsonl");
    let events = fs::read_to_string(&events_path).unwrap().replacen(
        "\"slot_id\":\"slot-001\"",
        "\"slot_id\":\"missing-slot\"",
        1,
    );
    fs::write(&events_path, events).unwrap();
    let store = BundleStore::new(&source);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_none());
    assert!(inspection
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == codes::UNKNOWN_EVENT_SLOT));
    let before = snapshot_files(&source).unwrap();
    let destination = temp.path().join("preserved-structural.session.wsprabundle");

    store.copy_losslessly_to(&destination).unwrap();

    assert_eq!(snapshot_files(&destination).unwrap(), before);
    assert!(BundleStore::new(destination)
        .read_normalized_validated()
        .is_err());
}

#[test]
fn reports_but_preserves_duplicate_members_in_legacy_raw_evidence() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let observations_path = source.join("observations.jsonl");
    let observations = fs::read_to_string(&observations_path).unwrap().replacen(
        "\"fixture\":\"canonical-sample-report\"",
        "\"fixture\":\"first\",\"fixture\":\"second\"",
        1,
    );
    fs::write(&observations_path, observations).unwrap();

    let store = BundleStore::new(&source);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_some());
    let duplicate = inspection
        .report()
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.code == codes::DUPLICATE_RAW_MEMBER)
        .expect("duplicate raw member warning");
    assert_eq!(
        duplicate.location.field_path.as_deref(),
        Some("/raw/fixture")
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

    let before = snapshot_files(&source).unwrap();
    let destination = temp.path().join("preserved-raw.session.wsprabundle");
    store.copy_losslessly_to(&destination).unwrap();
    assert_eq!(snapshot_files(&destination).unwrap(), before);
}

#[test]
fn rejects_existing_destination_without_merging_or_overwriting() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let destination = temp.path().join("existing.session.wsprabundle");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("keep.txt"), b"do not replace").unwrap();

    let error = BundleStore::new(source)
        .copy_losslessly_to(&destination)
        .expect_err("existing destination is rejected");

    assert!(matches!(error, BundleCopyError::DestinationExists { .. }));
    assert_eq!(
        fs::read(destination.join("keep.txt")).unwrap(),
        b"do not replace"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
}

#[test]
fn rejects_destination_inside_the_source_bundle() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let destination = source.join("attachments/nested-export.session.wsprabundle");

    let error = BundleStore::new(&source)
        .copy_losslessly_to(&destination)
        .expect_err("destination inside source is rejected");

    assert!(matches!(
        error,
        BundleCopyError::DestinationInsideSource { .. }
    ));
    assert!(!destination.exists());
}

#[cfg(unix)]
#[test]
fn rejects_nested_attachment_symlink_and_cleans_incomplete_destination() {
    let temp = TempDir::new().unwrap();
    let source = copy_fixture(&temp);
    let outside = temp.path().join("outside.bin");
    fs::write(&outside, b"outside stays untouched").unwrap();
    std::os::unix::fs::symlink(&outside, source.join("attachments/escape.bin")).unwrap();
    let destination = temp.path().join("unsafe-export.session.wsprabundle");

    let error = BundleStore::new(source)
        .copy_losslessly_to(&destination)
        .expect_err("attachment symlink is rejected");

    assert!(matches!(
        error,
        BundleCopyError::UnsupportedSourceEntry {
            entry_type: "symbolic link",
            ..
        }
    ));
    assert!(!destination.exists());
    assert_eq!(fs::read(&outside).unwrap(), b"outside stays untouched");
}

#[cfg(unix)]
#[test]
fn rejects_unsupported_attachment_entry_and_cleans_incomplete_destination() {
    use std::os::unix::net::UnixListener;

    let temp = tempfile::Builder::new()
        .prefix("ab")
        .tempdir_in("/tmp")
        .unwrap();
    let source = copy_fixture(&temp);
    let socket_path = source.join("attachments/unsupported.socket");
    let _listener = UnixListener::bind(&socket_path).unwrap();
    let destination = temp.path().join("unsupported-export.session.wsprabundle");

    let error = BundleStore::new(source)
        .copy_losslessly_to(&destination)
        .expect_err("socket entry is rejected");

    assert!(matches!(
        error,
        BundleCopyError::UnsupportedSourceEntry {
            entry_type: "unsupported filesystem entry",
            ..
        }
    ));
    assert!(!destination.exists());
}
