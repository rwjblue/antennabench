use askama::Template;

use super::super::view::{QualityView, SummaryQualityView, SummaryReferenceView};

#[derive(Template)]
#[template(path = "report/quality/start.html")]
pub(in crate::html) struct QualityStartTemplate {
    pub(in crate::html) view: QualityView,
}

#[derive(Template)]
#[template(path = "report/quality/after_exclusions.html")]
pub(in crate::html) struct QualityAfterExclusionsTemplate {
    pub(in crate::html) had_records: bool,
}

#[derive(Template)]
#[template(path = "report/quality/accounting_start.html")]
pub(in crate::html) struct QualityAccountingStartTemplate;

#[derive(Template)]
#[template(path = "report/quality/accounting_between.html")]
pub(in crate::html) struct QualityAccountingBetweenTemplate;

#[derive(Template)]
#[template(path = "report/quality/end.html")]
pub(in crate::html) struct QualityEndTemplate;

#[derive(Template)]
#[template(path = "report/quality/summary.html")]
pub(in crate::html) struct SummaryQualityTemplate {
    pub(in crate::html) view: SummaryQualityView,
}

#[derive(Template)]
#[template(path = "report/quality/summary_reference.html")]
pub(in crate::html) struct SummaryReferenceTemplate {
    pub(in crate::html) view: SummaryReferenceView,
}
