use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::v2::{V1_BUNDLE_SUFFIX, V2_BUNDLE_SUFFIX};
use tempfile::TempDir;

use super::*;
use crate::{
    open_session::{active_session_source, ActiveSessionState, SessionErrorKind},
    setup::create_e2e_session,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles")
        .join(name)
}

fn copy_directory(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target_path)?;
        } else {
            fs::copy(entry.path(), target_path)?;
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

fn create_managed_latest(root: &Path) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let active = ActiveSessionState::default();
    create_e2e_session(root, &active).path
}

fn listed_record(
    state: &ManagedSessionsState,
    catalog: &ManagedSessionCatalog,
    name: &str,
) -> catalog::LocatorRecord {
    let locator = catalog
        .entries
        .iter()
        .find(|entry| entry.bundle_name == name)
        .and_then(|entry| entry.locator_id.as_deref())
        .unwrap();
    state.resolve(locator).unwrap()
}

#[test]
fn missing_and_empty_roots_are_complete_empty_catalogs() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("missing");
    let state = ManagedSessionsState::default();

    let missing_catalog = list_managed_sessions_for(&state, &missing).unwrap();
    assert_eq!(missing_catalog.status, CatalogStatus::Complete);
    assert!(missing_catalog.entries.is_empty());
    assert!(missing_catalog.diagnostics.is_empty());

    fs::create_dir(&missing).unwrap();
    let empty_catalog = list_managed_sessions_for(&state, &missing).unwrap();
    assert_eq!(empty_catalog.status, CatalogStatus::Complete);
    assert!(empty_catalog.entries.is_empty());
}

#[cfg(unix)]
#[test]
fn managed_root_symlink_is_rejected_without_following_it() {
    let temp = TempDir::new().unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let root = temp.path().join("sessions");
    std::os::unix::fs::symlink(&outside, &root).unwrap();

    let error = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap_err();

    assert_eq!(error.kind, SessionErrorKind::Filesystem);
    assert!(fs::read_dir(outside).unwrap().next().is_none());
}

#[test]
fn supported_checkpointed_bundle_projects_bounded_plan_and_lifecycle_metadata() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let path = create_managed_latest(&root);
    let state = ManagedSessionsState::default();

    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let entry = &catalog.entries[0];

    assert_eq!(entry.status, ManagedEntryStatus::Available);
    assert_eq!(
        entry.bundle_name,
        path.file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(entry.origin, ManagedEntryOrigin::Managed);
    assert_eq!(entry.origin_label, "Saved by AntennaBench");
    assert_eq!(entry.callsign.as_deref(), Some("N1RWJ"));
    assert!(entry.created_at.is_some());
    assert_eq!(entry.lifecycle, Some(SessionLifecycleV2::Ready));
    assert_eq!(entry.schema_version, Some(6));
    assert_eq!(entry.revision, Some(0));
    assert_eq!(entry.mode, Some(ExperimentMode::WholeStationAb));
    assert_eq!(entry.planned_repetitions, Some(2));
    assert_eq!(
        entry.direction_coverage,
        Some(ManagedDirectionCoverage::TransmitAndReceive)
    );
    assert_eq!(entry.planned_cycle_count, Some(8));
    assert_eq!(
        entry.observation_counts,
        Some(ManagedObservationCounts {
            total: 0,
            local_decodes: 0,
            public_spots: 0,
            imported_spots: 0,
        })
    );
    assert_eq!(entry.bands, vec![Band::M20]);
    assert_eq!(entry.antenna_labels, ["Vertical", "Dipole"]);
    assert_eq!(entry.antenna_count, Some(2));
    assert!(entry.locator_id.is_some());
    let json = serde_json::to_string(&catalog).unwrap();
    assert!(!json.contains("reportHtml"));
    assert!(!json.contains(root.to_string_lossy().as_ref()));
}

#[test]
fn legacy_bundle_has_no_invented_lifecycle_or_revision() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let name = format!("legacy{V1_BUNDLE_SUFFIX}");
    copy_directory(
        &fixture("minimal-whole-station.session.wsprabundle"),
        &root.join(&name),
    )
    .unwrap();

    let catalog = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap();
    let entry = &catalog.entries[0];
    assert_eq!(entry.status, ManagedEntryStatus::Available);
    assert_eq!(entry.schema_version, Some(1));
    assert_eq!(entry.lifecycle, None);
    assert_eq!(entry.revision, None);
    assert_eq!(entry.planned_repetitions, None);
    assert_eq!(
        entry.direction_coverage,
        Some(ManagedDirectionCoverage::Unknown)
    );
    assert!(entry.planned_cycle_count.is_some());
    assert!(entry.observation_counts.is_some());
}

#[test]
fn malformed_unsupported_invalid_unsafe_and_unrelated_entries_are_fault_isolated() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let valid = create_managed_latest(&root);

    let malformed = root.join(format!("malformed{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&malformed).unwrap();
    fs::write(malformed.join("manifest.json"), b"{").unwrap();

    let unsupported = root.join(format!("unsupported{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&unsupported).unwrap();
    fs::write(
        unsupported.join("manifest.json"),
        br#"{"schema_version":99}"#,
    )
    .unwrap();

    let invalid = root.join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    copy_directory(&valid, &invalid).unwrap();
    let station = invalid.join("station.json");
    let changed = fs::read_to_string(&station)
        .unwrap()
        .replace("session-0001", "session-changed");
    fs::write(station, changed).unwrap();

    fs::create_dir(root.join("unrelated-directory")).unwrap();
    fs::write(root.join(format!("regular-file{V2_BUNDLE_SUFFIX}")), b"x").unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&valid, root.join(format!("unsafe{V2_BUNDLE_SUFFIX}"))).unwrap();

    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let status = |name: &str| {
        catalog
            .entries
            .iter()
            .find(|entry| entry.bundle_name == name)
            .map(|entry| entry.status)
    };
    assert_eq!(
        status(valid.file_name().unwrap().to_str().unwrap()),
        Some(ManagedEntryStatus::Available)
    );
    assert_eq!(
        status(malformed.file_name().unwrap().to_str().unwrap()),
        Some(ManagedEntryStatus::Invalid)
    );
    assert_eq!(
        status(unsupported.file_name().unwrap().to_str().unwrap()),
        Some(ManagedEntryStatus::Unsupported)
    );
    assert_eq!(
        catalog
            .entries
            .iter()
            .find(|entry| entry.bundle_name == unsupported.file_name().unwrap().to_str().unwrap())
            .and_then(|entry| entry.schema_version),
        Some(99)
    );
    assert_eq!(
        status(invalid.file_name().unwrap().to_str().unwrap()),
        Some(ManagedEntryStatus::Invalid)
    );
    #[cfg(unix)]
    assert_eq!(
        status(&format!("unsafe{V2_BUNDLE_SUFFIX}")),
        Some(ManagedEntryStatus::Unsafe)
    );
    assert!(catalog
        .entries
        .iter()
        .all(|entry| entry.bundle_name != "unrelated-directory"));
    assert!(catalog.entries.iter().any(|entry| {
        entry.status == ManagedEntryStatus::Unsupported && entry.locator_id.is_some()
    }));
    #[cfg(unix)]
    assert!(catalog
        .entries
        .iter()
        .any(|entry| { entry.status == ManagedEntryStatus::Unsafe && entry.locator_id.is_none() }));
    assert!(!serde_json::to_string(&catalog)
        .unwrap()
        .contains(root.to_string_lossy().as_ref()));
}

#[cfg(unix)]
#[test]
fn permission_denied_bundle_is_unreadable_and_does_not_hide_healthy_rows() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let valid = create_managed_latest(&root);
    let unreadable = root.join(format!("unreadable{V2_BUNDLE_SUFFIX}"));
    copy_directory(&valid, &unreadable).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();

    let catalog = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap();
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o700)).unwrap();

    assert!(catalog
        .entries
        .iter()
        .any(|entry| entry.status == ManagedEntryStatus::Available));
    assert!(catalog.entries.iter().any(|entry| {
        entry.bundle_name == unreadable.file_name().unwrap().to_str().unwrap()
            && entry.status == ManagedEntryStatus::Unreadable
    }));
}

#[test]
fn listing_is_observational_and_preserves_every_bundle_byte() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let before = snapshot_files(&bundle);

    let catalog = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap();

    assert_eq!(catalog.entries[0].revision, Some(0));
    assert_eq!(snapshot_files(&bundle), before);
}

#[test]
fn same_session_id_copies_remain_distinct_and_warn_without_merging() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let first = create_managed_latest(&root);
    let second = root.join(format!("copy{V2_BUNDLE_SUFFIX}"));
    copy_directory(&first, &second).unwrap();

    let catalog = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap();
    assert_eq!(catalog.entries.len(), 2);
    assert_ne!(
        catalog.entries[0].bundle_name,
        catalog.entries[1].bundle_name
    );
    assert_eq!(catalog.entries[0].session_id, catalog.entries[1].session_id);
    assert!(catalog.entries.iter().all(|entry| {
        entry.same_session_id_count == 2
            && entry
                .problems
                .iter()
                .any(|problem| problem.code == "managed.same_session_id")
    }));
}

#[test]
fn healthy_rows_sort_by_authoritative_creation_time_then_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let newer = root.join(format!("z-newer{V1_BUNDLE_SUFFIX}"));
    let older = root.join(format!("a-older{V1_BUNDLE_SUFFIX}"));
    copy_directory(
        &fixture("analysis-rich-whole-station.session.wsprabundle"),
        &newer,
    )
    .unwrap();
    copy_directory(
        &fixture("inconclusive-sample-report.session.wsprabundle"),
        &older,
    )
    .unwrap();

    let catalog = list_managed_sessions_for(&ManagedSessionsState::default(), &root).unwrap();
    assert_eq!(
        catalog.entries[0].bundle_name,
        newer.file_name().unwrap().to_str().unwrap()
    );
    assert_eq!(
        catalog.entries[1].bundle_name,
        older.file_name().unwrap().to_str().unwrap()
    );
}

#[test]
fn moving_a_listed_bundle_stales_its_locator_without_following_the_new_path() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let record = listed_record(
        &state,
        &catalog,
        bundle.file_name().unwrap().to_str().unwrap(),
    );
    let moved = temp.path().join(bundle.file_name().unwrap());
    fs::rename(&bundle, &moved).unwrap();

    assert_eq!(
        catalog::revalidate_direct_child(&root, &record)
            .unwrap_err()
            .kind,
        SessionErrorKind::Filesystem
    );
    assert!(moved.exists());
}

#[test]
fn successful_managed_open_uses_the_shared_activation_path() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let managed = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&managed, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    let active = ActiveSessionState::default();

    let outcome = open_managed_session_for(&managed, &active, &root, locator).unwrap();

    assert!(matches!(outcome, OpenSessionOutcome::Opened { .. }));
    assert_eq!(active_session_source(&active).unwrap().0, bundle);
}

#[test]
fn locators_reject_arbitrary_stale_traversal_replaced_and_modified_entries() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let name = bundle.file_name().unwrap().to_str().unwrap().to_string();
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let record = listed_record(&state, &catalog, &name);

    assert_eq!(
        state.resolve("../../arbitrary").unwrap_err().kind,
        SessionErrorKind::Selection
    );
    let mut traversal = record.clone();
    traversal.bundle_name = format!("..{}/outside{V2_BUNDLE_SUFFIX}", std::path::MAIN_SEPARATOR);
    assert_eq!(
        catalog::revalidate_direct_child(&root, &traversal)
            .unwrap_err()
            .kind,
        SessionErrorKind::Verification
    );

    let station = bundle.join("station.json");
    let changed = fs::read_to_string(&station)
        .unwrap()
        .replace("N1RWJ", "N9XYZ");
    fs::write(&station, changed).unwrap();
    assert_eq!(
        catalog::revalidate_available(&root, &record)
            .unwrap_err()
            .kind,
        SessionErrorKind::Verification
    );

    fs::remove_dir_all(&bundle).unwrap();
    fs::create_dir(&bundle).unwrap();
    assert_eq!(
        catalog::revalidate_direct_child(&root, &record)
            .unwrap_err()
            .kind,
        SessionErrorKind::Verification
    );
}

#[test]
fn failed_managed_revalidation_leaves_the_prior_active_session_unchanged() {
    let temp = TempDir::new().unwrap();
    let active_path = temp.path().join(format!("prior{V1_BUNDLE_SUFFIX}"));
    copy_directory(
        &fixture("minimal-whole-station.session.wsprabundle"),
        &active_path,
    )
    .unwrap();
    let active = ActiveSessionState::default();
    crate::open_session::open_session_at_path(&active, active_path.clone()).unwrap();

    let root = temp.path().join("sessions");
    let managed = create_managed_latest(&root);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let record = listed_record(
        &state,
        &catalog,
        managed.file_name().unwrap().to_str().unwrap(),
    );
    fs::remove_dir_all(managed).unwrap();

    assert!(catalog::revalidate_available(&root, &record).is_err());
    assert_eq!(active_session_source(&active).unwrap().0, active_path);
}

#[test]
fn creation_registration_returns_an_exact_narrow_managed_context() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let state = ManagedSessionsState::default();

    let context = state.register_created(&root, &bundle).unwrap();
    let record = state.resolve(&context.locator_id).unwrap();

    assert_eq!(context.bundle_name, record.bundle_name);
    assert_eq!(context.origin, ManagedEntryOrigin::Managed);
    assert_eq!(context.origin_label, "Saved by AntennaBench");
    assert_eq!(
        catalog::revalidate_available(&root, &record).unwrap(),
        bundle
    );
}

#[test]
fn candidate_bound_reports_n_minus_one_n_and_n_plus_one_honestly() {
    for (count, expected_status, expected_len) in [
        (
            MAX_CATALOG_CANDIDATES - 1,
            CatalogStatus::Complete,
            MAX_CATALOG_CANDIDATES - 1,
        ),
        (
            MAX_CATALOG_CANDIDATES,
            CatalogStatus::Complete,
            MAX_CATALOG_CANDIDATES,
        ),
        (
            MAX_CATALOG_CANDIDATES + 1,
            CatalogStatus::Incomplete,
            MAX_CATALOG_CANDIDATES,
        ),
    ] {
        let temp = TempDir::new().unwrap();
        for index in 0..count {
            fs::create_dir(
                temp.path()
                    .join(format!("candidate-{index:03}{V2_BUNDLE_SUFFIX}")),
            )
            .unwrap();
        }
        let catalog =
            list_managed_sessions_for(&ManagedSessionsState::default(), temp.path()).unwrap();
        assert_eq!(catalog.status, expected_status, "count={count}");
        assert_eq!(catalog.entries.len(), expected_len, "count={count}");
        assert_eq!(
            catalog.diagnostics.is_empty(),
            count <= MAX_CATALOG_CANDIDATES,
            "count={count}"
        );
    }
}

fn fabricated_entry(name: &str, message: &str) -> ManagedCatalogEntry {
    ManagedCatalogEntry {
        locator_id: Some(Uuid::new_v4().to_string()),
        bundle_name: name.into(),
        origin: ManagedEntryOrigin::Managed,
        origin_label: "Saved by AntennaBench".into(),
        status: ManagedEntryStatus::Invalid,
        session_id: None,
        callsign: None,
        created_at: None,
        lifecycle: None,
        schema_version: None,
        revision: None,
        mode: None,
        planned_repetitions: None,
        direction_coverage: None,
        planned_cycle_count: None,
        observation_counts: None,
        bands: Vec::new(),
        antenna_labels: Vec::new(),
        antenna_count: None,
        same_session_id_count: 0,
        problems: vec![ManagedSessionProblem {
            code: "test".into(),
            severity: ProblemSeverity::Error,
            message: message.into(),
        }],
    }
}

#[test]
fn ipc_bound_covers_n_minus_one_n_and_n_plus_one() {
    let base = ManagedSessionCatalog::complete(vec![fabricated_entry("one", "bounded")]);
    let n = serialized_len(&base).unwrap();

    for (limit, expected_entries) in [(n - 1, 0), (n, 1), (n + 1, 1)] {
        let mut catalog = base.clone();
        let mut locators = HashMap::new();
        fit_catalog_to_ipc_with_limit(&mut catalog, &mut locators, limit).unwrap();
        assert_eq!(
            catalog.entries.len(),
            expected_entries,
            "limit={limit} n={n}"
        );
        assert!(serialized_len(&catalog).unwrap() <= limit);
    }
}

#[derive(Default)]
struct RecordingRevealPort(Mutex<Vec<RevealTarget>>);

impl RevealPort for RecordingRevealPort {
    fn reveal(&self, target: &RevealTarget) -> io::Result<()> {
        self.0.lock().unwrap().push(target.clone());
        Ok(())
    }
}

struct RenameTrashPort {
    destination: PathBuf,
    fail: bool,
}

impl TrashPort for RenameTrashPort {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        if self.fail {
            return Err("injected Trash failure".into());
        }
        fs::create_dir_all(&self.destination).map_err(|error| error.to_string())?;
        fs::rename(path, self.destination.join(path.file_name().unwrap()))
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "macos")]
#[test]
fn system_trash_uses_native_ns_file_manager_backend() {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    let context = native_macos_trash_context();

    assert!(matches!(
        context.delete_method(),
        DeleteMethod::NsFileManager
    ));
}

#[test]
fn verified_direct_child_moves_to_trash_without_changing_siblings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let sibling = root.join("notes.txt");
    fs::write(&sibling, "keep").unwrap();
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    let trash = temp.path().join("Trash");

    let outcome = delete_managed_session_for(
        &state,
        &ActiveSessionState::default(),
        &root,
        locator,
        &RenameTrashPort {
            destination: trash.clone(),
            fail: false,
        },
    )
    .unwrap();

    assert_eq!(outcome.status, "trashed");
    assert_eq!(
        outcome.bundle_name,
        bundle.file_name().unwrap().to_str().unwrap()
    );
    assert!(!bundle.exists());
    assert!(trash.join(bundle.file_name().unwrap()).is_dir());
    assert_eq!(fs::read_to_string(sibling).unwrap(), "keep");
    assert_eq!(
        state.resolve(locator).unwrap_err().kind,
        SessionErrorKind::Selection
    );
}

#[test]
fn deletion_rejects_modified_and_active_bundles_and_trash_failure_is_non_mutating() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    fs::write(bundle.join("changed-after-listing"), "changed").unwrap();

    let error = delete_managed_session_for(
        &state,
        &ActiveSessionState::default(),
        &root,
        locator,
        &RenameTrashPort {
            destination: temp.path().join("Trash"),
            fail: false,
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, SessionErrorKind::Verification);
    assert!(bundle.exists());

    fs::remove_file(bundle.join("changed-after-listing")).unwrap();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    let active = ActiveSessionState::default();
    crate::open_session::open_session_at_path(&active, bundle.clone()).unwrap();
    let error = delete_managed_session_for(
        &state,
        &active,
        &root,
        locator,
        &RenameTrashPort {
            destination: temp.path().join("Trash"),
            fail: false,
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, SessionErrorKind::Selection);
    assert_eq!(active_session_source(&active).unwrap().0, bundle);

    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let locator = catalog.entries[0].locator_id.as_deref().unwrap();
    let error = delete_managed_session_for(
        &state,
        &ActiveSessionState::default(),
        &root,
        locator,
        &RenameTrashPort {
            destination: temp.path().join("Trash"),
            fail: true,
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, SessionErrorKind::Filesystem);
    assert!(bundle.exists());
}

#[test]
fn invalid_and_unsupported_direct_children_receive_safe_removal_locators() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    fs::create_dir(&root).unwrap();
    let invalid = root.join(format!("invalid{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&invalid).unwrap();
    fs::write(invalid.join("manifest.json"), "{").unwrap();
    let unsupported = root.join(format!("unsupported{V2_BUNDLE_SUFFIX}"));
    fs::create_dir(&unsupported).unwrap();
    fs::write(
        unsupported.join("manifest.json"),
        r#"{"schema_version":99}"#,
    )
    .unwrap();
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    assert!(catalog
        .entries
        .iter()
        .all(|entry| entry.locator_id.is_some()));

    for entry in catalog.entries {
        delete_managed_session_for(
            &state,
            &ActiveSessionState::default(),
            &root,
            entry.locator_id.as_deref().unwrap(),
            &RenameTrashPort {
                destination: temp.path().join("Trash"),
                fail: false,
            },
        )
        .unwrap();
    }
    assert!(!invalid.exists());
    assert!(!unsupported.exists());
}

#[test]
fn reveal_port_receives_only_the_resolved_root_or_direct_child() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("sessions");
    let bundle = create_managed_latest(&root);
    let state = ManagedSessionsState::default();
    let catalog = list_managed_sessions_for(&state, &root).unwrap();
    let record = listed_record(
        &state,
        &catalog,
        bundle.file_name().unwrap().to_str().unwrap(),
    );
    let port = RecordingRevealPort::default();

    reveal_with(&port, RevealTarget::Directory(root.clone())).unwrap();
    let resolved = catalog::revalidate_direct_child(&root, &record).unwrap();
    reveal_with(&port, RevealTarget::Entry(resolved)).unwrap();

    assert_eq!(
        *port.0.lock().unwrap(),
        [RevealTarget::Directory(root), RevealTarget::Entry(bundle)]
    );
}
