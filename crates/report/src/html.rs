use std::fmt::Write as _;

use crate::{
    check_cancelled, ReportCancellationToken, ReportError, ReportResourceLimits,
    ReportResourceStage, SessionReport, REPORT_RESOURCE_LIMITS,
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
mod questions;
mod shared;
mod styles;

pub use compact::{render_compact_summary_html, render_compact_summary_html_with_resources};

use audit::render_audit_appendix;
use questions::{
    render_answer_first_overview, render_distance_section, render_how_to_read,
    render_question_navigation, render_reach_section, render_run_quality_section,
    render_same_path_section,
};
use shared::{escape_html, CheckedHtmlWriter};
use styles::STYLES;

/// Renders a deterministic, standalone HTML document from renderer-neutral
/// report data. The output contains no scripts, external resources, or
/// unescaped report strings.
pub fn render_standalone_html(report: &SessionReport) -> Result<String, ReportError> {
    render_standalone_html_with_resources(
        report,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
    )
}

pub fn render_standalone_html_with_resources(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
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
    out.push_str("</style></head><body><main><a class=\"skip-link\" href=\"#what-run-show\">Skip to report findings</a>");

    write_html!(
        out,
        "<header class=\"hero\"><p class=\"eyebrow\">AntennaBench local report</p>\
<h1>Session evidence report</h1><p class=\"muted\">Session <code>{}</code></p></header>",
        escape_html(&report.overview.scope.session_id)
    );
    render_question_navigation(&mut out);
    render_how_to_read(&mut out);
    render_answer_first_overview(&mut out, report);
    render_same_path_section(&mut out, report);
    render_reach_section(&mut out, report);
    render_distance_section(&mut out, report);
    render_run_quality_section(&mut out, report);
    render_audit_appendix(&mut out, report);

    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This report is descriptive and does not select an antenna winner.</p></main></body></html>");
    out.finish().map_err(ReportError::from)
}
