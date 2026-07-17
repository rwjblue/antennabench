use std::{
    fs,
    path::{Path, PathBuf},
};

use antennabench_core::{BundleV3Contents, V2_BUNDLE_SUFFIX};
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
    let sessions_dir = app_data_dir.join("sessions");
    fs::create_dir_all(&sessions_dir).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The AntennaBench sessions directory could not be prepared.",
            format!("{}: {error}", sessions_dir.display()),
        )
    })?;
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
        sessions_dir.display().to_string(),
    ))
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

pub(super) fn resolved_app_data_dir(app: &AppHandle) -> Result<PathBuf, SessionErrorPayload> {
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
