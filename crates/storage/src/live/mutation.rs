use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::Write,
    path::Path,
};

use antennabench_core::{
    v2::{
        validate_lifecycle_transition_v2, validate_operator_event_append_v2, BundleV2Contents,
        MutationMember, NormalizedRecordKind, OperatorEventPayloadV2, OperatorEventV2,
        RecordMetaV2, SessionLifecycleV2, SessionStateV2,
    },
    v3::{BundleV3Contents, OperatorEventV3},
    validate_bundle_report, validate_machine_identity, BundleValidationProfile, SCHEMA_VERSION_V2,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{
    checkpoint::{all_streams, stream_checkpoint, stream_path},
    live_io, CommitReceiptV2, LiveAntennaControlMutationV5, LiveEvidenceMutationV3,
    LiveMutationMemberV2, LiveMutationV2, LivePersistenceError, LivePersistenceHooks,
    LivePersistencePoint, LiveStreamV2,
};
use crate::{v2::ResolvedBundlePathsV2, BundleStore};

impl LiveMutationMemberV2 {
    pub(super) fn stream(&self) -> LiveStreamV2 {
        match self {
            Self::Event(_) => LiveStreamV2::Events,
            Self::AdapterRecord(_) => LiveStreamV2::AdapterRecords,
            Self::Observation(_) => LiveStreamV2::Observations,
            Self::Rig(_) => LiveStreamV2::Rig,
            Self::Propagation(_) => LiveStreamV2::Propagation,
        }
    }

    pub(super) fn record_id(&self) -> &str {
        match self {
            Self::Event(record) => &record.event_id,
            Self::AdapterRecord(record) => &record.record_id,
            Self::Observation(record) => &record.observation_id,
            Self::Rig(record) => &record.record_id,
            Self::Propagation(record) => &record.record_id,
        }
    }

    pub(super) fn meta(&self) -> &RecordMetaV2 {
        match self {
            Self::Event(record) => &record.meta,
            Self::AdapterRecord(record) => &record.meta,
            Self::Observation(record) => &record.meta,
            Self::Rig(record) => &record.meta,
            Self::Propagation(record) => &record.meta,
        }
    }

    pub(super) fn meta_mut(&mut self) -> &mut RecordMetaV2 {
        match self {
            Self::Event(record) => &mut record.meta,
            Self::AdapterRecord(record) => &mut record.meta,
            Self::Observation(record) => &mut record.meta,
            Self::Rig(record) => &mut record.meta,
            Self::Propagation(record) => &mut record.meta,
        }
    }

    pub(super) fn append_to(self, bundle: &mut BundleV2Contents) {
        match self {
            Self::Event(record) => bundle.events.push(record),
            Self::AdapterRecord(record) => bundle.adapter_records.push(record),
            Self::Observation(record) => bundle.observations.push(record),
            Self::Rig(record) => bundle.rig.push(record),
            Self::Propagation(record) => bundle.propagation.push(record),
        }
    }

    pub(super) fn serialized_line(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = match self {
            Self::Event(record) => serde_json::to_vec(record)?,
            Self::AdapterRecord(record) => serde_json::to_vec(record)?,
            Self::Observation(record) => serde_json::to_vec(record)?,
            Self::Rig(record) => serde_json::to_vec(record)?,
            Self::Propagation(record) => serde_json::to_vec(record)?,
        };
        bytes.push(b'\n');
        Ok(bytes)
    }
}

pub(super) fn prepare_v3_evidence(
    mutation: &mut LiveEvidenceMutationV3,
    session_id: &str,
    schema_version: u16,
    recorded_at: DateTime<Utc>,
    runtime_context_id: Option<&str>,
    member_offset: u32,
    member_count: u32,
) -> Result<(), LivePersistenceError> {
    if validate_machine_identity(&mutation.mutation_id).is_err()
        || mutation.adapter_records.is_empty()
    {
        return Err(LivePersistenceError::InvalidMutation {
            message: "evidence mutation requires a bounded identity and adapter records".into(),
        });
    }
    let primary_count = u32::try_from(mutation.adapter_records.len() + mutation.observations.len())
        .map_err(|_| LivePersistenceError::InvalidMutation {
            message: "evidence mutation has too many members".into(),
        })?;
    if member_count < primary_count || member_offset > member_count - primary_count {
        return Err(LivePersistenceError::InvalidMutation {
            message: "evidence mutation membership is inconsistent".into(),
        });
    }
    for (index, record) in mutation.adapter_records.iter_mut().enumerate() {
        prepare_v3_evidence_meta(
            &mut record.meta,
            &record.record_id,
            session_id,
            schema_version,
            &mutation.mutation_id,
            member_offset + u32::try_from(index).expect("member count fits u32"),
            member_count,
            recorded_at,
            runtime_context_id,
        )?;
    }
    for (offset, record) in mutation.observations.iter_mut().enumerate() {
        prepare_v3_evidence_meta(
            &mut record.meta,
            &record.observation_id,
            session_id,
            schema_version,
            &mutation.mutation_id,
            member_offset
                + u32::try_from(mutation.adapter_records.len() + offset)
                    .expect("member count fits u32"),
            member_count,
            recorded_at,
            runtime_context_id,
        )?;
    }
    Ok(())
}

pub(super) fn committed_v5_antenna_control(
    bundle: &BundleV3Contents,
    mutation: &LiveAntennaControlMutationV5,
) -> Option<Result<CommitReceiptV2, LivePersistenceError>> {
    let existing_rig = bundle
        .rig
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation.mutation_id)
        .collect::<Vec<_>>();
    let existing_event = bundle
        .events
        .iter()
        .find(|event| event.meta.mutation.mutation_id == mutation.mutation_id);
    if existing_rig.is_empty() && existing_event.is_none() {
        return None;
    }
    let rig_matches = existing_rig.len() == mutation.rig_records.len()
        && existing_rig
            .iter()
            .zip(&mutation.rig_records)
            .all(|(existing, proposed)| {
                existing.record_id == proposed.record_id
                    && existing.adapter_record_ids == proposed.adapter_record_ids
                    && existing.status == proposed.status
                    && existing.frequency_hz == proposed.frequency_hz
                    && existing.mode == proposed.mode
                    && existing.power_watts == proposed.power_watts
                    && existing.antenna_control == proposed.antenna_control
                    && existing.raw == proposed.raw
            });
    let event_matches = match (existing_event, mutation.armed_event.as_ref()) {
        (None, None) => true,
        (Some(existing), Some(proposed)) => {
            existing.event_id == proposed.event_id
                && existing.occurred_at == proposed.occurred_at
                && existing.time_basis == proposed.time_basis
                && existing.uncertainty_seconds == proposed.uncertainty_seconds
                && existing.slot_id == proposed.slot_id
                && existing.payload == proposed.payload
        }
        _ => false,
    };
    Some(if rig_matches && event_matches {
        Ok(CommitReceiptV2 {
            revision: bundle.session_state.revision,
            mutation_id: mutation.mutation_id.clone(),
            idempotent: true,
        })
    } else {
        Err(LivePersistenceError::MutationConflict {
            mutation_id: mutation.mutation_id.clone(),
        })
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_v3_evidence_meta(
    meta: &mut RecordMetaV2,
    record_id: &str,
    session_id: &str,
    schema_version: u16,
    mutation_id: &str,
    member_index: u32,
    member_count: u32,
    recorded_at: DateTime<Utc>,
    runtime_context_id: Option<&str>,
) -> Result<(), LivePersistenceError> {
    if validate_machine_identity(record_id).is_err() {
        return Err(LivePersistenceError::InvalidMutation {
            message: "evidence record identities must be bounded nonempty ASCII".into(),
        });
    }
    meta.schema_version = schema_version;
    meta.session_id = session_id.into();
    meta.recorded_at = recorded_at;
    meta.mutation = MutationMember {
        mutation_id: mutation_id.into(),
        member_index,
        member_count,
    };
    meta.runtime_context_id = runtime_context_id.map(str::to_string);
    Ok(())
}

pub(super) fn validate_v3_evidence(
    bundle: &BundleV3Contents,
    mutation: &LiveEvidenceMutationV3,
) -> Result<(), LivePersistenceError> {
    if matches!(
        bundle.session_state.lifecycle,
        SessionLifecycleV2::Draft | SessionLifecycleV2::Ready
    ) {
        return Err(LivePersistenceError::InvalidMutation {
            message: "adapter evidence may append only after the session has started".into(),
        });
    }
    let mut ids = bundle
        .events
        .iter()
        .map(|record| record.event_id.as_str())
        .chain(
            bundle
                .adapter_records
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .chain(
            bundle
                .observations
                .iter()
                .map(|record| record.observation_id.as_str()),
        )
        .chain(bundle.rig.iter().map(|record| record.record_id.as_str()))
        .chain(
            bundle
                .propagation
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    for record in &mutation.adapter_records {
        if !ids.insert(&record.record_id) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!("record identity {:?} is already present", record.record_id),
            });
        }
        for link in &record.normalized_records {
            if link.record_kind != NormalizedRecordKind::Observation
                || !bundle
                    .observations
                    .iter()
                    .chain(mutation.observations.iter())
                    .any(|observation| observation.observation_id == link.record_id)
            {
                return Err(LivePersistenceError::InvalidMutation {
                    message: format!(
                        "adapter record {:?} has a missing normalized observation link",
                        record.record_id
                    ),
                });
            }
        }
    }
    for record in &mutation.observations {
        if !ids.insert(&record.observation_id) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "record identity {:?} is already present",
                    record.observation_id
                ),
            });
        }
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle
                    .adapter_records
                    .iter()
                    .chain(mutation.adapter_records.iter())
                    .any(|adapter| adapter.record_id == *adapter_id)
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "observation {:?} has missing adapter backlinks",
                    record.observation_id
                ),
            });
        }
    }
    Ok(())
}

pub(super) fn v3_committed_evidence(
    bundle: &BundleV3Contents,
    mutation_id: &str,
) -> Option<LiveEvidenceMutationV3> {
    let adapter_records = bundle
        .adapter_records
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .collect::<Vec<_>>();
    let observations = bundle
        .observations
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .collect::<Vec<_>>();
    (!adapter_records.is_empty() || !observations.is_empty()).then(|| LiveEvidenceMutationV3 {
        expected_revision: bundle.session_state.revision,
        mutation_id: mutation_id.into(),
        adapter_records,
        observations,
    })
}

pub(super) fn same_v3_evidence_business_value(
    existing: &LiveEvidenceMutationV3,
    proposed: &LiveEvidenceMutationV3,
) -> bool {
    if existing.adapter_records.len() != proposed.adapter_records.len()
        || existing.observations.len() != proposed.observations.len()
    {
        return false;
    }
    existing
        .adapter_records
        .iter()
        .zip(&proposed.adapter_records)
        .all(|(existing, proposed)| {
            let mut proposed = proposed.clone();
            proposed.meta = existing.meta.clone();
            existing == &proposed
        })
        && existing
            .observations
            .iter()
            .zip(&proposed.observations)
            .all(|(existing, proposed)| {
                let mut proposed = proposed.clone();
                proposed.meta = existing.meta.clone();
                existing == &proposed
            })
}

pub(super) fn serialize_v3_lines<T: Serialize>(
    records: &[T],
    label: &str,
) -> Result<Vec<u8>, LivePersistenceError> {
    let mut bytes = Vec::new();
    for record in records {
        serde_json::to_writer(&mut bytes, record).map_err(|source| {
            LivePersistenceError::InvalidMutation {
                message: format!("{label} serialization failed: {source}"),
            }
        })?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

pub(super) fn rollback_v3_streams(
    paths: &ResolvedBundlePathsV2,
    baseline: &SessionStateV2,
    streams: &[LiveStreamV2],
) -> Result<(), LivePersistenceError> {
    for stream in streams {
        let checkpoint = baseline
            .streams
            .get(stream.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: format!(
                    "baseline checkpoint is missing {}",
                    stream.checkpoint_name()
                ),
            })?;
        let path = stream_path(paths, *stream);
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|source| live_io("open schema-v3 evidence rollback", path, source))?;
        file.set_len(checkpoint.committed_bytes)
            .map_err(|source| live_io("truncate schema-v3 evidence rollback", path, source))?;
        file.sync_all()
            .map_err(|source| live_io("synchronize schema-v3 evidence rollback", path, source))?;
    }
    Ok(())
}

pub(super) fn same_v3_event_business_value(
    existing: &OperatorEventV3,
    proposed: &OperatorEventV3,
) -> bool {
    let mut proposed = proposed.clone();
    proposed.meta = existing.meta.clone();
    existing == &proposed
}

pub(super) fn v3_other_stream_has_mutation(bundle: &BundleV3Contents, mutation_id: &str) -> bool {
    bundle
        .adapter_records
        .iter()
        .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .observations
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .rig
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
        || bundle
            .propagation
            .iter()
            .any(|record| record.meta.mutation.mutation_id == mutation_id)
}

pub(super) fn prepare_mutation(
    mutation: &mut LiveMutationV2,
    session_id: &str,
    recorded_at: DateTime<Utc>,
) -> Result<(), LivePersistenceError> {
    if mutation.mutation_id.is_empty() || mutation.members.is_empty() {
        return Err(LivePersistenceError::InvalidMutation {
            message: "mutation ID and members must not be empty".into(),
        });
    }
    let member_count = u32::try_from(mutation.members.len()).map_err(|_| {
        LivePersistenceError::InvalidMutation {
            message: "mutation has too many members".into(),
        }
    })?;
    mutation
        .members
        .sort_by_key(|member| member.meta().mutation.member_index);
    for (index, member) in mutation.members.iter_mut().enumerate() {
        if member.record_id().is_empty() {
            return Err(LivePersistenceError::InvalidMutation {
                message: "record identities must not be empty".into(),
            });
        }
        let expected_index = u32::try_from(index).expect("member count fits u32");
        if member.meta().mutation.member_index != expected_index {
            return Err(LivePersistenceError::InvalidMutation {
                message: "member indexes must be contiguous from zero".into(),
            });
        }
        let meta = member.meta_mut();
        meta.schema_version = SCHEMA_VERSION_V2;
        meta.session_id = session_id.to_string();
        meta.recorded_at = recorded_at;
        meta.mutation.mutation_id = mutation.mutation_id.clone();
        meta.mutation.member_count = member_count;
    }
    Ok(())
}

pub(super) fn validate_mutation(
    bundle: &BundleV2Contents,
    mutation: &LiveMutationV2,
) -> Result<SessionLifecycleV2, LivePersistenceError> {
    let declared_count = u32::try_from(mutation.members.len()).map_err(|_| {
        LivePersistenceError::InvalidMutation {
            message: "mutation has too many members".into(),
        }
    })?;
    for (index, member) in mutation.members.iter().enumerate() {
        let meta = member.meta();
        if meta.schema_version != SCHEMA_VERSION_V2
            || meta.session_id != bundle.manifest.session_id
            || meta.mutation.mutation_id != mutation.mutation_id
            || meta.mutation.member_count != declared_count
            || meta.mutation.member_index != u32::try_from(index).expect("member count fits u32")
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: "mutation member envelope does not match the bundle and declared batch"
                    .into(),
            });
        }
    }
    let mut ids = bundle
        .events
        .iter()
        .map(|record| record.event_id.as_str())
        .chain(
            bundle
                .adapter_records
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .chain(
            bundle
                .observations
                .iter()
                .map(|record| record.observation_id.as_str()),
        )
        .chain(bundle.rig.iter().map(|record| record.record_id.as_str()))
        .chain(
            bundle
                .propagation
                .iter()
                .map(|record| record.record_id.as_str()),
        )
        .collect::<BTreeSet<_>>();
    let mut next_lifecycle = bundle.session_state.lifecycle;
    let mut event_count = 0;
    for member in &mutation.members {
        if !ids.insert(member.record_id()) {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "record identity {:?} is already present",
                    member.record_id()
                ),
            });
        }
        if let LiveMutationMemberV2::Event(event) = member {
            event_count += 1;
            let initial =
                if bundle.events.is_empty() {
                    bundle.session_state.lifecycle
                } else if bundle.events.iter().any(|event| {
                    matches!(event.payload, OperatorEventPayloadV2::SessionStarted { .. })
                }) {
                    SessionLifecycleV2::Ready
                } else {
                    SessionLifecycleV2::Draft
                };
            validate_operator_event_append_v2(
                initial,
                bundle.session_state.revision,
                mutation.expected_revision,
                &bundle.events,
                event,
            )
            .map_err(|error| LivePersistenceError::InvalidMutation {
                message: error.to_string(),
            })?;
            if is_lifecycle_payload(&event.payload) {
                next_lifecycle = validate_lifecycle_transition_v2(
                    next_lifecycle,
                    bundle.session_state.revision,
                    mutation.expected_revision,
                    &event.payload,
                )
                .map_err(|error| LivePersistenceError::InvalidMutation {
                    message: error.to_string(),
                })?;
            }
        }
    }
    if event_count > 1 {
        return Err(LivePersistenceError::InvalidMutation {
            message: "one operator action may append only one event".into(),
        });
    }
    if mutation
        .members
        .iter()
        .any(|member| !matches!(member, LiveMutationMemberV2::Event(_)))
        && bundle.session_state.lifecycle != SessionLifecycleV2::Running
    {
        return Err(LivePersistenceError::InvalidMutation {
            message: "adapter and normalized evidence may append only while running".into(),
        });
    }

    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.as_str())
        .collect::<BTreeSet<_>>();
    for member in &mutation.members {
        if let LiveMutationMemberV2::Event(OperatorEventV2 {
            payload: OperatorEventPayloadV2::AntennaStateConfirmed { antenna_label, .. },
            ..
        }) = member
        {
            if !antenna_labels.contains(antenna_label.as_str()) {
                return Err(LivePersistenceError::InvalidMutation {
                    message: format!(
                        "actual antenna label {antenna_label:?} is not in the active plan"
                    ),
                });
            }
        }
    }

    let mut candidate = bundle.clone();
    for member in mutation.members.clone() {
        member.append_to(&mut candidate);
    }
    candidate.session_state.lifecycle = next_lifecycle;
    validate_adapter_links(&candidate)?;
    let report = validate_bundle_report(&candidate.into_current().bundle);
    if !report.allows(BundleValidationProfile::StrictCreation) {
        return Err(LivePersistenceError::InvalidMutation {
            message: report
                .blocking_diagnostics(BundleValidationProfile::StrictCreation)
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    Ok(next_lifecycle)
}

pub(super) fn validate_adapter_links(
    bundle: &BundleV2Contents,
) -> Result<(), LivePersistenceError> {
    for observation in &bundle.observations {
        if observation.adapter_record_ids.is_empty()
            || !observation.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind
                                == antennabench_core::v2::NormalizedRecordKind::Observation
                                && link.record_id == observation.observation_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "observation {:?} lacks reciprocal adapter evidence",
                    observation.observation_id
                ),
            });
        }
    }
    for record in &bundle.rig {
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind == antennabench_core::v2::NormalizedRecordKind::Rig
                                && link.record_id == record.record_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "rig record {:?} lacks reciprocal adapter evidence",
                    record.record_id
                ),
            });
        }
    }
    for record in &bundle.propagation {
        if record.adapter_record_ids.is_empty()
            || !record.adapter_record_ids.iter().all(|adapter_id| {
                bundle.adapter_records.iter().any(|adapter| {
                    adapter.record_id == *adapter_id
                        && adapter.normalized_records.iter().any(|link| {
                            link.record_kind
                                == antennabench_core::v2::NormalizedRecordKind::Propagation
                                && link.record_id == record.record_id
                        })
                })
            })
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "propagation record {:?} lacks reciprocal adapter evidence",
                    record.record_id
                ),
            });
        }
    }
    Ok(())
}

pub(super) fn is_lifecycle_payload(payload: &OperatorEventPayloadV2) -> bool {
    matches!(
        payload,
        OperatorEventPayloadV2::SessionStarted { .. }
            | OperatorEventPayloadV2::SessionInterrupted { .. }
            | OperatorEventPayloadV2::InterruptionDetected { .. }
            | OperatorEventPayloadV2::SessionResumed { .. }
            | OperatorEventPayloadV2::SessionEnded { .. }
            | OperatorEventPayloadV2::SessionAbandoned { .. }
    )
}

pub(super) fn committed_mutation(
    bundle: &BundleV2Contents,
    mutation_id: &str,
) -> Option<Vec<LiveMutationMemberV2>> {
    let mut members = bundle
        .events
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .cloned()
        .map(LiveMutationMemberV2::Event)
        .chain(
            bundle
                .adapter_records
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::AdapterRecord),
        )
        .chain(
            bundle
                .observations
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Observation),
        )
        .chain(
            bundle
                .rig
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Rig),
        )
        .chain(
            bundle
                .propagation
                .iter()
                .filter(|record| record.meta.mutation.mutation_id == mutation_id)
                .cloned()
                .map(LiveMutationMemberV2::Propagation),
        )
        .collect::<Vec<_>>();
    if members.is_empty() {
        None
    } else {
        members.sort_by_key(|member| member.meta().mutation.member_index);
        Some(members)
    }
}

pub(super) fn same_business_members(
    existing: &[LiveMutationMemberV2],
    proposed: &[LiveMutationMemberV2],
) -> bool {
    if existing.len() != proposed.len() {
        return false;
    }
    let mut proposed = proposed.to_vec();
    proposed.sort_by_key(|member| member.meta().mutation.member_index);
    existing
        .iter()
        .zip(proposed)
        .all(|(existing, mut proposed)| {
            proposed.meta_mut().schema_version = existing.meta().schema_version;
            proposed.meta_mut().session_id = existing.meta().session_id.clone();
            proposed.meta_mut().recorded_at = existing.meta().recorded_at;
            proposed.meta_mut().mutation = existing.meta().mutation.clone();
            existing == &proposed
        })
}

pub(super) fn append_line(
    path: &Path,
    stream: LiveStreamV2,
    bytes: &[u8],
    hooks: &dyn LivePersistenceHooks,
) -> Result<(), LivePersistenceError> {
    hooks
        .check(LivePersistencePoint::BeforeStreamWrite(stream))
        .map_err(|source| live_io("stream write failpoint", path, source))?;
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|source| live_io("open stream append", path, source))?;
    let split = bytes.len() / 2;
    file.write_all(&bytes[..split])
        .map_err(|source| live_io("append stream prefix", path, source))?;
    hooks
        .check(LivePersistencePoint::MidStreamWrite(stream))
        .map_err(|source| live_io("stream mid-write failpoint", path, source))?;
    file.write_all(&bytes[split..])
        .map_err(|source| live_io("append stream suffix", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterStreamWrite(stream))
        .map_err(|source| live_io("stream post-write failpoint", path, source))?;
    hooks
        .check(LivePersistencePoint::BeforeStreamSync(stream))
        .map_err(|source| live_io("stream pre-sync failpoint", path, source))?;
    file.sync_all()
        .map_err(|source| live_io("synchronize stream", path, source))?;
    hooks
        .check(LivePersistencePoint::AfterStreamSync(stream))
        .map_err(|source| live_io("stream post-sync failpoint", path, source))?;
    Ok(())
}

pub(super) fn preflight_live_budget(
    store: &BundleStore,
    checkpoint: &SessionStateV2,
    serialized: &[(LiveStreamV2, Vec<u8>)],
) -> Result<(), LivePersistenceError> {
    let profile = store.profile();
    let mut added_bytes = BTreeMap::<LiveStreamV2, u64>::new();
    let mut added_records = BTreeMap::<LiveStreamV2, u64>::new();
    for (stream, bytes) in serialized {
        let (batch_bytes, batch_records) =
            serialized_jsonl_usage(*stream, bytes, profile.jsonl_line_bytes)?;
        *added_bytes.entry(*stream).or_default() += batch_bytes;
        *added_records.entry(*stream).or_default() += batch_records;
    }
    let mut total_bytes = 0_u64;
    let mut total_records = 0_u64;
    for stream in all_streams() {
        let current = stream_checkpoint(checkpoint, stream)?;
        let next_bytes = current
            .committed_bytes
            .checked_add(added_bytes.get(&stream).copied().unwrap_or_default())
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "stream byte accounting overflowed".into(),
            })?;
        let next_records = current
            .record_count
            .checked_add(added_records.get(&stream).copied().unwrap_or_default())
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "stream record accounting overflowed".into(),
            })?;
        if next_bytes > profile.jsonl_stream_bytes || next_records > profile.jsonl_stream_records {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "{} mutation would exceed the live stream resource profile",
                    stream.checkpoint_name()
                ),
            });
        }
        total_bytes = total_bytes.checked_add(next_bytes).ok_or_else(|| {
            LivePersistenceError::InvalidMutation {
                message: "modeled byte accounting overflowed".into(),
            }
        })?;
        total_records = total_records.checked_add(next_records).ok_or_else(|| {
            LivePersistenceError::InvalidMutation {
                message: "modeled record accounting overflowed".into(),
            }
        })?;
    }
    if let Some(bytes) = added_bytes.get(&LiveStreamV2::RuntimeContexts).copied() {
        let records = added_records
            .get(&LiveStreamV2::RuntimeContexts)
            .copied()
            .unwrap_or_default();
        let current = checkpoint
            .streams
            .get(LiveStreamV2::RuntimeContexts.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: "checkpoint is missing runtimeContexts".into(),
            })?;
        if current.committed_bytes.saturating_add(bytes)
            > antennabench_core::v6::RUNTIME_CONTEXT_STREAM_MAX_BYTES as u64
            || current.record_count.saturating_add(records)
                > antennabench_core::v6::RUNTIME_CONTEXT_MAX_RECORDS as u64
        {
            return Err(LivePersistenceError::InvalidMutation {
                message: "runtime context mutation exceeds the schema-v6 retention bound".into(),
            });
        }
        total_bytes = total_bytes.saturating_add(current.committed_bytes.saturating_add(bytes));
        total_records = total_records.saturating_add(current.record_count.saturating_add(records));
    }
    if total_bytes > profile.modeled_total_bytes || total_records > profile.modeled_total_records {
        return Err(LivePersistenceError::InvalidMutation {
            message: "mutation would exceed the aggregate live resource profile".into(),
        });
    }
    Ok(())
}

fn serialized_jsonl_usage(
    stream: LiveStreamV2,
    bytes: &[u8],
    line_limit: u64,
) -> Result<(u64, u64), LivePersistenceError> {
    let batch_bytes = u64::try_from(bytes.len()).expect("usize fits u64");
    let mut records = 0_u64;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let line_bytes = u64::try_from(line.len()).expect("usize fits u64");
        if line_bytes > line_limit {
            return Err(LivePersistenceError::InvalidMutation {
                message: format!(
                    "{} member exceeds the {} byte JSONL line limit",
                    stream.checkpoint_name(),
                    line_limit
                ),
            });
        }
        records = records
            .checked_add(1)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "stream record accounting overflowed".into(),
            })?;
    }
    Ok((batch_bytes, records))
}

pub(super) fn validate_generation_id(value: &str) -> Result<(), LivePersistenceError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Err(LivePersistenceError::InvalidMutation {
            message: "plan generation ID must be a nonempty ASCII path component".into(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{serialized_jsonl_usage, LivePersistenceError, LiveStreamV2};

    #[test]
    fn live_budget_counts_each_jsonl_member_in_a_serialized_batch() {
        assert_eq!(
            serialized_jsonl_usage(LiveStreamV2::AdapterRecords, b"{}\n{}\n", 3).unwrap(),
            (6, 2)
        );
    }

    #[test]
    fn live_budget_rejects_an_individual_oversized_jsonl_member() {
        let error =
            serialized_jsonl_usage(LiveStreamV2::AdapterRecords, b"[123]\n{}\n", 5).unwrap_err();
        assert!(matches!(
            error,
            LivePersistenceError::InvalidMutation { message }
                if message == "adapter_records member exceeds the 5 byte JSONL line limit"
        ));
    }
}
