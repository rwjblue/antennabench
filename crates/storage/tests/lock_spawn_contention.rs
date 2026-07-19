//! Writer locks must stay reliable while the process spawns children.
//!
//! Spawning clones the parent's descriptor table, so a child can briefly pin
//! an advisory lock another thread just released; without mitigation that
//! surfaces as spurious `WriterBusy` on bundles nobody else is using
//! (issue #196).

#![cfg(unix)]

use std::{
    path::Path,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use antennabench_core::{v2::SessionLifecycleV2, v2::V2_BUNDLE_SUFFIX};
use antennabench_storage::{BundleStore, LivePersistenceError};

fn ready_v2_store(root: &Path, name: &str) -> BundleStore {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let upgraded_path = root.join(format!("upgraded-{name}{V2_BUNDLE_SUFFIX}"));
    let upgraded = BundleStore::new(source)
        .upgrade_v1_to_v2(&upgraded_path)
        .unwrap();
    let mut bundle = upgraded.read_v2().unwrap();
    bundle.session_state.lifecycle = SessionLifecycleV2::Ready;
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();
    let store = BundleStore::new(root.join(format!("live-{name}{V2_BUNDLE_SUFFIX}")));
    store.write_v2(&bundle).unwrap();
    store
}

#[test]
fn concurrent_process_spawns_do_not_surface_writer_busy_on_private_bundles() {
    let temp = tempfile::tempdir().unwrap();
    let stop = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_millis(1500);

    thread::scope(|scope| {
        for index in 0..3 {
            let store = ready_v2_store(temp.path(), &index.to_string());
            let stop = &stop;
            scope.spawn(move || {
                while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                    for attempt in [
                        store.open_v2_writer().map(drop),
                        store.read_v2_checkpointed().map(drop),
                    ] {
                        if let Err(error) = attempt {
                            stop.store(true, Ordering::Relaxed);
                            assert!(
                                !matches!(error, LivePersistenceError::WriterBusy),
                                "spurious WriterBusy on a bundle without concurrent writers"
                            );
                            panic!("unexpected lock-cycle failure: {error:?}");
                        }
                    }
                }
            });
        }
        for _ in 0..2 {
            let stop = &stop;
            scope.spawn(move || {
                while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                    Command::new("true").output().unwrap();
                }
            });
        }
    });
}
