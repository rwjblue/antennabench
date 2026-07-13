fn main() {
    let app_manifest =
        tauri_build::AppManifest::new().commands(&["open_session_bundle", "active_session_report"]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build the AntennaBench desktop application");
}
