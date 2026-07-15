fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "review_session_setup",
        "create_session_from_review",
        "open_session_bundle",
        "export_active_session",
        "export_active_session_report",
        "active_session_report",
        "refresh_active_session_report",
        "active_session_conductor",
        "mutate_active_session_conductor",
        "active_session_wsjtx_status",
        "start_active_session_wsjtx",
        "stop_active_session_wsjtx",
        "import_active_session_wspr_live",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build the AntennaBench desktop application");
}
