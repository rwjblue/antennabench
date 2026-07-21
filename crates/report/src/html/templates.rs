use std::fmt;

use askama::Template;

use crate::ReportError;

use super::{shared::CheckedHtmlWriter, view::OperationalHistoryView};

mod activity;
mod audit;
mod coverage;
mod evidence;
mod overview;
mod paths;
mod quality;

pub(super) use activity::*;
pub(super) use audit::*;
pub(super) use coverage::*;
pub(super) use evidence::*;
pub(super) use overview::*;
pub(super) use paths::*;
pub(super) use quality::*;

#[derive(Template)]
#[template(path = "report/full_header.html")]
pub(super) struct FullHeaderTemplate<'a> {
    pub(super) view: super::view::FullHeaderView<'a>,
}

#[derive(Template)]
#[template(path = "report/operational_history.html")]
pub(super) struct OperationalHistoryTemplate<'a> {
    pub(super) view: OperationalHistoryView<'a>,
}

struct AskamaWriter<'writer, 'cancellation> {
    writer: &'writer mut CheckedHtmlWriter<'cancellation>,
}

impl fmt::Write for AskamaWriter<'_, '_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.writer.push_str(value);
        if self.writer.failure().is_some() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

pub(super) fn render_template(
    writer: &mut CheckedHtmlWriter<'_>,
    template: &impl Template,
) -> Result<(), ReportError> {
    let result = template.render_into(&mut AskamaWriter { writer });
    match result {
        Ok(()) => Ok(()),
        Err(_) if writer.failure().is_some() => {
            Err(ReportError::Resource(writer.failure().unwrap().clone()))
        }
        Err(error) => Err(ReportError::TemplateRendering {
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use askama::Template;

    use crate::{ReportCancellationToken, ReportError};

    use super::{render_template, CheckedHtmlWriter};

    struct FailingDisplay;

    impl fmt::Display for FailingDisplay {
        fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    #[derive(Template)]
    #[template(source = "{{ value }}", ext = "html")]
    struct FailingTemplate {
        value: FailingDisplay,
    }

    #[test]
    fn unexpected_template_failures_remain_typed() {
        let cancellation = ReportCancellationToken::default();
        let mut writer = CheckedHtmlWriter::new(1024, &cancellation);
        let error = render_template(
            &mut writer,
            &FailingTemplate {
                value: FailingDisplay,
            },
        )
        .unwrap_err();

        assert!(matches!(error, ReportError::TemplateRendering { .. }));
    }
}
