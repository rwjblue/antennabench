use super::*;

pub(in super::super) fn render_question_navigation(out: &mut CheckedHtmlWriter<'_>) {
    out.push_str(
        "<nav class=\"question-nav\" aria-label=\"Report questions\"><ul>\
<li><a href=\"#what-run-show\">What did the run show?</a></li>\
<li><a href=\"#same-path-signal\">Same-path signal</a></li>\
<li><a href=\"#reach-unique-paths\">Reach and unique paths</a></li>\
<li><a href=\"#distance-direction\">Distance and direction</a></li>\
<li><a href=\"#run-quality\">Run quality</a></li>\
<li><a href=\"#audit-appendix\">Audit appendix</a></li>\
</ul></nav>",
    );
}
pub(in super::super) fn render_answer_first_overview(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    render_answer_first_overview_with_reference(out, report, "the audit appendix");
}
pub(in super::super) fn render_answer_first_overview_with_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
) {
    let overview = &report.overview;
    let scope = &overview.scope;
    let antennas = if scope.antenna_labels.is_empty() {
        "None recorded".to_string()
    } else {
        scope.antenna_labels.join(" / ")
    };
    let bands = if scope.bands.is_empty() {
        "None recorded".to_string()
    } else {
        scope
            .bands
            .iter()
            .map(|value| band(*value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let directions = if scope.observed_directions.is_empty() {
        "No comparison direction observed".to_string()
    } else {
        scope
            .observed_directions
            .iter()
            .map(|value| path_direction(*value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mode = scope
        .experiment_mode
        .map(experiment_mode)
        .unwrap_or("Not recorded");
    let lifecycle_label = overview_lifecycle_label(overview.lifecycle.state);
    let paired_rows = overview
        .strata
        .iter()
        .map(|row| row.paired_row_count)
        .sum::<usize>();

    out.push_str("<section id=\"what-run-show\" class=\"panel overview\" aria-labelledby=\"what-run-show-title\"><p class=\"eyebrow\">Answer first</p><h2 id=\"what-run-show-title\">What did the run show?</h2>");
    write_html!(
        out,
        "<p class=\"scope-line\">Station <strong>{}</strong> at <strong>{}</strong>; goal: <strong>{}</strong>.</p>",
        escape_html(&scope.station.callsign),
        escape_html(&scope.station.grid),
        scope.goal.map(session_goal).unwrap_or("Not recorded"),
    );
    out.push_str("<dl class=\"facts headline-facts\">");
    fact(out, "Antennas", &antennas);
    fact(out, "Bands", &bands);
    fact(out, "Direction / mode", &format!("{directions}; {mode}"));
    fact(out, "Session state", lifecycle_label);
    out.push_str("</dl>");

    match &scope.delta_orientation {
        Some(orientation) => write_html!(
            out,
            "<p class=\"orientation\"><strong>Delta orientation:</strong> {} minus {} (right minus left). Every signed value below uses this fixed orientation.</p>",
            escape_html(&orientation.minuend_label),
            escape_html(&orientation.subtrahend_label),
        ),
        None => out.push_str("<p class=\"orientation\"><strong>Delta orientation:</strong> unavailable because this run does not provide a two-label paired orientation.</p>"),
    }
    write_html!(
        out,
        "<p class=\"answer\"><strong>Comparison availability: <span class=\"badge\">{}</span>.</strong> {}</p>",
        comparison_availability_label(overview.comparison_availability),
        comparison_availability_text(overview.comparison_availability),
    );

    out.push_str("<div class=\"table-wrap\"><table class=\"overview-table\"><caption>Descriptive result by comparison stratum</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Path delta</th><th scope=\"col\">Paths / rows</th><th scope=\"col\">Blocks</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    if overview.strata.is_empty() {
        out.push_str("<tr><td data-label=\"Stratum\" colspan=\"5\">No comparison strata are available for this run.</td></tr>");
    } else {
        for row in overview
            .strata
            .iter()
            .filter(|row| matches!(row.path_delta, ReportOverviewPathDelta::Available { .. }))
        {
            let (delta, coverage) = match row.path_delta {
                ReportOverviewPathDelta::Unavailable => {
                    ("Not available".to_string(), "Unavailable".to_string())
                }
                ReportOverviewPathDelta::Available {
                    minimum_delta_right_minus_left_db,
                    median_path_delta_right_minus_left_db,
                    maximum_delta_right_minus_left_db,
                } => (
                    format!(
                        "{} to {} dB; median across paths {} dB",
                        format_signed(minimum_delta_right_minus_left_db),
                        format_signed(maximum_delta_right_minus_left_db),
                        format_signed(median_path_delta_right_minus_left_db),
                    ),
                    "Available".to_string(),
                ),
            };
            write_html!(
                out,
                "<tr><td data-label=\"Stratum\">{}</td><td data-label=\"Path delta\">{}</td><td data-label=\"Paths / rows\">{} / {}</td><td data-label=\"Blocks\">{}</td><td data-label=\"Coverage\">{}</td></tr>",
                comparison_stratum(&row.stratum),
                delta,
                row.unique_path_count,
                row.paired_row_count,
                row.contributing_block_count,
                coverage,
            );
        }
        let unavailable = overview
            .strata
            .iter()
            .filter(|row| row.path_delta == ReportOverviewPathDelta::Unavailable)
            .collect::<Vec<_>>();
        if !unavailable.is_empty() {
            write_html!(out, "<tr class=\"collapsed-empty-strata\"><td data-label=\"Stratum\">No path delta in {}: {}</td><td data-label=\"Path delta\">Not available</td><td data-label=\"Paths / rows\">Not pooled</td><td data-label=\"Blocks\">Not pooled</td><td data-label=\"Coverage\">Unavailable</td></tr>", comparison_strata_label(unavailable.len()), comparison_strata_list(&unavailable));
        }
    }
    out.push_str("</tbody></table></div><div class=\"overview-support\"><section aria-labelledby=\"supported-title\"><h3 id=\"supported-title\">Supported by this run</h3><ul>");
    write_html!(
        out,
        "<li>The recorded comparison state is <strong>{}</strong>.</li>",
        comparison_availability_label(overview.comparison_availability),
    );
    if overview.strata.is_empty() {
        out.push_str("<li>The session scope and availability state remain explicit even without an available comparison stratum.</li>");
    } else {
        write_html!(
            out,
            "<li>{} paired row(s) are retained in {} unpooled stratum row(s).</li>",
            paired_rows,
            overview.strata.len(),
        );
    }
    out.push_str("</ul></section><section aria-labelledby=\"not-established-title\"><h3 id=\"not-established-title\">Not established by this run</h3><ul><li>This descriptive report does not select a winner or establish antenna superiority.</li><li>Adjacent switched slots reduce elapsed time but do not remove propagation or time confounding.</li>");
    for limitation in &overview.limitations {
        write_html!(out, "<li>{}</li>", overview_limitation_text(*limitation));
    }
    out.push_str("</ul></section></div>");
    render_visible_acquisition_limitations(out, report, audit_reference);
    out.push_str("</section>");
}
pub(in super::super) fn overview_lifecycle_label(
    state: ReportOverviewLifecycleState,
) -> &'static str {
    match state {
        ReportOverviewLifecycleState::NotRecorded => "Not recorded",
        ReportOverviewLifecycleState::Recorded(value) => lifecycle(value),
    }
}
pub(in super::super) fn overview_limitation_text(value: ReportOverviewLimitation) -> String {
    match value {
        ReportOverviewLimitation::ComparisonNotApplicable => {
            "A/B comparison: not established for single-antenna profiling.".into()
        }
        ReportOverviewLimitation::UnsupportedComparisonShape => {
            "A/B comparison: unavailable without the required two-label shape.".into()
        }
        ReportOverviewLimitation::NoEligibleBlocks => {
            "Eligible blocks: none with one usable actual slot for each label.".into()
        }
        ReportOverviewLimitation::NoMatchedPaths => {
            "Matched paths: no same-stratum path had finite SNR under both labels.".into()
        }
        ReportOverviewLimitation::UnmatchedPaths {
            left_count,
            right_count,
        } => format!("Unmatched paths: {left_count} left / {right_count} right."),
        ReportOverviewLimitation::MissingSnr {
            left_count,
            right_count,
        } => format!("Missing SNR: {left_count} left / {right_count} right."),
        ReportOverviewLimitation::DuplicateEvidence {
            exact_count,
            conflicting_group_count,
        } => format!(
            "Duplicates: {exact_count} exact / {conflicting_group_count} conflicting group(s)."
        ),
    }
}
pub(in super::super) fn render_visible_acquisition_limitations(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
) {
    let evidence = &report.snapshot.adapter_evidence;
    if !evidence.evidence_complete {
        let message = if evidence.gap_count == 1 {
            format!("1 recorded acquisition gap; inspect {audit_reference} for its durable recorded context")
        } else if evidence.gap_count > 1 {
            format!(
                "{} recorded acquisition gaps; inspect {audit_reference} for their durable recorded context",
                evidence.gap_count,
            )
        } else {
            format!("Recorded acquisition is incomplete; inspect {audit_reference} for its durable recorded context")
        };
        write_html!(
            out,
            "<p class=\"notice critical\"><strong>Recorded acquisition:</strong> {}.</p>",
            message
        );
    }
    if evidence
        .imports
        .iter()
        .any(|import| import.provider_id == "wspr-live")
    {
        out.push_str("<p class=\"notice\"><strong>Public-source boundary:</strong> AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee.</p>");
    }
}
