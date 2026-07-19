use std::{fs, path::Path};

use antennabench_core::{
    codes,
    v2::{AdapterInput, SessionLifecycleV2, V2_BUNDLE_SUFFIX},
    v3::{
        project_wspr_run_v3, reduce_operator_events_v3, validate_signal_plan_schedule_v3,
        validate_signal_state_confirmation_v3, validate_signal_state_event_v3, BundleManifestV3,
        BundleV3Contents, CorrectableOperatorEventPayloadV3, OperatorEventPayloadV3,
        SessionStateV3, WsprCycleDirection,
    },
    v5::validate_antenna_control_v5,
    v6::{
        BuildChannelV6, DiagnosticDetailStateV6, DiagnosticFactValueV6, DiagnosticTargetV6,
        DiagnosticsStateV6, SourceStateV6, DIAGNOSTIC_CAUSE_MAX_DEPTH, DIAGNOSTIC_FACT_MAX_COUNT,
        DIAGNOSTIC_IDENTIFIER_MAX_BYTES, DIAGNOSTIC_MACHINE_VALUE_MAX_BYTES,
        DIAGNOSTIC_MAX_RECORDS, DIAGNOSTIC_RECORD_MAX_BYTES, DIAGNOSTIC_STREAM_MAX_BYTES,
        DIAGNOSTIC_SUMMARY_MAX_BYTES, DIAGNOSTIC_TARGET_MAX_COUNT,
        OPERATIONAL_DIAGNOSTIC_SCHEMA_V1, RUNTIME_CONTEXT_MAX_RECORDS,
        RUNTIME_CONTEXT_RECORD_MAX_BYTES, RUNTIME_CONTEXT_STREAM_MAX_BYTES,
    },
    validate_bundle_report, BundleValidationProfile, BundleValidationReport, ExperimentMode,
    SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6,
};

use super::{
    create_directory,
    inspection::BundleInspection,
    resource::{
        inventory_attachment_tree, read_bounded, serialize_jsonl_bounded, serialize_root_bounded,
        ModeledBudget, ResourceOperation,
    },
    v2::{checkpoint_for_bytes, modeled_duplicate_member_diagnostics, sha256_hex},
    BundleStore, BundleStoreError,
};

impl BundleStore {
    pub(super) fn inspect_v3(
        &self,
        mut report: BundleValidationReport,
    ) -> Result<BundleInspection, BundleStoreError> {
        let bundle = self.read_v3()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        report.extend(modeled_duplicate_member_diagnostics(
            self,
            &paths,
            bundle.manifest.schema_version,
        ));
        let intent_native = !bundle.schedule.wspr_cycle_intents.is_empty();
        let mut native_planned_bands = Vec::new();
        for band in bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .map(|intent| intent.band)
            .chain(bundle.schedule.slots.iter().map(|slot| slot.band))
        {
            if !native_planned_bands.contains(&band) {
                native_planned_bands.push(band);
            }
        }
        let current = bundle.into_current();
        report.extend(
            validate_bundle_report(&current.bundle)
                .into_diagnostics()
                .into_iter()
                .filter(|diagnostic| {
                    !intent_native
                        || !matches!(
                            diagnostic.code.as_str(),
                            codes::EMPTY_SCHEDULE
                                | codes::EXPERIMENT_SHAPE_MISMATCH
                                | codes::UNKNOWN_EVENT_SLOT
                        )
                }),
        );
        let current = report
            .allows(BundleValidationProfile::CompatibilityRead)
            .then_some(current);
        Ok(BundleInspection {
            current,
            report,
            native_planned_bands,
        })
    }

    pub fn refresh_v3_checkpoint(bundle: &mut BundleV3Contents) -> Result<(), BundleStoreError> {
        let store = BundleStore::new(".");
        let mut budget = ModeledBudget::default();
        let station = serialize_root_bounded(
            &store,
            Path::new("station.json"),
            &bundle.station,
            &mut budget,
        )?;
        let antennas = serialize_root_bounded(
            &store,
            Path::new("antennas.json"),
            &bundle.antennas,
            &mut budget,
        )?;
        let schedule = serialize_root_bounded(
            &store,
            Path::new("schedule.json"),
            &bundle.schedule,
            &mut budget,
        )?;
        let station_sha256 = sha256_hex(&station);
        let antennas_sha256 = sha256_hex(&antennas);
        let schedule_sha256 = sha256_hex(&schedule);
        bundle.session_state.active_plan.station_sha256 = station_sha256.clone();
        bundle.session_state.active_plan.antennas_sha256 = antennas_sha256.clone();
        bundle.session_state.active_plan.schedule_sha256 = schedule_sha256.clone();
        bundle.session_state.active_plan.root_sha256 = sha256_hex(
            [station_sha256, antennas_sha256, schedule_sha256]
                .join("\n")
                .as_bytes(),
        );

        let events = serialize_jsonl_bounded(
            &store,
            Path::new("events.jsonl"),
            &bundle.events,
            &mut budget,
        )?;
        let observations = serialize_jsonl_bounded(
            &store,
            Path::new("observations.jsonl"),
            &bundle.observations,
            &mut budget,
        )?;
        let adapter_records = serialize_jsonl_bounded(
            &store,
            Path::new("adapter-records.jsonl"),
            &bundle.adapter_records,
            &mut budget,
        )?;
        let rig =
            serialize_jsonl_bounded(&store, Path::new("rig.jsonl"), &bundle.rig, &mut budget)?;
        let propagation = serialize_jsonl_bounded(
            &store,
            Path::new("propagation.jsonl"),
            &bundle.propagation,
            &mut budget,
        )?;
        let runtime_contexts = serialize_jsonl_bounded(
            &store,
            Path::new("runtime-contexts.jsonl"),
            &bundle.runtime_contexts,
            &mut budget,
        )?;
        let diagnostics = serialize_jsonl_bounded(
            &store,
            Path::new("diagnostics.jsonl"),
            &bundle.diagnostics,
            &mut budget,
        )?;
        let mut streams = vec![
            (
                "events".to_string(),
                checkpoint_for_bytes(
                    &events,
                    bundle.events.len(),
                    bundle.events.last().map(|record| record.event_id.clone()),
                ),
            ),
            (
                "observations".to_string(),
                checkpoint_for_bytes(
                    &observations,
                    bundle.observations.len(),
                    bundle
                        .observations
                        .last()
                        .map(|record| record.observation_id.clone()),
                ),
            ),
            (
                "adapter_records".to_string(),
                checkpoint_for_bytes(
                    &adapter_records,
                    bundle.adapter_records.len(),
                    bundle
                        .adapter_records
                        .last()
                        .map(|record| record.record_id.clone()),
                ),
            ),
            (
                "rig".to_string(),
                checkpoint_for_bytes(
                    &rig,
                    bundle.rig.len(),
                    bundle.rig.last().map(|record| record.record_id.clone()),
                ),
            ),
            (
                "propagation".to_string(),
                checkpoint_for_bytes(
                    &propagation,
                    bundle.propagation.len(),
                    bundle
                        .propagation
                        .last()
                        .map(|record| record.record_id.clone()),
                ),
            ),
        ];
        if bundle.manifest.schema_version == SCHEMA_VERSION_V6 {
            streams.push((
                "runtimeContexts".to_string(),
                checkpoint_for_bytes(
                    &runtime_contexts,
                    bundle.runtime_contexts.len(),
                    bundle
                        .runtime_contexts
                        .last()
                        .map(|context| context.context_id.clone()),
                ),
            ));
            streams.push((
                "diagnostics".to_string(),
                checkpoint_for_bytes(
                    &diagnostics,
                    bundle.diagnostics.len(),
                    bundle
                        .diagnostics
                        .last()
                        .map(|diagnostic| diagnostic.diagnostic_id.clone()),
                ),
            ));
        }
        bundle.session_state.streams = streams.into_iter().collect();
        Ok(())
    }

    pub fn read_v3(&self) -> Result<BundleV3Contents, BundleStoreError> {
        let mut budget = ModeledBudget::default();
        let manifest: BundleManifestV3 =
            self.read_json_bounded(&self.bundle_path("manifest.json")?, &mut budget)?;
        if !matches!(
            manifest.schema_version,
            SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6
        ) {
            return Err(BundleStoreError::UnsupportedSchemaVersion {
                actual: manifest.schema_version,
            });
        }
        let bootstrap_paths = self.v2_paths(&manifest.files)?;
        let session_state: SessionStateV3 =
            self.read_json_bounded(&bootstrap_paths.session_state, &mut budget)?;
        let paths = self.v2_paths_for_state(&manifest.files, &session_state)?;
        paths.ensure_readable()?;
        self.inventory_root(ResourceOperation::Read)?;
        inventory_attachment_tree(self, &paths.attachments_dir, ResourceOperation::Read)?;

        let runtime_contexts = match paths.runtime_contexts.as_deref() {
            Some(path) => self.read_jsonl_bounded(path, &mut budget)?,
            None => Vec::new(),
        };
        let diagnostics = match paths.diagnostics.as_deref() {
            Some(path) => self.read_jsonl_bounded(path, &mut budget)?,
            None => Vec::new(),
        };
        let bundle = BundleV3Contents {
            manifest,
            session_state,
            station: self.read_json_bounded(&paths.station, &mut budget)?,
            antennas: self.read_json_bounded(&paths.antennas, &mut budget)?,
            schedule: self.read_json_bounded(&paths.schedule, &mut budget)?,
            events: self.read_jsonl_bounded(&paths.events, &mut budget)?,
            observations: self.read_jsonl_bounded(&paths.observations, &mut budget)?,
            adapter_records: self.read_jsonl_bounded(&paths.adapter_records, &mut budget)?,
            rig: self.read_jsonl_bounded(&paths.rig, &mut budget)?,
            propagation: self.read_jsonl_bounded(&paths.propagation, &mut budget)?,
            analysis: self.read_json_bounded(&paths.analysis, &mut budget)?,
            runtime_contexts,
            diagnostics,
        };
        validate_v3_model(&bundle)?;
        validate_v3_checkpoint(self, &bundle, &paths)?;
        for record in &bundle.adapter_records {
            if let AdapterInput::Attachment { attachment } = &record.input {
                self.read_attachment(attachment)?;
            }
        }
        Ok(bundle)
    }

    pub fn write_v3(&self, bundle: &BundleV3Contents) -> Result<(), BundleStoreError> {
        self.write_v3_files(bundle, false)
    }

    pub(super) fn write_v3_for_upgrade(
        &self,
        bundle: &BundleV3Contents,
    ) -> Result<(), BundleStoreError> {
        self.write_v3_files(bundle, true)
    }

    fn write_v3_files(
        &self,
        bundle: &BundleV3Contents,
        allow_attachment_references: bool,
    ) -> Result<(), BundleStoreError> {
        if bundle
            .adapter_records
            .iter()
            .any(|record| matches!(&record.input, AdapterInput::Attachment { .. }))
            && !allow_attachment_references
        {
            return Err(invalid_v3(
                "v3 attachment-backed evidence requires a focused attachment writer",
            ));
        }
        validate_v3_model(bundle)?;
        ensure_v3_suffix(self.root())?;
        if fs::symlink_metadata(self.root()).is_ok() {
            return Err(BundleStoreError::DestinationExists {
                path: self.root().to_path_buf(),
            });
        }
        let paths = self.v2_paths(&bundle.manifest.files)?;
        let mut budget = ModeledBudget::default();
        let manifest =
            serialize_root_bounded(self, &paths.manifest, &bundle.manifest, &mut budget)?;
        let session_state = serialize_root_bounded(
            self,
            &paths.session_state,
            &bundle.session_state,
            &mut budget,
        )?;
        let station = serialize_root_bounded(self, &paths.station, &bundle.station, &mut budget)?;
        let antennas =
            serialize_root_bounded(self, &paths.antennas, &bundle.antennas, &mut budget)?;
        let schedule =
            serialize_root_bounded(self, &paths.schedule, &bundle.schedule, &mut budget)?;
        let events = serialize_jsonl_bounded(self, &paths.events, &bundle.events, &mut budget)?;
        let observations =
            serialize_jsonl_bounded(self, &paths.observations, &bundle.observations, &mut budget)?;
        let adapter_records = serialize_jsonl_bounded(
            self,
            &paths.adapter_records,
            &bundle.adapter_records,
            &mut budget,
        )?;
        let rig = serialize_jsonl_bounded(self, &paths.rig, &bundle.rig, &mut budget)?;
        let propagation =
            serialize_jsonl_bounded(self, &paths.propagation, &bundle.propagation, &mut budget)?;
        let analysis =
            serialize_root_bounded(self, &paths.analysis, &bundle.analysis, &mut budget)?;
        let runtime_contexts = match paths.runtime_contexts.as_deref() {
            Some(path) => Some(serialize_jsonl_bounded(
                self,
                path,
                &bundle.runtime_contexts,
                &mut budget,
            )?),
            None => None,
        };
        let diagnostics = match paths.diagnostics.as_deref() {
            Some(path) => Some(serialize_jsonl_bounded(
                self,
                path,
                &bundle.diagnostics,
                &mut budget,
            )?),
            None => None,
        };

        create_directory(self.root())?;
        let result = (|| {
            for (path, bytes) in [
                (&paths.manifest, manifest.as_slice()),
                (&paths.session_state, session_state.as_slice()),
                (&paths.station, station.as_slice()),
                (&paths.antennas, antennas.as_slice()),
                (&paths.schedule, schedule.as_slice()),
                (&paths.events, events.as_slice()),
                (&paths.observations, observations.as_slice()),
                (&paths.adapter_records, adapter_records.as_slice()),
                (&paths.rig, rig.as_slice()),
                (&paths.propagation, propagation.as_slice()),
                (&paths.analysis, analysis.as_slice()),
            ] {
                fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            if let (Some(path), Some(bytes)) = (
                paths.runtime_contexts.as_deref(),
                runtime_contexts.as_deref(),
            ) {
                fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            if let (Some(path), Some(bytes)) =
                (paths.diagnostics.as_deref(), diagnostics.as_deref())
            {
                fs::write(path, bytes).map_err(|source| BundleStoreError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            create_directory(&paths.attachments_dir)?;
            if !allow_attachment_references {
                let reopened = self.read_v3()?;
                if &reopened != bundle {
                    return Err(invalid_v3("written bundle did not round-trip exactly"));
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(self.root());
        }
        result
    }
}

pub(super) fn validate_v3_model(bundle: &BundleV3Contents) -> Result<(), BundleStoreError> {
    let session_id = bundle.manifest.session_id.as_str();
    let schema_version = bundle.manifest.schema_version;
    if !matches!(
        schema_version,
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6
    ) {
        return Err(BundleStoreError::UnsupportedSchemaVersion {
            actual: schema_version,
        });
    }
    validate_runtime_contexts_v6(bundle)?;
    for (schema, actual_session, name) in [
        (
            bundle.manifest.schema_version,
            bundle.manifest.session_id.as_str(),
            "manifest",
        ),
        (
            bundle.session_state.schema_version,
            bundle.session_state.session_id.as_str(),
            "session state",
        ),
        (
            bundle.station.schema_version,
            bundle.station.session_id.as_str(),
            "station",
        ),
        (
            bundle.antennas.schema_version,
            bundle.antennas.session_id.as_str(),
            "antennas",
        ),
        (
            bundle.schedule.schema_version,
            bundle.schedule.session_id.as_str(),
            "schedule",
        ),
        (
            bundle.analysis.schema_version,
            bundle.analysis.session_id.as_str(),
            "analysis",
        ),
    ] {
        if schema != schema_version || actual_session != session_id {
            return Err(invalid_v3(format!(
                "{name} schema/session identity does not match the v3 manifest"
            )));
        }
    }
    let plan_diagnostics =
        validate_signal_plan_schedule_v3(&bundle.station.callsign, &bundle.schedule);
    if let Some(diagnostic) = plan_diagnostics.first() {
        return Err(invalid_v3(format!(
            "{}: {}",
            diagnostic.code, diagnostic.message
        )));
    }
    let mut intent_ids = std::collections::BTreeSet::new();
    let mut intent_sequences = std::collections::BTreeSet::new();
    let antenna_labels = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| antenna.label.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for (index, intent) in bundle.schedule.wspr_cycle_intents.iter().enumerate() {
        if (schema_version == SCHEMA_VERSION_V4
            || (schema_version >= SCHEMA_VERSION_V5
                && !matches!(
                    bundle.schedule.antenna_control,
                    Some(antennabench_core::v5::AntennaControlPolicyV5::Manual)
                )))
            && intent.direction.is_none()
        {
            return Err(invalid_v3(
                "current command-controlled WSPR cycle intentions require an explicit direction",
            ));
        }
        if intent.intent_id.trim().is_empty()
            || intent.intent_id.trim() != intent.intent_id
            || !intent.intent_id.is_ascii()
            || !intent_ids.insert(intent.intent_id.as_str())
        {
            return Err(invalid_v3(
                "WSPR cycle intent identities must be unique, nonempty, trimmed ASCII",
            ));
        }
        if intent.sequence_number != u32::try_from(index + 1).unwrap_or(u32::MAX)
            || !intent_sequences.insert(intent.sequence_number)
        {
            return Err(invalid_v3(
                "WSPR cycle intentions must be stored in contiguous sequence-number order",
            ));
        }
        if !antenna_labels.contains(intent.antenna_label.as_str()) {
            return Err(invalid_v3(format!(
                "WSPR cycle intent {} references unknown antenna {:?}",
                intent.intent_id, intent.antenna_label
            )));
        }
        if intent.signal.is_some() && intent.direction == Some(WsprCycleDirection::Receive) {
            return Err(invalid_v3(
                "controlled signal intentions must have transmit direction",
            ));
        }
    }
    if schema_version >= SCHEMA_VERSION_V4
        && bundle.schedule.signal_plans.is_empty()
        && !bundle.schedule.wspr_cycle_intents.is_empty()
        && bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .all(|intent| intent.direction.is_some())
    {
        let directions = bundle
            .schedule
            .wspr_cycle_intents
            .iter()
            .filter_map(|intent| intent.direction)
            .collect::<std::collections::BTreeSet<_>>();
        let valid = match bundle.schedule.mode {
            ExperimentMode::TxFocused => {
                directions == std::collections::BTreeSet::from([WsprCycleDirection::Transmit])
            }
            ExperimentMode::RxFocused => {
                directions == std::collections::BTreeSet::from([WsprCycleDirection::Receive])
            }
            ExperimentMode::WholeStationAb | ExperimentMode::SingleAntennaProfiling => {
                directions
                    == std::collections::BTreeSet::from([
                        WsprCycleDirection::Receive,
                        WsprCycleDirection::Transmit,
                    ])
            }
        };
        if !valid {
            return Err(invalid_v3(
                "schema-v4 WSPR directions do not match the experiment mode",
            ));
        }
    }
    let run_projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    if let Some(diagnostic) = run_projection.diagnostics.first() {
        return Err(invalid_v3(format!(
            "{} at event {}: {}",
            diagnostic.code, diagnostic.event_id, diagnostic.message
        )));
    }
    for event in &bundle.events {
        if let OperatorEventPayloadV3::WsprCycleArmed { antenna_label, .. } = &event.payload {
            if !antenna_labels.contains(antenna_label.as_str()) {
                return Err(invalid_v3(format!(
                    "armed WSPR cycle references unknown antenna {antenna_label:?}"
                )));
            }
        }
    }
    let reduction = reduce_operator_events_v3(SessionLifecycleV2::Ready, &bundle.events);
    if let Some(diagnostic) = reduction.diagnostics.first() {
        return Err(invalid_v3(format!(
            "operator event {} is invalid: {}",
            diagnostic.event_id, diagnostic.message
        )));
    }
    if reduction.lifecycle != bundle.session_state.lifecycle {
        return Err(invalid_v3(
            "operator event lifecycle does not match the session checkpoint",
        ));
    }
    for effective in &reduction.effective_events {
        let CorrectableOperatorEventPayloadV3::SignalStateConfirmed { confirmation } =
            &effective.payload
        else {
            continue;
        };
        if let Some(diagnostic) = validate_signal_state_confirmation_v3(
            &bundle.schedule,
            effective.slot_id.as_deref(),
            confirmation,
        )
        .into_iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code,
                "signal_state.missing_slot"
                    | "signal_state.unknown_slot"
                    | "signal_state.slot_without_plan"
                    | "signal_state.unknown_plan"
                    | "signal_state.invalid_frequency"
                    | "signal_state.invalid_power"
            )
        }) {
            return Err(invalid_v3(format!(
                "{}: {}",
                diagnostic.code, diagnostic.message
            )));
        }
    }
    for event in &bundle.events {
        if event.meta.schema_version != schema_version || event.meta.session_id != session_id {
            return Err(invalid_v3(
                "event schema/session identity does not match manifest",
            ));
        }
        if let Some(diagnostic) = validate_signal_state_event_v3(&bundle.schedule, event)
            .into_iter()
            .find(|diagnostic| {
                matches!(
                    diagnostic.code,
                    "signal_state.missing_slot"
                        | "signal_state.unknown_slot"
                        | "signal_state.slot_without_plan"
                        | "signal_state.unknown_plan"
                        | "signal_state.invalid_frequency"
                        | "signal_state.invalid_power"
                )
            })
        {
            return Err(invalid_v3(format!(
                "{}: {}",
                diagnostic.code, diagnostic.message
            )));
        }
    }
    for (schema, actual_session, name) in bundle
        .observations
        .iter()
        .map(|record| {
            (
                record.meta.schema_version,
                record.meta.session_id.as_str(),
                "observation",
            )
        })
        .chain(bundle.adapter_records.iter().map(|record| {
            (
                record.meta.schema_version,
                record.meta.session_id.as_str(),
                "adapter record",
            )
        }))
        .chain(bundle.rig.iter().map(|record| {
            (
                record.meta.schema_version,
                record.meta.session_id.as_str(),
                "rig record",
            )
        }))
        .chain(bundle.propagation.iter().map(|record| {
            (
                record.meta.schema_version,
                record.meta.session_id.as_str(),
                "propagation record",
            )
        }))
    {
        if schema != schema_version || actual_session != session_id {
            return Err(invalid_v3(format!(
                "{name} schema/session identity does not match manifest"
            )));
        }
    }
    validate_antenna_control_v5(bundle).map_err(invalid_v3)?;
    Ok(())
}

fn validate_runtime_contexts_v6(bundle: &BundleV3Contents) -> Result<(), BundleStoreError> {
    if bundle.manifest.schema_version != SCHEMA_VERSION_V6 {
        if !bundle.runtime_contexts.is_empty()
            || !bundle.diagnostics.is_empty()
            || bundle.manifest.creator_runtime_context_id.is_some()
            || bundle.session_state.active_runtime_context_id.is_some()
            || bundle.session_state.diagnostics_status.is_some()
            || bundle.manifest.files.runtime_contexts.is_some()
            || bundle.manifest.files.diagnostics.is_some()
        {
            return Err(invalid_v3(
                "schema-v3 through v5 cannot declare schema-v6 runtime context fields",
            ));
        }
        return Ok(());
    }
    if bundle.manifest.files.runtime_contexts.as_deref() != Some("runtime-contexts.jsonl")
        || bundle.manifest.files.diagnostics.as_deref() != Some("diagnostics.jsonl")
    {
        return Err(invalid_v3(
            "schema-v6 requires the canonical runtime context and diagnostics stream paths",
        ));
    }
    if bundle.runtime_contexts.is_empty()
        || bundle.runtime_contexts.len() > RUNTIME_CONTEXT_MAX_RECORDS
    {
        return Err(invalid_v3(
            "schema-v6 requires between one and 256 runtime contexts",
        ));
    }
    let creator = bundle
        .manifest
        .creator_runtime_context_id
        .as_deref()
        .ok_or_else(|| invalid_v3("schema-v6 manifest is missing creator runtime context"))?;
    let active = bundle
        .session_state
        .active_runtime_context_id
        .as_deref()
        .ok_or_else(|| invalid_v3("schema-v6 checkpoint is missing active runtime context"))?;
    if bundle.runtime_contexts[0].context_id != creator {
        return Err(invalid_v3(
            "schema-v6 creator must reference the first runtime context",
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut stream_bytes = 0usize;
    for context in &bundle.runtime_contexts {
        if !context.has_valid_identity() || !ids.insert(context.context_id.as_str()) {
            return Err(invalid_v3(
                "runtime contexts require unique canonical context identities",
            ));
        }
        validate_build_identity_v6(context)?;
        let line_bytes = serde_json::to_vec(context)
            .map_err(|error| invalid_v3(format!("runtime context cannot serialize: {error}")))?
            .len()
            .saturating_add(1);
        if line_bytes > RUNTIME_CONTEXT_RECORD_MAX_BYTES {
            return Err(invalid_v3("runtime context exceeds the 4 KiB record bound"));
        }
        stream_bytes = stream_bytes.saturating_add(line_bytes);
    }
    if stream_bytes > RUNTIME_CONTEXT_STREAM_MAX_BYTES {
        return Err(invalid_v3("runtime context stream exceeds the 1 MiB bound"));
    }
    if !ids.contains(active) {
        return Err(invalid_v3(
            "active runtime context does not exist in the committed stream",
        ));
    }
    for context_id in bundle
        .events
        .iter()
        .map(|record| record.meta.runtime_context_id.as_deref())
        .chain(
            bundle
                .observations
                .iter()
                .map(|record| record.meta.runtime_context_id.as_deref()),
        )
        .chain(
            bundle
                .adapter_records
                .iter()
                .map(|record| record.meta.runtime_context_id.as_deref()),
        )
        .chain(
            bundle
                .rig
                .iter()
                .map(|record| record.meta.runtime_context_id.as_deref()),
        )
        .chain(
            bundle
                .propagation
                .iter()
                .map(|record| record.meta.runtime_context_id.as_deref()),
        )
        .chain(
            bundle
                .diagnostics
                .iter()
                .map(|record| Some(record.runtime_context_id.as_str())),
        )
    {
        if !context_id.is_some_and(|id| ids.contains(id)) {
            return Err(invalid_v3(
                "every schema-v6 record must reference a committed runtime context",
            ));
        }
    }
    validate_diagnostics_v6(bundle)?;
    Ok(())
}

fn validate_diagnostics_v6(bundle: &BundleV3Contents) -> Result<(), BundleStoreError> {
    let status = bundle
        .session_state
        .diagnostics_status
        .as_ref()
        .ok_or_else(|| invalid_v3("schema-v6 checkpoint is missing diagnostics status"))?;
    match status.state {
        DiagnosticsStateV6::Complete => {
            if status.omitted_count != 0
                || status.first_omitted_at.is_some()
                || status.reason_code.is_some()
            {
                return Err(invalid_v3(
                    "complete diagnostics status cannot claim omitted records",
                ));
            }
        }
        DiagnosticsStateV6::Saturated | DiagnosticsStateV6::Gap => {
            if status.omitted_count == 0
                || status.first_omitted_at.is_none()
                || !status
                    .reason_code
                    .as_deref()
                    .is_some_and(valid_diagnostic_machine_value)
            {
                return Err(invalid_v3(
                    "incomplete diagnostics status requires an omitted record, time, and reason",
                ));
            }
        }
    }
    if bundle.diagnostics.len() > DIAGNOSTIC_MAX_RECORDS {
        return Err(invalid_v3("diagnostic stream exceeds the record bound"));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut attempts = std::collections::BTreeSet::new();
    let mut stream_bytes = 0usize;
    for diagnostic in &bundle.diagnostics {
        if diagnostic.schema != OPERATIONAL_DIAGNOSTIC_SCHEMA_V1
            || !valid_diagnostic_identifier(&diagnostic.diagnostic_id)
            || !valid_diagnostic_identifier(&diagnostic.correlation_id)
            || !valid_diagnostic_identifier(&diagnostic.attempt_id)
            || !ids.insert(diagnostic.diagnostic_id.as_str())
            || !attempts.insert(diagnostic.attempt_id.as_str())
        {
            return Err(invalid_v3(
                "diagnostics require unique bounded identities and attempt IDs",
            ));
        }
        if !valid_diagnostic_machine_value(&diagnostic.code)
            || !valid_diagnostic_machine_value(&diagnostic.retry.guidance_code)
            || diagnostic.summary.is_empty()
            || diagnostic.summary.len() > DIAGNOSTIC_SUMMARY_MAX_BYTES
            || diagnostic.summary.chars().any(char::is_control)
        {
            return Err(invalid_v3(
                "diagnostic code, guidance, or summary is outside its bound",
            ));
        }
        if diagnostic.mutation.member_count == 0
            || diagnostic.mutation.member_index >= diagnostic.mutation.member_count
            || diagnostic.diagnostic_revision > bundle.session_state.revision
            || diagnostic
                .revision_before
                .zip(diagnostic.revision_after)
                .is_some_and(|(before, after)| {
                    before > after || after >= diagnostic.diagnostic_revision
                })
        {
            return Err(invalid_v3(
                "diagnostic revision or mutation membership is inconsistent",
            ));
        }
        if diagnostic.targets.len() > DIAGNOSTIC_TARGET_MAX_COUNT
            || diagnostic.causes.len() > DIAGNOSTIC_CAUSE_MAX_DEPTH
        {
            return Err(invalid_v3("diagnostic target or cause bound exceeded"));
        }
        for target in &diagnostic.targets {
            match target {
                DiagnosticTargetV6::Adapter { id }
                | DiagnosticTargetV6::Source { id }
                | DiagnosticTargetV6::Mutation { id }
                | DiagnosticTargetV6::Record { id }
                | DiagnosticTargetV6::Slot { id }
                | DiagnosticTargetV6::Intent { id } => {
                    if !valid_diagnostic_identifier(id) {
                        return Err(invalid_v3("diagnostic target identity is invalid"));
                    }
                }
                DiagnosticTargetV6::AcquisitionWindow { start, end } if start >= end => {
                    return Err(invalid_v3("diagnostic acquisition window is empty"));
                }
                DiagnosticTargetV6::AcquisitionWindow { .. } => {}
            }
        }
        let fact_count = diagnostic
            .causes
            .iter()
            .map(|cause| cause.facts.len())
            .sum::<usize>();
        if fact_count > DIAGNOSTIC_FACT_MAX_COUNT {
            return Err(invalid_v3("diagnostic fact bound exceeded"));
        }
        for cause in &diagnostic.causes {
            if !valid_diagnostic_machine_value(&cause.code) {
                return Err(invalid_v3("diagnostic cause code is invalid"));
            }
            for fact in &cause.facts {
                if !valid_diagnostic_machine_value(&fact.name)
                    || matches!(
                        &fact.value,
                        DiagnosticFactValueV6::Enum(value)
                            if !valid_diagnostic_machine_value(value)
                    )
                    || matches!(
                        &fact.value,
                        DiagnosticFactValueV6::Identifier(value)
                            if !valid_diagnostic_identifier(value)
                    )
                {
                    return Err(invalid_v3("diagnostic fact is invalid"));
                }
            }
        }
        let detail_valid = matches!(
            (
                diagnostic.detail_status.state,
                diagnostic.detail_status.omitted_fact_count
            ),
            (DiagnosticDetailStateV6::Complete, 0)
        ) || matches!(
            diagnostic.detail_status.state,
            DiagnosticDetailStateV6::Truncated
        ) && diagnostic.detail_status.omitted_fact_count > 0;
        if !detail_valid {
            return Err(invalid_v3("diagnostic detail status is inconsistent"));
        }
        let line_bytes = serde_json::to_vec(diagnostic)
            .map_err(|error| invalid_v3(format!("diagnostic cannot serialize: {error}")))?
            .len()
            .saturating_add(1);
        if line_bytes > DIAGNOSTIC_RECORD_MAX_BYTES {
            return Err(invalid_v3("diagnostic exceeds the 8 KiB record bound"));
        }
        stream_bytes = stream_bytes.saturating_add(line_bytes);
    }
    if stream_bytes > DIAGNOSTIC_STREAM_MAX_BYTES {
        return Err(invalid_v3("diagnostic stream exceeds the 16 MiB bound"));
    }
    Ok(())
}

fn valid_diagnostic_machine_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= DIAGNOSTIC_MACHINE_VALUE_MAX_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_diagnostic_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= DIAGNOSTIC_IDENTIFIER_MAX_BYTES
        && !value.chars().any(char::is_control)
}

fn validate_build_identity_v6(
    context: &antennabench_core::v6::RuntimeContextV6,
) -> Result<(), BundleStoreError> {
    let build = &context.build;
    let bounded = [
        build.app_version.as_deref(),
        build.source_commit.as_deref(),
        build.release_tag.as_deref(),
        build.target_triple.as_deref(),
        build.build_architecture.as_deref(),
        context.platform.os_family.as_deref(),
        context.platform.os_version.as_deref(),
        context.platform.runtime_architecture.as_deref(),
        context.platform.application_id.as_deref(),
    ];
    if bounded.into_iter().flatten().any(|value| {
        value.is_empty()
            || value.len() > 256
            || value.chars().any(|character| character.is_control())
    }) {
        return Err(invalid_v3(
            "runtime build and platform values must be bounded nonempty text",
        ));
    }
    if let Some(commit) = build.source_commit.as_deref() {
        if !matches!(commit.len(), 40 | 64)
            || !commit
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(invalid_v3(
                "source commit must be a full lowercase hexadecimal identity",
            ));
        }
    }
    match build.build_channel {
        BuildChannelV6::OfficialRelease => {
            if build.source_state != SourceStateV6::Clean
                || build.source_commit.is_none()
                || build.release_tag.is_none()
                || build.target_triple.is_none()
                || build.build_architecture.is_none()
                || build.app_version.is_none()
                || build.release_tag.as_deref()
                    != build
                        .app_version
                        .as_deref()
                        .map(|version| format!("v{version}"))
                        .as_deref()
            {
                return Err(invalid_v3(
                    "official release context requires agreeing version, tag, clean commit, target, and architecture",
                ));
            }
        }
        BuildChannelV6::Development | BuildChannelV6::Local | BuildChannelV6::Unknown => {
            if build.release_tag.is_some() {
                return Err(invalid_v3(
                    "non-official build contexts cannot claim a release tag",
                ));
            }
        }
    }
    Ok(())
}

fn validate_v3_checkpoint(
    store: &BundleStore,
    bundle: &BundleV3Contents,
    paths: &super::v2::ResolvedBundlePathsV2,
) -> Result<(), BundleStoreError> {
    let plan = [
        (
            &paths.station,
            bundle.session_state.active_plan.station_sha256.as_str(),
        ),
        (
            &paths.antennas,
            bundle.session_state.active_plan.antennas_sha256.as_str(),
        ),
        (
            &paths.schedule,
            bundle.session_state.active_plan.schedule_sha256.as_str(),
        ),
    ];
    let mut plan_digests = Vec::new();
    for (path, expected) in plan {
        let bytes = read_bounded(
            store,
            path,
            store.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            ResourceOperation::Inspect,
        )?;
        let actual = sha256_hex(&bytes);
        if actual != expected {
            return Err(invalid_v3(
                "active plan digest does not match durable bytes",
            ));
        }
        plan_digests.push(actual);
    }
    if sha256_hex(plan_digests.join("\n").as_bytes())
        != bundle.session_state.active_plan.root_sha256
    {
        return Err(invalid_v3(
            "active plan root digest does not match durable bytes",
        ));
    }

    let mut streams = vec![
        (
            "events",
            paths.events.as_path(),
            bundle.events.len(),
            bundle.events.last().map(|record| record.event_id.clone()),
        ),
        (
            "observations",
            paths.observations.as_path(),
            bundle.observations.len(),
            bundle
                .observations
                .last()
                .map(|record| record.observation_id.clone()),
        ),
        (
            "adapter_records",
            paths.adapter_records.as_path(),
            bundle.adapter_records.len(),
            bundle
                .adapter_records
                .last()
                .map(|record| record.record_id.clone()),
        ),
        (
            "rig",
            paths.rig.as_path(),
            bundle.rig.len(),
            bundle.rig.last().map(|record| record.record_id.clone()),
        ),
        (
            "propagation",
            paths.propagation.as_path(),
            bundle.propagation.len(),
            bundle
                .propagation
                .last()
                .map(|record| record.record_id.clone()),
        ),
    ];
    if bundle.manifest.schema_version == SCHEMA_VERSION_V6 {
        let runtime_path = paths
            .runtime_contexts
            .as_deref()
            .ok_or_else(|| invalid_v3("schema-v6 runtime context path is missing"))?;
        streams.push((
            "runtimeContexts",
            runtime_path,
            bundle.runtime_contexts.len(),
            bundle
                .runtime_contexts
                .last()
                .map(|context| context.context_id.clone()),
        ));
        let diagnostics_path = paths
            .diagnostics
            .as_deref()
            .ok_or_else(|| invalid_v3("schema-v6 diagnostics path is missing"))?;
        streams.push((
            "diagnostics",
            diagnostics_path,
            bundle.diagnostics.len(),
            bundle
                .diagnostics
                .last()
                .map(|diagnostic| diagnostic.diagnostic_id.clone()),
        ));
    }
    for (name, path, count, last_id) in streams {
        let expected = bundle
            .session_state
            .streams
            .get(name)
            .ok_or_else(|| invalid_v3(format!("checkpoint is missing stream {name}")))?;
        let bytes = read_bounded(
            store,
            path,
            store.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            ResourceOperation::Inspect,
        )?;
        if &checkpoint_for_bytes(&bytes, count, last_id) != expected {
            return Err(invalid_v3(format!(
                "checkpoint head for stream {name} does not match durable bytes"
            )));
        }
    }
    Ok(())
}

fn ensure_v3_suffix(path: &Path) -> Result<(), BundleStoreError> {
    if path.to_string_lossy().ends_with(V2_BUNDLE_SUFFIX) {
        Ok(())
    } else {
        Err(BundleStoreError::InvalidBundleSuffix {
            path: path.to_path_buf(),
            schema_version: SCHEMA_VERSION_V3,
        })
    }
}

fn invalid_v3(message: impl Into<String>) -> BundleStoreError {
    BundleStoreError::InvalidV3Bundle {
        message: message.into(),
    }
}
