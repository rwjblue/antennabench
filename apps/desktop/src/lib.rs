mod open_session;

use open_session::{active_session_report, open_session_bundle, ActiveSessionState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveSessionState::default())
        .invoke_handler(tauri::generate_handler![
            open_session_bundle,
            active_session_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running the AntennaBench desktop shell");
}
