use std::{fs, path::Path};

fn visit_templates(path: &Path, templates: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("read template directory") {
        let entry = entry.expect("read template entry");
        let path = entry.path();
        if path.is_dir() {
            visit_templates(&path, templates);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "html")
        {
            templates.push(path);
        }
    }
}

fn visit_rust_sources(path: &Path, sources: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).expect("read renderer source directory") {
        let entry = entry.expect("read renderer source entry");
        let path = entry.path();
        if path.is_dir() {
            visit_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

#[test]
fn report_templates_preserve_automatic_escaping() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("templates/report");
    let mut templates = Vec::new();
    visit_templates(&root, &mut templates);
    assert!(
        !templates.is_empty(),
        "the report must have compiled templates"
    );

    let mut rust_sources = Vec::new();
    visit_rust_sources(&manifest.join("src/html"), &mut rust_sources);
    let compiled_templates = rust_sources
        .iter()
        .map(|path| fs::read_to_string(path).expect("read renderer source"))
        .collect::<Vec<_>>()
        .join("\n");

    for template in templates {
        let source = fs::read_to_string(&template).expect("read report template");
        assert!(
            !source.contains("|safe") && !source.contains("| safe"),
            "{} disables escaping with the safe filter",
            template.display()
        );
        assert!(
            !source.contains("autoescape"),
            "{} changes the automatic escaping policy",
            template.display()
        );
        assert!(
            !source.to_ascii_lowercase().contains("<script"),
            "{} introduces executable script markup",
            template.display()
        );
        assert!(
            !source.contains(" style="),
            "{} introduces an inline style attribute",
            template.display()
        );
        let relative = template
            .strip_prefix(manifest.join("templates"))
            .expect("template lives below crate template root")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            compiled_templates.contains(&format!("path = \"{relative}\"")),
            "{} is not referenced by a compiled Askama type",
            template.display()
        );
    }

    assert!(
        !compiled_templates.contains("escape = \"none\"")
            && !compiled_templates.contains("escape=\"none\""),
        "renderer templates may not disable Askama escaping"
    );
}

#[test]
fn report_renderer_has_no_legacy_markup_assembly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/html");
    let mut sources = Vec::new();
    visit_rust_sources(&root, &mut sources);

    for path in sources {
        let source = fs::read_to_string(&path).expect("read renderer source");
        assert!(
            !source.contains("write_html"),
            "{} retains the legacy HTML assembly macro",
            path.display()
        );
        assert!(
            !source.contains("escape_html"),
            "{} retains manual report-data escaping",
            path.display()
        );
        for (line_number, line) in source.lines().enumerate() {
            if !line.contains("push_str(") {
                continue;
            }
            let allowed = match path.file_name().and_then(|name| name.to_str()) {
                Some("shared.rs") => {
                    line.contains("fn push_str")
                        || line.contains("self.output.push_str(value)")
                        || line.contains("self.push_str(value)")
                }
                Some("templates.rs") => line.contains("self.writer.push_str(value)"),
                Some("styles.rs") => line.contains("out.push_str("),
                _ => false,
            };
            assert!(
                allowed,
                "{}:{} contains a direct write outside the checked sink or trusted CSS/geometry allowlist: {}",
                path.display(),
                line_number + 1,
                line.trim()
            );
        }
    }
}

#[test]
fn report_css_is_compile_time_authored_source() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let styles = fs::read_to_string(manifest.join("src/html/styles.rs"))
        .expect("read canonical stylesheet assembly");

    for name in [
        "report.css",
        "coverage.css",
        "compact.css",
        "compact-small-print.css",
    ] {
        assert!(
            styles.contains(&format!("include_str!(\"../../styles/{name}\")")),
            "{name} must be embedded at compile time"
        );
        let source = fs::read_to_string(manifest.join("styles").join(name))
            .expect("read authored stylesheet");
        assert!(!source.contains('\r'), "{name} must use LF line endings");
        assert!(!source.contains("@import"), "{name} must not import CSS");
        assert!(!source.contains("url("), "{name} must not load assets");
    }

    assert!(
        !styles.contains("fs::") && !styles.contains("std::fs"),
        "stylesheet assembly must not read runtime files"
    );
    assert!(
        !styles.contains("r#\""),
        "authored stylesheet declarations must not return to Rust raw strings"
    );
}
