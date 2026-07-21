use crate::{ReportError, SessionReport};

use super::super::{
    shared::CheckedHtmlWriter,
    templates::{render_template, ReporterActivityAuditTemplate},
    view::ReporterActivityAuditView,
};

pub(in crate::html) fn render_reporter_activity_audit(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let Some(view) = ReporterActivityAuditView::new(report) else {
        return Ok(());
    };
    render_template(out, &ReporterActivityAuditTemplate { view })
}
