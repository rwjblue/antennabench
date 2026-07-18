use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use antennabench_core::{
    v2::{
        AttachmentReference, BundleV2Contents, PlanGenerationV2, SessionLifecycleV2, SessionStateV2,
    },
    validate_bundle_report, AntennasFile, BundleValidationProfile, Schedule, Station,
    SCHEMA_VERSION_V2,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    attachments::durable_attachment,
    checkpoint::{
        all_streams, commit_checkpoint, read_json_file, stream_checkpoint, stream_path,
        CHECKPOINT_TEMP,
    },
    durability::sync_directory,
    invalid_serialization, live_io, LiveMutationMemberV2, LiveMutationV2, LivePersistenceError,
    LivePersistenceHooks, LiveStreamV2, RecoveryArtifactV2, RecoveryDispositionV2,
};
use crate::{
    resource::{read_bounded, ResourceOperation},
    v2::{serialize_json, sha256_hex, ResolvedBundlePathsV2},
    BundleStore,
};

#[derive(Debug, Serialize)]
struct RecoveryArtifactMetadataV2<'a> {
    schema_version: u16,
    session_id: &'a str,
    source: &'a str,
    committed_offset: u64,
    detected_at: DateTime<Utc>,
    diagnosis: &'a str,
    raw_attachment: &'a AttachmentReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PlanGenerationMetadataV2 {
    pub(super) schema_version: u16,
    pub(super) session_id: String,
    pub(super) base_revision: u64,
    pub(super) generation: PlanGenerationV2,
}

pub(super) struct StreamTail {
    pub(super) stream: LiveStreamV2,
    pub(super) committed_offset: u64,
    pub(super) bytes: Vec<u8>,
}

struct RecoveryBytes<'a> {
    source: &'a str,
    committed_offset: u64,
    bytes: &'a [u8],
}

pub(super) fn recover_pending_plan_generation(
    store: &BundleStore,
    bundle: &mut BundleV2Contents,
    paths: &mut ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    hooks: &dyn LivePersistenceHooks,
) -> Result<Option<RecoveryDispositionV2>, LivePersistenceError> {
    let generations_dir = store.root().join("plan-generations");
    if !generations_dir.exists() {
        return Ok(None);
    }
    let mut pending = Vec::<(PathBuf, PlanGenerationMetadataV2)>::new();
    let mut unresolved = Vec::<PathBuf>::new();
    let entries = fs::read_dir(&generations_dir)
        .map_err(|source| live_io("inspect plan generations", &generations_dir, source))?;
    for entry in entries {
        let entry = entry
            .map_err(|source| live_io("inspect plan generation entry", &generations_dir, source))?;
        let path = entry.path();
        let metadata = entry
            .file_type()
            .map_err(|source| live_io("inspect plan generation type", &path, source))?;
        if !metadata.is_dir() || metadata.is_symlink() {
            unresolved.push(path);
            continue;
        }
        if entry.file_name().to_string_lossy() == bundle.session_state.active_plan.generation_id {
            continue;
        }
        let marker = path.join("generation.json");
        match read_json_file::<PlanGenerationMetadataV2>(store, &marker, "plan generation metadata")
        {
            Ok(metadata)
                if metadata.schema_version == SCHEMA_VERSION_V2
                    && metadata.session_id == bundle.manifest.session_id
                    && metadata.base_revision == bundle.session_state.revision =>
            {
                pending.push((path, metadata));
            }
            Ok(metadata) if metadata.base_revision < bundle.session_state.revision => {
                // A formerly active generation remains immutable history.
            }
            _ => unresolved.push(path),
        }
    }

    if pending.len() == 1 && unresolved.is_empty() {
        let (generation_dir, metadata) = pending.pop().expect("one pending generation");
        match load_pending_plan(store, &generation_dir, &metadata) {
            Ok((station, antennas, schedule))
                if matches!(
                    bundle.session_state.lifecycle,
                    SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
                ) =>
            {
                let mut candidate = bundle.clone();
                candidate.station = station;
                candidate.antennas = antennas;
                candidate.schedule = schedule;
                candidate.session_state.active_plan = metadata.generation.clone();
                candidate.session_state.lifecycle = SessionLifecycleV2::Ready;
                let report = validate_bundle_report(&candidate.clone().into_current().bundle);
                if report.allows(BundleValidationProfile::StrictCreation) {
                    let mut next = candidate.session_state.clone();
                    next.revision = next.revision.checked_add(1).ok_or_else(|| {
                        LivePersistenceError::CheckpointVerification {
                            message: "checkpoint revision overflowed during plan recovery".into(),
                        }
                    })?;
                    next.last_committed_mutation_id =
                        Some(format!("plan:{}", metadata.generation.generation_id));
                    commit_checkpoint(store.root(), &paths.session_state, &next, hooks)?;
                    candidate.session_state = next;
                    *bundle = candidate;
                    *paths =
                        store.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
                    return Ok(Some(RecoveryDispositionV2::RolledForward));
                }
            }
            _ => {}
        }
        unresolved.push(generation_dir);
    } else {
        unresolved.extend(pending.into_iter().map(|(path, _)| path));
    }

    if unresolved.is_empty() {
        return Ok(None);
    }
    unresolved.sort();
    let diagnosis = if unresolved.len() == 1 {
        "plan generation is incomplete, malformed, or invalid"
    } else {
        "multiple pending plan generations conflict"
    };
    for generation in unresolved {
        preserve_plan_generation(
            store,
            bundle,
            paths,
            &generation,
            diagnosis,
            hooks.now(),
            artifacts,
        )?;
        if generation.is_dir() {
            fs::remove_dir_all(&generation).map_err(|source| {
                live_io("remove recovered plan generation", &generation, source)
            })?;
        } else {
            fs::remove_file(&generation)
                .map_err(|source| live_io("remove recovered plan artifact", &generation, source))?;
        }
    }
    sync_directory(&generations_dir).map_err(|source| {
        live_io(
            "synchronize recovered plan generation cleanup",
            &generations_dir,
            source,
        )
    })?;
    Ok(Some(RecoveryDispositionV2::RolledBack))
}

fn load_pending_plan(
    store: &BundleStore,
    generation_dir: &Path,
    metadata: &PlanGenerationMetadataV2,
) -> Result<(Station, AntennasFile, Schedule), LivePersistenceError> {
    let station_path = generation_dir.join("station.json");
    let antennas_path = generation_dir.join("antennas.json");
    let schedule_path = generation_dir.join("schedule.json");
    let station_bytes = read_bounded(
        store,
        &station_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let antennas_bytes = read_bounded(
        store,
        &antennas_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let schedule_bytes = read_bounded(
        store,
        &schedule_path,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    let digests = [
        sha256_hex(&station_bytes),
        sha256_hex(&antennas_bytes),
        sha256_hex(&schedule_bytes),
    ];
    if digests[0] != metadata.generation.station_sha256
        || digests[1] != metadata.generation.antennas_sha256
        || digests[2] != metadata.generation.schedule_sha256
        || sha256_hex(digests.join("\n").as_bytes()) != metadata.generation.root_sha256
    {
        return Err(LivePersistenceError::CheckpointVerification {
            message: "pending plan generation digest does not match its durable metadata".into(),
        });
    }
    let station = serde_json::from_slice(&station_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending station plan is invalid: {source}"),
        }
    })?;
    let antennas = serde_json::from_slice(&antennas_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending antenna plan is invalid: {source}"),
        }
    })?;
    let schedule = serde_json::from_slice(&schedule_bytes).map_err(|source| {
        LivePersistenceError::CheckpointVerification {
            message: format!("pending schedule plan is invalid: {source}"),
        }
    })?;
    Ok((station, antennas, schedule))
}

fn preserve_plan_generation(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    generation: &Path,
    diagnosis: &str,
    detected_at: DateTime<Utc>,
    artifacts: &mut Vec<RecoveryArtifactV2>,
) -> Result<(), LivePersistenceError> {
    if generation.is_file() {
        let bytes = read_bounded(
            store,
            generation,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        let source = format!(
            "plan-generation/{}",
            generation.file_name().unwrap_or_default().to_string_lossy()
        );
        artifacts.push(preserve_recovery_bytes(
            store,
            SCHEMA_VERSION_V2,
            &bundle.manifest.session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: 0,
                bytes: &bytes,
            },
            diagnosis,
            detected_at,
        )?);
        return Ok(());
    }
    let mut entries = fs::read_dir(generation)
        .map_err(|source| live_io("inspect unresolved plan generation", generation, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| live_io("inspect unresolved plan generation", generation, source))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|source| live_io("inspect unresolved plan file", &path, source))?
            .is_file()
        {
            return Err(LivePersistenceError::RecoveryRequired {
                message: format!("unsupported entry remains in {}", generation.display()),
            });
        }
        let bytes = read_bounded(
            store,
            &path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Write,
        )?;
        let source = format!(
            "plan-generation/{}/{}",
            generation.file_name().unwrap_or_default().to_string_lossy(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        artifacts.push(preserve_recovery_bytes(
            store,
            SCHEMA_VERSION_V2,
            &bundle.manifest.session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: 0,
                bytes: &bytes,
            },
            diagnosis,
            detected_at,
        )?);
    }
    Ok(())
}

pub(super) fn read_stream_tails(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<Vec<StreamTail>, LivePersistenceError> {
    all_streams()
        .into_iter()
        .map(|stream| {
            let committed_offset = stream_checkpoint(checkpoint, stream)?.committed_bytes;
            let path = stream_path(paths, stream);
            let bytes = read_bounded(
                store,
                path,
                store.profile().jsonl_stream_bytes,
                "resource.jsonl.stream_bytes",
                ResourceOperation::Write,
            )?;
            let offset = usize::try_from(committed_offset).map_err(|_| {
                LivePersistenceError::CheckpointVerification {
                    message: "committed offset does not fit this platform".into(),
                }
            })?;
            Ok(StreamTail {
                stream,
                committed_offset,
                bytes: bytes[offset..].to_vec(),
            })
        })
        .collect()
}

pub(super) fn parse_tail_mutation(
    bundle: &BundleV2Contents,
    tails: &[StreamTail],
) -> Result<LiveMutationV2, String> {
    let mut members = Vec::new();
    for tail in tails.iter().filter(|tail| !tail.bytes.is_empty()) {
        if !tail.bytes.ends_with(b"\n") {
            return Err(format!(
                "{} tail is torn and not newline terminated",
                tail.stream.checkpoint_name()
            ));
        }
        for line in tail.bytes.split_inclusive(|byte| *byte == b'\n') {
            let line = &line[..line.len() - 1];
            if line.is_empty() {
                return Err(format!(
                    "{} tail contains an empty member",
                    tail.stream.checkpoint_name()
                ));
            }
            members.push(parse_tail_member(tail.stream, line).map_err(|error| {
                format!(
                    "{} tail contains malformed JSON: {error}",
                    tail.stream.checkpoint_name()
                )
            })?);
        }
    }
    if members.is_empty() {
        return Err("no complete tail mutation members were found".into());
    }
    members.sort_by_key(|member| member.meta().mutation.member_index);
    let mutation_id = members[0].meta().mutation.mutation_id.clone();
    let member_count = members[0].meta().mutation.member_count;
    if mutation_id.is_empty()
        || member_count != u32::try_from(members.len()).unwrap_or(u32::MAX)
        || members.iter().enumerate().any(|(index, member)| {
            member.meta().mutation.mutation_id != mutation_id
                || member.meta().mutation.member_count != member_count
                || member.meta().mutation.member_index
                    != u32::try_from(index).expect("tail member count fits u32")
        })
    {
        return Err("tail does not contain one complete declared mutation".into());
    }
    Ok(LiveMutationV2 {
        expected_revision: bundle.session_state.revision,
        mutation_id,
        members,
    })
}

fn parse_tail_member(
    stream: LiveStreamV2,
    line: &[u8],
) -> Result<LiveMutationMemberV2, serde_json::Error> {
    match stream {
        LiveStreamV2::Events => serde_json::from_slice(line).map(LiveMutationMemberV2::Event),
        LiveStreamV2::AdapterRecords => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::AdapterRecord)
        }
        LiveStreamV2::Observations => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::Observation)
        }
        LiveStreamV2::Rig => serde_json::from_slice(line).map(LiveMutationMemberV2::Rig),
        LiveStreamV2::Propagation => {
            serde_json::from_slice(line).map(LiveMutationMemberV2::Propagation)
        }
    }
}

pub(super) fn preserve_tails(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    tails: &[StreamTail],
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<Vec<RecoveryArtifactV2>, LivePersistenceError> {
    preserve_tails_for(
        store,
        SCHEMA_VERSION_V2,
        &bundle.manifest.session_id,
        paths,
        tails,
        diagnosis,
        detected_at,
    )
}

pub(super) fn preserve_tails_for(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    tails: &[StreamTail],
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<Vec<RecoveryArtifactV2>, LivePersistenceError> {
    let mut artifacts = Vec::new();
    for tail in tails.iter().filter(|tail| !tail.bytes.is_empty()) {
        let source = tail.stream.checkpoint_name().to_string();
        artifacts.push(preserve_recovery_bytes(
            store,
            schema_version,
            session_id,
            paths,
            RecoveryBytes {
                source: &source,
                committed_offset: tail.committed_offset,
                bytes: &tail.bytes,
            },
            diagnosis,
            detected_at,
        )?);
    }
    Ok(artifacts)
}

fn preserve_recovery_bytes(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    evidence: RecoveryBytes<'_>,
    diagnosis: &str,
    detected_at: DateTime<Utc>,
) -> Result<RecoveryArtifactV2, LivePersistenceError> {
    let RecoveryBytes {
        source,
        committed_offset,
        bytes,
    } = evidence;
    let raw_attachment = durable_attachment(
        store,
        paths,
        bytes,
        "application/octet-stream",
        Some(format!("recovery:{source}:{committed_offset}")),
    )?;
    let metadata = RecoveryArtifactMetadataV2 {
        schema_version,
        session_id,
        source,
        committed_offset,
        detected_at,
        diagnosis,
        raw_attachment: &raw_attachment,
    };
    let metadata_bytes = serialize_json(&metadata).map_err(invalid_serialization)?;
    let metadata_attachment = durable_attachment(
        store,
        paths,
        &metadata_bytes,
        "application/json",
        Some(format!("recovery-metadata:{source}:{committed_offset}")),
    )?;
    Ok(RecoveryArtifactV2 {
        source: source.into(),
        committed_offset,
        diagnosis: diagnosis.into(),
        raw_attachment,
        metadata_attachment,
    })
}

pub(super) fn truncate_tails(
    checkpoint: &SessionStateV2,
    paths: &ResolvedBundlePathsV2,
) -> Result<(), LivePersistenceError> {
    for stream in all_streams() {
        let committed = stream_checkpoint(checkpoint, stream)?.committed_bytes;
        let path = stream_path(paths, stream);
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|source| live_io("open recovery truncation", path, source))?;
        file.set_len(committed)
            .map_err(|source| live_io("truncate recovered stream", path, source))?;
        file.sync_all()
            .map_err(|source| live_io("synchronize recovered stream", path, source))?;
    }
    sync_directory(paths.session_state.parent().expect("checkpoint has parent")).map_err(|source| {
        live_io(
            "synchronize recovered bundle directory",
            paths.session_state.parent().expect("checkpoint has parent"),
            source,
        )
    })
}

pub(super) fn recover_checkpoint_temp(
    store: &BundleStore,
    bundle: &BundleV2Contents,
    paths: &ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    detected_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    recover_checkpoint_temp_for(
        store,
        SCHEMA_VERSION_V2,
        &bundle.manifest.session_id,
        paths,
        artifacts,
        detected_at,
    )
}

pub(super) fn recover_checkpoint_temp_for(
    store: &BundleStore,
    schema_version: u16,
    session_id: &str,
    paths: &ResolvedBundlePathsV2,
    artifacts: &mut Vec<RecoveryArtifactV2>,
    detected_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    let temp = store.root().join(CHECKPOINT_TEMP);
    if !temp.exists() {
        return Ok(());
    }
    let bytes = read_bounded(
        store,
        &temp,
        store.profile().root_json_bytes,
        "resource.bundle.root_json_bytes",
        ResourceOperation::Write,
    )?;
    if !bytes.is_empty() && serde_json::from_slice::<SessionStateV2>(&bytes).is_err() {
        let diagnosis = "checkpoint temporary file is malformed";
        let raw_attachment = durable_attachment(
            store,
            paths,
            &bytes,
            "application/json",
            Some("recovery:checkpoint-temp".into()),
        )?;
        let metadata = RecoveryArtifactMetadataV2 {
            schema_version,
            session_id,
            source: "checkpoint_temp",
            committed_offset: 0,
            detected_at,
            diagnosis,
            raw_attachment: &raw_attachment,
        };
        let metadata_bytes = serialize_json(&metadata).map_err(invalid_serialization)?;
        let metadata_attachment = durable_attachment(
            store,
            paths,
            &metadata_bytes,
            "application/json",
            Some("recovery-metadata:checkpoint-temp".into()),
        )?;
        artifacts.push(RecoveryArtifactV2 {
            source: "checkpoint_temp".into(),
            committed_offset: 0,
            diagnosis: diagnosis.into(),
            raw_attachment,
            metadata_attachment,
        });
    }
    fs::remove_file(&temp)
        .map_err(|source| live_io("remove recovered checkpoint temp", &temp, source))?;
    sync_directory(store.root())
        .map_err(|source| live_io("synchronize checkpoint temp cleanup", store.root(), source))
}
