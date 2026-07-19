use std::{
    fs, io,
    path::{Path, PathBuf},
};

use antennabench_storage::BundleStore;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::{
    open_session::{
        copy_error_payload, export_session_bundle_at_path, validate_portable_session_at_path,
        SessionErrorKind, SessionErrorPayload,
    },
    setup::{managed_sessions_dir, prepare_managed_sessions_dir, resolved_app_data_dir},
};

use super::{
    catalog::revalidate_available, is_supported_bundle_name, ManagedLocationContext,
    ManagedSessionsState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ImportManagedSessionOutcome {
    Cancelled,
    Imported { location: ManagedLocationContext },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ExportManagedSessionOutcome {
    Cancelled,
    Exported {
        #[serde(rename = "bundleName")]
        bundle_name: String,
        revision: Option<u64>,
    },
}

trait PublishPort {
    fn publish(&self, staging: &Path, destination: &Path) -> io::Result<()>;
}

struct SystemPublishPort;

impl PublishPort for SystemPublishPort {
    fn publish(&self, staging: &Path, destination: &Path) -> io::Result<()> {
        match fs::symlink_metadata(destination) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "destination exists",
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        fs::rename(staging, destination)
    }
}

#[tauri::command]
pub(crate) async fn import_managed_session(
    app: AppHandle,
    state: State<'_, ManagedSessionsState>,
) -> Result<ImportManagedSessionOutcome, SessionErrorPayload> {
    import_managed_session_with_selection(state.inner(), &resolved_app_data_dir(&app)?, || {
        let Some(selection) = app
            .dialog()
            .file()
            .set_title("Import an AntennaBench session into Saved sessions")
            .set_can_create_directories(false)
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        selection.into_path().map(Some).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Selection,
                "The selected session is not available as a local directory.",
                error.to_string(),
            )
        })
    })
}

#[tauri::command]
pub(crate) async fn export_managed_session(
    app: AppHandle,
    state: State<'_, ManagedSessionsState>,
    locator_id: String,
) -> Result<ExportManagedSessionOutcome, SessionErrorPayload> {
    let root = managed_sessions_dir(&resolved_app_data_dir(&app)?);
    export_managed_session_with_selection(state.inner(), &root, &locator_id, |source| {
        let suggested_name = suggested_export_name(source);
        let Some(selection) = app
            .dialog()
            .file()
            .set_title("Export a lossless AntennaBench session bundle copy")
            .set_file_name(suggested_name)
            .set_can_create_directories(true)
            .add_filter(
                "AntennaBench session bundle",
                &["antennabundle", "wsprabundle"],
            )
            .blocking_save_file()
        else {
            return Ok(None);
        };
        selection.into_path().map(Some).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "The selected export destination is not available as a local path.",
                error.to_string(),
            )
        })
    })
}

fn import_managed_session_with_selection<F>(
    state: &ManagedSessionsState,
    app_data_dir: &Path,
    select: F,
) -> Result<ImportManagedSessionOutcome, SessionErrorPayload>
where
    F: FnOnce() -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let Some(source) = select()? else {
        return Ok(ImportManagedSessionOutcome::Cancelled);
    };
    let root = prepare_managed_sessions_dir(app_data_dir)?;
    import_managed_session_for(state, &root, &source, &SystemPublishPort)
}

fn import_managed_session_for(
    state: &ManagedSessionsState,
    root: &Path,
    source: &Path,
    publisher: &dyn PublishPort,
) -> Result<ImportManagedSessionOutcome, SessionErrorPayload> {
    let destination = allocate_import_destination(root, source)?;
    let staging = root.join(format!(
        ".import-staging-{}{}",
        Uuid::new_v4().simple(),
        bundle_suffix_for(source)?
    ));
    let result = (|| {
        BundleStore::new(source)
            .copy_losslessly_to(&staging)
            .map_err(|error| {
                copy_error_payload(error).with_message(
                    "The selected session could not be copied safely into Saved sessions.",
                )
            })?;
        validate_portable_session_at_path(&staging)
            .map_err(|error| redact_paths(error, source, root, Some(&staging)))?;
        publisher.publish(&staging, &destination).map_err(|error| {
            SessionErrorPayload::new(
                if error.kind() == io::ErrorKind::AlreadyExists {
                    SessionErrorKind::Conflict
                } else {
                    SessionErrorKind::Filesystem
                },
                "The verified session could not be published to Saved sessions.",
                error.to_string(),
            )
        })?;
        let location = match state.register_created(root, &destination) {
            Ok(location) => location,
            Err(error) => {
                let _ = fs::remove_dir_all(&destination);
                return Err(error);
            }
        };
        Ok(ImportManagedSessionOutcome::Imported { location })
    })();
    if result.is_err() && staging.exists() {
        fs::remove_dir_all(&staging).map_err(|cleanup| {
            SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "The import failed and its private staging copy could not be removed.",
                cleanup.to_string(),
            )
        })?;
    }
    result.map_err(|error| redact_paths(error, source, root, Some(&staging)))
}

fn export_managed_session_with_selection<F>(
    state: &ManagedSessionsState,
    root: &Path,
    locator_id: &str,
    select: F,
) -> Result<ExportManagedSessionOutcome, SessionErrorPayload>
where
    F: FnOnce(&Path) -> Result<Option<PathBuf>, SessionErrorPayload>,
{
    let record = state.resolve(locator_id)?;
    let source = revalidate_available(root, &record)?;
    let Some(destination) = select(&source)? else {
        return Ok(ExportManagedSessionOutcome::Cancelled);
    };
    let source = revalidate_available(root, &record)?;
    let (bundle_name, revision) = export_session_bundle_at_path(&source, &destination)
        .map_err(|error| redact_paths(error, &source, root, Some(&destination)))?;
    Ok(ExportManagedSessionOutcome::Exported {
        bundle_name,
        revision,
    })
}

fn allocate_import_destination(root: &Path, source: &Path) -> Result<PathBuf, SessionErrorPayload> {
    let source_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| is_supported_bundle_name(name))
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Selection,
                "Choose a .session.antennabundle or .session.wsprabundle directory.",
                "the selected directory name does not use a supported bundle suffix",
            )
        })?;
    let suffix = bundle_suffix_for(source)?;
    let stem = source_name.strip_suffix(suffix).expect("matched suffix");
    for attempt in 1..=10_000 {
        let suffix_number = if attempt == 1 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let destination = root.join(format!("{stem}{suffix_number}{suffix}"));
        if !destination.exists() {
            return Ok(destination);
        }
    }
    Err(SessionErrorPayload::new(
        SessionErrorKind::Destination,
        "A collision-free imported session name could not be allocated.",
        "10,000 managed bundle names were already occupied",
    ))
}

fn bundle_suffix_for(path: &Path) -> Result<&'static str, SessionErrorPayload> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    [
        antennabench_core::v2::V2_BUNDLE_SUFFIX,
        antennabench_core::v2::V1_BUNDLE_SUFFIX,
    ]
    .into_iter()
    .find(|suffix| name.ends_with(suffix))
    .ok_or_else(|| {
        SessionErrorPayload::new(
            SessionErrorKind::Selection,
            "Choose a .session.antennabundle or .session.wsprabundle directory.",
            "the selected directory name does not use a supported bundle suffix",
        )
    })
}

fn suggested_export_name(source: &Path) -> String {
    let suffix = bundle_suffix_for(source).unwrap_or(antennabench_core::v2::V2_BUNDLE_SUFFIX);
    let stem = source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(suffix))
        .unwrap_or("session");
    format!("{stem}-copy{suffix}")
}

fn redact_paths(
    mut error: SessionErrorPayload,
    source: &Path,
    root: &Path,
    other: Option<&Path>,
) -> SessionErrorPayload {
    for (path, label) in [
        (Some(source), "selected session"),
        (Some(root), "Saved sessions"),
        (other, "selected destination"),
    ] {
        if let Some(path) = path.and_then(Path::to_str) {
            error.detail = error.detail.replace(path, label);
        }
    }
    error
}

#[cfg(test)]
mod tests;
