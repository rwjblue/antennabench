//! Bounded control-plane reads for an active checkpoint.

use std::{fs, fs::OpenOptions};

use antennabench_core::{
    v2::{BundleManifestV2, SessionStateV2},
    v3::BundleV3Contents,
    SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};

use super::{
    all_streams, live_io, lock, read_json_file, stream_path, verify_exact_checkpoint, BundleStore,
    BundleStoreError, LivePersistenceError, LiveStreamV2, LOCK_FILE,
};

pub(super) fn read_v3_checkpointed(
    store: &BundleStore,
) -> Result<BundleV3Contents, LivePersistenceError> {
    let lock_path = store.root().join(LOCK_FILE);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| live_io("open snapshot lock", &lock_path, source))?;
    match lock::try_lock_shared(&lock) {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
        Err(fs::TryLockError::Error(source)) => {
            return Err(LivePersistenceError::Capability {
                message: format!("shared OS file locking failed: {source}"),
            });
        }
    }
    let bundle = store.read_v3()?;
    let paths = store.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
    verify_exact_checkpoint(store, &bundle.session_state, &paths)?;
    Ok(bundle)
}

pub(super) fn read_v3_checkpoint_state(
    store: &BundleStore,
) -> Result<SessionStateV2, LivePersistenceError> {
    let lock_path = store.root().join(LOCK_FILE);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|source| live_io("open checkpoint lock", &lock_path, source))?;
    match lock::try_lock_shared(&lock) {
        Ok(()) => {}
        Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
        Err(fs::TryLockError::Error(source)) => {
            return Err(LivePersistenceError::Capability {
                message: format!("shared OS file locking failed: {source}"),
            });
        }
    }
    let manifest_path = store.root().join("manifest.json");
    let manifest: BundleManifestV2 = read_json_file(store, &manifest_path, "manifest")?;
    if !matches!(
        manifest.schema_version,
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6
    ) {
        return Err(BundleStoreError::UnsupportedSchemaVersion {
            actual: manifest.schema_version,
        }
        .into());
    }
    let bootstrap = store.v2_paths(&manifest.files)?;
    let checkpoint: SessionStateV2 = read_json_file(store, &bootstrap.session_state, "checkpoint")?;
    let paths = store.v2_paths_for_state(&manifest.files, &checkpoint)?;
    let mut streams = all_streams().to_vec();
    if checkpoint
        .streams
        .contains_key(LiveStreamV2::RuntimeContexts.checkpoint_name())
    {
        streams.push(LiveStreamV2::RuntimeContexts);
    }
    if checkpoint
        .streams
        .contains_key(LiveStreamV2::Diagnostics.checkpoint_name())
    {
        streams.push(LiveStreamV2::Diagnostics);
    }
    for stream in streams {
        let expected = checkpoint
            .streams
            .get(stream.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: format!(
                    "checkpoint is missing {} stream metadata",
                    stream.checkpoint_name()
                ),
            })?;
        let path = stream_path(&paths, stream);
        let actual = fs::metadata(path)
            .map_err(|source| live_io("inspect checkpoint stream", path, source))?
            .len();
        if actual != expected.committed_bytes {
            return Err(LivePersistenceError::ExternalModification {
                message: format!(
                    "{} length changed outside checkpoint control: expected {}, found {}",
                    stream.checkpoint_name(),
                    expected.committed_bytes,
                    actual
                ),
            });
        }
    }
    Ok(checkpoint)
}
