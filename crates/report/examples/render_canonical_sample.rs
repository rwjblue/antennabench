use std::{env, error::Error, fs, io, path::PathBuf};

use antennabench_report::{build_report, render_standalone_html};
use antennabench_storage::BundleStore;

fn main() -> Result<(), Box<dyn Error>> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("usage: render_canonical_sample <output.html>"))?;
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    let bundle = BundleStore::new(fixture).read_normalized_validated()?;
    let report = build_report(&bundle)?;
    let html = render_standalone_html(&report);

    fs::write(&output, html)?;
    println!("wrote {}", output.display());
    Ok(())
}
