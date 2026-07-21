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
        render_compact_observed_footprint_section, render_how_to_read, render_question_navigation,
        render_reporter_activity_section, render_same_path_view,
    },
    shared::*,
    styles::{COMPACT_SMALL_PRINT_STYLES, COMPACT_STYLES, COVERAGE_STYLES, STYLES},
    templates::{
        render_template, CompactHeaderTemplate, CompactQualityTemplate, CompactReferenceTemplate,
        CompactSamePathEndTemplate, CompactSamePathStartTemplate,
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
    render_template(
        &mut out,
        &CompactHeaderTemplate {
            main_class: compact_main_class,
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
    if is_single_antenna_lens(report) {
        out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This compact profiling summary describes only recorded evidence from the named antenna.</p></main></body></html>");
    } else {
        out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This compact summary is descriptive and does not select an antenna winner.</p></main></body></html>");
    }
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
