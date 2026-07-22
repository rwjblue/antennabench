use std::{env, error::Error, fs, io, path::PathBuf};

use antennabench_report::{build_report, render_standalone_html, render_summary_html};
use antennabench_storage::BundleStore;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let output = args.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::other(
            "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>]",
        )
    })?;
    let mut summary = false;
    let mut fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    while let Some(argument) = args.next() {
        if argument == "--summary" || argument == "--compact-summary" {
            summary = true;
        } else if argument == "--bundle" {
            fixture = args.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::other(
                    "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>]",
                )
            })?;
        } else {
            return Err(io::Error::other(
                "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>]",
            )
            .into());
        }
    }
    let bundle = BundleStore::new(fixture).read_normalized_validated()?;
    let report = build_report(&bundle)?;
    let html = if summary {
        render_summary_html(&report)?
    } else {
        render_standalone_html(&report)?
    };

    fs::write(&output, html)?;
    println!("wrote {}", output.display());
    Ok(())
}
