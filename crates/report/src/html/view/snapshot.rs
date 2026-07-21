use crate::{
    html::ControllerEvidenceHandling, ReportAcquisitionWorkflowStatus, ReportCompleteness,
    ReportLifecycleEventKind, ReportWsprAttribution, ReportWsprReadinessBasis, SessionReport,
};
use antennabench_core::{
    v2::SessionLifecycleV2, v3::WsprCycleDirection, v5::AntennaControlDispositionV5,
};

use super::super::shared::*;

#[derive(Debug, Clone)]
pub(in crate::html) struct FactView {
    pub(in crate::html) label: String,
    pub(in crate::html) value: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LifecycleRowView {
    pub(in crate::html) event: &'static str,
    pub(in crate::html) time: String,
    pub(in crate::html) detail: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OperatorEventRowView {
    pub(in crate::html) event_id: String,
    pub(in crate::html) time: String,
    pub(in crate::html) recorded_slot: String,
    pub(in crate::html) affected_slot: String,
    pub(in crate::html) kind: &'static str,
    pub(in crate::html) detail: String,
    pub(in crate::html) correction: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct WsprCycleRowView {
    pub(in crate::html) sequence: u32,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) direction: &'static str,
    pub(in crate::html) planned: String,
    pub(in crate::html) actual: String,
    pub(in crate::html) readiness: &'static str,
    pub(in crate::html) ready_at: String,
    pub(in crate::html) starts_at: String,
    pub(in crate::html) ends_at: String,
    pub(in crate::html) attribution: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ControllerAttemptView {
    pub(in crate::html) record_id: String,
    pub(in crate::html) role: String,
    pub(in crate::html) intent_id: String,
    pub(in crate::html) antenna: String,
    pub(in crate::html) target: String,
    pub(in crate::html) mode: &'static str,
    pub(in crate::html) controller_name: String,
    pub(in crate::html) controller_revision: String,
    pub(in crate::html) program: String,
    pub(in crate::html) arguments: String,
    pub(in crate::html) timing: String,
    pub(in crate::html) outcome: String,
    pub(in crate::html) stdout: String,
    pub(in crate::html) stderr: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SnapshotView {
    pub(in crate::html) facts: Vec<FactView>,
    pub(in crate::html) lifecycle_rows: Vec<LifecycleRowView>,
    pub(in crate::html) operator_rows: Vec<OperatorEventRowView>,
    pub(in crate::html) cycle_rows: Vec<WsprCycleRowView>,
    pub(in crate::html) controller_attempts: Vec<ControllerAttemptView>,
    pub(in crate::html) controller_omitted: bool,
}

impl SnapshotView {
    pub(in crate::html) fn new(
        report: &SessionReport,
        handling: ControllerEvidenceHandling,
    ) -> Self {
        let snapshot = &report.snapshot;
        let mut facts = vec![
            FactView {
                label: "Checkpoint revision".into(),
                value: snapshot.checkpoint_revision.map_or_else(
                    || "Legacy static bundle".into(),
                    |revision| revision.to_string(),
                ),
            },
            FactView {
                label: "Lifecycle".into(),
                value: snapshot
                    .lifecycle
                    .map_or("Not recorded", lifecycle_label)
                    .into(),
            },
            FactView {
                label: "Report detail".into(),
                value: match report.completeness {
                    ReportCompleteness::FullDetail => "Full detail",
                    ReportCompleteness::BoundedOverview => "Bounded overview",
                }
                .into(),
            },
            FactView {
                label: "Adapter evidence".into(),
                value: format!(
                    "{} records; {} accepted; {} malformed; {} unsupported; {} filtered; {} duplicate; {} conflict; {} partial",
                    snapshot.adapter_evidence.record_count,
                    snapshot.adapter_evidence.accepted_count,
                    snapshot.adapter_evidence.malformed_count,
                    snapshot.adapter_evidence.unsupported_count,
                    snapshot.adapter_evidence.filtered_count,
                    snapshot.adapter_evidence.duplicate_count,
                    snapshot.adapter_evidence.conflict_count,
                    snapshot.adapter_evidence.partially_normalized_count,
                ),
            },
            FactView {
                label: "Recorded acquisition".into(),
                value: acquisition_text(report),
            },
        ];
        let wspr_live_imports = snapshot
            .adapter_evidence
            .imports
            .iter()
            .filter(|import| import.provider_id == "wspr-live")
            .count();
        if wspr_live_imports > 0 {
            facts.push(FactView {
                label: "Public collection".into(),
                value: if snapshot.adapter_evidence.gap_count == 0
                    && snapshot.adapter_evidence.workflow_status
                        == ReportAcquisitionWorkflowStatus::Completed
                {
                    format!(
                        "Best-effort public collection completed for {} recorded requested window(s)",
                        wspr_live_imports
                    )
                } else if snapshot.adapter_evidence.gap_count > 0 {
                    format!(
                        "Best-effort public collection retained rows for {} recorded requested window(s); recorded acquisition gaps remain",
                        wspr_live_imports
                    )
                } else {
                    format!(
                        "Best-effort public collection retained rows for {} recorded requested window(s); the configured workflow did not complete",
                        wspr_live_imports
                    )
                },
            });
            facts.push(FactView {
                label: "Public-source boundary".into(),
                value: "AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee.".into(),
            });
        }
        facts.extend(snapshot.adapter_evidence.imports.iter().enumerate().map(
            |(index, import)| FactView {
                label: format!("Imported evidence {}", index + 1),
                value: format!(
                    "{} / {}; captured {}; half-open window {} to {}; bands {}; {} rows: {} accepted, {} malformed, {} unsupported, {} filtered, {} duplicate, {} conflict; {} observations created; {}",
                    import.provider_id,
                    import.source_id,
                    timestamp(import.captured_at),
                    timestamp(import.window_start),
                    timestamp(import.window_end),
                    import.selected_bands.iter().map(|value| band(*value)).collect::<Vec<_>>().join(", "),
                    import.total_count,
                    import.accepted_count,
                    import.malformed_count,
                    import.unsupported_count,
                    import.filtered_count,
                    import.duplicate_count,
                    import.conflict_count,
                    import.observations_created,
                    import_source_boundary(import),
                ),
            },
        ));

        Self {
            facts,
            lifecycle_rows: snapshot
                .lifecycle_events
                .iter()
                .map(|event| LifecycleRowView {
                    event: lifecycle_event_label(event.kind),
                    time: timestamp(event.occurred_at),
                    detail: event.detail.clone().unwrap_or_else(not_recorded),
                })
                .collect(),
            operator_rows: snapshot
                .operator_events
                .iter()
                .map(|event| OperatorEventRowView {
                    event_id: event.event_id.clone(),
                    time: timestamp(event.occurred_at),
                    recorded_slot: event.slot_id.clone().unwrap_or_else(not_recorded),
                    affected_slot: event.affected_slot_id.clone().unwrap_or_else(not_recorded),
                    kind: operator_event_kind(event.kind),
                    detail: event.detail.clone().unwrap_or_else(not_recorded),
                    correction: event.correction.as_ref().map_or_else(
                        || "None".into(),
                        |correction| {
                            format!(
                                "{} {}: {} ({})",
                                correction_action(correction.action),
                                correction.target_event_id,
                                correction.reason,
                                if correction.applied {
                                    "applied"
                                } else {
                                    "not applied"
                                }
                            )
                        },
                    ),
                })
                .collect(),
            cycle_rows: snapshot.wspr_cycles.iter().map(cycle_view).collect(),
            controller_attempts: snapshot
                .antenna_control_attempts
                .iter()
                .map(|attempt| controller_attempt_view(attempt, handling))
                .collect(),
            controller_omitted: handling == ControllerEvidenceHandling::OmittedAtExport,
        }
    }
}

fn acquisition_text(report: &SessionReport) -> String {
    let evidence = &report.snapshot.adapter_evidence;
    if evidence.gap_count == 1 {
        "1 recorded acquisition gap; inspect the durable adapter evidence and lifecycle history for its recorded reason".into()
    } else if evidence.gap_count > 1 {
        format!(
            "{} recorded acquisition gaps; inspect the durable adapter evidence and lifecycle history for their recorded reasons",
            evidence.gap_count
        )
    } else if evidence.workflow_status == ReportAcquisitionWorkflowStatus::Incomplete {
        "Recorded acquisition is incomplete; inspect the durable adapter evidence and lifecycle history for the recorded reason".into()
    } else if evidence.workflow_status == ReportAcquisitionWorkflowStatus::Completed {
        "Collection completed; no recorded acquisition gaps".into()
    } else {
        "No acquisition workflow configured".into()
    }
}

fn lifecycle_label(value: SessionLifecycleV2) -> &'static str {
    match value {
        SessionLifecycleV2::Draft => "Draft",
        SessionLifecycleV2::Ready => "Ready",
        SessionLifecycleV2::Running => "Running / in progress",
        SessionLifecycleV2::Interrupted => "Interrupted / in progress",
        SessionLifecycleV2::Ended => "Ended / final",
        SessionLifecycleV2::Abandoned => "Abandoned / final",
    }
}

fn lifecycle_event_label(value: ReportLifecycleEventKind) -> &'static str {
    match value {
        ReportLifecycleEventKind::Started => "Started",
        ReportLifecycleEventKind::Interrupted => "Interrupted",
        ReportLifecycleEventKind::InterruptionDetected => "Interruption detected",
        ReportLifecycleEventKind::Resumed => "Resumed",
        ReportLifecycleEventKind::Ended => "Ended",
        ReportLifecycleEventKind::Abandoned => "Abandoned",
    }
}

fn cycle_view(cycle: &crate::ReportWsprCycle) -> WsprCycleRowView {
    WsprCycleRowView {
        sequence: cycle.sequence_number,
        band: band(cycle.band),
        direction: match cycle.direction {
            Some(WsprCycleDirection::Receive) => "Receive",
            Some(WsprCycleDirection::Transmit) => "Transmit",
            None => "Not recorded",
        },
        planned: cycle.planned_antenna.clone(),
        actual: cycle.actual_antenna.clone().unwrap_or_else(not_recorded),
        readiness: match cycle.readiness_basis {
            Some(ReportWsprReadinessBasis::OperatorConfirmed) => "Operator confirmed",
            Some(ReportWsprReadinessBasis::CommandVerified) => "Command verified",
            Some(ReportWsprReadinessBasis::Continued) => "Continued readiness",
            None => "Not recorded",
        },
        ready_at: cycle.ready_at.map_or_else(|| "—".into(), timestamp),
        starts_at: cycle.starts_at.map_or_else(|| "—".into(), timestamp),
        ends_at: cycle
            .transmission_ends_at
            .map_or_else(|| "—".into(), timestamp),
        attribution: match cycle.attribution {
            ReportWsprAttribution::Pending => "Not yet run",
            ReportWsprAttribution::Skipped => "Skipped by operator",
            ReportWsprAttribution::Attributable => "Full antenna occupancy recorded",
            ReportWsprAttribution::UnknownAntennaOccupancy => {
                "Unknown — antenna changed during transmission"
            }
        },
    }
}

fn controller_attempt_view(
    attempt: &crate::ReportAntennaControlAttempt,
    handling: ControllerEvidenceHandling,
) -> ControllerAttemptView {
    const OMITTED: &str = "Omitted at export — retained in the session bundle";
    let omitted = handling == ControllerEvidenceHandling::OmittedAtExport;
    ControllerAttemptView {
        record_id: attempt.record_id.clone(),
        role: format!("{:?}", attempt.role),
        intent_id: attempt.intent_id.clone(),
        antenna: attempt.antenna.clone(),
        target: if omitted {
            OMITTED.into()
        } else {
            attempt.target.clone()
        },
        mode: experiment_mode(attempt.mode),
        controller_name: attempt.controller_profile_name.clone(),
        controller_revision: attempt.controller_profile_revision.clone(),
        program: if omitted {
            OMITTED.into()
        } else {
            attempt.resolved_program.clone()
        },
        arguments: if omitted {
            OMITTED.into()
        } else {
            attempt
                .resolved_arguments
                .iter()
                .enumerate()
                .map(|(index, value)| format!("[{index}]={value:?}"))
                .collect::<Vec<_>>()
                .join(" ")
        },
        timing: format!(
            "Started {}; completed {}; {} ms elapsed",
            timestamp(attempt.started_at),
            timestamp(attempt.completed_at),
            attempt.elapsed_milliseconds
        ),
        outcome: if omitted {
            match &attempt.disposition {
                AntennaControlDispositionV5::Exit { code } => format!("Exit code {code}"),
                AntennaControlDispositionV5::SpawnError { .. } => "Spawn error".into(),
                AntennaControlDispositionV5::Signaled { signal } => signal.map_or_else(
                    || "Terminated by signal; signal number not recorded".into(),
                    |signal| format!("Terminated by signal {signal}"),
                ),
                AntennaControlDispositionV5::Timeout => "Timed out".into(),
            }
        } else {
            format!(
                "{:?}; {} ms",
                attempt.disposition, attempt.elapsed_milliseconds
            )
        },
        stdout: if omitted {
            OMITTED.into()
        } else {
            format!(
                "{:?}; truncated={}; {}",
                attempt.stdout.encoding, attempt.stdout.truncated, attempt.stdout.data
            )
        },
        stderr: if omitted {
            OMITTED.into()
        } else {
            format!(
                "{:?}; truncated={}; {}",
                attempt.stderr.encoding, attempt.stderr.truncated, attempt.stderr.data
            )
        },
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AntennaContextView {
    pub(in crate::html) label: String,
    pub(in crate::html) details: Vec<FactView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ScheduleRowView {
    pub(in crate::html) sequence: u32,
    pub(in crate::html) slot_id: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) antenna: String,
    pub(in crate::html) starts_at: String,
    pub(in crate::html) ends_at: String,
    pub(in crate::html) guard_seconds: u32,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ContextView {
    pub(in crate::html) facts: Vec<FactView>,
    pub(in crate::html) antennas: Vec<AntennaContextView>,
    pub(in crate::html) schedule: Vec<ScheduleRowView>,
}

impl ContextView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let context = &report.context;
        Self {
            facts: vec![
                FactView {
                    label: "Callsign".into(),
                    value: context.station.callsign.clone(),
                },
                FactView {
                    label: "Grid".into(),
                    value: context.station.grid.clone(),
                },
                FactView {
                    label: "Power".into(),
                    value: context
                        .station
                        .power_watts
                        .map(|value| format!("{} W", format_number(f64::from(value))))
                        .unwrap_or_else(not_recorded),
                },
                FactView {
                    label: "Experiment mode".into(),
                    value: experiment_mode(context.experiment_mode).into(),
                },
                FactView {
                    label: "Goal".into(),
                    value: session_goal(context.goal).into(),
                },
                FactView {
                    label: "Scheduled range".into(),
                    value: context
                        .scheduled_time_range
                        .as_ref()
                        .map(|range| {
                            format!(
                                "{} – {}",
                                timestamp(range.starts_at),
                                timestamp(range.ends_at)
                            )
                        })
                        .unwrap_or_else(|| "No scheduled time range".into()),
                },
                FactView {
                    label: "Scheduled bands".into(),
                    value: if context.bands.is_empty() {
                        "None".into()
                    } else {
                        context
                            .bands
                            .iter()
                            .map(|value| band(*value))
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                },
                FactView {
                    label: "Scheduled slots".into(),
                    value: context.schedule.slot_count.to_string(),
                },
            ],
            antennas: context
                .antennas
                .iter()
                .map(|antenna| AntennaContextView {
                    label: antenna.label.clone(),
                    details: vec![
                        FactView {
                            label: "Facets".into(),
                            value: optional_join(&antenna.facets),
                        },
                        FactView {
                            label: "Height".into(),
                            value: optional_measure(antenna.height_m, "m"),
                        },
                        FactView {
                            label: "Radials".into(),
                            value: antenna
                                .radial_count
                                .map(|value| value.to_string())
                                .unwrap_or_else(not_recorded),
                        },
                        FactView {
                            label: "Radial length".into(),
                            value: optional_measure(antenna.radial_length_m, "m"),
                        },
                        FactView {
                            label: "Orientation".into(),
                            value: optional_measure(antenna.orientation_degrees, "°"),
                        },
                        FactView {
                            label: "Tuner".into(),
                            value: antenna.tuner.clone().unwrap_or_else(not_recorded),
                        },
                        FactView {
                            label: "Feedline".into(),
                            value: antenna.feedline.clone().unwrap_or_else(not_recorded),
                        },
                        FactView {
                            label: "Notes".into(),
                            value: antenna.notes.clone().unwrap_or_else(not_recorded),
                        },
                    ],
                })
                .collect(),
            schedule: context
                .schedule
                .slots
                .iter()
                .map(|slot| ScheduleRowView {
                    sequence: slot.sequence_number,
                    slot_id: slot.slot_id.clone(),
                    band: band(slot.band),
                    antenna: slot.planned_label.clone(),
                    starts_at: timestamp(slot.starts_at),
                    ends_at: timestamp(slot.ends_at),
                    guard_seconds: slot.guard_seconds,
                })
                .collect(),
        }
    }
}
