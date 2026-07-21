use crate::{ReportError, ReportEvidenceSummary, SessionReport};

use super::{
    shared::CheckedHtmlWriter,
    templates::{
        render_template, AntennaSectionTemplate, BandSectionTemplate, EvidenceSummaryTemplate,
        SlotSectionTemplate,
    },
    view::{AntennaSectionView, BandSectionView, EvidenceSummaryView, SlotSectionView},
};

pub(super) fn render_antenna_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &AntennaSectionTemplate {
            view: AntennaSectionView::new(report),
        },
    )
}

pub(super) fn render_band_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &BandSectionTemplate {
            view: BandSectionView::new(report),
        },
    )
}

pub(super) fn render_slot_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &SlotSectionTemplate {
            view: SlotSectionView::new(report),
        },
    )
}

pub(super) fn evidence_summary(
    out: &mut CheckedHtmlWriter<'_>,
    evidence: &ReportEvidenceSummary,
) -> Result<(), ReportError> {
    render_template(
        out,
        &EvidenceSummaryTemplate {
            view: EvidenceSummaryView::new(evidence),
        },
    )
}
