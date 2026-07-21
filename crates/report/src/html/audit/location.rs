use crate::{ReportError, SessionReport};

use super::super::{
    shared::{format_number, not_available, CheckedHtmlWriter},
    templates::{render_template, LocationViewsTemplate, SolarContextTemplate},
    view::{LocationViewsView, SolarContextView},
};

pub(in super::super) fn render_location_views(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &LocationViewsTemplate {
            view: LocationViewsView::new(report),
        },
    )
}

pub(in super::super) fn render_solar_context(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &SolarContextTemplate {
            view: SolarContextView::new(report),
        },
    )
}

pub(in super::super) fn optional_measure_f64(value: Option<f64>, unit: &str) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{} {unit}", format_number(value)))
        .unwrap_or_else(not_available)
}
