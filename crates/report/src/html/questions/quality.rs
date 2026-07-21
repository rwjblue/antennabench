use super::*;
use crate::{
    html::{
        templates::{
            render_template, QualityAccountingBetweenTemplate, QualityAccountingStartTemplate,
            QualityAfterExclusionsTemplate, QualityEndTemplate, QualityStartTemplate,
        },
        view::{
            AcquisitionQualityView, AnswerabilityRowView, ExclusionSummaryRowView,
            LifecycleChipView, QualityFactView, QualityTimelineRowView, QualityView,
            TimelineEventView,
        },
    },
    ReportAcquisitionWorkflowStatus,
};

pub(in super::super) fn render_run_quality_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let view = quality_view(report);
    let has_exclusion_records = view.has_exclusion_records;
    render_template(out, &QualityStartTemplate { view })?;
    if has_exclusion_records {
        render_exclusion_records(out, report)?;
    }
    render_template(
        out,
        &QualityAfterExclusionsTemplate {
            had_records: has_exclusion_records,
        },
    )?;
    render_notices(out, &report.notices)?;
    render_eligibility(out, report)?;
    render_template(out, &QualityAccountingStartTemplate)?;
    render_overall(out, report)?;
    render_antenna_section(out, report)?;
    render_band_section(out, report)?;
    render_template(out, &QualityAccountingBetweenTemplate)?;
    render_slot_section(out, report)?;
    render_template(out, &QualityEndTemplate)
}

fn quality_view(report: &SessionReport) -> QualityView {
    let (left_label, right_label) = labels(report);
    QualityView {
        no_strata: report.overview.strata.is_empty(),
        comparison_state: comparison_availability_label(report.overview.comparison_availability),
        comparison_text: comparison_availability_text(report.overview.comparison_availability),
        answerability: report
            .overview
            .strata
            .iter()
            .map(|row| AnswerabilityRowView {
                group: stratum(&row.stratum),
                availability: match row.availability {
                    ReportStratumAvailability::DescriptivePairsAvailable => {
                        "Answerable — matched pairs available"
                    }
                    ReportStratumAvailability::NoFinitePairedPaths => {
                        "Unavailable — no usable same-path pair"
                    }
                },
                pairs: row.paired_row_count,
                blocks: row.contributing_block_count,
                unique_paths: row.unique_path_count,
                left_then_right: row.left_then_right_block_count,
                right_then_left: row.right_then_left_block_count,
                unmatched_left: row.unmatched_left_count,
                unmatched_right: row.unmatched_right_count,
                missing_left: row.missing_snr_left_count,
                missing_right: row.missing_snr_right_count,
                excluded: row.excluded_observation_count,
                duplicates: row.exact_duplicate_count,
                conflicts: row.conflicting_duplicate_group_count,
            })
            .collect(),
        lifecycle: report
            .snapshot
            .lifecycle_events
            .iter()
            .map(|event| LifecycleChipView {
                symbol: match event.kind {
                    ReportLifecycleEventKind::Interrupted
                    | ReportLifecycleEventKind::InterruptionDetected => "!",
                    ReportLifecycleEventKind::Resumed => "↻",
                    ReportLifecycleEventKind::Abandoned => "×",
                    ReportLifecycleEventKind::Ended => "■",
                    ReportLifecycleEventKind::Started => "▶",
                },
                event: lifecycle_event(event.kind),
                occurred_at: timestamp(event.occurred_at),
            })
            .collect(),
        timeline: report
            .overview
            .timeline
            .iter()
            .map(|row| timeline_view(row, report))
            .collect(),
        acquisition: acquisition_view(report),
        exclusions: report
            .evidence
            .overall
            .exclusions
            .iter()
            .map(|exclusion| ExclusionSummaryRowView {
                reason: exclusion_reason(exclusion.reason),
                count: exclusion.count,
            })
            .collect(),
        has_exclusion_records: !report.exclusion_records.is_empty(),
        left_label,
        right_label,
    }
}

fn timeline_view(row: &ReportRunTimelineRow, report: &SessionReport) -> QualityTimelineRowView {
    let (class, state, symbol) = timeline_compact_state(row, report);
    let block = row.block_index.map_or_else(
        || "Not applicable".into(),
        |index| {
            format!(
                "{}; {}",
                index + 1,
                row.block_eligibility
                    .map(block_eligibility)
                    .unwrap_or("Not recorded")
            )
        },
    );
    let actual_window = match (row.actual_starts_at, row.actual_ends_at) {
        (Some(start), Some(end)) => format!("{} – {}", timestamp(start), timestamp(end)),
        (Some(start), None) => format!("{} – end not recorded", timestamp(start)),
        _ => "Not recorded".into(),
    };
    QualityTimelineRowView {
        class,
        state,
        symbol,
        sequence: row.sequence_number,
        planned_antenna: row.planned_antenna.clone(),
        actual_antenna: row
            .actual_antenna
            .clone()
            .unwrap_or_else(|| "Not recorded".into()),
        usable: row.usable_observation_count,
        excluded: row.excluded_observation_count,
        facts: vec![
            QualityFactView {
                label: "Item",
                value: row.item_id.clone(),
            },
            QualityFactView {
                label: "State",
                value: state.to_string(),
            },
            QualityFactView {
                label: "Band / direction",
                value: format!(
                    "{} / {}",
                    band(row.band),
                    row.direction.map(wspr_direction).unwrap_or("Not recorded")
                ),
            },
            QualityFactView {
                label: "Block",
                value: block,
            },
            QualityFactView {
                label: "Planned window",
                value: format!(
                    "{} – {}",
                    timestamp(row.planned_starts_at),
                    timestamp(row.planned_ends_at)
                ),
            },
            QualityFactView {
                label: "Actual window",
                value: actual_window,
            },
            QualityFactView {
                label: "Readiness",
                value: row
                    .readiness_basis
                    .map(wspr_readiness)
                    .unwrap_or("Not recorded")
                    .to_string(),
            },
            QualityFactView {
                label: "Attribution",
                value: row
                    .attribution
                    .map(wspr_attribution)
                    .unwrap_or("Not recorded")
                    .to_string(),
            },
            QualityFactView {
                label: "Evidence",
                value: format!(
                    "{} total; {} usable; {} excluded",
                    row.total_observation_count,
                    row.usable_observation_count,
                    row.excluded_observation_count
                ),
            },
        ],
        events: row.event_history.iter().map(timeline_event_view).collect(),
    }
}

fn timeline_event_view(event: &ReportOperatorEvent) -> TimelineEventView {
    let (correction_action_value, correction_target, correction_reason, correction_state) = event
        .correction
        .as_ref()
        .map(|correction| {
            (
                Some(correction_action(correction.action)),
                Some(correction.target_event_id.clone()),
                Some(correction.reason.clone()),
                Some(if correction.applied {
                    "applied"
                } else {
                    "not applied"
                }),
            )
        })
        .unwrap_or((None, None, None, None));
    TimelineEventView {
        event_id: event.event_id.clone(),
        occurred_at: timestamp(event.occurred_at),
        kind: operator_event_kind(event.kind),
        detail: event.detail.clone(),
        correction_action: correction_action_value,
        correction_target,
        correction_reason,
        correction_state,
    }
}

fn acquisition_view(report: &SessionReport) -> AcquisitionQualityView {
    let evidence = &report.snapshot.adapter_evidence;
    if evidence.gap_count > 0 {
        AcquisitionQualityView {
            paragraph_class: "notice critical",
            lead: Some("Explicit acquisition gap:"),
            body: format!(
                "{} recorded gap{}; retained unrelated valid evidence remains usable. Inspect provider windows and durable adapter records in the audit appendix.",
                evidence.gap_count,
                plural_suffix(evidence.gap_count)
            ),
        }
    } else if evidence.workflow_status == ReportAcquisitionWorkflowStatus::Incomplete {
        AcquisitionQualityView {
            paragraph_class: "notice critical",
            lead: Some("Recorded acquisition is incomplete."),
            body: "No durable gap count is available; inspect provider windows and lifecycle records for the recorded reason."
                .into(),
        }
    } else if evidence.workflow_status == ReportAcquisitionWorkflowStatus::Completed {
        let body = if evidence
            .imports
            .iter()
            .any(|import| import.provider_id == "wspr-live")
        {
            "AntennaBench retained the spots returned for the configured WSPR.live request windows; upstream completeness is not independently guaranteed."
                .into()
        } else {
            format!(
                "{} No explicit acquisition gap is recorded; adapter row dispositions remain available in the audit appendix.",
                provider_completeness_sentence(evidence.provider_completeness)
            )
        };
        AcquisitionQualityView {
            paragraph_class: "",
            lead: Some("Collection completed."),
            body,
        }
    } else {
        AcquisitionQualityView {
            paragraph_class: "empty",
            lead: None,
            body: "No acquisition workflow was configured. No workflow-completion or provider-completeness claim is inferred."
                .into(),
        }
    }
}

fn labels(report: &SessionReport) -> (String, String) {
    (
        report
            .comparison
            .left_label
            .clone()
            .unwrap_or_else(|| "Left".into()),
        report
            .comparison
            .right_label
            .clone()
            .unwrap_or_else(|| "Right".into()),
    )
}

fn stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        value.mode.as_str(),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}

pub(in super::super) fn timeline_compact_state(
    row: &ReportRunTimelineRow,
    report: &SessionReport,
) -> (&'static str, &'static str, &'static str) {
    if row.event_history.iter().any(|event| {
        event.kind == ReportOperatorEventKind::EventCorrected
            && event
                .correction
                .as_ref()
                .is_some_and(|correction| correction.applied)
    }) {
        return ("corrected", "Corrected", "✓");
    }
    if report.snapshot.lifecycle_events.iter().any(|event| {
        matches!(
            event.kind,
            ReportLifecycleEventKind::Interrupted | ReportLifecycleEventKind::InterruptionDetected
        ) && event.occurred_at >= row.planned_starts_at
            && event.occurred_at < row.planned_ends_at
    }) {
        return ("interrupted", "Interrupted", "!");
    }
    match row.status {
        AlignedSlotStatus::Missed => ("missed", "Missed", "—"),
        AlignedSlotStatus::Bad | AlignedSlotStatus::ConflictingEvidence => {
            ("bad", slot_status(row.status), "×")
        }
        AlignedSlotStatus::UnknownActualState => ("unknown", "Unknown occupancy", "?"),
        AlignedSlotStatus::LateSwitch => ("late", "Late switch", "△"),
        _ if row.attribution == Some(crate::ReportWsprAttribution::UnknownAntennaOccupancy) => {
            ("unknown", "Unknown occupancy", "?")
        }
        _ if row.attribution == Some(crate::ReportWsprAttribution::Skipped) => {
            ("missed", "Skipped", "—")
        }
        _ => ("ordinary", slot_status(row.status), "●"),
    }
}
