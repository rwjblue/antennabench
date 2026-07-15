mod conductor;
mod open_session;
mod setup;
mod wsjtx_session;

use conductor::{active_session_conductor, mutate_active_session_conductor, ConductorSessionState};
use open_session::{
    active_session_report, export_active_session, open_session_bundle, ActiveSessionState,
};
use setup::{create_session_from_review, review_session_setup, SetupSessionState};
use wsjtx_session::{
    active_session_wsjtx_status, start_active_session_wsjtx, stop_active_session_wsjtx,
    WsjtxSessionState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveSessionState::default())
        .manage(ConductorSessionState::default())
        .manage(SetupSessionState::default())
        .manage(WsjtxSessionState::default())
        .invoke_handler(tauri::generate_handler![
            review_session_setup,
            create_session_from_review,
            open_session_bundle,
            export_active_session,
            active_session_report,
            active_session_conductor,
            mutate_active_session_conductor,
            active_session_wsjtx_status,
            start_active_session_wsjtx,
            stop_active_session_wsjtx
        ])
        .run(tauri::generate_context!())
        .expect("error while running the AntennaBench desktop shell");
}
