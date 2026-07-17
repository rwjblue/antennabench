mod builder;
mod html;
mod model;
mod resource;

pub use builder::{
    build_report, build_report_with_resources, build_report_with_snapshot,
    build_report_with_validation,
};
pub use html::{
    render_compact_summary_html, render_compact_summary_html_with_resources,
    render_standalone_html, render_standalone_html_with_resources,
};
pub use model::*;
pub use resource::*;
