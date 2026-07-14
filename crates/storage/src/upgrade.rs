use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use antennabench_core::{
    validate_machine_identity, AdapterDisposition, AdapterInput, AdapterReasonId, AdapterRecordV2,
    BundleContents, BundleFilesV2, BundleManifestV2, BundleV2Contents, BundleValidationError,
    BundleValidationProfile, MutationMember, NormalizedRecordKind, NormalizedRecordLink,
    ObservationRecordV2, OperatorEventType, OperatorEventV2, PlanGenerationV2, PropagationRecordV2,
    Provenance, RecordMeta, RecordMetaV2, RigRecordV2, SessionLifecycleV2, SessionStateV2,
    StreamCheckpointV2, SCHEMA_VERSION_V1, SCHEMA_VERSION_V2,
};
use thiserror::Error;

use super::{
    v2::{checkpoint_for_bytes, serialize_json, serialize_jsonl, sha256_hex},
    BundleStore, BundleStoreError,
};

impl BundleStore {
    /// Creates a new schema-v2 bundle without mutating the schema-v1 source.
    pub fn upgrade_v1_to_v2(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BundleStore, BundleUpgradeError> {
        let source_before = snapshot_tree(self.root())?;
        ensure_destination_outside_source(self.root(), destination.as_ref())?;
        let inspection = self.inspect()?;
        if !inspection.report().allows(BundleValidationProfile::Upgrade) {
            return Err(BundleUpgradeError::Ineligible {
                source: BundleValidationError::from_report(inspection.report().clone()),
            });
        }
        let bundle =
            inspection
                .bundle()
                .cloned()
                .ok_or_else(|| BundleUpgradeError::Ineligible {
                    source: BundleValidationError::from_report(inspection.report().clone()),
                })?;
        if bundle.manifest.schema_version != SCHEMA_VERSION_V1 {
            return Err(BundleUpgradeError::NotVersionOne {
                actual: bundle.manifest.schema_version,
            });
        }
        let wsjtx_lines = fs::read_to_string(self.root().join(&bundle.manifest.files.wsjtx))
            .map_err(|source| BundleUpgradeError::ReadLegacyEvidence {
                path: self.root().join(&bundle.manifest.files.wsjtx),
                source,
            })?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let v2 = migrate_bundle(&bundle, &wsjtx_lines)?;
        verify_semantic_projection(&bundle, v2.clone().into_current().bundle)?;

        let destination_store = BundleStore::new(destination);
        destination_store.write_v2_for_upgrade(&v2)?;
        let result = (|| {
            copy_legacy_attachments(
                &self.root().join(&bundle.manifest.files.attachments_dir),
                &destination_store
                    .root()
                    .join(&v2.manifest.files.attachments_dir),
            )?;
            let upgraded = destination_store.read_current()?.bundle;
            verify_semantic_projection(&bundle, upgraded)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(destination_store.root());
            return Err(error);
        }

        let source_after = snapshot_tree(self.root())?;
        if source_before != source_after {
            let _ = fs::remove_dir_all(destination_store.root());
            return Err(BundleUpgradeError::SourceChanged);
        }
        Ok(destination_store)
    }
}

fn migrate_bundle(
    v1: &BundleContents,
    wsjtx_lines: &[String],
) -> Result<BundleV2Contents, BundleUpgradeError> {
    if wsjtx_lines.len() != v1.wsjtx.len() {
        return Err(BundleUpgradeError::LegacyEvidenceCount {
            records: v1.wsjtx.len(),
            lines: wsjtx_lines.len(),
        });
    }
    let app_version = v1.manifest.app_version.clone();
    let mut station = v1.station.clone();
    station.schema_version = SCHEMA_VERSION_V2;
    let mut antennas = v1.antennas.clone();
    antennas.schema_version = SCHEMA_VERSION_V2;
    let mut schedule = v1.schedule.clone();
    schedule.schema_version = SCHEMA_VERSION_V2;
    let mut analysis = v1.analysis.clone();
    analysis.schema_version = SCHEMA_VERSION_V2;

    let events = v1
        .events
        .iter()
        .map(|event| OperatorEventV2 {
            meta: migrate_meta(&event.meta, "events", &event.event_id, 0, 1, &app_version),
            event_id: event.event_id.clone(),
            slot_id: event.slot_id.clone(),
            event_type: event.event_type,
            note: event.note.clone(),
        })
        .collect::<Vec<_>>();

    let mut adapter_records = Vec::new();
    let mut wsjtx_observation_links = HashMap::<String, (String, String, u32, u32)>::new();
    for (index, (record, line)) in v1.wsjtx.iter().zip(wsjtx_lines).enumerate() {
        let malformed = record.message_type.contains("malformed");
        let evidence_id = bounded_migration_id("legacy-wsjtx", &record.record_id);
        let mutation_id = migration_mutation_id("wsjtx", &record.record_id);
        let matching_observations = v1
            .observations
            .iter()
            .filter(|observation| legacy_wsjtx_matches(record, observation))
            .collect::<Vec<_>>();
        let member_count = u32::try_from(matching_observations.len() + 1)
            .expect("migration member count fits u32");
        for (member_index, observation) in matching_observations.iter().enumerate() {
            wsjtx_observation_links.insert(
                observation.observation_id.clone(),
                (
                    evidence_id.clone(),
                    mutation_id.clone(),
                    u32::try_from(member_index + 1).expect("migration member index fits u32"),
                    member_count,
                ),
            );
        }
        adapter_records.push(AdapterRecordV2 {
            meta: migrate_meta_with_mutation(
                &record.meta,
                mutation_id,
                0,
                member_count,
                &app_version,
            ),
            record_id: evidence_id,
            source_time: Some(record.meta.timestamp),
            record_type: record.message_type.clone(),
            disposition: if malformed {
                AdapterDisposition::Malformed
            } else {
                AdapterDisposition::Accepted
            },
            reason: AdapterReasonId::new(if malformed {
                "legacy.malformed"
            } else {
                "legacy.accepted"
            })
            .expect("static reason identity"),
            normalized_records: matching_observations
                .iter()
                .map(|observation| NormalizedRecordLink {
                    record_kind: NormalizedRecordKind::Observation,
                    record_id: observation.observation_id.clone(),
                })
                .collect(),
            input: AdapterInput::Inline {
                data: format!("{line}\n"),
                media_type: "application/x-ndjson".into(),
                encoding: Some("utf-8".into()),
                source_locator: Some(format!("wsjtx.jsonl#line={}", index + 1)),
            },
        });
    }

    let observations = v1
        .observations
        .iter()
        .map(|record| {
            let (evidence_id, mutation_id, member_index, member_count) = wsjtx_observation_links
                .get(&record.observation_id)
                .cloned()
                .unwrap_or_else(|| {
                    let evidence_id =
                        bounded_migration_id("legacy-observation", &record.observation_id);
                    let mutation_id = migration_mutation_id("observations", &record.observation_id);
                    adapter_records.push(AdapterRecordV2 {
                        meta: migrate_meta_with_mutation(
                            &record.meta,
                            mutation_id.clone(),
                            0,
                            2,
                            &app_version,
                        ),
                        record_id: evidence_id.clone(),
                        source_time: Some(record.meta.timestamp),
                        record_type: "legacy-observation-evidence".into(),
                        disposition: AdapterDisposition::Accepted,
                        reason: AdapterReasonId::new("legacy.normalized")
                            .expect("static reason identity"),
                        normalized_records: vec![NormalizedRecordLink {
                            record_kind: NormalizedRecordKind::Observation,
                            record_id: record.observation_id.clone(),
                        }],
                        input: AdapterInput::Inline {
                            data: serde_json::to_string(&record.raw)
                                .expect("JSON value serializes"),
                            media_type: "application/json".into(),
                            encoding: Some("utf-8".into()),
                            source_locator: Some(format!(
                                "observations.jsonl#observation_id={}",
                                record.observation_id
                            )),
                        },
                    });
                    (evidence_id, mutation_id, 1, 2)
                });
            ObservationRecordV2 {
                meta: migrate_meta_with_mutation(
                    &record.meta,
                    mutation_id,
                    member_index,
                    member_count,
                    &app_version,
                ),
                observation_id: record.observation_id.clone(),
                adapter_record_ids: vec![evidence_id],
                observation_kind: record.observation_kind,
                band: record.band,
                frequency_hz: record.frequency_hz,
                mode: record.mode.clone(),
                reporter_call: record.reporter_call.clone(),
                heard_call: record.heard_call.clone(),
                reporter_grid: record.reporter_grid.clone(),
                heard_grid: record.heard_grid.clone(),
                distance_km: record.distance_km,
                azimuth_degrees: record.azimuth_degrees,
                snr_db: record.snr_db,
                drift_hz_per_minute: record.drift_hz_per_minute,
                power_watts: record.power_watts,
                slot_id: record.slot_id.clone(),
                slot_label: record.slot_label.clone(),
                slot_confidence: record.slot_confidence,
                raw: record.raw.clone(),
            }
        })
        .collect::<Vec<_>>();

    let rig = v1
        .rig
        .iter()
        .map(|record| {
            let evidence_id = legacy_evidence_id(NormalizedRecordKind::Rig, &record.record_id);
            let mutation_id = migration_mutation_id("rig", &record.record_id);
            adapter_records.push(adapter_from_raw(
                &record.meta,
                &record.record_id,
                mutation_id.clone(),
                NormalizedRecordKind::Rig,
                &record.raw,
                &app_version,
            ));
            RigRecordV2 {
                meta: migrate_meta_with_mutation(&record.meta, mutation_id, 1, 2, &app_version),
                record_id: record.record_id.clone(),
                adapter_record_ids: vec![evidence_id],
                status: record.status.clone(),
                frequency_hz: record.frequency_hz,
                mode: record.mode.clone(),
                raw: record.raw.clone(),
            }
        })
        .collect::<Vec<_>>();

    let propagation = v1
        .propagation
        .iter()
        .map(|record| {
            let evidence_id =
                legacy_evidence_id(NormalizedRecordKind::Propagation, &record.record_id);
            let mutation_id = migration_mutation_id("propagation", &record.record_id);
            adapter_records.push(adapter_from_raw(
                &record.meta,
                &record.record_id,
                mutation_id.clone(),
                NormalizedRecordKind::Propagation,
                &record.raw,
                &app_version,
            ));
            PropagationRecordV2 {
                meta: migrate_meta_with_mutation(&record.meta, mutation_id, 1, 2, &app_version),
                record_id: record.record_id.clone(),
                adapter_record_ids: vec![evidence_id],
                observed_at: record.observed_at,
                solar_flux_f107: record.solar_flux_f107,
                sunspot_number: record.sunspot_number,
                kp_index: record.kp_index,
                a_index: record.a_index,
                solar_wind_speed_kms: record.solar_wind_speed_kms,
                bz_nt: record.bz_nt,
                alerts: record.alerts.clone(),
                daylight_state: record.daylight_state.clone(),
                raw: record.raw.clone(),
            }
        })
        .collect::<Vec<_>>();

    let station_bytes = serialize_json(&station)?;
    let antennas_bytes = serialize_json(&antennas)?;
    let schedule_bytes = serialize_json(&schedule)?;
    let station_digest = sha256_hex(&station_bytes);
    let antennas_digest = sha256_hex(&antennas_bytes);
    let schedule_digest = sha256_hex(&schedule_bytes);
    let active_plan = PlanGenerationV2 {
        generation_id: "migration-v1".into(),
        root_sha256: sha256_hex(
            [
                station_digest.as_str(),
                antennas_digest.as_str(),
                schedule_digest.as_str(),
            ]
            .join("\n")
            .as_bytes(),
        ),
        station_sha256: station_digest,
        antennas_sha256: antennas_digest,
        schedule_sha256: schedule_digest,
    };
    let mut streams = std::collections::BTreeMap::<String, StreamCheckpointV2>::new();
    let event_bytes = serialize_jsonl(&events)?;
    streams.insert(
        "events".into(),
        checkpoint_for_bytes(
            &event_bytes,
            events.len(),
            events.last().map(|record| record.event_id.clone()),
        ),
    );
    let observation_bytes = serialize_jsonl(&observations)?;
    streams.insert(
        "observations".into(),
        checkpoint_for_bytes(
            &observation_bytes,
            observations.len(),
            observations
                .last()
                .map(|record| record.observation_id.clone()),
        ),
    );
    let adapter_bytes = serialize_jsonl(&adapter_records)?;
    streams.insert(
        "adapter_records".into(),
        checkpoint_for_bytes(
            &adapter_bytes,
            adapter_records.len(),
            adapter_records
                .last()
                .map(|record| record.record_id.clone()),
        ),
    );
    let rig_bytes = serialize_jsonl(&rig)?;
    streams.insert(
        "rig".into(),
        checkpoint_for_bytes(
            &rig_bytes,
            rig.len(),
            rig.last().map(|record| record.record_id.clone()),
        ),
    );
    let propagation_bytes = serialize_jsonl(&propagation)?;
    streams.insert(
        "propagation".into(),
        checkpoint_for_bytes(
            &propagation_bytes,
            propagation.len(),
            propagation.last().map(|record| record.record_id.clone()),
        ),
    );

    let lifecycle = if v1
        .events
        .iter()
        .any(|event| event.event_type == OperatorEventType::SessionEnded)
    {
        SessionLifecycleV2::Ended
    } else if v1
        .events
        .iter()
        .any(|event| event.event_type == OperatorEventType::SessionStarted)
    {
        SessionLifecycleV2::Interrupted
    } else {
        SessionLifecycleV2::Ready
    };
    let last_committed_mutation_id = propagation
        .last()
        .map(|record| record.meta.mutation.mutation_id.clone())
        .or_else(|| {
            rig.last()
                .map(|record| record.meta.mutation.mutation_id.clone())
        })
        .or_else(|| {
            observations
                .last()
                .map(|record| record.meta.mutation.mutation_id.clone())
        })
        .or_else(|| {
            adapter_records
                .last()
                .map(|record| record.meta.mutation.mutation_id.clone())
        })
        .or_else(|| {
            events
                .last()
                .map(|record| record.meta.mutation.mutation_id.clone())
        });

    Ok(BundleV2Contents {
        manifest: BundleManifestV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: v1.manifest.session_id.clone(),
            created_at: v1.manifest.created_at,
            app_version,
            files: BundleFilesV2::default(),
        },
        session_state: SessionStateV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: v1.manifest.session_id.clone(),
            revision: 1,
            lifecycle,
            active_plan,
            streams,
            last_committed_mutation_id,
        },
        station,
        antennas,
        schedule,
        events,
        observations,
        adapter_records,
        rig,
        propagation,
        analysis,
    })
}

fn legacy_wsjtx_matches(
    wsjtx: &antennabench_core::WsjtXRecord,
    observation: &antennabench_core::ObservationRecord,
) -> bool {
    if wsjtx.meta.source != observation.meta.source
        || wsjtx.meta.timestamp != observation.meta.timestamp
        || !wsjtx.message_type.contains("decode")
    {
        return false;
    }
    match (
        wsjtx
            .raw
            .get("line_number")
            .and_then(serde_json::Value::as_u64),
        observation
            .raw
            .get("line_number")
            .and_then(serde_json::Value::as_u64),
    ) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn adapter_from_raw(
    meta: &RecordMeta,
    normalized_id: &str,
    mutation_id: String,
    record_kind: NormalizedRecordKind,
    raw: &serde_json::Value,
    app_version: &str,
) -> AdapterRecordV2 {
    AdapterRecordV2 {
        meta: migrate_meta_with_mutation(meta, mutation_id, 0, 2, app_version),
        record_id: legacy_evidence_id(record_kind, normalized_id),
        source_time: Some(meta.timestamp),
        record_type: match record_kind {
            NormalizedRecordKind::Rig => "legacy-rig-evidence",
            NormalizedRecordKind::Propagation => "legacy-propagation-evidence",
            NormalizedRecordKind::Observation => "legacy-observation-evidence",
        }
        .into(),
        disposition: AdapterDisposition::Accepted,
        reason: AdapterReasonId::new("legacy.normalized").expect("static reason identity"),
        normalized_records: vec![NormalizedRecordLink {
            record_kind,
            record_id: normalized_id.into(),
        }],
        input: AdapterInput::Inline {
            data: serde_json::to_string(raw).expect("JSON value serializes"),
            media_type: "application/json".into(),
            encoding: Some("utf-8".into()),
            source_locator: None,
        },
    }
}

fn legacy_evidence_id(record_kind: NormalizedRecordKind, normalized_id: &str) -> String {
    let kind = match record_kind {
        NormalizedRecordKind::Observation => "observation",
        NormalizedRecordKind::Rig => "rig",
        NormalizedRecordKind::Propagation => "propagation",
    };
    bounded_migration_id(&format!("legacy-{kind}"), normalized_id)
}

fn bounded_migration_id(prefix: &str, legacy_id: &str) -> String {
    let candidate = format!("{prefix}-{legacy_id}");
    if validate_machine_identity(&candidate).is_ok() {
        candidate
    } else {
        format!("{prefix}-{}", sha256_hex(legacy_id.as_bytes()))
    }
}

fn migrate_meta(
    meta: &RecordMeta,
    stream: &str,
    id: &str,
    member_index: u32,
    member_count: u32,
    app_version: &str,
) -> RecordMetaV2 {
    migrate_meta_with_mutation(
        meta,
        migration_mutation_id(stream, id),
        member_index,
        member_count,
        app_version,
    )
}

fn migrate_meta_with_mutation(
    meta: &RecordMeta,
    mutation_id: String,
    member_index: u32,
    member_count: u32,
    app_version: &str,
) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V2,
        session_id: meta.session_id.clone(),
        recorded_at: meta.timestamp,
        provenance: Provenance::from_legacy(meta.source, app_version),
        mutation: MutationMember {
            mutation_id,
            member_index,
            member_count,
        },
    }
}

fn migration_mutation_id(stream: &str, id: &str) -> String {
    format!(
        "migration-{}",
        sha256_hex(format!("{stream}\0{id}").as_bytes())
    )
}

fn verify_semantic_projection(
    v1: &BundleContents,
    mut projected: BundleContents,
) -> Result<(), BundleUpgradeError> {
    let mut expected = v1.clone();
    expected.manifest.schema_version = SCHEMA_VERSION_V2;
    expected.manifest.files = antennabench_core::BundleFiles::default();
    expected.station.schema_version = SCHEMA_VERSION_V2;
    expected.antennas.schema_version = SCHEMA_VERSION_V2;
    expected.schedule.schema_version = SCHEMA_VERSION_V2;
    expected.analysis.schema_version = SCHEMA_VERSION_V2;
    for meta in expected
        .events
        .iter_mut()
        .map(|record| &mut record.meta)
        .chain(
            expected
                .observations
                .iter_mut()
                .map(|record| &mut record.meta),
        )
        .chain(expected.rig.iter_mut().map(|record| &mut record.meta))
        .chain(
            expected
                .propagation
                .iter_mut()
                .map(|record| &mut record.meta),
        )
    {
        meta.schema_version = SCHEMA_VERSION_V2;
    }
    expected.wsjtx.clear();
    projected.manifest.files = antennabench_core::BundleFiles::default();
    if expected == projected {
        Ok(())
    } else {
        Err(BundleUpgradeError::SemanticMismatch)
    }
}

fn copy_legacy_attachments(source: &Path, destination: &Path) -> Result<(), BundleUpgradeError> {
    for entry in
        fs::read_dir(source).map_err(|source_error| BundleUpgradeError::CopyAttachments {
            path: source.to_path_buf(),
            source: source_error,
        })?
    {
        let entry = entry.map_err(|source_error| BundleUpgradeError::CopyAttachments {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&from).map_err(|source_error| {
            BundleUpgradeError::CopyAttachments {
                path: from.clone(),
                source: source_error,
            }
        })?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(BundleUpgradeError::UnsafeAttachment { path: from });
        }
        if metadata.is_dir() {
            fs::create_dir(&to).map_err(|source_error| BundleUpgradeError::CopyAttachments {
                path: to.clone(),
                source: source_error,
            })?;
            copy_legacy_attachments(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source_error| BundleUpgradeError::CopyAttachments {
                path: from.clone(),
                source: source_error,
            })?;
        }
    }
    Ok(())
}

fn snapshot_tree(root: &Path) -> Result<Vec<(PathBuf, String)>, BundleUpgradeError> {
    fn visit(
        root: &Path,
        current: &Path,
        output: &mut Vec<(PathBuf, String)>,
    ) -> Result<(), BundleUpgradeError> {
        for entry in fs::read_dir(current).map_err(|source| BundleUpgradeError::SnapshotSource {
            path: current.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| BundleUpgradeError::SnapshotSource {
                path: current.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|source| {
                BundleUpgradeError::SnapshotSource {
                    path: path.clone(),
                    source,
                }
            })?;
            if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(BundleUpgradeError::UnsafeAttachment { path });
            }
            if metadata.is_dir() {
                visit(root, &path, output)?;
            } else {
                let bytes =
                    fs::read(&path).map_err(|source| BundleUpgradeError::SnapshotSource {
                        path: path.clone(),
                        source,
                    })?;
                output.push((
                    path.strip_prefix(root)
                        .expect("visited below root")
                        .to_path_buf(),
                    sha256_hex(&bytes),
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

fn ensure_destination_outside_source(
    source_root: &Path,
    destination: &Path,
) -> Result<(), BundleUpgradeError> {
    let source_root =
        fs::canonicalize(source_root).map_err(|source| BundleUpgradeError::SnapshotSource {
            path: source_root.to_path_buf(),
            source,
        })?;
    let destination_parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = fs::canonicalize(destination_parent).map_err(|source| {
        BundleUpgradeError::SnapshotSource {
            path: destination_parent.to_path_buf(),
            source,
        }
    })?;
    if destination_parent.starts_with(source_root) {
        Err(BundleUpgradeError::DestinationInsideSource {
            path: destination.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum BundleUpgradeError {
    #[error(transparent)]
    Storage(#[from] BundleStoreError),
    #[error("version 1 bundle is not eligible for deterministic upgrade")]
    Ineligible {
        #[source]
        source: BundleValidationError,
    },
    #[error("only schema version 1 can be upgraded, found {actual}")]
    NotVersionOne { actual: u16 },
    #[error("legacy WSJT-X evidence has {records} projected records but {lines} physical records")]
    LegacyEvidenceCount { records: usize, lines: usize },
    #[error("failed to read legacy evidence {path}")]
    ReadLegacyEvidence {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to serialize migrated schema-v2 evidence")]
    Serialize(#[from] serde_json::Error),
    #[error("schema-v2 projection is not semantically equivalent to the version 1 source")]
    SemanticMismatch,
    #[error("source bytes changed during a non-destructive upgrade")]
    SourceChanged,
    #[error("upgrade destination cannot be inside the source bundle: {path}")]
    DestinationInsideSource { path: PathBuf },
    #[error("unsafe legacy attachment entry: {path}")]
    UnsafeAttachment { path: PathBuf },
    #[error("failed to copy legacy attachments at {path}")]
    CopyAttachments {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to snapshot source entry {path}")]
    SnapshotSource {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use antennabench_core::{validate_machine_identity, MACHINE_ID_MAX_BYTES};

    use super::bounded_migration_id;

    #[test]
    fn derived_evidence_ids_stay_bounded_for_maximum_legacy_ids() {
        let legacy = "a".repeat(MACHINE_ID_MAX_BYTES);
        let first = bounded_migration_id("legacy-observation", &legacy);
        let second = bounded_migration_id("legacy-observation", &legacy);
        assert_eq!(first, second);
        assert!(validate_machine_identity(&first).is_ok());
        assert_ne!(first, format!("legacy-observation-{legacy}"));
    }
}
