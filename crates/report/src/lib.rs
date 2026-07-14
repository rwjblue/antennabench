mod builder;
mod html;
mod model;

pub use builder::{build_report, build_report_with_validation};
pub use html::render_standalone_html;
pub use model::*;
