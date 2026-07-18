use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use super::{
    durability::{replace_checkpoint, sync_directory},
    invalid_serialization, live_io, LivePersistenceError, LivePersistenceHooks,
    LivePersistencePoint, LivePlanFile, LiveStreamV2,
};
use crate::{
    resource::{read_bounded, ResourceOperation},
    v2::{checkpoint_for_bytes, serialize_json, sha256_hex, ResolvedBundlePathsV2},
    BundleStore, BundleStoreError,
};
use antennabench_core::{
    v2::{BundleManifestV2, BundleV2Contents, SessionStateV2, StreamCheckpointV2},
    AnalysisFile, SCHEMA_VERSION_V2,
};

pub(super) const CHECKPOINT_TEMP: &str = ".session-state.next.json";
const CHECKPOINT_PREVIOUS: &str = "session-state.previous.json";
const CHECKPOINT_MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(super) fn load_checkpointed_bundle(
    store: &BundleStore,
) -> Result<BundleV2Contents, LivePersistenceError> {
    let manifest_path = store.root().join("manifest.json");
    let manifest: BundleManifestV2 = read_json_file(store, &manifest_path, "manifest")?;
    if manifest.schema_version != SCHEMA_VERSION_V2 {
        return Err(LivePersistenceError::Store(
            BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            },
        ));
    }
    let bootstrap = store.v2_paths(&manifest.files)?;
    let session_state: SessionStateV2 =
        read_json_file(store, &bootstrap.session_state, "checkpoint")?;
    load_checkpointed_bundle_from_state(store, manifest, session_state)
}

pub(super) fn load_recovery_bundle(
    store: &BundleStore,
) -> Result<(BundleV2Contents, bool), LivePersistenceError> {
    let manifest_path = store.root().join("manifest.json");
    let manifest: BundleManifestV2 = read_json_file(store, &manifest_path, "manifest")?;
    if manifest.schema_version != SCHEMA_VERSION_V2 {
        return Err(LivePersistenceError::Store(
            BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            },
        ));
    }
    let bootstrap = store.v2_paths(&manifest.files)?;
    let candidates = [
        (bootstrap.session_state.clone(), false),
        (store.root().join(CHECKPOINT_PREVIOUS), true),
    ];
    let mut valid = Vec::new();
    let mut last_error = None;
    for (path, previous) in candidates {
        if !path.exists() {
            continue;
        }
        let state = match read_json_file(store, &path, "recovery checkpoint") {
            Ok(state) => state,
            Err(error) => {
                last_error = Some(error);
                continue;
            }
        };
        match load_checkpointed_bundle_from_state(store, manifest.clone(), state) {
            Ok(bundle) => valid.push((bundle, previous)),
            Err(error) => last_error = Some(error),
        }
    }
    valid.sort_by_key(|(bundle, _)| bundle.session_state.revision);
    valid.pop().ok_or_else(|| {
        last_error.unwrap_or_else(|| LivePersistenceError::CheckpointVerification {
            message: "neither current nor previous checkpoint is valid".into(),
        })
    })
}

fn load_checkpointed_bundle_from_state(
    store: &BundleStore,
    manifest: BundleManifestV2,
    session_state: SessionStateV2,
) -> Result<BundleV2Contents, LivePersistenceError> {
    let paths = store.v2_paths_for_state(&manifest.files, &session_state)?;
    verify_committed_prefixes(store, &session_state, &paths)?;
    let bundle = BundleV2Contents {
        manifest,
        session_state: session_state.clone(),
        station: read_json_file(store, &paths.station, "active station plan")?,
        antennas: read_json_file(store, &paths.antennas, "active antenna plan")?,
        schedule: read_json_file(store, &paths.schedule, "active schedule plan")?,
        events: read_jsonl_prefix(
            store,
            &paths.events,
            stream_checkpoint(&session_state, LiveStreamV2::Events)?.committed_bytes,
        )?,
        observations: read_jsonl_prefix(
            store,
            &paths.observations,
            stream_checkpoint(&session_state, LiveStreamV2::Observations)?.committed_bytes,
        )?,
        adapter_records: read_jsonl_prefix(
            store,
            &paths.adapter_records,
            stream_checkpoint(&session_state, LiveStreamV2::AdapterRecords)?.committed_bytes,
        )?,
        rig: read_jsonl_prefix(
            store,
            &paths.rig,
            stream_checkpoint(&session_state, LiveStreamV2::Rig)?.committed_bytes,
        )?,
        propagation: read_jsonl_prefix(
            store,
            &paths.propagation,
            stream_checkpoint(&session_state, LiveStreamV2::Propagation)?.committed_bytes,
        )?,
        analysis: read_json_file::<AnalysisFile>(store, &paths.analysis, "analysis metadata")?,
    };
    verify_loaded_counts(&bundle)?;
    Ok(bundle)
}

fn verify_loaded_counts(bundle: &BundleV2Contents) -> Result<(), LivePersistenceError> {
    for (stream, count, last_id) in [
        (
            LiveStreamV2::Events,
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.as_str()),
        ),
        (
            LiveStreamV2::AdapterRecords,
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.as_str()),
        ),
        (
            LiveStreamV2::Observations,
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.as_str()),
        ),
        (
            LiveStreamV2::Rig,
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.as_str()),
        ),
        (
            LiveStreamV2::Propagation,
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.as_str()),
        ),
    ] {
        let checkpoint = stream_checkpoint(&bundle.session_state, stream)?;
        if checkpoint.record_count != u64::try_from(count).expect("usize fits u64")
            || checkpoint.last_record_id.as_deref() != last_id
        {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} committed record count or last identity is inconsistent",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    Ok(())
}

pub(super) fn verify_committed_prefixes(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let expected = stream_checkpoint(checkpoint, stream)?;
        let path = stream_path(paths, stream);
        let bytes = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        let committed = usize::try_from(expected.committed_bytes).map_err(|_| {
            LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} committed offset does not fit this platform",
                    stream.checkpoint_name()
                ),
            }
        })?;
        if bytes.len() < committed {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "{} is shorter than its committed prefix",
                    stream.checkpoint_name()
                ),
            });
        }
        if sha256_hex(&bytes[..committed]) != expected.committed_sha256 {
            return Err(LivePersistenceError::ExternalModification {
                message: format!(
                    "{} has corruption inside its committed prefix",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    for (name, path, expected) in [
        (
            "station",
            &paths.station,
            &checkpoint.active_plan.station_sha256,
        ),
        (
            "antennas",
            &paths.antennas,
            &checkpoint.active_plan.antennas_sha256,
        ),
        (
            "schedule",
            &paths.schedule,
            &checkpoint.active_plan.schedule_sha256,
        ),
    ] {
        let bytes = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        if sha256_hex(&bytes) != *expected {
            return Err(LivePersistenceError::ExternalModification {
                message: format!("active plan {name} has committed corruption"),
            });
        }
    }
    Ok(())
}

pub(super) fn stream_checkpoint(
    state: &SessionStateV2,
    stream: LiveStreamV2,
) -> Result<&StreamCheckpointV2, LivePersistenceError> {
    state.streams.get(stream.checkpoint_name()).ok_or_else(|| {
        LivePersistenceError::CheckpointVerification {
            message: format!("checkpoint is missing {}", stream.checkpoint_name()),
        }
    })
}

pub(super) fn read_json_file<T: serde::de::DeserializeOwned>(
    store: &BundleStore,
    path: &Path,
    description: &str,
) -> Result<T, LivePersistenceError> {
    let bytes = read_bounded(
        store,
        path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Read,
    )?;
    serde_json::from_slice(&bytes).map_err(|source| LivePersistenceError::CheckpointVerification {
        message: format!("{description} at {} is invalid: {source}", path.display()),
    })
}

fn read_jsonl_prefix<T: serde::de::DeserializeOwned>(
    store: &BundleStore,
    path: &Path,
    committed_bytes: u64,
) -> Result<Vec<T>, LivePersistenceError> {
    let bytes = read_bounded(
        store,
        path,
        store.profile().jsonl_stream_bytes,
        "resource.jsonl.stream_bytes",
        ResourceOperation::Read,
    )?;
    let end = usize::try_from(committed_bytes).map_err(|_| {
        LivePersistenceError::CheckpointVerification {
            message: format!(
                "committed prefix for {} does not fit this platform",
                path.display()
            ),
        }
    })?;
    let prefix = bytes
        .get(..end)
        .ok_or_else(|| LivePersistenceError::CheckpointVerification {
            message: format!("{} is shorter than its committed prefix", path.display()),
        })?;
    if !prefix.is_empty() && !prefix.ends_with(b"\n") {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "{} committed prefix is not newline terminated",
                path.display()
            ),
        });
    }
    prefix
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_slice(line).map_err(|source| {
                LivePersistenceError::CheckpointVerification {
                    message: format!(
                        "{} contains malformed committed JSONL: {source}",
                        path.display()
                    ),
                }
            })
        })
        .collect()
}

pub(super) fn write_plan_file(
    path: &Path,
    kind: LivePlanFile,
    bytes: &[u8],
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    hooks
        .check(LivePersistencePoint::BeforePlanWrite(kind))
        .map_err(|source| live_io("plan pre-write failpoint", path, source))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| live_io("create plan file", path, source))?;
    file.write_all(bytes)
        .map_err(|source| live_io("write plan file", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterPlanWrite(kind))
        .map_err(|source| live_io("plan post-write failpoint", path, source))?;
    hooks
        .check(LivePersistencePoint::BeforePlanSync(kind))
        .map_err(|source| live_io("plan pre-sync failpoint", path, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize plan file", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterPlanSync(kind))
        .map_err(|source| live_io("plan post-sync failpoint", path, source))?;
    Ok(())
}

pub(super) fn commit_checkpoint(
    root: &Path,
    current: &Path,
    checkpoint: &SessionStateV2,
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    let temp = root.join(CHECKPOINT_TEMP);
    let previous = root.join(CHECKPOINT_PREVIOUS);
    if temp.exists() {
        fs::remove_file(&temp)
            .map_err(|source| live_io("remove stale checkpoint temp", &temp, source))?;
    }
    let bytes = serialize_json(checkpoint).map_err(invalid_serialization)?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointWrite)
        .map_err(|source| live_io("checkpoint pre-write failpoint", &temp, source))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|source| live_io("create checkpoint temp", &temp, source))?;
    file.write_all(&bytes)
        .map_err(|source| live_io("write checkpoint temp", &temp, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointWrite)
        .map_err(|source| live_io("checkpoint post-write failpoint", &temp, source))?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointSync)
        .map_err(|source| live_io("checkpoint pre-sync failpoint", &temp, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize checkpoint temp", &temp, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointSync)
        .map_err(|source| live_io("checkpoint post-sync failpoint", &temp, source))?;
    drop(file);
    hooks
        .check(LivePersistencePoint::BeforeCheckpointReplace)
        .map_err(|source| live_io("checkpoint pre-replace failpoint", &temp, source))?;
    replace_checkpoint(&temp, current, &previous)
        .map_err(|source| live_io("atomically replace checkpoint", current, source))?;
    hooks
        .check(LivePersistencePoint::AfterCheckpointReplace)
        .map_err(|source| live_io("checkpoint post-replace failpoint", current, source))?;
    hooks
        .check(LivePersistencePoint::BeforeDirectorySync)
        .map_err(|source| live_io("directory pre-sync failpoint", root, source))?;
    sync_directory(root).map_err(|source| live_io("synchronize bundle directory", root, source))?;
    hooks
        .check(LivePersistencePoint::AfterDirectorySync)
        .map_err(|source| live_io("directory post-sync failpoint", root, source))?;
    hooks
        .check(LivePersistencePoint::BeforeCheckpointVerify)
        .map_err(|source| live_io("checkpoint pre-verify failpoint", current, source))?;
    let reopened = read_state(current)?;
    if &reopened != checkpoint {
        return Err(LivePersistenceError::CheckpointVerification {
            message: "reopened checkpoint differs from the promoted value".into(),
        });
    }
    hooks
        .check(LivePersistencePoint::AfterCheckpointVerify)
        .map_err(|source| live_io("checkpoint post-verify failpoint", current, source))?;
    Ok(())
}

pub(super) fn checkpoint_from_paths(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
) -> Result<SessionStateV2, LivePersistenceError> {
    let mut state = bundle.session_state.clone();
    state.streams = BTreeMap::new();
    for (stream, count, last_id) in [
        (
            LiveStreamV2::Events,
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.clone()),
        ),
        (
            LiveStreamV2::AdapterRecords,
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.clone()),
        ),
        (
            LiveStreamV2::Observations,
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.clone()),
        ),
        (
            LiveStreamV2::Rig,
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.clone()),
        ),
        (
            LiveStreamV2::Propagation,
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.clone()),
        ),
    ] {
        let bytes = read_bounded(
            store,
            stream_path(paths, stream),
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        state.streams.insert(
            stream.checkpoint_name().into(),
            checkpoint_for_bytes(&bytes, count, last_id),
        );
    }
    Ok(state)
}

pub(super) fn verify_exact_checkpoint(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let expected = checkpoint
            .streams
            .get(stream.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: format!("checkpoint is missing {}", stream.checkpoint_name()),
            })?;
        let path = stream_path(paths, stream);
        let bytes = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
        )?;
        if u64::try_from(bytes.len()).ok() != Some(expected.committed_bytes) {
            return Err(LivePersistenceError::RecoveryRequired {
                message: format!(
                    "{} has bytes outside its committed head",
                    stream.checkpoint_name()
                ),
            });
        }
        if sha256_hex(&bytes) != expected.committed_sha256 {
            return Err(LivePersistenceError::ExternalModification {
                message: format!(
                    "{} committed prefix digest changed",
                    stream.checkpoint_name()
                ),
            });
        }
    }
    for (name, path, expected) in [
        (
            "station",
            &paths.station,
            &checkpoint.active_plan.station_sha256,
        ),
        (
            "antennas",
            &paths.antennas,
            &checkpoint.active_plan.antennas_sha256,
        ),
        (
            "schedule",
            &paths.schedule,
            &checkpoint.active_plan.schedule_sha256,
        ),
    ] {
        let bytes = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        if sha256_hex(&bytes) != *expected {
            return Err(LivePersistenceError::ExternalModification {
                message: format!("active plan {name} digest changed"),
            });
        }
    }
    Ok(())
}

pub(super) fn read_state(path: &Path) -> Result<SessionStateV2, LivePersistenceError> {
    let size = fs::metadata(path)
        .map_err(|source| live_io("inspect checkpoint", path, source))?
        .len();
    if size > CHECKPOINT_MAX_BYTES {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "checkpoint {} exceeds the {} byte limit",
                path.display(),
                CHECKPOINT_MAX_BYTES
            ),
        });
    }
    let bytes = fs::read(path).map_err(|source| live_io("read checkpoint", path, source))?;
    serde_json::from_slice(&bytes).map_err(|source| LivePersistenceError::CheckpointVerification {
        message: format!("{} is not a valid checkpoint: {source}", path.display()),
    })
}

pub(super) fn stream_path(paths: &ResolvedBundlePathsV2, stream: LiveStreamV2) -> &Path {
    match stream {
        LiveStreamV2::AdapterRecords => &paths.adapter_records,
        LiveStreamV2::Observations => &paths.observations,
        LiveStreamV2::Events => &paths.events,
        LiveStreamV2::Rig => &paths.rig,
        LiveStreamV2::Propagation => &paths.propagation,
    }
}

pub(super) fn all_streams() -> [LiveStreamV2; 5] {
    [
        LiveStreamV2::AdapterRecords,
        LiveStreamV2::Observations,
        LiveStreamV2::Events,
        LiveStreamV2::Rig,
        LiveStreamV2::Propagation,
    ]
}
