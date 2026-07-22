use std::{env, error::Error, fs, io, path::PathBuf};

use antennabench_report::{
    build_report, render_standalone_html, render_standalone_html_with_metadata,
    render_summary_html, render_summary_html_with_metadata, HtmlDocumentMetadata,
};
use antennabench_storage::BundleStore;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let output = args.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::other(
            "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>] [--public-metadata <full|summary|inconclusive>]",
        )
    })?;
    let mut summary = false;
    let mut public_metadata = None;
    let mut fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    while let Some(argument) = args.next() {
        if argument == "--summary" || argument == "--compact-summary" {
            summary = true;
        } else if argument == "--bundle" {
            fixture = args.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::other(
                    "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>] [--public-metadata <full|summary|inconclusive>]",
                )
            })?;
        } else if argument == "--public-metadata" {
            public_metadata = Some(args.next().ok_or_else(|| {
                io::Error::other(
                    "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>] [--public-metadata <full|summary|inconclusive>]",
                )
            })?);
        } else {
            return Err(io::Error::other(
                "usage: render_canonical_sample <output.html> [--summary] [--bundle <path>] [--public-metadata <full|summary|inconclusive>]",
            )
            .into());
        }
    }
    let bundle = BundleStore::new(fixture).read_normalized_validated()?;
    let report = build_report(&bundle)?;
    let metadata = public_metadata
        .as_deref()
        .map(public_document_metadata)
        .transpose()?;
    if summary
        != matches!(
            public_metadata.as_deref().and_then(std::ffi::OsStr::to_str),
            Some("summary")
        )
        && public_metadata.is_some()
    {
        return Err(io::Error::other(
            "--public-metadata summary requires --summary; full and inconclusive require Full evidence",
        )
        .into());
    }
    let html = match (summary, metadata.as_ref()) {
        (true, Some(metadata)) => render_summary_html_with_metadata(&report, metadata)?,
        (true, None) => render_summary_html(&report)?,
        (false, Some(metadata)) => render_standalone_html_with_metadata(&report, metadata)?,
        (false, None) => render_standalone_html(&report)?,
    };

    fs::write(&output, html)?;
    println!("wrote {}", output.display());
    Ok(())
}

fn public_document_metadata(kind: &std::ffi::OsStr) -> Result<HtmlDocumentMetadata, io::Error> {
    let (path, title, description) = match kind.to_str() {
        Some("full") => (
            "/sample-report/",
            "AntennaBench Full evidence — Canonical sample",
            "Inspect the canonical AntennaBench Full evidence report, including the complete human-readable findings, limitations, and audit detail from a real sanitized WSPR comparison.",
        ),
        Some("summary") => (
            "/sample-report/summary/",
            "AntennaBench Summary — Canonical sample",
            "Read the canonical AntennaBench Summary: answer-first findings, separate evidence populations, support counts, and principal limitations from a real sanitized WSPR comparison.",
        ),
        Some("inconclusive") => (
            "/sample-report/inconclusive/",
            "AntennaBench Full evidence — Inconclusive sample",
            "See how AntennaBench reports an inconclusive WSPR comparison with no shared paths, without inventing a winner or zero-valued signal evidence.",
        ),
        _ => {
            return Err(io::Error::other(
                "--public-metadata must be full, summary, or inconclusive",
            ));
        }
    };
    Ok(HtmlDocumentMetadata {
        canonical_url: format!("https://antennabench.com{path}"),
        description: description.to_string(),
        social_title: title.to_string(),
        social_image_url: "https://antennabench.com/social-card.png".to_string(),
        social_image_alt: "AntennaBench — better antenna comparisons, evidence included"
            .to_string(),
    })
}
