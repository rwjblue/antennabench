use super::*;
use crate::html::{
    templates::{render_template, NavigationTemplate, SummaryReadingGuideTemplate},
    view::{NavigationLinkView, NavigationView},
};

pub(in super::super) fn render_summary_navigation(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    include_quality: bool,
) -> Result<(), ReportError> {
    let mut links = vec![NavigationLinkView {
        href: "#what-run-show",
        label: "What did the run show?",
    }];
    if report.overview.answerability.same_path_signal == SamePathSignalAnswerability::Available {
        links.push(NavigationLinkView {
            href: "#same-path-signal",
            label: "Shared-path signal",
        });
    }
    if report.overview.strata.iter().any(|row| {
        row.reach.left_only_unique_path_count
            + row.reach.both_unique_path_count
            + row.reach.right_only_unique_path_count
            > 0
    }) {
        links.push(NavigationLinkView {
            href: "#observed-footprint",
            label: "Observed paths",
        });
    }
    if include_quality {
        links.push(NavigationLinkView {
            href: "#run-quality",
            label: "Run quality",
        });
    }
    render_template(
        out,
        &NavigationTemplate {
            view: NavigationView { links },
        },
    )
}

pub(in super::super) fn render_summary_how_to_read(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &SummaryReadingGuideTemplate {
            single_antenna: is_single_antenna_lens(report),
        },
    )
}
