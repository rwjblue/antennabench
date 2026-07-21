use askama::Template;

use super::super::view::CoverageView;

#[derive(Template)]
#[template(path = "report/coverage/coverage.html")]
pub(in crate::html) struct CoverageTemplate {
    pub(in crate::html) view: CoverageView,
}
