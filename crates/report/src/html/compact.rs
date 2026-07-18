use std::fmt::Write as _;

use crate::{
    check_cancelled, ReportCancellationToken, ReportCompleteness, ReportError,
    ReportResourceLimits, ReportResourceStage, SessionReport, REPORT_RESOURCE_LIMITS,
};

use super::{
    questions::{
        overview_lifecycle_label, render_answer_first_overview_with_reference,
        render_same_path_stratum,
    },
    shared::*,
    styles::{COMPACT_SMALL_PRINT_STYLES, COMPACT_STYLES, STYLES},
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
    render_answer_first_overview_with_reference(
        &mut out,
        report,
        "the full evidence report and session bundle",
    );
    out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Same-path signal</h2>");
    render_compact_same_path_view(&mut out, report);
    out.push_str("</section><section id=\"reach-unique-paths\" class=\"panel question-section\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Reach and unique paths</h2>");
    render_compact_reach_view(&mut out, report);
    out.push_str("</section>");
    render_compact_run_quality(&mut out, report);
    render_compact_reference(&mut out, report);
    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This compact summary is descriptive and does not select an antenna winner.</p></main></body></html>");
    out.finish().map_err(ReportError::from)
}

pub(super) fn render_compact_run_quality(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let overall = &report.evidence.overall;
    let availability = comparison_availability_label(report.overview.comparison_availability);
    let lifecycle = overview_lifecycle_label(report.overview.lifecycle.state);
    let acquisition = if report.snapshot.adapter_evidence.evidence_complete {
        "Complete recorded acquisition".to_string()
    } else if report.snapshot.adapter_evidence.gap_count > 0 {
        format!(
            "{} recorded acquisition gap(s)",
            report.snapshot.adapter_evidence.gap_count
        )
    } else {
        "Recorded acquisition incomplete".to_string()
    };
    out.push_str("<section id=\"run-quality\" class=\"panel question-section\" aria-labelledby=\"run-quality-title\"><h2 id=\"run-quality-title\">Run quality and answerability</h2><p class=\"muted\">These are typed availability and count facts, not a strength grade or winner.</p><div class=\"run-summary\">");
    write_html!(out, "<div><span>Comparison state</span><strong>{availability}</strong></div><div><span>Session state</span><strong>{lifecycle}</strong></div><div><span>Usable / excluded</span><strong>{} / {}</strong></div><div><span>Acquisition</span><strong>{}</strong></div>", overall.observation_counts.usable, overall.observation_counts.excluded, escape_html(&acquisition));
    out.push_str("</div>");
    if report.completeness == ReportCompleteness::BoundedOverview {
        out.push_str("<p class=\"notice\"><strong>Bounded overview:</strong> detailed report families were intentionally omitted by the resource policy; no rows were sampled. Use the full evidence report and lossless session bundle for available audit detail.</p>");
    }
    out.push_str("<p class=\"muted\">The result table above keeps every stratum separate. It does not pool paths or turn unavailable same-path evidence into a zero result.</p>");
    out.push_str("<p class=\"muted compact-omission\">This compact summary intentionally omits unmatched-path, missing-SNR, exclusion, duplicate, conflict, timeline, lifecycle-history, controller-output, import, solar, and raw-observation audit rows. The full evidence report and lossless session bundle retain that complete audit evidence; no rows are sampled here.</p></section>");
}

pub(super) fn render_compact_same_path_view(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    let orientation = report.overview.scope.delta_orientation.as_ref();
    let available = report
        .overview
        .strata
        .iter()
        .filter(|row| !row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    if available.is_empty() {
        write_html!(out, "<p class=\"empty\">No finite same-path path-median delta is available across the {} reported stratum row(s). This is not a 0 dB result; availability remains explicit in the result table.</p>", report.overview.strata.len());
        return;
    }
    if let Some(orientation) = orientation {
        write_html!(out, "<p class=\"orientation\"><strong>Orientation:</strong> each value is <strong>{} − {}</strong> SNR in dB. Negative values are toward {}; positive values are toward {}. The vertical reference is zero.</p>", escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.minuend_label));
    }
    out.push_str("<p class=\"path-view-note\">Each blue dot is one unique remote path’s median across its paired rows; the purple diamond is the median across those path medians. Unavailable strata remain in the result table without repeated empty visuals.</p>");
    for row in available {
        render_same_path_stratum(out, row, orientation);
    }
}

pub(super) fn render_compact_reach_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison strata are available for reach counts.</p>");
        return;
    }
    let any_reach = report.overview.strata.iter().any(|row| {
        row.reach.left_only_unique_path_count > 0
            || row.reach.both_unique_path_count > 0
            || row.reach.right_only_unique_path_count > 0
    });
    if !any_reach {
        out.push_str("<p class=\"empty\">No unique observed paths are available for reach overlap. This is not a coverage score or evidence of equal reach.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Unique observed-path overlap by separate stratum</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Left only</th><th scope=\"col\">Both</th><th scope=\"col\">Right only</th></tr></thead><tbody>");
    for row in &report.overview.strata {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            comparison_stratum(&row.stratum),
            row.reach.left_only_unique_path_count,
            row.reach.both_unique_path_count,
            row.reach.right_only_unique_path_count
        );
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
