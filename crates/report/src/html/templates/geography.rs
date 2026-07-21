use askama::Template;

use super::super::view::{CompactFootprintView, GeographyView, ObservedPathAuditView};

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
#[template(path = "report/geography/compact.html")]
pub(in crate::html) struct CompactFootprintTemplate {
    pub(in crate::html) view: CompactFootprintView,
}

#[derive(Template)]
#[template(path = "report/geography/compact_close.html")]
pub(in crate::html) struct CompactFootprintCloseTemplate;

#[derive(Template)]
#[template(path = "report/geography/compact_end.html")]
pub(in crate::html) struct CompactFootprintEndTemplate {
    pub(in crate::html) audit: ObservedPathAuditView,
}
