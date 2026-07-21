use askama::Template;

use super::super::view::ReporterActivityView;

#[derive(Template)]
#[template(path = "report/activity/reporter_activity.html")]
pub(in crate::html) struct ReporterActivityTemplate {
    pub(in crate::html) view: ReporterActivityView,
}
