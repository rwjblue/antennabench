use askama::Template;

use super::super::view::{NavigationView, OverviewView, ReadingGuideView};

#[derive(Template)]
#[template(path = "report/overview/navigation.html")]
pub(in crate::html) struct NavigationTemplate {
    pub(in crate::html) view: NavigationView,
}

#[derive(Template)]
#[template(path = "report/overview/reading_guide.html")]
pub(in crate::html) struct ReadingGuideTemplate {
    pub(in crate::html) view: ReadingGuideView,
}

#[derive(Template)]
#[template(path = "report/overview/answer_first.html")]
pub(in crate::html) struct AnswerFirstTemplate {
    pub(in crate::html) view: OverviewView,
}

#[derive(Template)]
#[template(path = "report/overview/compact_header.html")]
pub(in crate::html) struct CompactHeaderTemplate<'a> {
    pub(in crate::html) session_id: &'a str,
}
