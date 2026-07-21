use askama::Template;

use super::super::view::{
    EligibilityView, ExclusionRecordsView, NoticesView, ReporterActivityAuditView,
};

#[derive(Template)]
#[template(path = "report/audit/reporter_activity.html")]
pub(in crate::html) struct ReporterActivityAuditTemplate {
    pub(in crate::html) view: ReporterActivityAuditView,
}

#[derive(Template)]
#[template(path = "report/audit/eligibility.html")]
pub(in crate::html) struct EligibilityTemplate {
    pub(in crate::html) view: EligibilityView,
}

#[derive(Template)]
#[template(path = "report/audit/exclusion_records.html")]
pub(in crate::html) struct ExclusionRecordsTemplate {
    pub(in crate::html) view: ExclusionRecordsView,
}

#[derive(Template)]
#[template(path = "report/audit/notices.html")]
pub(in crate::html) struct NoticesTemplate {
    pub(in crate::html) view: NoticesView,
}

#[derive(Template)]
#[template(path = "report/audit/appendix_start.html")]
pub(in crate::html) struct AuditAppendixStartTemplate {
    pub(in crate::html) controller_details_omitted: bool,
}

#[derive(Template)]
#[template(path = "report/audit/snapshot_disclosure_start.html")]
pub(in crate::html) struct AuditSnapshotDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/context_disclosure_start.html")]
pub(in crate::html) struct AuditContextDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/comparison_disclosure_start.html")]
pub(in crate::html) struct AuditComparisonDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/disclosure_end.html")]
pub(in crate::html) struct AuditDisclosureEndTemplate;

#[derive(Template)]
#[template(path = "report/audit/appendix_end.html")]
pub(in crate::html) struct AuditAppendixEndTemplate;
