use std::{
    fs,
    path::{Path, PathBuf},
};

use antennabench_core::{v2::V2_BUNDLE_SUFFIX, v3::BundleV3Contents};
use tauri::{AppHandle, Manager};

use super::{SessionErrorKind, SessionErrorPayload, StationPreferences};

/// Owns the private, non-session station form projection and app-data naming.
pub(super) fn station_preferences(bundle: &BundleV3Contents) -> StationPreferences {
    StationPreferences {
        callsign: bundle.station.callsign.clone(),
        grid: bundle.station.grid.clone(),
        power_watts: bundle.station.power_watts.map(|value| value.to_string()),
        operator_notes: bundle.station.operator_notes.clone(),
    }
}

pub(super) fn automatic_session_destination(
    app_data_dir: &Path,
    bundle: &BundleV3Contents,
) -> Result<PathBuf, SessionErrorPayload> {
    let sessions_dir = prepare_managed_sessions_dir(app_data_dir)?;
    let timestamp = bundle.manifest.created_at.format("%Y%m%dT%H%M%SZ");
    let base = format!("{}-{timestamp}", safe_callsign(bundle));
    for attempt in 1..=10_000 {
        let suffix = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let destination = sessions_dir.join(format!("{base}{suffix}{V2_BUNDLE_SUFFIX}"));
        if !destination.exists() {
            return Ok(destination);
        }
    }
    Err(SessionErrorPayload::new(
        SessionErrorKind::Destination,
        "A collision-free session name could not be allocated.",
        "10,000 managed bundle names were already occupied for this callsign and timestamp",
    ))
}

pub(crate) fn managed_sessions_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("sessions")
}

pub(crate) fn prepare_managed_sessions_dir(
    app_data_dir: &Path,
) -> Result<PathBuf, SessionErrorPayload> {
    let sessions_dir = managed_sessions_dir(app_data_dir);
    match fs::symlink_metadata(&sessions_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The AntennaBench sessions directory is not a safe local directory.",
                "the managed sessions entry cannot be a symlink or non-directory",
            ));
        }
        Ok(_) => return Ok(sessions_dir),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The AntennaBench sessions directory could not be inspected.",
                error.to_string(),
            ));
        }
    }
    fs::create_dir_all(&sessions_dir).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The AntennaBench sessions directory could not be prepared.",
            error.to_string(),
        )
    })?;
    Ok(sessions_dir)
}

pub(super) fn read_station_preferences(
    app_data_dir: &Path,
) -> Result<Option<StationPreferences>, SessionErrorPayload> {
    let path = station_preferences_path(app_data_dir);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "Saved station details could not be read.",
                format!("{}: {error}", path.display()),
            ))
        }
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Saved station details are not valid.",
            format!("{}: {error}", path.display()),
        )
    })
}

pub(super) fn write_station_preferences(
    app_data_dir: &Path,
    preferences: &StationPreferences,
) -> Result<(), SessionErrorPayload> {
    fs::create_dir_all(app_data_dir).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "Saved station details could not be prepared.",
            format!("{}: {error}", app_data_dir.display()),
        )
    })?;
    let path = station_preferences_path(app_data_dir);
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| {
        SessionErrorPayload::report_pipeline(format!(
            "station preferences serialization failed: {error}"
        ))
    })?;
    fs::write(&path, bytes).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "Saved station details could not be updated.",
            format!("{}: {error}", path.display()),
        )
    })
}

pub(crate) fn resolved_app_data_dir(app: &AppHandle) -> Result<PathBuf, SessionErrorPayload> {
    app.path().app_data_dir().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The system application-data directory is unavailable.",
            error.to_string(),
        )
    })
}

fn safe_callsign(bundle: &BundleV3Contents) -> String {
    let callsign = bundle
        .station
        .callsign
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    callsign.trim_matches('-').to_string()
}

fn station_preferences_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("station-preferences.json")
}
