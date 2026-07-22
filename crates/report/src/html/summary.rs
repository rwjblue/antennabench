use crate::{
    check_cancelled, ReportCancellationToken, ReportCompleteness, ReportError, ReportNotice,
    ReportOverviewLifecycleState, ReportResourceLimits, ReportResourceStage, SessionReport,
    REPORT_RESOURCE_LIMITS,
};
use antennabench_core::v2::SessionLifecycleV2;

use super::{
    audit::lifecycle,
    questions::{
        is_single_antenna_lens, render_summary_answer_first_overview, render_summary_how_to_read,
        render_summary_navigation, render_summary_observed_footprint_section,
        render_summary_same_path_section,
    },
    shared::*,
    styles::{stylesheet_csp_source, write_stylesheet_to_html, StylesheetVariant},
    templates::{
        render_template, BodyStartTemplate, DocumentEndTemplate, DocumentStartTemplate,
        PublicDocumentStartTemplate, SummaryHeaderTemplate, SummaryQualityTemplate,
        SummaryReferenceTemplate,
    },
    view::{SummaryQualityFactView, SummaryQualityView, SummaryReferenceView},
    HtmlDocumentMetadata,
};

/// Renders a concise, deterministic, standalone HTML summary from the same
/// renderer-neutral report revision as the full evidence report. It intentionally
/// omits the audit appendix and never recomputes or reinterprets report facts.
pub fn render_summary_html(report: &SessionReport) -> Result<String, ReportError> {
    render_summary_html_document(
        report,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        None,
    )
}

/// Renders the same standalone Summary with optional public-page discovery
/// metadata while preserving the script-free and offline document boundary.
pub fn render_summary_html_with_metadata(
    report: &SessionReport,
    metadata: &HtmlDocumentMetadata,
) -> Result<String, ReportError> {
    render_summary_html_document(
        report,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        Some(metadata),
    )
}

pub fn render_summary_html_with_resources(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<String, ReportError> {
    render_summary_html_document(report, limits, cancellation, None)
}

fn render_summary_html_document(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
    metadata: Option<&HtmlDocumentMetadata>,
) -> Result<String, ReportError> {
    check_cancelled(cancellation, ReportResourceStage::Render, "summary_html")?;
    let mut out = CheckedHtmlWriter::new(limits.html_bytes, cancellation);
    let stylesheet_variant = StylesheetVariant::Summary;
    let style_source = stylesheet_csp_source(stylesheet_variant);
    if let Some(metadata) = metadata {
        render_template(
            &mut out,
            &PublicDocumentStartTemplate {
                title: "AntennaBench Summary",
                style_source: &style_source,
                canonical_url: &metadata.canonical_url,
                description: &metadata.description,
                social_title: &metadata.social_title,
                social_image_url: &metadata.social_image_url,
                social_image_alt: &metadata.social_image_alt,
            },
        )?;
    } else {
        render_template(
            &mut out,
            &DocumentStartTemplate {
                title: "AntennaBench Summary",
                style_source: &style_source,
            },
        )?;
    }
    let summary_main_class = if report.overview.strata.len() <= 2
        && report
            .overview
            .strata
            .iter()
            .map(|row| row.path_median_deltas.len())
            .sum::<usize>()
            <= 4
    {
        "summary summary-small"
    } else {
        "summary"
    };
    write_stylesheet_to_html(&mut out, stylesheet_variant);
    render_template(
        &mut out,
        &BodyStartTemplate {
            main_class: summary_main_class,
        },
    )?;
    render_template(&mut out, &SummaryHeaderTemplate)?;
    render_summary_answer_first_overview(&mut out, report)?;
    let has_shared_path = report.overview.answerability.same_path_signal
        == crate::SamePathSignalAnswerability::Available;
    let has_observed_paths = report.overview.strata.iter().any(|row| {
        row.reach.left_only_unique_path_count
            + row.reach.both_unique_path_count
            + row.reach.right_only_unique_path_count
            > 0
    });
    let has_material_quality = summary_has_material_quality(report);
    render_summary_navigation(&mut out, report, has_material_quality)?;
    render_summary_how_to_read(&mut out, report)?;
    if has_shared_path {
        render_summary_same_path_section(&mut out, report)?;
    }
    if has_observed_paths {
        render_summary_observed_footprint_section(&mut out, report)?;
    }
    render_summary_run_quality(&mut out, report)?;
    render_summary_reference(&mut out, report)?;
    render_template(
        &mut out,
        &DocumentEndTemplate {
            summary: true,
            single_antenna: is_single_antenna_lens(report),
        },
    )?;
    out.finish().map_err(ReportError::from)
}

pub(super) fn render_summary_run_quality(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let Some(view) = summary_quality_view(report) else {
        return Ok(());
    };
    render_template(out, &SummaryQualityTemplate { view })
}

fn summary_has_material_quality(report: &SessionReport) -> bool {
    summary_quality_view(report).is_some()
}

fn summary_quality_view(report: &SessionReport) -> Option<SummaryQualityView> {
    let excluded = report.evidence.overall.observation_counts.excluded;
    let mut facts = Vec::new();
    if excluded > 0 {
        facts.push(SummaryQualityFactView {
            label: "Excluded evidence",
            value: format!(
                "{excluded} observation(s) were excluded from reported results; Full evidence records every reason."
            ),
        });
    }
    let lifecycle_fact = match report.overview.lifecycle.state {
        ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Interrupted) => Some(
            "The run was interrupted; interpret only the evidence recorded before interruption.",
        ),
        ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Abandoned) => {
            Some("The run was abandoned; its recorded evidence may not cover the planned session.")
        }
        ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Draft) => {
            Some("The session remained a draft and did not reach a completed run state.")
        }
        ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Ready) => {
            Some("The session was ready but did not reach a completed run state.")
        }
        ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Running) => {
            Some("The session was still running at this committed revision.")
        }
        ReportOverviewLifecycleState::NotRecorded
        | ReportOverviewLifecycleState::Recorded(SessionLifecycleV2::Ended) => None,
    };
    if let Some(value) = lifecycle_fact {
        facts.push(SummaryQualityFactView {
            label: "Run status",
            value: value.to_string(),
        });
    }
    let bounded = report.completeness == ReportCompleteness::BoundedOverview;
    let detail_omitted = report
        .notices
        .iter()
        .any(|notice| matches!(notice, ReportNotice::DetailOmitted { .. }));
    (bounded || detail_omitted || !facts.is_empty()).then_some(SummaryQualityView {
        facts,
        bounded,
        detail_omitted,
    })
}

pub(super) fn render_summary_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let revision = report
        .snapshot
        .checkpoint_revision
        .or(report.overview.lifecycle.checkpoint_revision)
        .map_or_else(not_recorded, |value| value.to_string());
    let lifecycle = match report.overview.lifecycle.state {
        ReportOverviewLifecycleState::NotRecorded => {
            "Lifecycle unavailable for this session".to_string()
        }
        ReportOverviewLifecycleState::Recorded(value) => lifecycle(value).to_string(),
    };
    let evidence = &report.snapshot.adapter_evidence;
    let acquisition = match (
        evidence.workflow_status,
        evidence.provider_completeness,
        evidence.gap_count,
    ) {
        (_, _, gaps) if gaps > 0 => format!("{gaps} recorded acquisition gap(s)"),
        (crate::ReportAcquisitionWorkflowStatus::Incomplete, _, _) => {
            "Recorded live collection did not complete".to_string()
        }
        (crate::ReportAcquisitionWorkflowStatus::NotConfigured, _, _) => {
            "Imported or locally recorded evidence; live collection was not configured".to_string()
        }
        (
            crate::ReportAcquisitionWorkflowStatus::Completed,
            crate::ReportProviderCompleteness::Known,
            _,
        ) => "Live collection completed; provider completeness is recorded as known".to_string(),
        (
            crate::ReportAcquisitionWorkflowStatus::Completed,
            crate::ReportProviderCompleteness::Unknown,
            _,
        ) => "Live collection completed; upstream completeness is not independently guaranteed"
            .to_string(),
        (
            crate::ReportAcquisitionWorkflowStatus::Completed,
            crate::ReportProviderCompleteness::Unsupported,
            _,
        ) => "Live collection completed; provider completeness is unsupported".to_string(),
    };
    render_template(
        out,
        &SummaryReferenceTemplate {
            view: SummaryReferenceView {
                session_id: report.overview.scope.session_id.clone(),
                revision,
                lifecycle,
                acquisition,
            },
        },
    )
}
