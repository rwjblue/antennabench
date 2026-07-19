fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(&[
        "review_session_setup",
        "request_station_location",
        "load_station_preferences",
        "create_session_from_review",
        "list_managed_sessions",
        "open_managed_session",
        "reveal_managed_sessions_directory",
        "reveal_managed_session",
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
        "advance_active_session_wspr_live",
        "import_active_session_wspr_live",
        "import_active_session_rbn",
        "antenna_controller_profiles",
        "save_antenna_controller_profile",
        "active_session_antenna_controller",
        "attach_active_session_antenna_controller",
        "run_active_session_antenna_controller",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app_manifest))
        .expect("failed to build the AntennaBench desktop application");
}
