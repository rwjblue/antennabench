use std::{env, error::Error, fs, io, path::PathBuf};

use antennabench_report::{build_report, render_compact_summary_html, render_standalone_html};
use antennabench_storage::BundleStore;

fn stylesheet(html: &str) -> Result<&str, io::Error> {
    let start = html
        .find("<style>")
        .map(|index| index + "<style>".len())
        .ok_or_else(|| io::Error::other("rendered report is missing <style>"))?;
    let end = html[start..]
        .find("</style>")
        .map(|index| start + index)
        .ok_or_else(|| io::Error::other("rendered report is missing </style>"))?;
    Ok(&html[start..end])
}

fn main() -> Result<(), Box<dyn Error>> {
    let check = match env::args().nth(1).as_deref() {
        None => false,
        Some("--check") => true,
        Some(_) => {
            return Err(io::Error::other("usage: sync_desktop_report_styles [--check]").into());
        }
    };
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture =
        repository.join("fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    let report = build_report(&BundleStore::new(fixture).read_normalized_validated()?)?;
    let rendered = [
        (
            repository.join("apps/desktop/frontend/report.css"),
            render_standalone_html(&report)?,
        ),
        (
            repository.join("apps/desktop/frontend/report-compact.css"),
            render_compact_summary_html(&report)?,
        ),
    ];

    for (path, html) in rendered {
        let expected = stylesheet(&html)?.as_bytes();
        if check {
            let actual = fs::read(&path)?;
            if actual != expected {
                return Err(io::Error::other(format!(
                    "{} is stale; run mise run desktop:report-style-update",
                    path.display()
                ))
                .into());
            }
            println!("checked {}", path.display());
        } else {
            fs::write(&path, expected)?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}
