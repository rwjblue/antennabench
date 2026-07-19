use std::{cell::Cell, fs, io, path::Path};

use antennabench_core::v2::V2_BUNDLE_SUFFIX;
use tempfile::TempDir;

use super::*;
use crate::{
    managed_sessions::{list_managed_sessions_for, CatalogStatus},
    open_session::{ActiveSessionState, SessionErrorKind},
    setup::create_e2e_session,
};

fn create_external_bundle(parent: &Path) -> PathBuf {
    fs::create_dir_all(parent).unwrap();
    create_e2e_session(parent, &ActiveSessionState::default()).path
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination_entry = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination_entry)?;
        } else {
            fs::copy(entry.path(), destination_entry)?;
        }
    }
    Ok(())
}

fn snapshot_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn collect(root: &Path, current: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn staging_entries(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".import-staging-"))
        })
        .collect()
}

#[test]
fn valid_bundle_imports_losslessly_then_registers_the_published_direct_child() {
    let temp = TempDir::new().unwrap();
    let source = create_external_bundle(&temp.path().join("external"));
    let before = snapshot_files(&source);
    let root = temp.path().join("app-data/sessions");
    fs::create_dir_all(&root).unwrap();
    let state = ManagedSessionsState::default();

    let outcome = import_managed_session_for(&state, &root, &source, &SystemPublishPort).unwrap();
    let ImportManagedSessionOutcome::Imported { location } = outcome else {
        panic!("deterministic import cancelled");
    };
    let destination = root.join(&location.bundle_name);

    assert_eq!(
        snapshot_files(&source),
        before,
        "source must remain untouched"
    );
    assert_eq!(snapshot_files(&destination), before);
    assert!(staging_entries(&root).is_empty());
    assert_eq!(
        revalidate_available(&root, &state.resolve(&location.locator_id).unwrap()).unwrap(),
        destination
    );
}

#[test]
fn cancellation_does_not_prepare_or_change_the_managed_root() {
    let temp = TempDir::new().unwrap();
    let app_data = temp.path().join("app-data");

    let outcome =
        import_managed_session_with_selection(&ManagedSessionsState::default(), &app_data, || {
            Ok(None)
        })
        .unwrap();

    assert_eq!(outcome, ImportManagedSessionOutcome::Cancelled);
    assert!(!managed_sessions_dir(&app_data).exists());
}

#[test]
fn same_identity_imports_stay_distinct_and_catalog_as_duplicates() {
    let temp = TempDir::new().unwrap();
    let source = create_external_bundle(&temp.path().join("external"));
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let state = ManagedSessionsState::default();

    let first = import_managed_session_for(&state, &root, &source, &SystemPublishPort).unwrap();
    let second = import_managed_session_for(&state, &root, &source, &SystemPublishPort).unwrap();
    let (
        ImportManagedSessionOutcome::Imported { location: first },
        ImportManagedSessionOutcome::Imported { location: second },
    ) = (first, second)
    else {
        panic!("deterministic imports cancelled");
    };

    assert_ne!(first.bundle_name, second.bundle_name);
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    assert_eq!(catalog.status, CatalogStatus::Complete);
    assert_eq!(catalog.entries.len(), 2);
    assert!(catalog
        .entries
        .iter()
        .all(|entry| entry.same_session_id_count == 2));
}

struct FailingPublisher {
    kind: io::ErrorKind,
    called: Cell<bool>,
}

impl PublishPort for FailingPublisher {
    fn publish(&self, _staging: &Path, _destination: &Path) -> io::Result<()> {
        self.called.set(true);
        Err(io::Error::new(self.kind, "injected publication failure"))
    }
}

#[test]
fn publication_failure_or_collision_leaves_no_staging_or_partial_bundle() {
    for (kind, expected) in [
        (io::ErrorKind::Other, SessionErrorKind::Filesystem),
        (io::ErrorKind::AlreadyExists, SessionErrorKind::Conflict),
    ] {
        let temp = TempDir::new().unwrap();
        let source = create_external_bundle(&temp.path().join("external"));
        let root = temp.path().join("sessions");
        fs::create_dir(&root).unwrap();
        let publisher = FailingPublisher {
            kind,
            called: Cell::new(false),
        };

        let error = import_managed_session_for(
            &ManagedSessionsState::default(),
            &root,
            &source,
            &publisher,
        )
        .unwrap_err();

        assert_eq!(error.kind, expected);
        assert!(publisher.called.get());
        assert!(staging_entries(&root).is_empty());
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        assert!(source.exists());
    }
}

#[test]
fn invalid_unsupported_unsafe_and_over_budget_sources_publish_nothing() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let state = ManagedSessionsState::default();

    let invalid = temp.path().join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&invalid).unwrap();
    fs::write(invalid.join("manifest.json"), "{").unwrap();
    let unsupported = temp.path().join(format!("unsupported{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&unsupported).unwrap();
    fs::write(
        unsupported.join("manifest.json"),
        r#"{"schema_version":99}"#,
    )
    .unwrap();
    let over_budget = create_external_bundle(&temp.path().join("large"));
    let manifest_path = over_budget.join("manifest.json");
    let mut oversized_manifest = fs::read(&manifest_path).unwrap();
    oversized_manifest.extend(vec![b' '; 4 * 1024 * 1024 + 1]);
    fs::write(manifest_path, oversized_manifest).unwrap();

    for (source, expected_kind) in [
        (&invalid, SessionErrorKind::JsonParse),
        (&unsupported, SessionErrorKind::Unsupported),
        (&over_budget, SessionErrorKind::Resource),
    ] {
        assert_eq!(
            import_managed_session_for(&state, &root, source, &SystemPublishPort)
                .unwrap_err()
                .kind,
            expected_kind,
        );
        assert!(fs::read_dir(&root).unwrap().next().is_none());
    }

    #[cfg(unix)]
    {
        let unsafe_source = create_external_bundle(&temp.path().join("unsafe"));
        std::os::unix::fs::symlink(
            unsafe_source.join("manifest.json"),
            unsafe_source.join("unsafe-link"),
        )
        .unwrap();
        assert!(
            import_managed_session_for(&state, &root, &unsafe_source, &SystemPublishPort).is_err()
        );
        assert!(fs::read_dir(&root).unwrap().next().is_none());
    }
}

#[test]
fn staged_semantic_validation_failure_is_rolled_back_before_publication() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join(format!(
        "invalid-semantic{}",
        antennabench_core::v2::V1_BUNDLE_SUFFIX
    ));
    copy_directory(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle"),
        &source,
    )
    .unwrap();
    let station_path = source.join("station.json");
    let mut invalid_station: serde_json::Value =
        serde_json::from_slice(&fs::read(&station_path).unwrap()).unwrap();
    invalid_station.as_object_mut().unwrap().remove("callsign");
    fs::write(station_path, serde_json::to_vec(&invalid_station).unwrap()).unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();

    let error = import_managed_session_for(
        &ManagedSessionsState::default(),
        &root,
        &source,
        &SystemPublishPort,
    )
    .unwrap_err();

    assert_eq!(error.kind, SessionErrorKind::Validation);
    assert!(fs::read_dir(&root).unwrap().next().is_none());
    assert!(staging_entries(&root).is_empty());
}

#[test]
fn managed_row_exports_without_activation_and_revalidates_after_selection() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let source = create_external_bundle(&root);
    let before = snapshot_files(&source);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    let destination = temp.path().join(format!("exported{V2_BUNDLE_SUFFIX}"));

    let outcome = export_managed_session_with_selection(&state, &root, locator, |selected| {
        assert_eq!(selected, source);
        Ok(Some(destination.clone()))
    })
    .unwrap();

    assert!(matches!(
        outcome,
        ExportManagedSessionOutcome::Exported {
            revision: Some(0),
            ..
        }
    ));
    assert_eq!(snapshot_files(&source), before);
    assert_eq!(
        BundleStore::new(&destination)
            .read_v3_checkpointed()
            .unwrap()
            .session_state
            .revision,
        0
    );

    let stale_destination = temp.path().join(format!("stale{V2_BUNDLE_SUFFIX}"));
    let error = export_managed_session_with_selection(&state, &root, locator, |_| {
        fs::write(source.join("changed-after-picker"), "changed").unwrap();
        Ok(Some(stale_destination.clone()))
    })
    .unwrap_err();
    assert_eq!(error.kind, SessionErrorKind::Verification);
    assert!(!stale_destination.exists());

    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let traversal_locator = catalog.entries[0].locator_id.as_deref().unwrap();
    state
        .0
        .lock()
        .unwrap()
        .get_mut(traversal_locator)
        .unwrap()
        .bundle_name = format!("..{}outside{V2_BUNDLE_SUFFIX}", std::path::MAIN_SEPARATOR);
    let outside_destination = temp.path().join(format!("outside{V2_BUNDLE_SUFFIX}"));
    assert_eq!(
        export_managed_session_with_selection(&state, &root, traversal_locator, |_| Ok(Some(
            outside_destination.clone()
        )),)
        .unwrap_err()
        .kind,
        SessionErrorKind::Verification,
    );
    assert!(!outside_destination.exists());
}

#[test]
fn managed_row_export_cancellation_preserves_the_source_and_catalog() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let source = create_external_bundle(&root);
    let before = snapshot_files(&source);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();

    assert_eq!(
        export_managed_session_with_selection(&state, &root, locator, |_| Ok(None)).unwrap(),
        ExportManagedSessionOutcome::Cancelled
    );
    assert_eq!(snapshot_files(&source), before);
    assert!(state.resolve(locator).is_ok());
}
