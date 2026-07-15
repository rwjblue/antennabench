mod conductor;
mod open_session;
mod setup;

use conductor::{active_session_conductor, mutate_active_session_conductor, ConductorSessionState};
use open_session::{
    active_session_report, export_active_session, open_session_bundle, ActiveSessionState,
};
use setup::{create_session_from_review, review_session_setup, SetupSessionState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveSessionState::default())
        .manage(ConductorSessionState::default())
        .manage(SetupSessionState::default())
        .invoke_handler(tauri::generate_handler![
            review_session_setup,
            create_session_from_review,
            open_session_bundle,
            export_active_session,
            active_session_report,
            active_session_conductor,
            mutate_active_session_conductor
        ])
        .run(tauri::generate_context!())
        .expect("error while running the AntennaBench desktop shell");
}
