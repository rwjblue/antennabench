use std::path::Path;

use antennabench_report::{build_report, render_standalone_html};
use antennabench_storage::BundleStore;

#[test]
fn upgraded_v2_fixtures_analyze_and_render_through_the_shared_projection() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles");
    let mut sources = std::fs::read_dir(fixtures)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".session.wsprabundle"))
        .collect::<Vec<_>>();
    sources.sort();

    for source in sources {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("upgraded.session.antennabundle");
        let store = BundleStore::new(&source)
            .upgrade_v1_to_v2(destination)
            .unwrap();
        let bundle = store.read_normalized_validated().unwrap();
        let report = build_report(&bundle).unwrap();
        let html = render_standalone_html(&report).unwrap();
        assert_eq!(report.context.session_id, bundle.manifest.session_id);
        assert!(html.contains(&bundle.manifest.session_id));
    }
}
