use super::*;

pub(super) struct LoadedSnapshot {
    pub(super) bundle: BundleContents,
    pub(super) intended_cycle_count: usize,
    pub(super) schema_version: u16,
    pub(super) validation: BundleValidationReport,
    pub(super) report_snapshot: ReportSnapshotContext,
    pub(super) revision: Option<u64>,
    pub(super) lifecycle: Option<SessionLifecycleV2>,
}

pub(super) fn open_bundle(path: &Path) -> Result<ActiveSession, OpenSessionError> {
    let bundle_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| bundle_suffix(name).is_some())
        .ok_or_else(|| OpenSessionError::InvalidBundleSelection {
            name: path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into(),
            ),
        })?
        .to_string();

    let snapshot = load_snapshot(path, &bundle_name)?;
    Ok(build_active_session(
        path.to_path_buf(),
        bundle_name,
        snapshot,
    ))
}

pub(super) fn load_snapshot(
    path: &Path,
    bundle_name: &str,
) -> Result<LoadedSnapshot, OpenSessionError> {
    let store = BundleStore::new(path);
    if bundle_name.ends_with(V2_BUNDLE_SUFFIX) {
        let schema_version = store.schema_version()?;
        let (current, revision, lifecycle, report_snapshot, intended_cycle_count) =
            match schema_version {
                SCHEMA_VERSION_V2 => {
                    let bundle = store.read_v2_checkpointed()?;
                    let revision = bundle.session_state.revision;
                    let lifecycle = bundle.session_state.lifecycle;
                    let report_snapshot = report_snapshot(&bundle);
                    let intended_cycle_count = bundle.schedule.slots.len();
                    (
                        bundle.into_current(),
                        revision,
                        lifecycle,
                        report_snapshot,
                        intended_cycle_count,
                    )
                }
                SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 => {
                    let bundle = store.read_v3_checkpointed()?;
                    let revision = bundle.session_state.revision;
                    let lifecycle = bundle.session_state.lifecycle;
                    let report_snapshot = report_snapshot_v3(&bundle);
                    let intended_cycle_count = bundle
                        .schedule
                        .wspr_cycle_intents
                        .len()
                        .max(bundle.schedule.slots.len());
                    (
                        bundle.into_current(),
                        revision,
                        lifecycle,
                        report_snapshot,
                        intended_cycle_count,
                    )
                }
                actual => {
                    return Err(OpenSessionError::Storage(
                        BundleStoreError::UnsupportedSchemaVersion { actual },
                    ));
                }
            };
        let (inspected, validation) = store.inspect()?.into_current_parts();
        let inspected = inspected.ok_or_else(|| {
            OpenSessionError::Storage(BundleStoreError::Validation {
                source: BundleValidationError::from_report(validation.clone()),
            })
        })?;
        if inspected != current {
            return Err(OpenSessionError::SnapshotChanged);
        }
        let bundle = normalize_bundle(current.bundle);
        Ok(LoadedSnapshot {
            bundle,
            intended_cycle_count,
            schema_version,
            validation,
            report_snapshot,
            revision: Some(revision),
            lifecycle: Some(lifecycle),
        })
    } else {
        let (bundle, validation) = store.read_for_analysis()?;
        Ok(LoadedSnapshot {
            intended_cycle_count: bundle.schedule.slots.len(),
            schema_version: bundle.manifest.schema_version,
            bundle,
            validation,
            report_snapshot: ReportSnapshotContext::default(),
            revision: None,
            lifecycle: None,
        })
    }
}

pub(super) fn report_snapshot(
    bundle: &antennabench_core::v2::BundleV2Contents,
) -> ReportSnapshotContext {
    let adapter = report_adapter_evidence(&bundle.adapter_records);
    let lifecycle_events = bundle
        .events
        .iter()
        .filter_map(|event| {
            let (kind, detail) = match &event.payload {
                OperatorEventPayloadV2::SessionStarted { note } => {
                    (ReportLifecycleEventKind::Started, note.clone())
                }
                OperatorEventPayloadV2::SessionInterrupted { reason } => {
                    (ReportLifecycleEventKind::Interrupted, reason.clone())
                }
                OperatorEventPayloadV2::InterruptionDetected { reason } => (
                    ReportLifecycleEventKind::InterruptionDetected,
                    reason.clone(),
                ),
                OperatorEventPayloadV2::SessionResumed { note } => {
                    (ReportLifecycleEventKind::Resumed, note.clone())
                }
                OperatorEventPayloadV2::SessionEnded { reason } => {
                    (ReportLifecycleEventKind::Ended, reason.clone())
                }
                OperatorEventPayloadV2::SessionAbandoned { reason } => {
                    (ReportLifecycleEventKind::Abandoned, reason.clone())
                }
                _ => return None,
            };
            Some(ReportLifecycleEvent {
                kind,
                occurred_at: event.occurred_at,
                detail,
            })
        })
        .collect();
    ReportSnapshotContext {
        checkpoint_revision: Some(bundle.session_state.revision),
        lifecycle: Some(bundle.session_state.lifecycle),
        lifecycle_events,
        operator_events: project_operator_events_v2(&bundle.events),
        wspr_cycles: Vec::new(),
        antenna_control_attempts: Vec::new(),
        adapter_evidence: adapter,
    }
}

pub(super) fn report_snapshot_v3(bundle: &BundleV3Contents) -> ReportSnapshotContext {
    let adapter = report_adapter_evidence(&bundle.adapter_records);
    let lifecycle_events = bundle
        .events
        .iter()
        .filter_map(|event| {
            let (kind, detail) = match &event.payload {
                OperatorEventPayloadV3::SessionStarted { note } => {
                    (ReportLifecycleEventKind::Started, note.clone())
                }
                OperatorEventPayloadV3::SessionInterrupted { reason } => {
                    (ReportLifecycleEventKind::Interrupted, reason.clone())
                }
                OperatorEventPayloadV3::InterruptionDetected { reason } => (
                    ReportLifecycleEventKind::InterruptionDetected,
                    reason.clone(),
                ),
                OperatorEventPayloadV3::SessionResumed { note } => {
                    (ReportLifecycleEventKind::Resumed, note.clone())
                }
                OperatorEventPayloadV3::SessionEnded { reason } => {
                    (ReportLifecycleEventKind::Ended, reason.clone())
                }
                OperatorEventPayloadV3::SessionAbandoned { reason } => {
                    (ReportLifecycleEventKind::Abandoned, reason.clone())
                }
                _ => return None,
            };
            Some(ReportLifecycleEvent {
                kind,
                occurred_at: event.occurred_at,
                detail,
            })
        })
        .collect();
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    let wspr_cycles = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .map(|intent| {
            let observed = projection
                .cycles
                .iter()
                .find(|cycle| cycle.intent_id == intent.intent_id);
            ReportWsprCycle {
                intent_id: intent.intent_id.clone(),
                sequence_number: intent.sequence_number,
                band: intent.band,
                direction: intent.direction,
                planned_antenna: intent.antenna_label.clone(),
                actual_antenna: observed.map(|cycle| cycle.antenna_label.clone()),
                ready_at: observed.map(|cycle| cycle.ready_at),
                starts_at: observed.map(|cycle| cycle.window.starts_at),
                transmission_ends_at: observed.map(|cycle| cycle.window.transmission_ends_at),
                attribution: observed.map_or_else(
                    || {
                        if projection
                            .skipped_intent_ids
                            .iter()
                            .any(|intent_id| intent_id == &intent.intent_id)
                        {
                            ReportWsprAttribution::Skipped
                        } else {
                            ReportWsprAttribution::Pending
                        }
                    },
                    |cycle| {
                        if cycle.occupancy_fully_covers_transmission {
                            ReportWsprAttribution::Attributable
                        } else {
                            ReportWsprAttribution::UnknownAntennaOccupancy
                        }
                    },
                ),
                readiness_basis: bundle.events.iter().find_map(|event| {
                    if event.slot_id.as_deref() != Some(intent.intent_id.as_str()) {
                        return None;
                    }
                    let OperatorEventPayloadV3::WsprCycleArmed { readiness, .. } = &event.payload
                    else {
                        return None;
                    };
                    Some(match readiness {
                        Some(WsprReadinessBasisV5::CommandVerified { .. }) => {
                            ReportWsprReadinessBasis::CommandVerified
                        }
                        Some(WsprReadinessBasisV5::OperatorConfirmed) | None => {
                            ReportWsprReadinessBasis::OperatorConfirmed
                        }
                    })
                }),
            }
        })
        .collect();
    let antenna_control_attempts = bundle
        .rig
        .iter()
        .filter_map(|record| {
            let invocation = record.antenna_control.as_ref()?;
            Some(ReportAntennaControlAttempt {
                record_id: record.record_id.clone(),
                role: invocation.role,
                controller_profile_name: invocation.controller_profile_name.clone(),
                controller_profile_revision: invocation.controller_profile_revision.clone(),
                resolved_program: invocation.command.resolved_program.clone(),
                resolved_arguments: invocation.command.resolved_arguments.clone(),
                intent_id: invocation.context.intent_id.clone(),
                antenna: invocation.context.antenna.clone(),
                target: invocation.context.target.clone(),
                mode: invocation.context.mode,
                started_at: invocation.started_at,
                completed_at: invocation.completed_at,
                elapsed_milliseconds: invocation.elapsed_milliseconds,
                disposition: invocation.disposition.clone(),
                stdout: invocation.stdout.clone(),
                stderr: invocation.stderr.clone(),
            })
        })
        .collect();
    ReportSnapshotContext {
        checkpoint_revision: Some(bundle.session_state.revision),
        lifecycle: Some(bundle.session_state.lifecycle),
        lifecycle_events,
        operator_events: project_operator_events_v3(&bundle.events),
        wspr_cycles,
        antenna_control_attempts,
        adapter_evidence: adapter,
    }
}

pub(super) fn project_operator_events_v2(
    events: &[antennabench_core::v2::OperatorEventV2],
) -> Vec<ReportOperatorEvent> {
    let rejected = reduce_operator_events_v2(SessionLifecycleV2::Ready, events)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.event_id)
        .collect::<std::collections::HashSet<_>>();
    events
        .iter()
        .map(|event| {
            let (kind, detail, correction, replacement_slot) = match &event.payload {
                OperatorEventPayloadV2::SessionStarted { note } => (
                    ReportOperatorEventKind::SessionStarted,
                    note.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SessionInterrupted { reason } => (
                    ReportOperatorEventKind::SessionInterrupted,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::InterruptionDetected { reason } => (
                    ReportOperatorEventKind::InterruptionDetected,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SessionResumed { note } => (
                    ReportOperatorEventKind::SessionResumed,
                    note.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SessionEnded { reason } => (
                    ReportOperatorEventKind::SessionEnded,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SessionAbandoned { reason } => (
                    ReportOperatorEventKind::SessionAbandoned,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::AntennaStateConfirmed {
                    antenna_label,
                    note,
                } => (
                    ReportOperatorEventKind::AntennaStateConfirmed,
                    Some(match note {
                        Some(note) => format!("Actual antenna {antenna_label}; {note}"),
                        None => format!("Actual antenna {antenna_label}"),
                    }),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SlotMissed { reason } => (
                    ReportOperatorEventKind::SlotMissed,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::SlotBad { reason } => (
                    ReportOperatorEventKind::SlotBad,
                    Some(reason.clone()),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::NoteAdded { note } => (
                    ReportOperatorEventKind::NoteAdded,
                    Some(note.clone()),
                    None,
                    None,
                ),
                OperatorEventPayloadV2::EventCorrected {
                    target_event_id,
                    correction,
                    reason,
                } => {
                    let (action, detail, slot) = match correction {
                        EventCorrectionActionV2::Retract => {
                            (ReportEventCorrectionAction::Retracted, None, None)
                        }
                        EventCorrectionActionV2::Replace { replacement } => (
                            ReportEventCorrectionAction::Replaced,
                            Some(correctable_detail_v2(&replacement.payload)),
                            replacement.slot_id.clone(),
                        ),
                    };
                    (
                        ReportOperatorEventKind::EventCorrected,
                        detail,
                        Some(ReportEventCorrection {
                            target_event_id: target_event_id.clone(),
                            action,
                            reason: reason.clone(),
                            applied: !rejected.contains(&event.event_id),
                        }),
                        slot,
                    )
                }
            };
            let affected_slot_id = replacement_slot.or_else(|| {
                correction.as_ref().and_then(|correction| {
                    events
                        .iter()
                        .find(|candidate| candidate.event_id == correction.target_event_id)
                        .and_then(|candidate| candidate.slot_id.clone())
                })
            });
            ReportOperatorEvent {
                event_id: event.event_id.clone(),
                occurred_at: event.occurred_at,
                slot_id: event.slot_id.clone(),
                affected_slot_id: affected_slot_id.or_else(|| event.slot_id.clone()),
                kind,
                detail,
                correction,
            }
        })
        .collect()
}

pub(super) fn project_operator_events_v3(
    events: &[antennabench_core::v3::OperatorEventV3],
) -> Vec<ReportOperatorEvent> {
    let rejected = reduce_operator_events_v3(SessionLifecycleV2::Ready, events)
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.event_id)
        .collect::<std::collections::HashSet<_>>();
    events
        .iter()
        .map(|event| {
            let (kind, detail, correction, replacement_slot) = match &event.payload {
                OperatorEventPayloadV3::SessionStarted { note } => (
                    ReportOperatorEventKind::SessionStarted,
                    note.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SessionInterrupted { reason } => (
                    ReportOperatorEventKind::SessionInterrupted,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::InterruptionDetected { reason } => (
                    ReportOperatorEventKind::InterruptionDetected,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SessionResumed { note } => (
                    ReportOperatorEventKind::SessionResumed,
                    note.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SessionEnded { reason } => (
                    ReportOperatorEventKind::SessionEnded,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SessionAbandoned { reason } => (
                    ReportOperatorEventKind::SessionAbandoned,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::AntennaSwitchStarted { note } => (
                    ReportOperatorEventKind::AntennaSwitchStarted,
                    note.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label,
                    cycle_starts_at,
                    readiness,
                } => (
                    ReportOperatorEventKind::WsprCycleArmed,
                    Some(format!(
                        "{antenna_label} armed for {cycle_starts_at}; readiness {readiness:?}"
                    )),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::AntennaStateConfirmed {
                    antenna_label,
                    note,
                } => (
                    ReportOperatorEventKind::AntennaStateConfirmed,
                    Some(match note {
                        Some(note) => format!("Actual antenna {antenna_label}; {note}"),
                        None => format!("Actual antenna {antenna_label}"),
                    }),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SignalStateConfirmed { confirmation } => (
                    ReportOperatorEventKind::SignalStateConfirmed,
                    Some(format!("Signal-state confirmation {confirmation:?}")),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SlotMissed { reason } => (
                    ReportOperatorEventKind::SlotMissed,
                    reason.clone(),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::SlotBad { reason } => (
                    ReportOperatorEventKind::SlotBad,
                    Some(reason.clone()),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::NoteAdded { note } => (
                    ReportOperatorEventKind::NoteAdded,
                    Some(note.clone()),
                    None,
                    None,
                ),
                OperatorEventPayloadV3::EventCorrected {
                    target_event_id,
                    correction,
                    reason,
                } => {
                    let (action, detail, slot) = match correction {
                        EventCorrectionActionV3::Retract => {
                            (ReportEventCorrectionAction::Retracted, None, None)
                        }
                        EventCorrectionActionV3::Replace { replacement } => (
                            ReportEventCorrectionAction::Replaced,
                            Some(correctable_detail_v3(&replacement.payload)),
                            replacement.slot_id.clone(),
                        ),
                    };
                    (
                        ReportOperatorEventKind::EventCorrected,
                        detail,
                        Some(ReportEventCorrection {
                            target_event_id: target_event_id.clone(),
                            action,
                            reason: reason.clone(),
                            applied: !rejected.contains(&event.event_id),
                        }),
                        slot,
                    )
                }
            };
            let affected_slot_id = replacement_slot.or_else(|| {
                correction.as_ref().and_then(|correction| {
                    events
                        .iter()
                        .find(|candidate| candidate.event_id == correction.target_event_id)
                        .and_then(|candidate| candidate.slot_id.clone())
                })
            });
            ReportOperatorEvent {
                event_id: event.event_id.clone(),
                occurred_at: event.occurred_at,
                slot_id: event.slot_id.clone(),
                affected_slot_id: affected_slot_id.or_else(|| event.slot_id.clone()),
                kind,
                detail,
                correction,
            }
        })
        .collect()
}

pub(super) fn correctable_detail_v2(payload: &CorrectableOperatorEventPayloadV2) -> String {
    match payload {
        CorrectableOperatorEventPayloadV2::AntennaStateConfirmed {
            antenna_label,
            note,
        } => note.as_ref().map_or_else(
            || format!("Actual antenna {antenna_label}"),
            |note| format!("Actual antenna {antenna_label}; {note}"),
        ),
        CorrectableOperatorEventPayloadV2::SlotMissed { reason } => {
            reason.clone().unwrap_or_else(|| "Slot missed".into())
        }
        CorrectableOperatorEventPayloadV2::SlotBad { reason } => reason.clone(),
        CorrectableOperatorEventPayloadV2::NoteAdded { note } => note.clone(),
    }
}

pub(super) fn correctable_detail_v3(payload: &CorrectableOperatorEventPayloadV3) -> String {
    match payload {
        CorrectableOperatorEventPayloadV3::AntennaStateConfirmed {
            antenna_label,
            note,
        } => note.as_ref().map_or_else(
            || format!("Actual antenna {antenna_label}"),
            |note| format!("Actual antenna {antenna_label}; {note}"),
        ),
        CorrectableOperatorEventPayloadV3::SignalStateConfirmed { confirmation } => {
            format!("Signal-state confirmation {confirmation:?}")
        }
        CorrectableOperatorEventPayloadV3::SlotMissed { reason } => {
            reason.clone().unwrap_or_else(|| "Slot missed".into())
        }
        CorrectableOperatorEventPayloadV3::SlotBad { reason } => reason.clone(),
        CorrectableOperatorEventPayloadV3::NoteAdded { note } => note.clone(),
    }
}

pub(super) fn report_adapter_evidence(records: &[AdapterRecordV2]) -> ReportAdapterEvidence {
    let mut adapter = ReportAdapterEvidence {
        record_count: records.len(),
        ..ReportAdapterEvidence::default()
    };
    for record in records {
        match record.disposition {
            AdapterDisposition::Accepted => adapter.accepted_count += 1,
            AdapterDisposition::Malformed => adapter.malformed_count += 1,
            AdapterDisposition::Unsupported => adapter.unsupported_count += 1,
            AdapterDisposition::Filtered => adapter.filtered_count += 1,
            AdapterDisposition::Duplicate => adapter.duplicate_count += 1,
            AdapterDisposition::Conflict => adapter.conflict_count += 1,
            AdapterDisposition::PartiallyNormalized => adapter.partially_normalized_count += 1,
        }
        if record.record_type == "acquisition_gap" {
            adapter.gap_count += 1;
        }
        if record.record_type == "wspr_live_import_summary" {
            if let AdapterInput::Inline { data, .. } = &record.input {
                if let Ok(import) = serde_json::from_str::<WsprLiveReportImport>(data) {
                    adapter.imports.push(import.into_report());
                }
            }
        }
    }
    adapter.evidence_complete = adapter.gap_count == 0
        && adapter
            .imports
            .iter()
            .all(|import| import.completeness_known);
    adapter
}

#[derive(Debug, Deserialize)]
pub(super) struct WsprLiveReportImport {
    provider_id: String,
    source_id: String,
    captured_at: chrono::DateTime<chrono::Utc>,
    window_start: chrono::DateTime<chrono::Utc>,
    window_end: chrono::DateTime<chrono::Utc>,
    selected_bands: Vec<Band>,
    completeness: String,
    counts: WsprLiveReportCounts,
}

#[derive(Debug, Deserialize)]
pub(super) struct WsprLiveReportCounts {
    total: usize,
    accepted: usize,
    malformed: usize,
    filtered: usize,
    unsupported: usize,
    duplicate: usize,
    conflict: usize,
    observations_created: usize,
}

impl WsprLiveReportImport {
    fn into_report(self) -> ReportImportedEvidence {
        ReportImportedEvidence {
            provider_id: self.provider_id,
            source_id: self.source_id,
            captured_at: self.captured_at,
            window_start: self.window_start,
            window_end: self.window_end,
            selected_bands: self.selected_bands,
            total_count: self.counts.total,
            accepted_count: self.counts.accepted,
            malformed_count: self.counts.malformed,
            filtered_count: self.counts.filtered,
            unsupported_count: self.counts.unsupported,
            duplicate_count: self.counts.duplicate,
            conflict_count: self.counts.conflict,
            observations_created: self.counts.observations_created,
            completeness_known: self.completeness == "known",
        }
    }
}

pub(super) fn build_active_session(
    source: PathBuf,
    bundle_name: String,
    snapshot: LoadedSnapshot,
) -> ActiveSession {
    let presentation = prepare_presentation(&snapshot).ok();
    ActiveSession {
        source,
        summary: OpenedSession {
            bundle_name,
            session_id: snapshot.bundle.manifest.session_id.clone(),
            callsign: snapshot.bundle.station.callsign.clone(),
            grid: snapshot.bundle.station.grid.clone(),
            antenna_count: snapshot.bundle.antennas.antennas.len(),
            slot_count: snapshot.intended_cycle_count,
            observation_count: snapshot.bundle.observations.len(),
            schema_version: snapshot.schema_version,
            revision: snapshot.revision,
            lifecycle: snapshot.lifecycle,
            report_available: presentation.is_some(),
        },
        presentation,
    }
}

pub(super) fn prepare_presentation(
    snapshot: &LoadedSnapshot,
) -> Result<ReportPresentation, ReportError> {
    let report = build_report_with_snapshot(
        &snapshot.bundle,
        &snapshot.validation,
        snapshot.report_snapshot.clone(),
    )?;
    let has_controller_evidence = !report.snapshot.antenna_control_attempts.is_empty();
    let report_html = render_standalone_html(&report)?;
    let compact_summary_html = render_compact_summary_html(&report)?;
    let controller_omitted_report_html = has_controller_evidence
        .then(|| {
            render_standalone_html_with_options(
                &report,
                StandaloneHtmlOptions {
                    controller_evidence: ControllerEvidenceHandling::OmittedAtExport,
                },
            )
        })
        .transpose()?;
    Ok(ReportPresentation {
        presentation_id: 0,
        session_id: snapshot.bundle.manifest.session_id.clone(),
        revision: snapshot.revision,
        lifecycle: snapshot.lifecycle,
        completeness: report.completeness,
        has_controller_evidence,
        report_html,
        compact_summary_html,
        controller_omitted_report_html,
    })
}
