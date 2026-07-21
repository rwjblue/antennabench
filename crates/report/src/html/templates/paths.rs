use askama::Template;

use super::super::view::{ReachView, SamePathView};

#[derive(Template)]
#[template(path = "report/paths/same_path_section_start.html")]
pub(in crate::html) struct SamePathSectionStartTemplate;

#[derive(Template)]
#[template(path = "report/paths/compact_same_path_start.html")]
pub(in crate::html) struct CompactSamePathStartTemplate;

#[derive(Template)]
#[template(path = "report/paths/compact_same_path_end.html")]
pub(in crate::html) struct CompactSamePathEndTemplate;

#[derive(Template)]
#[template(path = "report/paths/same_path_audit_start.html")]
pub(in crate::html) struct SamePathAuditStartTemplate;

#[derive(Template)]
#[template(path = "report/paths/reach_section_start.html")]
pub(in crate::html) struct ReachSectionStartTemplate;

#[derive(Template)]
#[template(path = "report/paths/reach_audit_start.html")]
pub(in crate::html) struct ReachAuditStartTemplate;

#[derive(Template)]
#[template(path = "report/paths/question_section_end.html")]
pub(in crate::html) struct PathQuestionSectionEndTemplate;

#[derive(Template)]
#[template(path = "report/paths/same_path.html")]
pub(in crate::html) struct SamePathTemplate {
    pub(in crate::html) view: SamePathView,
}

#[derive(Template)]
#[template(path = "report/paths/reach.html")]
pub(in crate::html) struct ReachTemplate {
    pub(in crate::html) view: ReachView,
}
