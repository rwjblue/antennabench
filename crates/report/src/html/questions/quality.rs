use super::*;

pub(in super::super) fn render_run_quality_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section id=\"run-quality\" class=\"question-section\" aria-labelledby=\"run-quality-title\"><div class=\"panel\"><h2 id=\"run-quality-title\">Run quality and answerability</h2><p class=\"muted\">Availability is reported from typed evidence states and raw counts. It is not a run-quality score or a strength grade.</p>");
    render_answerability(out, report);
    out.push_str("</div><div class=\"panel\"><h2>Planned versus actual</h2><p class=\"muted\">Symbols, words, and border styles distinguish state without relying on color. Open a row for exact times, readiness, attribution, counts, notes, and corrections.</p>");
    render_lifecycle_strip(out, report);
    render_run_timeline(out, report);
    out.push_str("</div>");
    render_acquisition_summary(out, report);
    render_exclusion_summary(out, report);
    render_notices(out, &report.notices);
    render_eligibility(out, report);
    out.push_str("<div class=\"panel\"><details class=\"audit-disclosure\"><summary>Review overall, antenna, and band evidence accounting</summary><div class=\"disclosure-body\">");
    render_overall(out, report);
    render_antenna_section(out, report);
    render_band_section(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review per-slot evidence accounting</summary><div class=\"disclosure-body\">");
    render_slot_section(out, report);
    out.push_str("</div></details></div></section>");
}
pub(in super::super) fn render_answerability(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.overview.strata.is_empty() {
        write_html!(
            out,
            "<p class=\"empty\"><strong>{}.</strong> {}</p>",
            comparison_availability_label(report.overview.comparison_availability),
            comparison_availability_text(report.overview.comparison_availability)
        );
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table class=\"answerability-table\"><caption>Answerability by separate comparison stratum</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Availability</th><th scope=\"col\">Unique paired paths</th><th scope=\"col\">Paired rows</th><th scope=\"col\">Blocks</th><th scope=\"col\">A→B / B→A</th><th scope=\"col\">Unmatched L / R</th><th scope=\"col\">Missing SNR L / R</th><th scope=\"col\">Excluded</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>");
    for row in &report.overview.strata {
        let availability = match row.availability {
            ReportStratumAvailability::DescriptivePairsAvailable => {
                "Answerable — descriptive pairs available"
            }
            ReportStratumAvailability::NoFinitePairedPaths => {
                "Unavailable — no finite same-path pair"
            }
        };
        write_html!(out, "<tr><td data-label=\"Stratum\">{}</td><td data-label=\"Availability\">{}</td><td data-label=\"Unique paired paths\">{}</td><td data-label=\"Paired rows\">{}</td><td data-label=\"Blocks\">{}</td><td data-label=\"A→B / B→A\">{} / {}</td><td data-label=\"Unmatched L / R\">{} / {}</td><td data-label=\"Missing SNR L / R\">{} / {}</td><td data-label=\"Excluded\">{}</td><td data-label=\"Duplicates\">{}</td><td data-label=\"Conflicts\">{}</td></tr>", comparison_stratum(&row.stratum), availability, row.unique_path_count, row.paired_row_count, row.contributing_block_count, row.left_then_right_block_count, row.right_then_left_block_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.excluded_observation_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
    out.push_str("</tbody></table></div><p class=\"muted\">Unmatched paths, missing values, exclusions, duplicates, and conflicts remain separate facts.</p>");
}
pub(in super::super) fn render_lifecycle_strip(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.snapshot.lifecycle_events.is_empty() {
        return;
    }
    out.push_str("<div class=\"lifecycle-strip\" aria-label=\"Lifecycle sequence\">");
    for event in &report.snapshot.lifecycle_events {
        let symbol = match event.kind {
            ReportLifecycleEventKind::Interrupted
            | ReportLifecycleEventKind::InterruptionDetected => "!",
            ReportLifecycleEventKind::Resumed => "↻",
            ReportLifecycleEventKind::Abandoned => "×",
            ReportLifecycleEventKind::Ended => "■",
            ReportLifecycleEventKind::Started => "▶",
        };
        write_html!(
            out,
            "<span class=\"lifecycle-chip\"><strong aria-hidden=\"true\">{}</strong>{} · {}</span>",
            symbol,
            lifecycle_event(event.kind),
            timestamp(event.occurred_at)
        );
    }
    out.push_str("</div>");
}
pub(in super::super) fn render_run_timeline(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.overview.timeline.is_empty() {
        out.push_str("<p class=\"empty\">No valid planned cycle or slot rows are available for the compact timeline.</p>");
        return;
    }
    out.push_str("<div class=\"run-timeline\">");
    for row in &report.overview.timeline {
        let (class, state, symbol) = timeline_compact_state(row, report);
        write_html!(out, "<details class=\"state-{}\"><summary><span class=\"timeline-state\" aria-hidden=\"true\">{}</span>#{} {} → {}<br><small>{} · {} usable / {} excluded</small></summary><div class=\"timeline-detail\"><dl class=\"facts\">", class, symbol, row.sequence_number, escape_html(&row.planned_antenna), escape_html(row.actual_antenna.as_deref().unwrap_or("Not recorded")), state, row.usable_observation_count, row.excluded_observation_count);
        fact(out, "Item", &row.item_id);
        fact(out, "State", state);
        fact(
            out,
            "Band / direction",
            &format!(
                "{} / {}",
                band(row.band),
                row.direction.map(wspr_direction).unwrap_or("Not recorded")
            ),
        );
        fact(
            out,
            "Block",
            &row.block_index.map_or_else(
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
            ),
        );
        fact(
            out,
            "Planned window",
            &format!(
                "{} – {}",
                timestamp(row.planned_starts_at),
                timestamp(row.planned_ends_at)
            ),
        );
        fact(
            out,
            "Actual window",
            &match (row.actual_starts_at, row.actual_ends_at) {
                (Some(start), Some(end)) => format!("{} – {}", timestamp(start), timestamp(end)),
                (Some(start), None) => format!("{} – end not recorded", timestamp(start)),
                _ => "Not recorded".into(),
            },
        );
        fact(
            out,
            "Readiness",
            row.readiness_basis
                .map(wspr_readiness)
                .unwrap_or("Not recorded"),
        );
        fact(
            out,
            "Attribution",
            row.attribution
                .map(wspr_attribution)
                .unwrap_or("Not recorded"),
        );
        fact(
            out,
            "Evidence",
            &format!(
                "{} total; {} usable; {} excluded",
                row.total_observation_count,
                row.usable_observation_count,
                row.excluded_observation_count
            ),
        );
        out.push_str("</dl>");
        render_timeline_events(out, &row.event_history);
        out.push_str("</div></details>");
    }
    out.push_str("</div>");
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
pub(in super::super) fn render_timeline_events(
    out: &mut CheckedHtmlWriter<'_>,
    events: &[ReportOperatorEvent],
) {
    if events.is_empty() {
        out.push_str(
            "<p class=\"muted\">No slot-specific operator note or correction is recorded.</p>",
        );
        return;
    }
    out.push_str("<ul class=\"audit-event-list\">");
    for event in events {
        write_html!(
            out,
            "<li><code>{}</code> · {} · {}",
            escape_html(&event.event_id),
            timestamp(event.occurred_at),
            operator_event_kind(event.kind)
        );
        if let Some(detail) = &event.detail {
            write_html!(out, " · {}", escape_html(detail));
        }
        if let Some(correction) = &event.correction {
            write_html!(
                out,
                " · {} <code>{}</code>: {} ({})",
                correction_action(correction.action),
                escape_html(&correction.target_event_id),
                escape_html(&correction.reason),
                if correction.applied {
                    "applied"
                } else {
                    "not applied"
                }
            );
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
}
pub(in super::super) fn render_acquisition_summary(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    let evidence = &report.snapshot.adapter_evidence;
    out.push_str("<section class=\"panel\" aria-labelledby=\"acquisition-quality-title\"><h2 id=\"acquisition-quality-title\">Acquisition status</h2>");
    if evidence
        .imports
        .iter()
        .any(|import| import.provider_id == "wspr-live")
        && evidence.evidence_complete
    {
        out.push_str("<p><strong>Best-effort public collection completed for the requested windows.</strong> AntennaBench retained the spots returned by the configured WSPR.live queries. The upstream mirror does not provide an independent completeness guarantee.</p>");
    } else if evidence.gap_count > 0 {
        write_html!(out, "<p class=\"notice critical\"><strong>Explicit acquisition gap:</strong> {} recorded gap{}; retained unrelated valid evidence remains usable. Inspect provider windows and durable adapter records in the audit appendix.</p>", evidence.gap_count, plural_suffix(evidence.gap_count));
    } else if !evidence.evidence_complete {
        out.push_str("<p class=\"notice critical\"><strong>Recorded acquisition is incomplete.</strong> No durable gap count is available; inspect provider windows and lifecycle records for the recorded reason.</p>");
    } else if evidence.record_count > 0 {
        out.push_str("<p>No explicit acquisition gap is recorded. Adapter row dispositions remain available in the audit appendix.</p>");
    } else {
        out.push_str("<p class=\"empty\">No adapter or import acquisition record is available; no completeness claim is inferred.</p>");
    }
    out.push_str("</section>");
}
pub(in super::super) fn render_exclusion_summary(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"exclusion-summary-title\"><h2 id=\"exclusion-summary-title\">Exclusion summary</h2><p class=\"notice\">Affected evidence is excluded only from calculations that require it. Unrelated valid evidence remains usable.</p>");
    if report.evidence.overall.exclusions.is_empty() {
        out.push_str("<p class=\"empty\">No observation exclusions are recorded.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Observation exclusions by existing reason</caption><thead><tr><th scope=\"col\">Reason</th><th scope=\"col\">Count</th></tr></thead><tbody>");
        for exclusion in &report.evidence.overall.exclusions {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td></tr>",
                exclusion_reason(exclusion.reason),
                exclusion.count
            );
        }
        out.push_str("</tbody></table></div>");
    }
    if !report.exclusion_records.is_empty() {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review exact excluded observation records</summary><div class=\"disclosure-body\">");
        render_exclusion_records(out, report);
        out.push_str("</div></details>");
    }
    out.push_str("</section>");
}
