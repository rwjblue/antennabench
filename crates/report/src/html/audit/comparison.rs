use crate::{ReportError, SessionReport};

use super::super::{
    shared::CheckedHtmlWriter,
    templates::{
        render_template, ComparisonBlocksTemplate, ComparisonDiagnosticsTemplate, OverlapTemplate,
        PairedDifferencesTemplate, PairedSnrTimeTemplate, StratumSummariesTemplate,
        TimelineTemplate,
    },
    view::{
        comparison_blocks, comparison_diagnostic_stats, OverlapView, PairedRowsView,
        StratumSummariesView, TimelineView,
    },
};

pub(in super::super) fn render_comparison_diagnostics(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ComparisonDiagnosticsTemplate {
            stats: comparison_diagnostic_stats(report),
        },
    )
}

pub(in super::super) fn render_overlap(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &OverlapTemplate {
            view: OverlapView::new(report),
        },
    )
}

pub(in super::super) fn render_comparison_timeline(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &TimelineTemplate {
            view: TimelineView::new(report),
        },
    )
}

pub(in super::super) fn render_comparison_blocks(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ComparisonBlocksTemplate {
            rows: comparison_blocks(report),
        },
    )
}

pub(in super::super) fn render_paired_differences(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &PairedDifferencesTemplate {
            view: PairedRowsView::new(report),
        },
    )
}

pub(in super::super) fn render_paired_snr_time(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &PairedSnrTimeTemplate {
            view: PairedRowsView::new(report),
        },
    )
}

pub(in super::super) fn render_stratum_summaries(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &StratumSummariesTemplate {
            view: StratumSummariesView::new(report),
        },
    )
}
