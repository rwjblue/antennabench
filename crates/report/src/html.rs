use std::fmt::Write as _;

use crate::{
    check_cancelled, GeographicProfileAnswerability, ObservedReachAnswerability,
    PairedDetectabilityAnswerability, ReportCancellationToken, ReportError, ReportResourceLimits,
    ReportResourceStage, SamePathSignalAnswerability, SessionReport, REPORT_RESOURCE_LIMITS,
};

macro_rules! write_html {
    ($output:expr, $($argument:tt)*) => {
        write!($output, $($argument)*).expect("checked HTML writer records failures")
    };
}

// Keep the public rendering surface here; section ownership remains renderer-private.
mod audit;
mod compact;
mod evidence;
mod geometry;
mod questions;
mod shared;
mod styles;

pub use compact::{render_compact_summary_html, render_compact_summary_html_with_resources};

use audit::render_audit_appendix;
use geometry::render_geometry_styles;
use questions::{
    render_answer_first_overview, render_coverage_map_section, render_distance_section,
    render_how_to_read, render_overlap_repeatability_section, render_question_navigation,
    render_reach_section, render_reporter_activity_section, render_run_quality_section,
    render_same_path_section,
};
use shared::{escape_html, CheckedHtmlWriter};
use styles::{COVERAGE_STYLES, STYLES};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerEvidenceHandling {
    #[default]
    Complete,
    OmittedAtExport,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StandaloneHtmlOptions {
    pub controller_evidence: ControllerEvidenceHandling,
}

/// Renders a deterministic, standalone HTML document from renderer-neutral
/// report data. The output contains no scripts, external resources, or
/// unescaped report strings.
pub fn render_standalone_html(report: &SessionReport) -> Result<String, ReportError> {
    render_standalone_html_with_options_and_resources(
        report,
        StandaloneHtmlOptions::default(),
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
    )
}

pub fn render_standalone_html_with_options(
    report: &SessionReport,
    options: StandaloneHtmlOptions,
) -> Result<String, ReportError> {
    render_standalone_html_with_options_and_resources(
        report,
        options,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
    )
}

/// Renders the full evidence report with a separately authorized, redacted
/// operational-history appendix. Callers own the allowlist and disclosure UI;
/// the renderer escapes the complete bounded summary as inert text.
pub fn render_standalone_html_with_operational_history(
    report: &SessionReport,
    controller_evidence: ControllerEvidenceHandling,
    redacted_support_summary: &str,
) -> Result<String, ReportError> {
    render_standalone_html_document(
        report,
        StandaloneHtmlOptions {
            controller_evidence,
        },
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        Some(redacted_support_summary),
    )
}

pub fn render_standalone_html_with_resources(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<String, ReportError> {
    render_standalone_html_with_options_and_resources(
        report,
        StandaloneHtmlOptions::default(),
        limits,
        cancellation,
    )
}

pub fn render_standalone_html_with_options_and_resources(
    report: &SessionReport,
    options: StandaloneHtmlOptions,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<String, ReportError> {
    render_standalone_html_document(report, options, limits, cancellation, None)
}

fn render_standalone_html_document(
    report: &SessionReport,
    options: StandaloneHtmlOptions,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
    operational_history: Option<&str>,
) -> Result<String, ReportError> {
    check_cancelled(cancellation, ReportResourceStage::Render, "standalone_html")?;
    let mut out = CheckedHtmlWriter::new(limits.html_bytes, cancellation);
    out.push_str(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\">\
<title>AntennaBench session report</title><style>",
    );
    out.push_str(STYLES);
    render_geometry_styles(&mut out);
    out.push_str(COVERAGE_STYLES);
    out.push_str("</style></head><body><main><a class=\"skip-link\" href=\"#what-run-show\">Skip to report findings</a>");

    write_html!(
        out,
        "<header class=\"hero\"><p class=\"eyebrow\">AntennaBench local report</p>\
<h1>Session evidence report</h1><p class=\"muted\">Session <code>{}</code></p></header>",
        escape_html(&report.overview.scope.session_id)
    );
    render_question_navigation(&mut out, report, true);
    render_how_to_read(&mut out);
    render_answer_first_overview(&mut out, report);
    if report.overview.answerability.same_path_signal == SamePathSignalAnswerability::Available {
        render_same_path_section(&mut out, report);
    }
    if report.overview.answerability.paired_detectability
        == PairedDetectabilityAnswerability::Available
    {
        render_reporter_activity_section(&mut out, report);
        render_coverage_map_section(&mut out, report);
    }
    if report.overview.answerability.observed_reach == ObservedReachAnswerability::Available {
        render_reach_section(&mut out, report);
    }
    if !report.coverage_overlap.is_empty() {
        render_overlap_repeatability_section(&mut out, report);
    }
    if report.overview.answerability.geographic_profile == GeographicProfileAnswerability::Available
    {
        render_distance_section(&mut out, report);
    }
    render_run_quality_section(&mut out, report);
    render_audit_appendix(&mut out, report, options.controller_evidence);

    if let Some(summary) = operational_history {
        write_html!(
            out,
            "<section class=\"panel\" aria-labelledby=\"operational-history-title\"><h2 id=\"operational-history-title\">Operational support history</h2><p class=\"notice\"><strong>Explicitly included at export:</strong> This bounded, redacted support view is operational metadata, not experiment evidence or a scientific conclusion. The lossless bundle may contain additional approved operational records.</p><pre>{}</pre></section>",
            escape_html(summary)
        );
    }

    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This report is descriptive and does not select an antenna winner.</p></main></body></html>");
    out.finish().map_err(ReportError::from)
}
