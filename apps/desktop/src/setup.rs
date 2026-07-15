use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::{
    validate_bundle_report, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band,
    BundleDiagnostic, BundleDiagnosticSeverity, BundleFileRole, BundleFilesV2, BundleManifestV2,
    BundleV2Contents, BundleValidationProfile, ExperimentMode, PlanGenerationV2, PlannedSlot,
    Schedule, SessionGoal, SessionLifecycleV2, SessionStateV2, Station, SCHEMA_VERSION_V2,
    V2_BUNDLE_SUFFIX,
};
use antennabench_storage::{
    BundleStore, BundleStoreError, LivePersistenceError, LivePersistenceHooks,
    SystemLivePersistenceHooks,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::open_session::{
    activate_created_bundle, with_foreground_operation, ActiveSessionState, OpenedSession,
    SessionErrorKind, SessionErrorPayload,
};

const SETUP_REVIEW_IPC_BYTES: u64 = 512 * 1024;
const SETUP_DRAFT_IPC_BYTES: u64 = 256 * 1024;
const MAX_SETUP_ANTENNAS: usize = 16;
const MAX_SETUP_SLOTS: usize = 256;
const MAX_SETUP_ROUNDS: u32 = 128;

#[derive(Default)]
pub(crate) struct SetupSessionState(Mutex<Option<PendingSetup>>);

#[derive(Clone)]
struct PendingSetup {
    review_id: String,
    bundle: BundleV2Contents,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupDraft {
    station: SetupStationDraft,
    antennas: Vec<SetupAntennaDraft>,
    schedule: SetupScheduleDraft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupStationDraft {
    callsign: String,
    grid: String,
    power_watts: String,
    operator_notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupAntennaDraft {
    label: String,
    facets: String,
    height_m: String,
    radial_count: String,
    radial_length_m: String,
    orientation_degrees: String,
    tuner: String,
    feedline: String,
    notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupScheduleDraft {
    mode: ExperimentMode,
    goal: SessionGoal,
    starts_at: String,
    band: Band,
    duration_seconds: String,
    guard_seconds: String,
    rounds: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupDiagnostic {
    code: String,
    field: String,
    message: String,
    severity: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupReview {
    review_id: Option<String>,
    valid: bool,
    diagnostics: Vec<SetupDiagnostic>,
    plan: Option<SetupPlanReview>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupPlanReview {
    session_id: String,
    created_at: DateTime<Utc>,
    station: SetupStationReview,
    antennas: Vec<SetupAntennaReview>,
    mode: ExperimentMode,
    goal: SessionGoal,
    slots: Vec<SetupSlotReview>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupStationReview {
    callsign: String,
    grid: String,
    power_watts: Option<f32>,
    operator_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupAntennaReview {
    label: String,
    context: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupSlotReview {
    slot_id: String,
    sequence_number: u32,
    starts_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
    duration_seconds: u32,
    guard_seconds: u32,
    band: Band,
    antenna_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum CreateSessionOutcome {
    Cancelled,
    Created { session: OpenedSession },
}

fn field_diagnostic(
    code: impl Into<String>,
    field: impl Into<String>,
    message: impl Into<String>,
) -> SetupDiagnostic {
    SetupDiagnostic {
        code: code.into(),
        field: field.into(),
        message: message.into(),
        severity: "error",
    }
}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn parse_optional<T>(
    value: &str,
    field: &str,
    description: &str,
    diagnostics: &mut Vec<SetupDiagnostic>,
) -> Option<T>
where
    T: std::str::FromStr,
{
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    match value.parse() {
        Ok(value) => Some(value),
        Err(_) => {
            diagnostics.push(field_diagnostic(
                "setup.wire.invalid_number",
                field,
                format!("{description} must be a valid number"),
            ));
            None
        }
    }
}

fn parse_required<T>(
    value: &str,
    field: &str,
    description: &str,
    diagnostics: &mut Vec<SetupDiagnostic>,
) -> Option<T>
where
    T: std::str::FromStr,
{
    if value.trim().is_empty() {
        diagnostics.push(field_diagnostic(
            "setup.wire.required",
            field,
            format!("{description} is required"),
        ));
        return None;
    }
    parse_optional(value, field, description, diagnostics)
}

fn build_review(
    state: &SetupSessionState,
    draft: SetupDraft,
    hooks: &dyn LivePersistenceHooks,
) -> Result<SetupReview, SessionErrorPayload> {
    let draft_bytes = serde_json::to_vec(&draft).map_err(|error| {
        SessionErrorPayload::report_pipeline(format!("setup draft serialization failed: {error}"))
    })?;
    if draft_bytes.len() as u64 > SETUP_DRAFT_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            "setup_draft",
            SETUP_DRAFT_IPC_BYTES,
            Some(draft_bytes.len() as u64),
            "bytes",
        ));
    }
    let mut diagnostics = Vec::new();
    let power_watts = parse_optional(
        &draft.station.power_watts,
        "station.powerWatts",
        "Transmit power",
        &mut diagnostics,
    );

    if draft.antennas.len() > MAX_SETUP_ANTENNAS {
        diagnostics.push(field_diagnostic(
            "setup.resource.too_many_antennas",
            "antennas",
            format!("Setup supports at most {MAX_SETUP_ANTENNAS} antennas"),
        ));
    }
    let minimum_antennas = if draft.schedule.mode == ExperimentMode::SingleAntennaProfiling {
        1
    } else {
        2
    };
    if draft.antennas.len() < minimum_antennas {
        diagnostics.push(field_diagnostic(
            "setup.structure.minimum_antennas",
            "antennas",
            format!("This experiment mode requires at least {minimum_antennas} antenna entries"),
        ));
    }

    let mut antennas = Vec::with_capacity(draft.antennas.len().min(MAX_SETUP_ANTENNAS));
    for (index, antenna) in draft.antennas.iter().take(MAX_SETUP_ANTENNAS).enumerate() {
        antennas.push(Antenna {
            label: antenna.label.trim().to_string(),
            facets: antenna
                .facets
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            height_m: parse_optional(
                &antenna.height_m,
                &format!("antennas.{index}.heightM"),
                "Antenna height",
                &mut diagnostics,
            ),
            radial_count: parse_optional(
                &antenna.radial_count,
                &format!("antennas.{index}.radialCount"),
                "Radial count",
                &mut diagnostics,
            ),
            radial_length_m: parse_optional(
                &antenna.radial_length_m,
                &format!("antennas.{index}.radialLengthM"),
                "Radial length",
                &mut diagnostics,
            ),
            orientation_degrees: parse_optional(
                &antenna.orientation_degrees,
                &format!("antennas.{index}.orientationDegrees"),
                "Orientation",
                &mut diagnostics,
            ),
            tuner: optional_text(&antenna.tuner),
            feedline: optional_text(&antenna.feedline),
            notes: optional_text(&antenna.notes),
        });
    }

    let starts_at = match DateTime::parse_from_rfc3339(draft.schedule.starts_at.trim()) {
        Ok(value) => Some(value.with_timezone(&Utc)),
        Err(_) => {
            diagnostics.push(field_diagnostic(
                "setup.wire.invalid_timestamp",
                "schedule.startsAt",
                "Start time must include an unambiguous UTC offset",
            ));
            None
        }
    };
    let duration_seconds = parse_required::<u32>(
        &draft.schedule.duration_seconds,
        "schedule.durationSeconds",
        "Slot duration",
        &mut diagnostics,
    );
    let guard_seconds = parse_required::<u32>(
        &draft.schedule.guard_seconds,
        "schedule.guardSeconds",
        "Guard time",
        &mut diagnostics,
    );
    let rounds = parse_required::<u32>(
        &draft.schedule.rounds,
        "schedule.rounds",
        "Schedule rounds",
        &mut diagnostics,
    );
    if rounds.is_some_and(|rounds| rounds == 0 || rounds > MAX_SETUP_ROUNDS) {
        diagnostics.push(field_diagnostic(
            "setup.resource.invalid_round_count",
            "schedule.rounds",
            format!("Schedule rounds must be between 1 and {MAX_SETUP_ROUNDS}"),
        ));
    }

    let scheduled_labels = if draft.schedule.mode == ExperimentMode::SingleAntennaProfiling {
        antennas
            .iter()
            .take(1)
            .map(|antenna| &antenna.label)
            .collect()
    } else {
        antennas
            .iter()
            .map(|antenna| &antenna.label)
            .collect::<Vec<_>>()
    };
    let slot_count = rounds
        .and_then(|rounds| usize::try_from(rounds).ok())
        .and_then(|rounds| rounds.checked_mul(scheduled_labels.len()));
    if slot_count.is_some_and(|count| count > MAX_SETUP_SLOTS) {
        diagnostics.push(field_diagnostic(
            "setup.resource.too_many_slots",
            "schedule.rounds",
            format!("The normalized schedule may contain at most {MAX_SETUP_SLOTS} slots"),
        ));
    }

    if !diagnostics.is_empty() {
        *state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("setup review state is unavailable")
        })? = None;
        return Ok(SetupReview {
            review_id: None,
            valid: false,
            diagnostics,
            plan: None,
        });
    }

    let starts_at = starts_at.expect("timestamp parsed without diagnostics");
    let duration_seconds = duration_seconds.expect("duration parsed without diagnostics");
    let guard_seconds = guard_seconds.expect("guard parsed without diagnostics");
    let rounds = rounds.expect("rounds parsed without diagnostics");
    let session_id = hooks.new_id("session");
    let created_at = hooks.now();
    let generation_id = hooks.new_id("plan");
    let review_id = hooks.new_id("review");
    let mut slots = Vec::with_capacity(slot_count.expect("bounded slot count"));
    let mut next_start = starts_at;
    for _ in 0..rounds {
        for antenna_label in &scheduled_labels {
            let sequence_number = u32::try_from(slots.len() + 1).expect("slot count is bounded");
            slots.push(PlannedSlot {
                slot_id: hooks.new_id("slot"),
                sequence_number,
                starts_at: next_start,
                duration_seconds,
                guard_seconds,
                band: draft.schedule.band,
                antenna_label: (*antenna_label).clone(),
            });
            next_start += Duration::seconds(i64::from(duration_seconds));
        }
    }

    let station = Station {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.clone(),
        callsign: draft.station.callsign.trim().to_string(),
        grid: draft.station.grid.trim().to_string(),
        power_watts,
        operator_notes: optional_text(&draft.station.operator_notes),
    };
    let antennas_file = AntennasFile {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.clone(),
        antennas,
    };
    let schedule = Schedule {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.clone(),
        mode: draft.schedule.mode,
        goal: draft.schedule.goal,
        slots,
    };
    let mut bundle = BundleV2Contents {
        manifest: BundleManifestV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: session_id.clone(),
            created_at,
            app_version: env!("CARGO_PKG_VERSION").into(),
            files: BundleFilesV2::default(),
        },
        session_state: SessionStateV2 {
            schema_version: SCHEMA_VERSION_V2,
            session_id: session_id.clone(),
            revision: 0,
            lifecycle: SessionLifecycleV2::Ready,
            active_plan: PlanGenerationV2 {
                generation_id,
                station_sha256: String::new(),
                antennas_sha256: String::new(),
                schedule_sha256: String::new(),
                root_sha256: String::new(),
            },
            streams: BTreeMap::new(),
            last_committed_mutation_id: None,
        },
        station,
        antennas: antennas_file,
        schedule,
        events: Vec::new(),
        observations: Vec::new(),
        adapter_records: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION_V2,
            session_id: session_id.clone(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    };
    BundleStore::refresh_v2_checkpoint(&mut bundle).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The normalized setup plan could not be prepared.",
            error.to_string(),
        )
    })?;

    let report = validate_bundle_report(&bundle.clone().into_current().bundle);
    let core_diagnostics = report
        .diagnostics()
        .iter()
        .map(setup_diagnostic_from_core)
        .collect::<Vec<_>>();
    if !report.allows(BundleValidationProfile::StrictCreation) {
        *state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("setup review state is unavailable")
        })? = None;
        return Ok(SetupReview {
            review_id: None,
            valid: false,
            diagnostics: core_diagnostics,
            plan: None,
        });
    }

    let plan = setup_plan_review(&bundle);
    *state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("setup review state is unavailable"))? =
        Some(PendingSetup {
            review_id: review_id.clone(),
            bundle,
        });
    let review = SetupReview {
        review_id: Some(review_id),
        valid: true,
        diagnostics: core_diagnostics,
        plan: Some(plan),
    };
    check_review_ipc(&review)?;
    Ok(review)
}

fn setup_diagnostic_from_core(diagnostic: &BundleDiagnostic) -> SetupDiagnostic {
    let field = match diagnostic.location.file {
        BundleFileRole::Station => match diagnostic.location.field_path.as_deref() {
            Some("/callsign") => "station.callsign".into(),
            Some("/grid") => "station.grid".into(),
            Some("/power_watts") => "station.powerWatts".into(),
            _ => "station".into(),
        },
        BundleFileRole::Antennas => diagnostic.location.record_index.map_or_else(
            || "antennas".into(),
            |index| {
                let suffix = diagnostic
                    .location
                    .field_path
                    .as_deref()
                    .and_then(|path| path.rsplit('/').next())
                    .map_or(String::new(), |field| {
                        let field = match field {
                            "height_m" => "heightM",
                            "radial_count" => "radialCount",
                            "radial_length_m" => "radialLengthM",
                            "orientation_degrees" => "orientationDegrees",
                            field => field,
                        };
                        format!(".{field}")
                    });
                format!("antennas.{index}{suffix}")
            },
        ),
        BundleFileRole::Schedule => {
            let path = diagnostic
                .location
                .field_path
                .as_deref()
                .unwrap_or_default();
            if path == "/mode" {
                "schedule.mode".into()
            } else if path == "/slots" {
                "schedule.rounds".into()
            } else if path.ends_with("/duration_seconds") {
                "schedule.durationSeconds".into()
            } else if path.ends_with("/guard_seconds") {
                "schedule.guardSeconds".into()
            } else if path.ends_with("/starts_at") {
                "schedule.startsAt".into()
            } else {
                "schedule".into()
            }
        }
        _ => "setup".into(),
    };
    SetupDiagnostic {
        code: diagnostic.code.clone(),
        field,
        message: diagnostic.message.clone(),
        severity: match diagnostic.severity {
            BundleDiagnosticSeverity::Warning => "warning",
            BundleDiagnosticSeverity::Error => "error",
        },
    }
}

fn setup_plan_review(bundle: &BundleV2Contents) -> SetupPlanReview {
    SetupPlanReview {
        session_id: bundle.manifest.session_id.clone(),
        created_at: bundle.manifest.created_at,
        station: SetupStationReview {
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            power_watts: bundle.station.power_watts,
            operator_notes: bundle.station.operator_notes.clone(),
        },
        antennas: bundle
            .antennas
            .antennas
            .iter()
            .map(|antenna| SetupAntennaReview {
                label: antenna.label.clone(),
                context: antenna_context(antenna),
            })
            .collect(),
        mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        slots: bundle
            .schedule
            .slots
            .iter()
            .map(|slot| SetupSlotReview {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                starts_at: slot.starts_at,
                ends_at: slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds)),
                duration_seconds: slot.duration_seconds,
                guard_seconds: slot.guard_seconds,
                band: slot.band,
                antenna_label: slot.antenna_label.clone(),
            })
            .collect(),
    }
}

fn antenna_context(antenna: &Antenna) -> String {
    let mut context = Vec::new();
    if !antenna.facets.is_empty() {
        context.push(antenna.facets.join(", "));
    }
    if let Some(height) = antenna.height_m {
        context.push(format!("{height} m high"));
    }
    if let Some(orientation) = antenna.orientation_degrees {
        context.push(format!("{orientation}° orientation"));
    }
    if let Some(feedline) = &antenna.feedline {
        context.push(format!("feedline: {feedline}"));
    }
    if let Some(notes) = &antenna.notes {
        context.push(notes.clone());
    }
    context.join(" · ")
}

fn check_review_ipc(review: &SetupReview) -> Result<(), SessionErrorPayload> {
    let bytes = serde_json::to_vec(review).map_err(|error| {
        SessionErrorPayload::report_pipeline(format!("setup review serialization failed: {error}"))
    })?;
    if bytes.len() as u64 > SETUP_REVIEW_IPC_BYTES {
        Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.ipc_bytes",
            "setup_review",
            SETUP_REVIEW_IPC_BYTES,
            Some(bytes.len() as u64),
            "bytes",
        ))
    } else {
        Ok(())
    }
}

fn suggested_bundle_name(bundle: &BundleV2Contents) -> String {
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
    format!("{callsign}-session{V2_BUNDLE_SUFFIX}")
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

fn creation_error(error: LivePersistenceError) -> SessionErrorPayload {
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

fn create_with_selection(
    setup_state: &SetupSessionState,
    active_state: &ActiveSessionState,
    review_id: &str,
    select: impl FnOnce(&BundleV2Contents) -> Result<Option<PathBuf>, SessionErrorPayload>,
) -> Result<CreateSessionOutcome, SessionErrorPayload> {
    with_foreground_operation(active_state, || {
        let pending = setup_state
            .0
            .lock()
            .map_err(|_| SessionErrorPayload::report_pipeline("setup review state is unavailable"))?
            .clone()
            .filter(|pending| pending.review_id == review_id)
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "Review the current setup before creating its bundle.",
                    "the supplied setup review is missing or stale",
                )
            })?;
        let Some(destination) = select(&pending.bundle)? else {
            return Ok(CreateSessionOutcome::Cancelled);
        };
        validate_destination(&destination)?;
        BundleStore::new(&destination)
            .create_v2_checkpointed(&pending.bundle)
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

#[tauri::command]
pub(crate) fn review_session_setup(
    state: State<'_, SetupSessionState>,
    draft: SetupDraft,
) -> Result<SetupReview, SessionErrorPayload> {
    build_review(state.inner(), draft, &SystemLivePersistenceHooks)
}

#[tauri::command]
pub(crate) async fn create_session_from_review(
    app: AppHandle,
    setup_state: State<'_, SetupSessionState>,
    active_state: State<'_, ActiveSessionState>,
    review_id: String,
) -> Result<CreateSessionOutcome, SessionErrorPayload> {
    create_with_selection(
        setup_state.inner(),
        active_state.inner(),
        &review_id,
        |bundle| {
            let Some(selection) = app
                .dialog()
                .file()
                .set_title("Create an AntennaBench session bundle")
                .set_file_name(suggested_bundle_name(bundle))
                .set_can_create_directories(true)
                .add_filter("AntennaBench session bundle", &["antennabundle"])
                .blocking_save_file()
            else {
                return Ok(None);
            };
            selection.into_path().map(Some).map_err(|error| {
                SessionErrorPayload::new(
                    SessionErrorKind::Destination,
                    "The selected destination is not available as a local path.",
                    error.to_string(),
                )
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};

    use super::*;

    #[derive(Debug)]
    struct FixedHooks(Mutex<u32>);

    impl FixedHooks {
        fn new() -> Self {
            Self(Mutex::new(1))
        }
    }

    impl LivePersistenceHooks for FixedHooks {
        fn now(&self) -> DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 7, 15, 1, 0, 0).unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.0.lock().unwrap();
            let value = format!("{kind}-{next:04}");
            *next += 1;
            value
        }
    }

    fn valid_draft() -> SetupDraft {
        SetupDraft {
            station: SetupStationDraft {
                callsign: " N1RWJ ".into(),
                grid: " FN42 ".into(),
                power_watts: "5".into(),
                operator_notes: " backyard comparison ".into(),
            },
            antennas: vec![
                SetupAntennaDraft {
                    label: "Vertical".into(),
                    facets: "omnidirectional, ground mounted".into(),
                    height_m: "2.5".into(),
                    radial_count: "16".into(),
                    radial_length_m: "5".into(),
                    orientation_degrees: "".into(),
                    tuner: "none".into(),
                    feedline: "RG-8X".into(),
                    notes: "north lawn".into(),
                },
                SetupAntennaDraft {
                    label: "Dipole".into(),
                    facets: "broadside east-west".into(),
                    height_m: "8".into(),
                    radial_count: "".into(),
                    radial_length_m: "".into(),
                    orientation_degrees: "90".into(),
                    tuner: "internal".into(),
                    feedline: "RG-213".into(),
                    notes: "".into(),
                },
            ],
            schedule: SetupScheduleDraft {
                mode: ExperimentMode::WholeStationAb,
                goal: SessionGoal::GeneralCoverage,
                starts_at: "2026-07-15T02:00:00-04:00".into(),
                band: Band::M20,
                duration_seconds: "120".into(),
                guard_seconds: "10".into(),
                rounds: "2".into(),
            },
        }
    }

    #[test]
    fn review_normalizes_exact_deterministic_bundle_plan() {
        let state = SetupSessionState::default();
        let review = build_review(&state, valid_draft(), &FixedHooks::new()).unwrap();

        assert!(review.valid);
        assert!(review.diagnostics.is_empty());
        assert_eq!(review.review_id.as_deref(), Some("review-0003"));
        let plan = review.plan.unwrap();
        assert_eq!(plan.session_id, "session-0001");
        assert_eq!(plan.station.callsign, "N1RWJ");
        assert_eq!(plan.station.grid, "FN42");
        assert_eq!(plan.slots.len(), 4);
        assert_eq!(plan.slots[0].slot_id, "slot-0004");
        assert_eq!(plan.slots[0].antenna_label, "Vertical");
        assert_eq!(plan.slots[1].antenna_label, "Dipole");
        assert_eq!(
            plan.slots[2].starts_at,
            plan.slots[0].starts_at + Duration::seconds(240)
        );
    }

    #[test]
    fn review_returns_stable_field_diagnostics_without_pending_creation() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.station.callsign = " ".into();
        draft.antennas[1].label = "Vertical".into();
        draft.schedule.guard_seconds = "120".into();

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();

        assert!(!review.valid);
        assert!(review.review_id.is_none());
        assert!(review.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "bundle.semantic.invalid_required_text"
                && diagnostic.field == "station.callsign"
        }));
        assert!(review.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "bundle.semantic.duplicate_antenna_label"
                && diagnostic.field == "antennas.1.label"
        }));
        assert!(review
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "bundle.semantic.invalid_slot_guard"));
        assert!(state.0.lock().unwrap().is_none());
    }

    #[test]
    fn desktop_e2e_reviewed_creation_is_atomic_cancel_safe_and_activates_exact_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let setup_state = SetupSessionState::default();
        let active_state = ActiveSessionState::default();
        let review = build_review(&setup_state, valid_draft(), &FixedHooks::new()).unwrap();
        let review_id = review.review_id.unwrap();

        assert_eq!(
            create_with_selection(&setup_state, &active_state, &review_id, |_| Ok(None)).unwrap(),
            CreateSessionOutcome::Cancelled
        );
        assert!(setup_state.0.lock().unwrap().is_some());

        let destination = temp.path().join(format!("created{V2_BUNDLE_SUFFIX}"));
        let outcome = create_with_selection(&setup_state, &active_state, &review_id, |_| {
            Ok(Some(destination.clone()))
        })
        .unwrap();
        let CreateSessionOutcome::Created { session } = outcome else {
            panic!("reviewed setup should create a session");
        };
        assert_eq!(session.session_id, "session-0001");
        assert_eq!(session.antenna_count, 2);
        assert_eq!(session.slot_count, 4);
        assert!(setup_state.0.lock().unwrap().is_none());
        let persisted = BundleStore::new(destination)
            .read_v2_checkpointed()
            .unwrap();
        assert_eq!(persisted.manifest.session_id, session.session_id);
        assert_eq!(persisted.session_state.lifecycle, SessionLifecycleV2::Ready);
        assert_eq!(persisted.schedule.slots[0].slot_id, "slot-0004");
        println!(
            "desktop-e2e result=setup-created revision={} slots={}",
            persisted.session_state.revision,
            persisted.schedule.slots.len()
        );
    }

    #[test]
    fn stale_review_and_existing_destination_fail_without_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let setup_state = SetupSessionState::default();
        let active_state = ActiveSessionState::default();
        let hooks = FixedHooks::new();
        let first = build_review(&setup_state, valid_draft(), &hooks).unwrap();
        let second = build_review(&setup_state, valid_draft(), &hooks).unwrap();
        assert!(create_with_selection(
            &setup_state,
            &active_state,
            first.review_id.as_deref().unwrap(),
            |_| unreachable!()
        )
        .is_err());

        let destination = temp.path().join(format!("existing{V2_BUNDLE_SUFFIX}"));
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("owner.txt"), b"keep").unwrap();
        assert!(create_with_selection(
            &setup_state,
            &active_state,
            second.review_id.as_deref().unwrap(),
            |_| Ok(Some(destination.clone()))
        )
        .is_err());
        assert_eq!(
            std::fs::read(destination.join("owner.txt")).unwrap(),
            b"keep"
        );
        assert!(setup_state.0.lock().unwrap().is_some());
    }
}
