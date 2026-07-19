mod antenna_control;
mod build_context;
#[cfg(test)]
#[path = "../build_identity.rs"]
mod build_identity;
mod conductor;
mod location;
mod managed_sessions;
mod open_session;
mod operation_diagnostics;
mod rbn_import;
mod setup;
mod wsjtx_session;
mod wsjtx_session_record;
mod wspr_live_acquisition;
mod wspr_live_import;

use antenna_control::{
    active_session_antenna_controller, antenna_controller_profiles,
    attach_active_session_antenna_controller, delete_antenna_controller_profile,
    run_active_session_antenna_controller, save_antenna_controller_profile, AntennaControllerState,
};
use conductor::{active_session_conductor, mutate_active_session_conductor, ConductorSessionState};
use location::{request_station_location, LocationState};
use managed_sessions::{
    delete_managed_session, list_managed_sessions, open_managed_session, reveal_managed_session,
    reveal_managed_sessions_directory, ManagedSessionsState,
};
use open_session::{
    active_session_report, export_active_session, export_active_session_report,
    open_session_bundle, refresh_active_session_report, ActiveSessionState,
};
use rbn_import::import_active_session_rbn;
use setup::{
    create_session_from_review, load_station_preferences, review_session_setup, SetupSessionState,
};
use tauri::Manager;
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
        .manage(AntennaControllerState::default())
        .manage(ConductorSessionState::default())
        .manage(LocationState::default())
        .manage(ManagedSessionsState::default())
        .manage(SetupSessionState::default())
        .manage(WsjtxSessionState::default())
        .manage(WsprLiveAcquisitionState::default())
        .invoke_handler(tauri::generate_handler![
            review_session_setup,
            request_station_location,
            load_station_preferences,
            create_session_from_review,
            list_managed_sessions,
            open_managed_session,
            reveal_managed_sessions_directory,
            reveal_managed_session,
            delete_managed_session,
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
            import_active_session_rbn,
            antenna_controller_profiles,
            save_antenna_controller_profile,
            delete_antenna_controller_profile,
            active_session_antenna_controller,
            attach_active_session_antenna_controller,
            run_active_session_antenna_controller
        ])
        .build(tauri::generate_context!())
        .expect("error while building the AntennaBench desktop shell")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                app.state::<AntennaControllerState>().revoke();
            }
        });
}

#[cfg(test)]
mod zz_controller_process_tests {
    #[test]
    fn verification_runs_only_after_a_zero_switch_exit() {
        crate::antenna_control::tests::assert_verification_runs_only_after_a_zero_switch_exit();
    }

    #[test]
    fn fake_process_covers_exit_binary_truncation_timeout_and_spawn_failure() {
        crate::antenna_control::tests::assert_fake_process_covers_exit_binary_truncation_timeout_and_spawn_failure();
    }

    #[test]
    fn cancellation_terminates_the_child_without_claiming_hardware_restoration() {
        crate::antenna_control::tests::assert_cancellation_terminates_the_child_without_claiming_hardware_restoration();
    }
}
