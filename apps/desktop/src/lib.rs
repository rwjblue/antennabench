mod conductor;
mod location;
mod open_session;
mod rbn_import;
mod setup;
mod wsjtx_session;
mod wspr_live_acquisition;
mod wspr_live_import;

use conductor::{active_session_conductor, mutate_active_session_conductor, ConductorSessionState};
use location::{request_station_location, LocationState};
use open_session::{
    active_session_report, export_active_session, export_active_session_report,
    open_session_bundle, refresh_active_session_report, ActiveSessionState,
};
use rbn_import::import_active_session_rbn;
use setup::{
    create_session_from_review, load_station_preferences, review_session_setup, SetupSessionState,
};
use wsjtx_session::{
    active_session_wsjtx_status, start_active_session_wsjtx, stop_active_session_wsjtx,
    WsjtxSessionState,
};
use wspr_live_acquisition::{advance_active_session_wspr_live, WsprLiveAcquisitionState};
use wspr_live_import::import_active_session_wspr_live;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ActiveSessionState::default())
        .manage(ConductorSessionState::default())
        .manage(LocationState::default())
        .manage(SetupSessionState::default())
        .manage(WsjtxSessionState::default())
        .manage(WsprLiveAcquisitionState::default())
        .invoke_handler(tauri::generate_handler![
            review_session_setup,
            request_station_location,
            load_station_preferences,
            create_session_from_review,
            open_session_bundle,
            export_active_session,
            export_active_session_report,
            active_session_report,
            refresh_active_session_report,
            active_session_conductor,
            mutate_active_session_conductor,
            active_session_wsjtx_status,
            start_active_session_wsjtx,
            stop_active_session_wsjtx,
            advance_active_session_wspr_live,
            import_active_session_wspr_live,
            import_active_session_rbn
        ])
        .run(tauri::generate_context!())
        .expect("error while running the AntennaBench desktop shell");
}
