use std::fmt::Write as _;

use crate::{
    check_cancelled, ReportAcquisitionWorkflowStatus, ReportCancellationToken, ReportCompleteness,
    ReportError, ReportProviderCompleteness, ReportQuestionFamily, ReportResourceLimits,
    ReportResourceStage, SessionReport, REPORT_RESOURCE_LIMITS,
};

use super::{
    geometry::render_geometry_styles,
    questions::{
        is_single_antenna_lens, ordered_question_families, overview_lifecycle_label,
        render_answer_first_overview_with_reference, render_compact_coverage_map_section,
        render_compact_distance_section, render_compact_overlap_repeatability_section,
        render_how_to_read, render_question_navigation, render_reach_bar,
        render_reporter_activity_section, render_same_path_stratum,
    },
    shared::*,
    styles::{COMPACT_SMALL_PRINT_STYLES, COMPACT_STYLES, COVERAGE_STYLES, STYLES},
};

/// Renders a concise, deterministic, standalone HTML summary from the same
/// renderer-neutral report revision as the full evidence report. It intentionally
/// omits the audit appendix and never recomputes or reinterprets report facts.
pub fn render_compact_summary_html(report: &SessionReport) -> Result<String, ReportError> {
    render_compact_summary_html_with_resources(
        report,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
    )
}

pub fn render_compact_summary_html_with_resources(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<String, ReportError> {
    check_cancelled(
        cancellation,
        ReportResourceStage::Render,
        "compact_summary_html",
    )?;
    let mut out = CheckedHtmlWriter::new(limits.html_bytes, cancellation);
    out.push_str(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\">\
<title>AntennaBench compact share summary</title><style>",
    );
    out.push_str(STYLES);
    render_geometry_styles(&mut out);
    out.push_str(COVERAGE_STYLES);
    out.push_str(COMPACT_STYLES);
    out.push_str(COMPACT_SMALL_PRINT_STYLES);
    let compact_main_class = if report.overview.strata.len() <= 2
        && report
            .overview
            .strata
            .iter()
            .map(|row| row.path_median_deltas.len())
            .sum::<usize>()
            <= 4
    {
        "compact-summary compact-small"
    } else {
        "compact-summary"
    };
    write_html!(
        out,
        "</style></head><body><main class=\"{}\"><a class=\"skip-link\" href=\"#what-run-show\">Skip to summary findings</a>",
        compact_main_class,
    );
    write_html!(
        out,
        "<header class=\"hero\"><p class=\"eyebrow\">AntennaBench compact local share summary</p>\
<h1>Compact session summary</h1><p class=\"muted\">Not the full audit report · Session <code>{}</code></p></header>",
        escape_html(&report.overview.scope.session_id)
    );
    render_question_navigation(&mut out, report, false);
    render_how_to_read(&mut out, report);
    render_answer_first_overview_with_reference(
        &mut out,
        report,
        "the full evidence report and session bundle",
    );
    for family in ordered_question_families(report) {
        match family {
            ReportQuestionFamily::SharedPathSignal => {
                out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Shared-path signal</h2>");
                render_compact_same_path_view(&mut out, report);
                out.push_str("</section>");
            }
            ReportQuestionFamily::CommonOpportunityDetection => {
                render_reporter_activity_section(&mut out, report);
                render_compact_coverage_map_section(&mut out, report);
            }
            ReportQuestionFamily::ObservedReach => {
                out.push_str("<section id=\"reach-unique-paths\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Observed reach</h2>");
                render_compact_reach_view(&mut out, report);
                out.push_str("</section>");
            }
            ReportQuestionFamily::GeographicProfile => {
                render_compact_distance_section(&mut out, report)
            }
            ReportQuestionFamily::Repeatability => {
                render_compact_overlap_repeatability_section(&mut out, report)
            }
        }
    }
    render_compact_run_quality(&mut out, report);
    render_compact_reference(&mut out, report);
    if is_single_antenna_lens(report) {
        out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This compact profiling summary describes only recorded evidence from the named antenna.</p></main></body></html>");
    } else {
        out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This compact summary is descriptive and does not select an antenna winner.</p></main></body></html>");
    }
    out.finish().map_err(ReportError::from)
}

pub(super) fn render_compact_run_quality(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let overall = &report.evidence.overall;
    let availability = comparison_availability_label(report.overview.comparison_availability);
    let lifecycle = overview_lifecycle_label(report.overview.lifecycle.state);
    let evidence = &report.snapshot.adapter_evidence;
    let acquisition = if evidence.gap_count > 0 {
        format!("{} recorded acquisition gap(s)", evidence.gap_count)
    } else {
        match (evidence.workflow_status, evidence.provider_completeness) {
            (ReportAcquisitionWorkflowStatus::Incomplete, _) => {
                "Recorded acquisition incomplete".to_string()
            }
            (ReportAcquisitionWorkflowStatus::NotConfigured, _) => {
                "No configured acquisition".to_string()
            }
            (ReportAcquisitionWorkflowStatus::Completed, ReportProviderCompleteness::Known) => {
                "Collection completed; provider completeness is recorded as known".to_string()
            }
            (ReportAcquisitionWorkflowStatus::Completed, ReportProviderCompleteness::Unknown) => {
                "Collection completed; upstream completeness is not independently guaranteed"
                    .to_string()
            }
            (
                ReportAcquisitionWorkflowStatus::Completed,
                ReportProviderCompleteness::Unsupported,
            ) => "Collection completed; provider completeness is unsupported".to_string(),
        }
    };
    out.push_str("<section id=\"run-quality\" class=\"panel question-section\" tabindex=\"-1\" aria-labelledby=\"run-quality-title\"><h2 id=\"run-quality-title\">Run quality and answerability</h2><p class=\"muted\">These are typed availability and count facts.</p><div class=\"run-summary\">");
    write_html!(out, "<div><span>Comparison state</span><strong>{availability}</strong></div><div><span>Session state</span><strong>{lifecycle}</strong></div><div><span>Usable / excluded</span><strong>{} / {}</strong></div><div><span>Acquisition</span><strong>{}</strong></div>", overall.observation_counts.usable, overall.observation_counts.excluded, escape_html(&acquisition));
    out.push_str("</div>");
    if report.completeness == ReportCompleteness::BoundedOverview {
        out.push_str("<p class=\"notice\"><strong>Bounded overview:</strong> detailed report families were intentionally omitted by the resource policy; no rows were sampled. Use the full evidence report and lossless session bundle for available audit detail.</p>");
    }
    out.push_str("<p class=\"muted compact-omission\">This compact summary intentionally omits unmatched-path, missing-SNR, exclusion, duplicate, conflict, timeline, lifecycle-history, controller-output, import, solar, and raw-observation audit rows. The full evidence report and lossless session bundle retain that complete audit evidence; no rows are sampled here.</p></section>");
}

pub(super) fn render_compact_same_path_view(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison group has same-path evidence available. This is not a 0 dB result.</p>");
        return;
    }
    let orientation = report.overview.scope.delta_orientation.as_ref();
    let available = report
        .overview
        .strata
        .iter()
        .filter(|row| !row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|row| row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    if available.is_empty() {
        write_html!(out, "<p class=\"empty\">No usable same-path path-median delta is available across {} ({}). This is not a 0 dB result; availability remains explicit in the result table.</p>", comparison_groups_label(unavailable.len()), comparison_strata_list(&unavailable));
        return;
    }
    if let Some(orientation) = orientation {
        write_html!(out, "<p class=\"orientation\"><strong>Signed values:</strong> Positive values mean {} was stronger; negative values mean {} was stronger. The vertical reference is zero.</p>", escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label));
    }
    out.push_str("<p class=\"path-view-note\">Each blue dot is one unique remote path’s median across its matched pairs; the purple diamond is the median across those path medians.</p>");
    for row in available {
        render_same_path_stratum(out, row, orientation);
    }
    if !unavailable.is_empty() {
        write_html!(out, "<p class=\"empty collapsed-empty-strata\">No usable same-path path-median delta in {} of {} comparison groups: {}. Availability remains explicit in the result table.</p>", unavailable.len(), report.overview.strata.len(), comparison_strata_list(&unavailable));
    }
}

pub(super) fn render_compact_reach_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison groups are available for reach counts.</p>");
        return;
    }
    let any_reach = report.overview.strata.iter().any(|row| {
        row.reach.left_only_unique_path_count > 0
            || row.reach.both_unique_path_count > 0
            || row.reach.right_only_unique_path_count > 0
    });
    if !any_reach {
        let strata = report.overview.strata.iter().collect::<Vec<_>>();
        write_html!(out, "<p class=\"empty\">No unique observed paths are available for reach overlap across {} ({}).</p>", comparison_groups_label(strata.len()), comparison_strata_list(&strata));
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "<p class=\"muted reach-note\" aria-hidden=\"true\">Overlap bars — <span class=\"swatch left\"></span>{} only · <span class=\"swatch both\"></span>heard by both · <span class=\"swatch right\"></span>{} only; segment widths are proportional to counts.</p>", left_label, right_label);
    write_html!(out, "<div class=\"table-wrap\"><table><caption>Unique observed-path overlap by separate comparison group</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">{} only</th><th scope=\"col\">Heard by both</th><th scope=\"col\">{} only</th><th scope=\"col\">Overlap</th></tr></thead><tbody>", left_label, right_label);
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                == 0
        })
        .collect::<Vec<_>>();
    for row in report.overview.strata.iter().filter(|row| {
        row.reach.left_only_unique_path_count
            + row.reach.both_unique_path_count
            + row.reach.right_only_unique_path_count
            > 0
    }) {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>",
            comparison_stratum(&row.stratum),
            row.reach.left_only_unique_path_count,
            row.reach.both_unique_path_count,
            row.reach.right_only_unique_path_count
        );
        render_reach_bar(out, &row.reach, "reach-mini");
        out.push_str("</td></tr>");
    }
    if !unavailable.is_empty() {
        write_html!(out, "<tr class=\"collapsed-empty-strata\"><td colspan=\"5\">No usable reach evidence in {}: {}.</td></tr>", comparison_groups_label(unavailable.len()), comparison_strata_list(&unavailable));
    }
    out.push_str("</tbody></table></div><p class=\"muted\">These are unique observed paths, not a coverage score or a claim about unobserved paths.</p>");
}

pub(super) fn render_compact_reference(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let revision = report
        .snapshot
        .checkpoint_revision
        .or(report.overview.lifecycle.checkpoint_revision)
        .map_or_else(not_recorded, |value| value.to_string());
    write_html!(out, "<section class=\"panel compact-reference\" aria-labelledby=\"source-reference-title\"><h2 id=\"source-reference-title\">Source reference</h2><p>Full evidence report and lossless session bundle: session <code>{}</code>, committed revision <strong>{}</strong>.</p><p class=\"muted\">Use those authoritative local outputs for the complete audit appendix and durable session evidence.</p></section>", escape_html(&report.overview.scope.session_id), escape_html(&revision));
}
