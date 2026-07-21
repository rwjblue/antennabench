use askama::Template;

use super::super::view::OverlapQuestionView;

#[derive(Template)]
#[template(path = "report/overlap/overlap.html")]
pub(in crate::html) struct OverlapQuestionTemplate {
    pub(in crate::html) view: OverlapQuestionView,
}
