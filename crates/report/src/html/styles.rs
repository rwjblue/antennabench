use base64::{engine::general_purpose::STANDARD, Engine as _};
use sha2::{Digest, Sha256};

use super::{geometry::write_geometry_styles, shared::CheckedHtmlWriter};

const REPORT_CSS: &str = include_str!("../../styles/report.css");
const COVERAGE_CSS: &str = include_str!("../../styles/coverage.css");
const SUMMARY_CSS: &str = include_str!("../../styles/summary.css");
const SUMMARY_SMALL_PRINT_CSS: &str = include_str!("../../styles/summary-small-print.css");
const MODULE_SEPARATOR: &str = "\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StylesheetVariant {
    Full,
    Summary,
}

pub(super) fn write_stylesheet(variant: StylesheetVariant, write: &mut impl FnMut(&str)) {
    write(REPORT_CSS);
    write_geometry_styles(write);
    write(MODULE_SEPARATOR);
    write(COVERAGE_CSS);
    if variant == StylesheetVariant::Summary {
        write(SUMMARY_CSS);
        write(SUMMARY_SMALL_PRINT_CSS);
    }
}

pub(super) fn write_stylesheet_to_html(
    out: &mut CheckedHtmlWriter<'_>,
    variant: StylesheetVariant,
) {
    write_stylesheet(variant, &mut |css| out.push_str(css));
}

pub(super) fn stylesheet_csp_source(variant: StylesheetVariant) -> String {
    let mut digest = Sha256::new();
    write_stylesheet(variant, &mut |css| digest.update(css.as_bytes()));
    format!("sha256-{}", STANDARD.encode(digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_stylesheet(variant: StylesheetVariant) -> String {
        let mut out = String::new();
        write_stylesheet(variant, &mut |css| out.push_str(css));
        out
    }

    #[test]
    fn canonical_stylesheets_preserve_explicit_assembly_order() {
        let full = render_stylesheet(StylesheetVariant::Full);
        let summary = render_stylesheet(StylesheetVariant::Summary);

        assert!(full.starts_with(":root {"));
        assert!(full.contains(".geometry-left"));
        assert!(full.contains(".g0{--g:0%}"));
        assert!(full.contains(".g1000{--g:100%}"));
        assert!(full.contains("--coverage-both"));
        assert!(!full.contains("main.summary"));

        assert!(summary.starts_with(&full));
        assert!(summary.contains("main.summary"));
        assert!(summary.contains(".summary.summary-small"));
    }

    #[test]
    fn authored_css_is_lf_only_and_offline() {
        for (name, css) in [
            ("report.css", REPORT_CSS),
            ("coverage.css", COVERAGE_CSS),
            ("summary.css", SUMMARY_CSS),
            ("summary-small-print.css", SUMMARY_SMALL_PRINT_CSS),
        ] {
            assert!(!css.contains('\r'), "{name} must use LF line endings");
            assert!(css.ends_with('\n'), "{name} must end with LF");
            assert!(!css.contains("@import"), "{name} must not import CSS");
            assert!(
                !css.contains("url("),
                "{name} must not load external assets"
            );
        }
    }
}
