use crate::{ReportError, ReportNotice, SessionReport};

use super::super::{
    shared::CheckedHtmlWriter,
    templates::{render_template, EligibilityTemplate, ExclusionRecordsTemplate, NoticesTemplate},
    view::{EligibilityView, ExclusionRecordsView, NoticesView},
};

pub(in super::super) fn render_eligibility(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let Some(view) = EligibilityView::new(report) else {
        return Ok(());
    };
    render_template(out, &EligibilityTemplate { view })
}

pub(in super::super) fn render_exclusion_records(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ExclusionRecordsTemplate {
            view: ExclusionRecordsView::new(report),
        },
    )
}

pub(in super::super) fn render_notices(
    out: &mut CheckedHtmlWriter<'_>,
    notices: &[ReportNotice],
) -> Result<(), ReportError> {
    let Some(view) = NoticesView::new(notices) else {
        return Ok(());
    };
    render_template(out, &NoticesTemplate { view })
}
