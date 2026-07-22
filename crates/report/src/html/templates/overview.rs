use askama::Template;

use super::super::view::{NavigationView, OverviewView, ReadingGuideView, SummaryOverviewView};

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
#[template(path = "report/overview/summary_answer_first.html")]
pub(in crate::html) struct SummaryAnswerFirstTemplate {
    pub(in crate::html) view: SummaryOverviewView,
}

#[derive(Template)]
#[template(path = "report/overview/summary_header.html")]
pub(in crate::html) struct SummaryHeaderTemplate<'a> {
    pub(in crate::html) session_id: &'a str,
}
