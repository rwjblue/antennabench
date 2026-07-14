use std::{
    fs, io,
    path::{Path, PathBuf},
};

use antennabench_core::{codes, BundleValidationProfile, V2_BUNDLE_SUFFIX};
use antennabench_storage::{BundleStore, BundleStoreError};

fn fixture_roots() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles");
    let mut fixtures = fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".session.wsprabundle"))
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
}

#[test]
fn every_v1_fixture_mutation_is_rejected_before_strict_write_creates_files() {
    for source_path in fixture_roots() {
        let mut bundle = BundleStore::new(&source_path).read().unwrap();
        bundle.station.power_watts = Some(f32::NAN);
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("invalid.session.wsprabundle");
        let error = BundleStore::new(&destination).write(&bundle).unwrap_err();
        let BundleStoreError::Validation { source: validation } = error else {
            panic!(
                "expected validation error for {}, got {error:?}",
                source_path.display()
            );
        };
        assert!(validation
            .report()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == codes::NON_FINITE_NUMBER));
        assert!(!destination.exists());
    }
}

#[test]
fn v2_nonfinite_values_are_rejected_before_destination_creation() {
    let source = fixture_roots().remove(0);
    let temp = tempfile::tempdir().unwrap();
    let baseline = temp.path().join(format!("baseline{V2_BUNDLE_SUFFIX}"));
    let baseline_store = BundleStore::new(source)
        .upgrade_v1_to_v2(&baseline)
        .unwrap();
    let mut bundle = baseline_store.read_v2().unwrap();
    bundle.observations[0].distance_km = Some(f64::INFINITY);
    let destination = temp.path().join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    let error = BundleStore::new(&destination)
        .write_v2(&bundle)
        .unwrap_err();
    assert!(matches!(error, BundleStoreError::Validation { .. }));
    assert!(!destination.exists());
}

#[test]
fn v2_adapter_machine_id_is_preflighted_before_destination_creation() {
    let source = fixture_roots().remove(0);
    let temp = tempfile::tempdir().unwrap();
    let baseline = temp.path().join(format!("baseline-id{V2_BUNDLE_SUFFIX}"));
    let baseline_store = BundleStore::new(source)
        .upgrade_v1_to_v2(&baseline)
        .unwrap();
    let mut bundle = baseline_store.read_v2().unwrap();
    bundle.adapter_records[0].record_id = String::new();
    let destination = temp.path().join(format!("invalid-id{V2_BUNDLE_SUFFIX}"));
    assert!(matches!(
        BundleStore::new(&destination).write_v2(&bundle),
        Err(BundleStoreError::InvalidV2Bundle { .. })
    ));
    assert!(!destination.exists());

    let mut duplicate = baseline_store.read_v2().unwrap();
    duplicate.adapter_records[1].record_id = duplicate.adapter_records[0].record_id.clone();
    let duplicate_destination = temp.path().join(format!("duplicate-id{V2_BUNDLE_SUFFIX}"));
    assert!(matches!(
        BundleStore::new(&duplicate_destination).write_v2(&duplicate),
        Err(BundleStoreError::InvalidV2Bundle { .. })
    ));
    assert!(!duplicate_destination.exists());
}

#[test]
fn warning_bearing_v1_is_unchanged_readable_copyable_and_upgradeable() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("warning.session.wsprabundle");
    copy_directory(&fixture_roots().remove(0), &source).unwrap();
    let station_path = source.join("station.json");
    let mut station: serde_json::Value =
        serde_json::from_slice(&fs::read(&station_path).unwrap()).unwrap();
    station["callsign"] = serde_json::Value::String(String::new());
    let mut bytes = serde_json::to_vec_pretty(&station).unwrap();
    bytes.push(b'\n');
    fs::write(&station_path, bytes).unwrap();
    let before = snapshot_files(&source).unwrap();

    let store = BundleStore::new(&source);
    let inspection = store.inspect().unwrap();
    assert!(inspection.bundle().is_some());
    assert!(inspection
        .report()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == codes::INVALID_REQUIRED_TEXT));
    assert!(inspection
        .report()
        .allows(BundleValidationProfile::CompatibilityRead));
    assert!(inspection
        .report()
        .allows(BundleValidationProfile::Analysis));
    assert!(inspection.report().allows(BundleValidationProfile::Upgrade));
    assert!(!inspection
        .report()
        .allows(BundleValidationProfile::StrictCreation));
    store.read().unwrap();

    let copied = temp.path().join("warning-copy.session.wsprabundle");
    store.copy_losslessly_to(&copied).unwrap();
    assert_eq!(snapshot_files(&copied).unwrap(), before);

    let upgraded = temp.path().join(format!("warning{V2_BUNDLE_SUFFIX}"));
    store
        .upgrade_v1_to_v2(&upgraded)
        .unwrap()
        .read_current()
        .unwrap();
    assert_eq!(snapshot_files(&source).unwrap(), before);
}

#[test]
fn warning_level_authored_values_fail_strict_preflight() {
    let source = fixture_roots().remove(0);
    let mut bundle = BundleStore::new(source).read().unwrap();
    bundle.station.callsign.clear();
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("warning.session.wsprabundle");
    assert!(matches!(
        BundleStore::new(&destination).write(&bundle),
        Err(BundleStoreError::Validation { .. })
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
