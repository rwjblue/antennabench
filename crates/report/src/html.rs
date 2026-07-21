use crate::{
    check_cancelled, ReportCancellationToken, ReportError, ReportQuestionFamily,
    ReportResourceLimits, ReportResourceStage, SessionReport, REPORT_RESOURCE_LIMITS,
};

// Keep the public rendering surface here; section ownership remains renderer-private.
mod audit;
mod compact;
mod evidence;
mod geometry;
mod presentation;
mod questions;
mod shared;
mod styles;
mod templates;
mod view;

pub use compact::{render_compact_summary_html, render_compact_summary_html_with_resources};

use audit::render_audit_appendix;
use questions::{
    is_single_antenna_lens, ordered_question_families, render_answer_first_overview,
    render_coverage_map_section, render_distance_section, render_how_to_read,
    render_overlap_repeatability_section, render_question_navigation, render_reach_section,
    render_reporter_activity_section, render_run_quality_section, render_same_path_section,
};
use shared::CheckedHtmlWriter;
use styles::{stylesheet_csp_source, write_stylesheet_to_html, StylesheetVariant};
use templates::{
    render_template, BodyStartTemplate, DocumentEndTemplate, DocumentStartTemplate,
    FullHeaderTemplate, OperationalHistoryTemplate,
};
use view::{FullHeaderView, OperationalHistoryView};

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
    let stylesheet_variant = StylesheetVariant::Full;
    let style_source = stylesheet_csp_source(stylesheet_variant);
    render_template(
        &mut out,
        &DocumentStartTemplate {
            title: "AntennaBench session report",
            style_source: &style_source,
        },
    )?;
    write_stylesheet_to_html(&mut out, stylesheet_variant);
    render_template(&mut out, &BodyStartTemplate { main_class: "" })?;

    render_template(
        &mut out,
        &FullHeaderTemplate {
            view: FullHeaderView {
                session_id: &report.overview.scope.session_id,
            },
        },
    )?;
    render_question_navigation(&mut out, report, true)?;
    render_how_to_read(&mut out, report, false)?;
    render_answer_first_overview(&mut out, report)?;
    for family in ordered_question_families(report) {
        match family {
            ReportQuestionFamily::SharedPathSignal => render_same_path_section(&mut out, report)?,
            ReportQuestionFamily::CommonOpportunityDetection => {
                render_reporter_activity_section(&mut out, report)?;
                render_coverage_map_section(&mut out, report)?;
            }
            ReportQuestionFamily::ObservedReach => render_reach_section(&mut out, report)?,
            ReportQuestionFamily::GeographicProfile => render_distance_section(&mut out, report)?,
            ReportQuestionFamily::Repeatability => {
                render_overlap_repeatability_section(&mut out, report)?
            }
        }
    }
    render_run_quality_section(&mut out, report)?;
    render_audit_appendix(&mut out, report, options.controller_evidence)?;

    if let Some(summary) = operational_history {
        render_template(
            &mut out,
            &OperationalHistoryTemplate {
                view: OperationalHistoryView { summary },
            },
        )?;
    }

    render_template(
        &mut out,
        &DocumentEndTemplate {
            compact: false,
            single_antenna: is_single_antenna_lens(report),
        },
    )?;
    out.finish().map_err(ReportError::from)
}
