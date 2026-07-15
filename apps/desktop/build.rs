fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "review_session_setup",
        "create_session_from_review",
        "open_session_bundle",
        "export_active_session",
        "active_session_report",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build the AntennaBench desktop application");
}
