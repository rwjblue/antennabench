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

#[test]
fn report_templates_preserve_automatic_escaping() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/report");
    let mut templates = Vec::new();
    visit_templates(&root, &mut templates);
    assert!(
        !templates.is_empty(),
        "the report must have compiled templates"
    );

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
    }
}
