use crate::{
    check_cancelled, ReportAcquisitionWorkflowStatus, ReportCancellationToken, ReportCompleteness,
    ReportError, ReportProviderCompleteness, ReportQuestionFamily, ReportResourceLimits,
    ReportResourceStage, SessionReport, REPORT_RESOURCE_LIMITS,
};

use super::{
    questions::{
        is_single_antenna_lens, ordered_question_families, overview_lifecycle_label,
        render_answer_first_overview_with_reference, render_compact_coverage_map_section,
        render_compact_observed_footprint_section, render_how_to_read, render_question_navigation,
        render_reporter_activity_section, render_same_path_view,
    },
    shared::*,
    styles::{stylesheet_csp_source, write_stylesheet_to_html, StylesheetVariant},
    templates::{
        render_template, BodyStartTemplate, CompactHeaderTemplate, CompactQualityTemplate,
        CompactReferenceTemplate, CompactSamePathEndTemplate, CompactSamePathStartTemplate,
        DocumentEndTemplate, DocumentStartTemplate,
    },
    view::{CompactQualityView, CompactReferenceView},
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
    let stylesheet_variant = StylesheetVariant::Compact;
    let style_source = stylesheet_csp_source(stylesheet_variant);
    render_template(
        &mut out,
        &DocumentStartTemplate {
            title: "AntennaBench compact share summary",
            style_source: &style_source,
        },
    )?;
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
    write_stylesheet_to_html(&mut out, stylesheet_variant);
    render_template(
        &mut out,
        &BodyStartTemplate {
            main_class: compact_main_class,
        },
    )?;
    render_template(
        &mut out,
        &CompactHeaderTemplate {
            session_id: &report.overview.scope.session_id,
        },
    )?;
    render_question_navigation(&mut out, report, false)?;
    render_how_to_read(&mut out, report, true)?;
    render_answer_first_overview_with_reference(
        &mut out,
        report,
        "the full evidence report and session bundle",
        true,
    )?;
    let mut rendered_observed_footprint = false;
    for family in ordered_question_families(report) {
        match family {
            ReportQuestionFamily::SharedPathSignal => {
                render_template(&mut out, &CompactSamePathStartTemplate)?;
                render_same_path_view(&mut out, report, true)?;
                render_template(&mut out, &CompactSamePathEndTemplate)?;
            }
            ReportQuestionFamily::CommonOpportunityDetection => {
                render_reporter_activity_section(&mut out, report)?;
                render_compact_coverage_map_section(&mut out, report)?;
            }
            ReportQuestionFamily::ObservedReach => {
                if !rendered_observed_footprint {
                    render_compact_observed_footprint_section(&mut out, report)?;
                    rendered_observed_footprint = true;
                }
            }
            ReportQuestionFamily::GeographicProfile => {
                if !rendered_observed_footprint {
                    render_compact_observed_footprint_section(&mut out, report)?;
                    rendered_observed_footprint = true;
                }
            }
            ReportQuestionFamily::Repeatability => {
                if !rendered_observed_footprint {
                    render_compact_observed_footprint_section(&mut out, report)?;
                    rendered_observed_footprint = true;
                }
            }
        }
    }
    render_compact_run_quality(&mut out, report)?;
    render_compact_reference(&mut out, report)?;
    render_template(
        &mut out,
        &DocumentEndTemplate {
            compact: true,
            single_antenna: is_single_antenna_lens(report),
        },
    )?;
    out.finish().map_err(ReportError::from)
}

pub(super) fn render_compact_run_quality(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
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
    render_template(
        out,
        &CompactQualityTemplate {
            view: CompactQualityView {
                comparison_state: availability,
                lifecycle,
                usable: overall.observation_counts.usable,
                excluded: overall.observation_counts.excluded,
                acquisition,
                bounded: report.completeness == ReportCompleteness::BoundedOverview,
            },
        },
    )
}

pub(super) fn render_compact_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let revision = report
        .snapshot
        .checkpoint_revision
        .or(report.overview.lifecycle.checkpoint_revision)
        .map_or_else(not_recorded, |value| value.to_string());
    render_template(
        out,
        &CompactReferenceTemplate {
            view: CompactReferenceView {
                session_id: report.overview.scope.session_id.clone(),
                revision,
            },
        },
    )
}
