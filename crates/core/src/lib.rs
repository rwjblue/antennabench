pub mod alignment;
mod diagnostics;
mod model;
pub mod normalization;
mod operator_events;
mod semantics;
pub mod v2;
pub mod v3;
#[path = "v5_antenna_control.rs"]
pub mod v5;
pub mod v6;
mod validation;
mod wspr;
mod wspr_live_projection;

pub use alignment::{
    align_schedule_slots, apply_slot_assignments, AlignedSlot, AlignedSlotStatus,
    ObservationSlotAssignment, ScheduleSlotAlignment, SlotAlignmentPolicy, SlotAssignmentReason,
};
pub use diagnostics::{
    codes, BundleDiagnostic, BundleDiagnosticCategory, BundleDiagnosticLocation,
    BundleDiagnosticSeverity, BundleRecordKind, BundleValidationProfile, BundleValidationReport,
    ALL_TYPED_OPERATIONS, ANALYSIS_AND_WRITE_OPERATIONS, WRITE_OPERATIONS,
};
pub use model::{
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents, BundleFiles,
    BundleManifest, ExperimentMode, ObservationKind, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, PropagationRecord, RecordMeta, RecordSource, RigRecord,
    Schedule, SessionGoal, Station, WsjtXRecord,
};
pub use normalization::{annotate_bundle_observations, normalize_bundle};
pub use semantics::{
    validate_machine_identity, MachineIdentityError, ANTENNA_LABEL_MAX_BYTES, MACHINE_ID_MAX_BYTES,
};
pub use validation::{
    validate_bundle, validate_bundle_report, AlignmentAnnotationField, BundleFileRole,
    BundleIdKind, BundleValidationError, BundleValidationIssue,
};
pub use wspr::{
    is_wspr_cycle_start, next_wspr_cycle_after_ready, next_wspr_cycle_at_or_after,
    WsprCycleTimingError, WsprCycleWindow, WSPR_CYCLE_SECONDS, WSPR_NOMINAL_START_OFFSET_SECONDS,
    WSPR_SYMBOL_COUNT, WSPR_SYMBOL_DURATION_DENOMINATOR, WSPR_SYMBOL_DURATION_NUMERATOR,
    WSPR_TRANSMISSION_MILLISECONDS,
};

// Keep implementation modules decoupled while the public API requires callers
// to name the version owner explicitly.
pub(crate) use operator_events::*;
pub(crate) use semantics::semantic_diagnostics;
pub(crate) use v2::*;
pub(crate) use v3::*;

/// Schema used by legacy adapter APIs.
pub const SCHEMA_VERSION: u16 = SCHEMA_VERSION_V1;
pub const SCHEMA_VERSION_V1: u16 = 1;
pub const SCHEMA_VERSION_V2: u16 = 2;
pub const SCHEMA_VERSION_V3: u16 = 3;
pub const SCHEMA_VERSION_V4: u16 = 4;
pub const SCHEMA_VERSION_V5: u16 = 5;
pub const SCHEMA_VERSION_V6: u16 = 6;
pub const LATEST_SCHEMA_VERSION: u16 = SCHEMA_VERSION_V6;
