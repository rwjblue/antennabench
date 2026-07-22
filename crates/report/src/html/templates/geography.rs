use askama::Template;

use super::super::view::{GeographyView, ObservedPathAuditView, SummaryFootprintView};

#[derive(Template)]
#[template(path = "report/geography/full.html")]
pub(in crate::html) struct GeographyTemplate {
    pub(in crate::html) view: GeographyView,
}

#[derive(Template)]
#[template(path = "report/geography/before_solar.html")]
pub(in crate::html) struct GeographyBeforeSolarTemplate {
    pub(in crate::html) single_antenna: bool,
}

#[derive(Template)]
#[template(path = "report/geography/end.html")]
pub(in crate::html) struct GeographyEndTemplate;

#[derive(Template)]
#[template(path = "report/geography/summary.html")]
pub(in crate::html) struct SummaryFootprintTemplate {
    pub(in crate::html) view: SummaryFootprintView,
}

#[derive(Template)]
#[template(path = "report/geography/summary_close.html")]
pub(in crate::html) struct SummaryFootprintCloseTemplate;

#[derive(Template)]
#[template(path = "report/geography/summary_end.html")]
pub(in crate::html) struct SummaryFootprintEndTemplate {
    pub(in crate::html) audit: ObservedPathAuditView,
}
