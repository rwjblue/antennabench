use askama::Template;

use super::super::view::{CompactQualityView, CompactReferenceView, QualityView};

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
#[template(path = "report/quality/compact.html")]
pub(in crate::html) struct CompactQualityTemplate {
    pub(in crate::html) view: CompactQualityView,
}

#[derive(Template)]
#[template(path = "report/quality/compact_reference.html")]
pub(in crate::html) struct CompactReferenceTemplate {
    pub(in crate::html) view: CompactReferenceView,
}
