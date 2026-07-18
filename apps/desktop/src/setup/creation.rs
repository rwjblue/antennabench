use std::path::{Path, PathBuf};

use antennabench_core::{v2::V2_BUNDLE_SUFFIX, v3::BundleV3Contents};
use antennabench_storage::{BundleStore, BundleStoreError, LivePersistenceError};

use super::{
    preferences::station_preferences, CreateSessionOutcome, PendingSetup, PreparedSetupController,
    SessionErrorKind, SessionErrorPayload, SetupSessionState, StationPreferences,
};
use crate::open_session::{activate_created_bundle, with_foreground_operation, ActiveSessionState};

/// Owns the exact-review handoff, checkpoint publication, activation, and
/// externally stable failure classification for committed setup creation.
pub(super) fn reviewed_station_preferences(
    state: &SetupSessionState,
    review_id: &str,
) -> Result<StationPreferences, SessionErrorPayload> {
    reviewed_pending(state, review_id).map(|pending| station_preferences(&pending.bundle))
}

pub(super) fn reviewed_setup_controller(
    state: &SetupSessionState,
    review_id: &str,
) -> Result<Option<PreparedSetupController>, SessionErrorPayload> {
    reviewed_pending(state, review_id).map(|pending| pending.controller)
}

pub(super) fn create_with_selection(
    setup_state: &SetupSessionState,
    active_state: &ActiveSessionState,
    review_id: &str,
    select: impl FnOnce(&BundleV3Contents) -> Result<Option<PathBuf>, SessionErrorPayload>,
) -> Result<CreateSessionOutcome, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let pending = reviewed_pending(setup_state, review_id)?;
        let Some(destination) = select(&pending.bundle)? else {
            return Ok(CreateSessionOutcome::Cancelled);
        };
        validate_destination(&destination)?;
        BundleStore::new(&destination)
            .create_v3_checkpointed(&pending.bundle)
            .map_err(creation_error)?;
        let session = activate_created_bundle(active_state, destination)?;
        let mut reviewed = setup_state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("setup review state is unavailable")
        })?;
        if reviewed
            .as_ref()
            .is_some_and(|current| current.review_id == review_id)
        {
            *reviewed = None;
        }
        Ok(CreateSessionOutcome::Created { session })
    })
}

fn reviewed_pending(
    state: &SetupSessionState,
    review_id: &str,
) -> Result<PendingSetup, SessionErrorPayload> {
    state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("setup review state is unavailable"))?
        .clone()
        .filter(|pending| pending.review_id == review_id)
        .ok_or_else(stale_review_error)
}

fn validate_destination(path: &Path) -> Result<(), SessionErrorPayload> {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(V2_BUNDLE_SUFFIX))
    {
        Ok(())
    } else {
        Err(SessionErrorPayload::new(
            SessionErrorKind::Destination,
            "Keep the new session's .session.antennabundle suffix.",
            path.display().to_string(),
        ))
    }
}

pub(super) fn creation_error(error: LivePersistenceError) -> SessionErrorPayload {
    match error {
        LivePersistenceError::Store(BundleStoreError::DestinationExists { path }) => {
            SessionErrorPayload::new(
                SessionErrorKind::Destination,
                "A file or directory already exists at that destination.",
                path.display().to_string(),
            )
        }
        LivePersistenceError::Store(BundleStoreError::Validation { source }) => {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The reviewed session no longer passes strict creation validation.",
                source.to_string(),
            )
        }
        LivePersistenceError::Capability { message } => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The selected filesystem cannot provide durable live sessions.",
            message,
        ),
        LivePersistenceError::WriterBusy => SessionErrorPayload::new(
            SessionErrorKind::Busy,
            "The new session is already in use.",
            "another writer owns the session lock",
        ),
        error => SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The new session bundle could not be created.",
            error.to_string(),
        ),
    }
}

fn stale_review_error() -> SessionErrorPayload {
    SessionErrorPayload::new(
        SessionErrorKind::Validation,
        "Review the current setup before creating its bundle.",
        "the supplied setup review is missing or stale",
    )
}
