use super::*;

pub(in super::super) fn render_snapshot(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    controller_evidence: ControllerEvidenceHandling,
) {
    let snapshot = &report.snapshot;
    if !snapshot_has_detail(report) {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"snapshot-title\"><h2 id=\"snapshot-title\">Committed session snapshot</h2><dl class=\"facts\">");
    fact(
        out,
        "Checkpoint revision",
        &snapshot.checkpoint_revision.map_or_else(
            || "Legacy static bundle".into(),
            |revision| revision.to_string(),
        ),
    );
    fact(
        out,
        "Lifecycle",
        snapshot.lifecycle.map_or("Not recorded", lifecycle),
    );
    fact(
        out,
        "Report detail",
        match report.completeness {
            crate::ReportCompleteness::FullDetail => "Full detail",
            crate::ReportCompleteness::BoundedOverview => "Bounded overview",
        },
    );
    fact(
        out,
        "Adapter evidence",
        &format!(
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
    );
    fact(
        out,
        "Recorded acquisition",
        &if snapshot.adapter_evidence.evidence_complete {
            "No recorded acquisition gaps".into()
        } else if snapshot.adapter_evidence.gap_count == 1 {
            "1 recorded acquisition gap; inspect the durable adapter evidence and lifecycle history for its recorded reason".into()
        } else if snapshot.adapter_evidence.gap_count > 1 {
            format!(
                "{} recorded acquisition gaps; inspect the durable adapter evidence and lifecycle history for their recorded reasons",
                snapshot.adapter_evidence.gap_count,
            )
        } else {
            "Recorded acquisition is incomplete; inspect the durable adapter evidence and lifecycle history for the recorded reason".into()
        },
    );
    let wspr_live_imports = snapshot
        .adapter_evidence
        .imports
        .iter()
        .filter(|import| import.provider_id == "wspr-live")
        .count();
    if wspr_live_imports > 0 {
        fact(
            out,
            "Public collection",
            &if snapshot.adapter_evidence.evidence_complete {
                format!(
                    "Best-effort public collection completed for {} recorded requested window(s)",
                    wspr_live_imports,
                )
            } else {
                format!(
                    "Best-effort public collection retained rows for {} recorded requested window(s); recorded acquisition gaps remain",
                    wspr_live_imports,
                )
            },
        );
        fact(
            out,
            "Public-source boundary",
            "AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee.",
        );
    }
    for (index, import) in snapshot.adapter_evidence.imports.iter().enumerate() {
        let bands = import
            .selected_bands
            .iter()
            .map(|value| band(*value))
            .collect::<Vec<_>>()
            .join(", ");
        fact(
            out,
            &format!("Imported evidence {}", index + 1),
            &format!(
                "{} / {}; captured {}; half-open window {} to {}; bands {}; {} rows: {} accepted, {} malformed, {} unsupported, {} filtered, {} duplicate, {} conflict; {} observations created; {}",
                import.provider_id,
                import.source_id,
                timestamp(import.captured_at),
                timestamp(import.window_start),
                timestamp(import.window_end),
                bands,
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
        );
    }
    out.push_str("</dl>");
    if !snapshot.lifecycle_events.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Lifecycle and interruption history</caption><thead><tr><th scope=\"col\">Event</th><th scope=\"col\">Time</th><th scope=\"col\">Detail</th></tr></thead><tbody>");
        for event in &snapshot.lifecycle_events {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                lifecycle_event(event.kind),
                timestamp(event.occurred_at),
                escape_html(event.detail.as_deref().unwrap_or("Not recorded")),
            );
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.operator_events.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Complete operator note and correction history</caption><thead><tr><th scope=\"col\">Event</th><th scope=\"col\">Time</th><th scope=\"col\">Recorded slot</th><th scope=\"col\">Affected slot</th><th scope=\"col\">Kind</th><th scope=\"col\">Detail</th><th scope=\"col\">Correction</th></tr></thead><tbody>");
        for event in &snapshot.operator_events {
            let correction = event.correction.as_ref().map_or_else(
                || "None".to_string(),
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
            );
            write_html!(out, "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(&event.event_id), timestamp(event.occurred_at), escape_html(event.slot_id.as_deref().unwrap_or("Not recorded")), escape_html(event.affected_slot_id.as_deref().unwrap_or("Not recorded")), operator_event_kind(event.kind), escape_html(event.detail.as_deref().unwrap_or("Not recorded")), escape_html(&correction));
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.wspr_cycles.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Intended WSPR order and observed antenna use</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Band</th><th scope=\"col\">Direction</th><th scope=\"col\">Intended antenna</th><th scope=\"col\">Observed antenna</th><th scope=\"col\">Readiness basis</th><th scope=\"col\">Ready</th><th scope=\"col\">Period start</th><th scope=\"col\">Period end</th><th scope=\"col\">Attribution</th></tr></thead><tbody>");
        for cycle in &snapshot.wspr_cycles {
            let direction = match cycle.direction {
                Some(WsprCycleDirection::Receive) => "Receive",
                Some(WsprCycleDirection::Transmit) => "Transmit",
                None => "Not recorded",
            };
            let attribution = match cycle.attribution {
                crate::ReportWsprAttribution::Pending => "Not yet run",
                crate::ReportWsprAttribution::Skipped => "Skipped by operator",
                crate::ReportWsprAttribution::Attributable => "Full antenna occupancy recorded",
                crate::ReportWsprAttribution::UnknownAntennaOccupancy => {
                    "Unknown — antenna changed during transmission"
                }
            };
            let readiness = match cycle.readiness_basis {
                Some(crate::ReportWsprReadinessBasis::OperatorConfirmed) => "Operator confirmed",
                Some(crate::ReportWsprReadinessBasis::CommandVerified) => "Command verified",
                None => "Not recorded",
            };
            write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", cycle.sequence_number, band(cycle.band), direction, escape_html(&cycle.planned_antenna), escape_html(cycle.actual_antenna.as_deref().unwrap_or("Not recorded")), readiness, cycle.ready_at.map_or_else(|| "—".into(), timestamp), cycle.starts_at.map_or_else(|| "—".into(), timestamp), cycle.transmission_ends_at.map_or_else(|| "—".into(), timestamp), attribution);
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.antenna_control_attempts.is_empty() {
        if controller_evidence == ControllerEvidenceHandling::OmittedAtExport {
            render_omitted_antenna_control_attempts(out, report);
        } else {
            render_complete_antenna_control_attempts(out, report);
        }
    }
    out.push_str("</section>");
}

fn render_complete_antenna_control_attempts(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    let snapshot = &report.snapshot;
    out.push_str("<div class=\"table-wrap\"><table><caption>Antenna-control command attempts</caption><thead><tr><th scope=\"col\">Record</th><th scope=\"col\">Role</th><th scope=\"col\">Intent / target / mode</th><th scope=\"col\">Controller</th><th scope=\"col\">Resolved invocation</th><th scope=\"col\">Outcome</th><th scope=\"col\">Stdout</th><th scope=\"col\">Stderr</th></tr></thead><tbody>");
    for attempt in &snapshot.antenna_control_attempts {
        let arguments = attempt
            .resolved_arguments
            .iter()
            .enumerate()
            .map(|(index, value)| format!("[{index}]={value:?}"))
            .collect::<Vec<_>>()
            .join(" ");
        let outcome = format!(
            "{:?}; {} ms",
            attempt.disposition, attempt.elapsed_milliseconds
        );
        let stdout = format!(
            "{:?}; truncated={}; {}",
            attempt.stdout.encoding, attempt.stdout.truncated, attempt.stdout.data
        );
        let stderr = format!(
            "{:?}; truncated={}; {}",
            attempt.stderr.encoding, attempt.stderr.truncated, attempt.stderr.data
        );
        write_html!(out, "<tr><td><code>{}</code></td><td>{:?}</td><td><code>{}</code><br>{} → {}<br>{}</td><td>{} / {}</td><td><code>{}</code><br>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(&attempt.record_id), attempt.role, escape_html(&attempt.intent_id), escape_html(&attempt.antenna), escape_html(&attempt.target), experiment_mode(attempt.mode), escape_html(&attempt.controller_profile_name), escape_html(&attempt.controller_profile_revision), escape_html(&attempt.resolved_program), escape_html(&arguments), escape_html(&outcome), escape_html(&stdout), escape_html(&stderr));
    }
    out.push_str("</tbody></table></div>");
}

fn render_omitted_antenna_control_attempts(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    const OMITTED: &str = "Omitted at export — retained in the session bundle";
    out.push_str("<div class=\"table-wrap\"><table><caption>Antenna-control command attempts with command details omitted at export</caption><thead><tr><th scope=\"col\">Record</th><th scope=\"col\">Role</th><th scope=\"col\">Intent / target / mode</th><th scope=\"col\">Controller</th><th scope=\"col\">Resolved invocation</th><th scope=\"col\">Timing</th><th scope=\"col\">Outcome</th><th scope=\"col\">Stdout</th><th scope=\"col\">Stderr</th></tr></thead><tbody>");
    for attempt in &report.snapshot.antenna_control_attempts {
        let outcome = match &attempt.disposition {
            AntennaControlDispositionV5::Exit { code } => format!("Exit code {code}"),
            AntennaControlDispositionV5::SpawnError { .. } => "Spawn error".into(),
            AntennaControlDispositionV5::Signaled { signal } => signal.map_or_else(
                || "Terminated by signal; signal number not recorded".into(),
                |signal| format!("Terminated by signal {signal}"),
            ),
            AntennaControlDispositionV5::Timeout => "Timed out".into(),
        };
        let timing = format!(
            "Started {}; completed {}; {} ms elapsed",
            timestamp(attempt.started_at),
            timestamp(attempt.completed_at),
            attempt.elapsed_milliseconds,
        );
        write_html!(out, "<tr><td><code>{}</code></td><td>{:?}</td><td><code>{}</code><br>{} → {}<br>{}</td><td>{} / {}</td><td>Program: {}<br>Arguments: {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(&attempt.record_id), attempt.role, escape_html(&attempt.intent_id), escape_html(&attempt.antenna), OMITTED, experiment_mode(attempt.mode), escape_html(&attempt.controller_profile_name), escape_html(&attempt.controller_profile_revision), OMITTED, OMITTED, escape_html(&timing), escape_html(&outcome), OMITTED, OMITTED);
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn snapshot_has_detail(report: &SessionReport) -> bool {
    let snapshot = &report.snapshot;
    snapshot.checkpoint_revision.is_some()
        || snapshot.lifecycle.is_some()
        || !snapshot.lifecycle_events.is_empty()
        || !snapshot.operator_events.is_empty()
        || !snapshot.wspr_cycles.is_empty()
        || !snapshot.antenna_control_attempts.is_empty()
        || snapshot.adapter_evidence.record_count > 0
        || snapshot.adapter_evidence.gap_count > 0
        || !snapshot.adapter_evidence.evidence_complete
        || !snapshot.adapter_evidence.imports.is_empty()
}
pub(in super::super) fn lifecycle(value: SessionLifecycleV2) -> &'static str {
    match value {
        SessionLifecycleV2::Draft => "Draft",
        SessionLifecycleV2::Ready => "Ready",
        SessionLifecycleV2::Running => "Running / in progress",
        SessionLifecycleV2::Interrupted => "Interrupted / in progress",
        SessionLifecycleV2::Ended => "Ended / final",
        SessionLifecycleV2::Abandoned => "Abandoned / final",
    }
}
pub(in super::super) fn lifecycle_event(value: ReportLifecycleEventKind) -> &'static str {
    match value {
        ReportLifecycleEventKind::Started => "Started",
        ReportLifecycleEventKind::Interrupted => "Interrupted",
        ReportLifecycleEventKind::InterruptionDetected => "Interruption detected",
        ReportLifecycleEventKind::Resumed => "Resumed",
        ReportLifecycleEventKind::Ended => "Ended",
        ReportLifecycleEventKind::Abandoned => "Abandoned",
    }
}
pub(in super::super) fn render_context(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let context = &report.context;
    out.push_str("<section class=\"panel\" aria-labelledby=\"context-title\"><h2 id=\"context-title\">Session context</h2><dl class=\"facts\">");
    fact(out, "Callsign", &context.station.callsign);
    fact(out, "Grid", &context.station.grid);
    fact(
        out,
        "Power",
        &context
            .station
            .power_watts
            .map(|value| format!("{} W", format_number(f64::from(value))))
            .unwrap_or_else(not_recorded),
    );
    fact(
        out,
        "Experiment mode",
        experiment_mode(context.experiment_mode),
    );
    fact(out, "Goal", session_goal(context.goal));
    fact(
        out,
        "Scheduled range",
        &context
            .scheduled_time_range
            .as_ref()
            .map(|range| {
                format!(
                    "{} – {}",
                    timestamp(range.starts_at),
                    timestamp(range.ends_at)
                )
            })
            .unwrap_or_else(|| "No scheduled time range".to_string()),
    );
    fact(
        out,
        "Scheduled bands",
        &if context.bands.is_empty() {
            "None".to_string()
        } else {
            context
                .bands
                .iter()
                .map(|value| band(*value))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    fact(
        out,
        "Scheduled slots",
        &context.schedule.slot_count.to_string(),
    );
    out.push_str("</dl><h3>Antennas</h3>");
    if context.antennas.is_empty() {
        out.push_str("<p class=\"empty\">No antennas are present in this report.</p>");
    } else {
        out.push_str("<div class=\"antenna-grid\">");
        for antenna in &context.antennas {
            write_html!(
                out,
                "<article class=\"antenna-card\"><h3>{}</h3><dl>",
                escape_html(&antenna.label)
            );
            detail(out, "Facets", &optional_join(&antenna.facets));
            detail(out, "Height", &optional_measure(antenna.height_m, "m"));
            detail(
                out,
                "Radials",
                &antenna
                    .radial_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(not_recorded),
            );
            detail(
                out,
                "Radial length",
                &optional_measure(antenna.radial_length_m, "m"),
            );
            detail(
                out,
                "Orientation",
                &optional_measure(antenna.orientation_degrees, "°"),
            );
            detail(
                out,
                "Tuner",
                antenna.tuner.as_deref().unwrap_or("Not recorded"),
            );
            detail(
                out,
                "Feedline",
                antenna.feedline.as_deref().unwrap_or("Not recorded"),
            );
            detail(
                out,
                "Notes",
                antenna.notes.as_deref().unwrap_or("Not recorded"),
            );
            out.push_str("</dl></article>");
        }
        out.push_str("</div>");
    }
    render_schedule_table(out, report);
    out.push_str("</section>");
}
pub(in super::super) fn render_schedule_table(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Schedule overview</h3>");
    if report.context.schedule.slots.is_empty() {
        out.push_str("<p class=\"empty\">No scheduled slots are available.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Planned slots</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Band</th><th scope=\"col\">Antenna</th><th scope=\"col\">Starts</th><th scope=\"col\">Ends</th><th scope=\"col\">Guard</th></tr></thead><tbody>");
    for slot in &report.context.schedule.slots {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} s</td></tr>", slot.sequence_number, escape_html(&slot.slot_id), band(slot.band), escape_html(&slot.planned_label), timestamp(slot.starts_at), timestamp(slot.ends_at), slot.guard_seconds);
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_overall(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"evidence-title\"><h2 id=\"evidence-title\">Evidence overview</h2>");
    write_html!(
        out,
        "<p>Evidence coverage: <span class=\"badge\">{}</span></p>\
<p class=\"muted\">Coverage reflects usable observations and contributing slots; it is not evidence that one antenna is better.</p>",
        evidence_coverage(report.evidence.evidence_quality)
    );
    evidence_summary(out, &report.evidence.overall);
    out.push_str("</section>");
}
