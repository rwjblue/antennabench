use std::{io, path::PathBuf};

use antennabench_core::v2::SessionLifecycleV2;
use thiserror::Error;

use crate::BundleStoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticPersistenceStatusV6 {
    NotApplicable,
    Persisted { diagnostic_id: Option<String> },
    NotPersisted { reason_code: &'static str },
}

#[derive(Debug, Error)]
pub enum LivePersistenceError {
    #[error(transparent)]
    Store(#[from] BundleStoreError),
    #[error("another writer owns the schema-v2 session lock")]
    WriterBusy,
    #[error("live mutation capability is unavailable: {message}")]
    Capability { message: String },
    #[error("expected checkpoint revision {expected}, but current revision is {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("live session requires recovery before mutation: {message}")]
    RecoveryRequired { message: String },
    #[error("external modification froze live mutation: {message}")]
    ExternalModification { message: String },
    #[error("invalid live mutation: {message}")]
    InvalidMutation { message: String },
    #[error("resource limit {code} for {stream}: observed {observed}, limit {limit}")]
    ResourceLimit {
        code: &'static str,
        stream: &'static str,
        observed: u64,
        limit: u64,
    },
    #[error("mutation ID {mutation_id} is already committed with different content")]
    MutationConflict { mutation_id: String },
    #[error("the active plan is frozen in lifecycle {lifecycle:?}")]
    PlanFrozen { lifecycle: SessionLifecycleV2 },
    #[error("live persistence {operation} failed for {path}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("new checkpoint could not be verified: {message}")]
    CheckpointVerification { message: String },
}
