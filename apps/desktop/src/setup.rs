use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Mutex,
};

use antennabench_core::{
    upgrade_v2_bundle_model, validate_bundle_report, validate_signal_plan_schedule_v3,
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleDiagnostic,
    BundleDiagnosticSeverity, BundleFileRole, BundleFilesV2, BundleManifestV2, BundleV2Contents,
    BundleV3Contents, BundleValidationProfile, CounterbalanceBlockIdV3, ExperimentMode,
    PlanGenerationV2, PlannedSlot, PlannedSlotV3, Schedule, SessionGoal, SessionLifecycleV2,
    SessionStateV2, SignalAllocationV3, SignalCadenceV3, SignalCollectionProfileV3, SignalModeV3,
    SignalPlanIdV3, SignalPlanV3, SignalVariantIdV3, Station, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3,
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
use crate::wsjtx_session::WsjtxSessionState;

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
    bundle: PendingBundle,
}

#[derive(Clone)]
enum PendingBundle {
    V2(BundleV2Contents),
    V3(BundleV3Contents),
}

impl PendingBundle {
    fn callsign(&self) -> &str {
        match self {
            Self::V2(bundle) => &bundle.station.callsign,
            Self::V3(bundle) => &bundle.station.callsign,
        }
    }

    #[cfg(test)]
    fn wspr_live_acquisition_enabled(&self) -> bool {
        match self {
            Self::V2(bundle) => bundle.session_state.wspr_live_acquisition_enabled,
            Self::V3(bundle) => bundle.session_state.wspr_live_acquisition_enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupDraft {
    station: SetupStationDraft,
    antennas: Vec<SetupAntennaDraft>,
    schedule: SetupScheduleDraft,
    #[serde(default)]
    wspr_live_acquisition_enabled: bool,
    #[serde(default)]
    signal_plan: Option<SetupSignalPlanDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupSignalPlanDraft {
    mode: SignalModeV3,
    collection_profile: SignalCollectionProfileV3,
    planned_power_watts: String,
    transmitted_callsign: String,
    differing_identity_validated: bool,
    message: String,
    repetition_count: String,
    key_speed_wpm: String,
    transmit_seconds: String,
    interval_seconds: String,
    frequencies_hz: String,
}

struct ParsedSignalPlan {
    mode: SignalModeV3,
    collection_profile: SignalCollectionProfileV3,
    planned_power_watts: Option<f32>,
    transmitted_callsign: String,
    differing_identity_validated: bool,
    message: String,
    repetition_count: u16,
    key_speed_wpm: Option<u16>,
    transmit_seconds: u32,
    interval_seconds: u32,
    frequencies_hz: Vec<u64>,
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
    schema_version: u16,
    session_id: String,
    created_at: DateTime<Utc>,
    station: SetupStationReview,
    antennas: Vec<SetupAntennaReview>,
    mode: ExperimentMode,
    goal: SessionGoal,
    wspr_live_acquisition_enabled: bool,
    signal_plan: Option<SetupSignalPlanReview>,
    slots: Vec<SetupSlotReview>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupSignalPlanReview {
    mode: SignalModeV3,
    collection_profile: SignalCollectionProfileV3,
    planned_power_watts: Option<f32>,
    transmitted_callsign: String,
    message: String,
    repetition_count: u16,
    key_speed_wpm: Option<u16>,
    transmit_seconds: u32,
    interval_seconds: u32,
    frequencies_hz: Vec<u64>,
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
    signal: Option<SetupSlotSignalReview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupSlotSignalReview {
    frequency_hz: u64,
    frequency_variant_id: String,
    counterbalance_block_id: String,
    counterbalance_position: u16,
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

fn parse_signal_plan(
    draft: &SetupSignalPlanDraft,
    diagnostics: &mut Vec<SetupDiagnostic>,
) -> Option<ParsedSignalPlan> {
    let planned_power_watts = parse_optional(
        &draft.planned_power_watts,
        "signalPlan.plannedPowerWatts",
        "Planned signal power",
        diagnostics,
    );
    let repetition_count = parse_required(
        &draft.repetition_count,
        "signalPlan.repetitionCount",
        "Cadence repetition count",
        diagnostics,
    );
    let key_speed_wpm = parse_optional(
        &draft.key_speed_wpm,
        "signalPlan.keySpeedWpm",
        "CW key speed",
        diagnostics,
    );
    let transmit_seconds = parse_required(
        &draft.transmit_seconds,
        "signalPlan.transmitSeconds",
        "Transmit duration",
        diagnostics,
    );
    let interval_seconds = parse_required(
        &draft.interval_seconds,
        "signalPlan.intervalSeconds",
        "Cadence interval",
        diagnostics,
    );
    let transmitted_callsign = draft.transmitted_callsign.trim().to_string();
    if transmitted_callsign.is_empty() {
        diagnostics.push(field_diagnostic(
            "setup.wire.required",
            "signalPlan.transmittedCallsign",
            "Exact transmitted callsign is required",
        ));
    }
    let message = draft.message.trim().to_string();
    if message.is_empty() {
        diagnostics.push(field_diagnostic(
            "setup.wire.required",
            "signalPlan.message",
            "The transmitted message is required",
        ));
    }
    let mut frequencies_hz = Vec::new();
    for (index, value) in draft.frequencies_hz.split(',').enumerate() {
        let value = value.trim();
        match value.parse::<u64>() {
            Ok(frequency) if frequency > 0 => frequencies_hz.push(frequency),
            _ => diagnostics.push(field_diagnostic(
                "setup.wire.invalid_frequency",
                "signalPlan.frequenciesHz",
                format!(
                    "Frequency {} must be a positive integer in hertz",
                    index + 1
                ),
            )),
        }
    }
    frequencies_hz.sort_unstable();
    frequencies_hz.dedup();
    if frequencies_hz.is_empty() {
        diagnostics.push(field_diagnostic(
            "setup.wire.required",
            "signalPlan.frequenciesHz",
            "At least one exact frequency is required",
        ));
    }
    if frequencies_hz.len() > 32 {
        diagnostics.push(field_diagnostic(
            "setup.resource.too_many_frequencies",
            "signalPlan.frequenciesHz",
            "At most 32 exact frequency variants are supported",
        ));
    }
    if draft.mode == SignalModeV3::Cw && key_speed_wpm.is_none() {
        diagnostics.push(field_diagnostic(
            "setup.wire.required",
            "signalPlan.keySpeedWpm",
            "CW plans require a key speed",
        ));
    }
    Some(ParsedSignalPlan {
        mode: draft.mode,
        collection_profile: draft.collection_profile,
        planned_power_watts,
        transmitted_callsign,
        differing_identity_validated: draft.differing_identity_validated,
        message,
        repetition_count: repetition_count?,
        key_speed_wpm,
        transmit_seconds: transmit_seconds?,
        interval_seconds: interval_seconds?,
        frequencies_hz,
    })
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
    let signal_plan = draft
        .signal_plan
        .as_ref()
        .and_then(|plan| parse_signal_plan(plan, &mut diagnostics));
    if signal_plan.is_some() && draft.wspr_live_acquisition_enabled {
        diagnostics.push(field_diagnostic(
            "setup.signal_plan.wspr_live_conflict",
            "wsprLiveAcquisitionEnabled",
            "WSPR.live acquisition applies only to WSPR sessions, not controlled CW/RTTY plans",
        ));
    }
    if signal_plan.is_some() && rounds.is_some_and(|rounds| rounds % 2 != 0) {
        diagnostics.push(field_diagnostic(
            "setup.signal_plan.unbalanced_rounds",
            "schedule.rounds",
            "Controlled signal plans require an even number of counterbalance blocks",
        ));
    }
    if matches!(
        (signal_plan.as_ref(), duration_seconds),
        (Some(plan), Some(duration)) if plan.transmit_seconds > duration
    ) {
        diagnostics.push(field_diagnostic(
            "setup.signal_plan.transmit_exceeds_slot",
            "signalPlan.transmitSeconds",
            "Transmit duration cannot exceed the schedule slot duration",
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
        .and_then(|rounds| rounds.checked_mul(scheduled_labels.len()))
        .and_then(|count| {
            count.checked_mul(
                signal_plan
                    .as_ref()
                    .map_or(1, |plan| plan.frequencies_hz.len()),
            )
        });
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
    let scheduled_label_values = scheduled_labels
        .iter()
        .map(|label| (*label).clone())
        .collect::<Vec<_>>();
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
            wspr_live_acquisition_enabled: draft.wspr_live_acquisition_enabled,
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

    if let Some(signal_plan) = signal_plan {
        let mut bundle = build_v3_setup_bundle(
            bundle,
            signal_plan,
            &scheduled_label_values,
            starts_at,
            duration_seconds,
            guard_seconds,
            rounds,
            draft.schedule.band,
            hooks,
        )?;
        BundleStore::refresh_v3_checkpoint(&mut bundle).map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The normalized schema-v3 setup plan could not be prepared.",
                error.to_string(),
            )
        })?;
        let report = validate_bundle_report(&bundle.clone().into_current().bundle);
        let mut core_diagnostics = report
            .diagnostics()
            .iter()
            .map(setup_diagnostic_from_core)
            .collect::<Vec<_>>();
        core_diagnostics.extend(
            validate_signal_plan_schedule_v3(&bundle.station.callsign, &bundle.schedule)
                .into_iter()
                .map(|diagnostic| SetupDiagnostic {
                    code: diagnostic.code.into(),
                    field: if diagnostic.path.starts_with("/signal_plans") {
                        "signalPlan".into()
                    } else {
                        "schedule".into()
                    },
                    message: diagnostic.message,
                    severity: "error",
                }),
        );
        if !report.allows(BundleValidationProfile::StrictCreation)
            || core_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == "error")
        {
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
        let plan = setup_plan_review_v3(&bundle);
        *state.0.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("setup review state is unavailable")
        })? = Some(PendingSetup {
            review_id: review_id.clone(),
            bundle: PendingBundle::V3(bundle),
        });
        let review = SetupReview {
            review_id: Some(review_id),
            valid: true,
            diagnostics: core_diagnostics,
            plan: Some(plan),
        };
        check_review_ipc(&review)?;
        return Ok(review);
    }

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
            bundle: PendingBundle::V2(bundle),
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

#[allow(clippy::too_many_arguments)]
fn build_v3_setup_bundle(
    bundle: BundleV2Contents,
    plan: ParsedSignalPlan,
    antenna_labels: &[String],
    starts_at: DateTime<Utc>,
    duration_seconds: u32,
    guard_seconds: u32,
    blocks: u32,
    band: Band,
    hooks: &dyn LivePersistenceHooks,
) -> Result<BundleV3Contents, SessionErrorPayload> {
    let mut bundle = upgrade_v2_bundle_model(bundle);
    let signal_plan_id = SignalPlanIdV3::new("primary").expect("fixed plan identity is valid");
    let variants = plan
        .frequencies_hz
        .iter()
        .enumerate()
        .map(|(index, frequency_hz)| {
            Ok((
                SignalVariantIdV3::new(format!("f-{}", index + 1)).map_err(|_| {
                    SessionErrorPayload::report_pipeline("generated frequency identity is invalid")
                })?,
                *frequency_hz,
            ))
        })
        .collect::<Result<Vec<_>, SessionErrorPayload>>()?;
    let block_size = antenna_labels
        .len()
        .checked_mul(variants.len())
        .ok_or_else(|| SessionErrorPayload::report_pipeline("signal schedule size overflowed"))?;
    let mut slots = Vec::with_capacity(
        block_size
            .checked_mul(usize::try_from(blocks).unwrap_or(usize::MAX))
            .ok_or_else(|| {
                SessionErrorPayload::report_pipeline("signal schedule size overflowed")
            })?,
    );
    let mut next_start = starts_at;
    for block_index in 0..blocks {
        let block_id =
            CounterbalanceBlockIdV3::new(format!("block-{}", block_index + 1)).map_err(|_| {
                SessionErrorPayload::report_pipeline("generated counterbalance identity is invalid")
            })?;
        let mut pairs = Vec::with_capacity(block_size);
        for (antenna_index, antenna_label) in antenna_labels.iter().enumerate() {
            for (variant_index, (variant_id, frequency_hz)) in variants.iter().enumerate() {
                let forward = antenna_index * variants.len() + variant_index;
                let position = if block_index % 2 == 0 {
                    forward
                } else {
                    block_size - 1 - forward
                };
                pairs.push((
                    position,
                    antenna_label.clone(),
                    variant_id.clone(),
                    *frequency_hz,
                ));
            }
        }
        pairs.sort_by_key(|(position, _, _, _)| *position);
        for (position, antenna_label, frequency_variant_id, frequency_hz) in pairs {
            let sequence_number = u32::try_from(slots.len() + 1).map_err(|_| {
                SessionErrorPayload::report_pipeline("signal slot count overflowed")
            })?;
            slots.push(PlannedSlotV3 {
                slot_id: hooks.new_id("slot"),
                sequence_number,
                starts_at: next_start,
                duration_seconds,
                guard_seconds,
                band,
                antenna_label,
                signal: Some(SignalAllocationV3 {
                    signal_plan_id: signal_plan_id.clone(),
                    frequency_hz,
                    frequency_variant_id,
                    counterbalance_block_id: block_id.clone(),
                    counterbalance_position: u16::try_from(position).map_err(|_| {
                        SessionErrorPayload::report_pipeline(
                            "counterbalance position exceeds the supported range",
                        )
                    })?,
                }),
            });
            next_start += Duration::seconds(i64::from(duration_seconds));
        }
    }
    bundle.schedule.signal_plans = vec![SignalPlanV3 {
        signal_plan_id,
        mode: plan.mode,
        planned_power_watts: plan.planned_power_watts,
        transmitted_callsign: plan.transmitted_callsign,
        differing_identity_validated: plan.differing_identity_validated,
        cadence: SignalCadenceV3 {
            message: plan.message,
            repetition_count: plan.repetition_count,
            key_speed_wpm: plan.key_speed_wpm,
            transmit_seconds: plan.transmit_seconds,
            interval_seconds: plan.interval_seconds,
        },
        collection_profile: plan.collection_profile,
    }];
    bundle.schedule.slots = slots;
    Ok(bundle)
}

fn setup_plan_review(bundle: &BundleV2Contents) -> SetupPlanReview {
    SetupPlanReview {
        schema_version: SCHEMA_VERSION_V2,
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
        wspr_live_acquisition_enabled: bundle.session_state.wspr_live_acquisition_enabled,
        signal_plan: None,
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
                signal: None,
            })
            .collect(),
    }
}

fn setup_plan_review_v3(bundle: &BundleV3Contents) -> SetupPlanReview {
    let plan = &bundle.schedule.signal_plans[0];
    SetupPlanReview {
        schema_version: SCHEMA_VERSION_V3,
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
        wspr_live_acquisition_enabled: false,
        signal_plan: Some(SetupSignalPlanReview {
            mode: plan.mode,
            collection_profile: plan.collection_profile,
            planned_power_watts: plan.planned_power_watts,
            transmitted_callsign: plan.transmitted_callsign.clone(),
            message: plan.cadence.message.clone(),
            repetition_count: plan.cadence.repetition_count,
            key_speed_wpm: plan.cadence.key_speed_wpm,
            transmit_seconds: plan.cadence.transmit_seconds,
            interval_seconds: plan.cadence.interval_seconds,
            frequencies_hz: bundle
                .schedule
                .slots
                .iter()
                .filter_map(|slot| slot.signal.as_ref().map(|signal| signal.frequency_hz))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        }),
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
                signal: slot.signal.as_ref().map(|signal| SetupSlotSignalReview {
                    frequency_hz: signal.frequency_hz,
                    frequency_variant_id: signal.frequency_variant_id.as_str().into(),
                    counterbalance_block_id: signal.counterbalance_block_id.as_str().into(),
                    counterbalance_position: signal.counterbalance_position,
                }),
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

fn suggested_bundle_name(bundle: &PendingBundle) -> String {
    let callsign = bundle
        .callsign()
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
    select: impl FnOnce(&PendingBundle) -> Result<Option<PathBuf>, SessionErrorPayload>,
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
        let store = BundleStore::new(&destination);
        match &pending.bundle {
            PendingBundle::V2(bundle) => store.create_v2_checkpointed(bundle),
            PendingBundle::V3(bundle) => store.create_v3_checkpointed(bundle),
        }
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
    wsjtx_state: State<'_, WsjtxSessionState>,
    review_id: String,
) -> Result<CreateSessionOutcome, SessionErrorPayload> {
    let outcome = create_with_selection(
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
    )?;
    if matches!(outcome, CreateSessionOutcome::Created { .. }) {
        wsjtx_state.stop_all(
            "WSJT-X reception stopped because a different session was created and activated.",
        );
    }
    Ok(outcome)
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct E2eCreatedSession {
    pub(crate) path: PathBuf,
    pub(crate) session_id: String,
    pub(crate) slot_ids: Vec<String>,
    pub(crate) antenna_labels: Vec<String>,
}

#[cfg(test)]
pub(crate) fn create_e2e_session(
    root: &Path,
    active_state: &ActiveSessionState,
) -> E2eCreatedSession {
    #[derive(Debug)]
    struct DeterministicHooks(Mutex<u64>);

    impl LivePersistenceHooks for DeterministicHooks {
        fn now(&self) -> DateTime<Utc> {
            "2026-07-15T19:59:30Z".parse().unwrap()
        }

        fn new_id(&self, kind: &str) -> String {
            let mut next = self.0.lock().unwrap();
            let id = format!("e2e-{kind}-{next:04}");
            *next += 1;
            id
        }
    }

    let setup_state = SetupSessionState::default();
    let draft = SetupDraft {
        station: SetupStationDraft {
            callsign: " N1RWJ ".into(),
            grid: " FN42 ".into(),
            power_watts: "5".into(),
            operator_notes: "deterministic complete workflow".into(),
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
            starts_at: "2026-07-15T16:00:00-04:00".into(),
            band: Band::M20,
            duration_seconds: "120".into(),
            guard_seconds: "10".into(),
            rounds: "2".into(),
        },
        wspr_live_acquisition_enabled: false,
        signal_plan: None,
    };
    let review = build_review(&setup_state, draft, &DeterministicHooks(Mutex::new(1)))
        .expect("deterministic setup review");
    assert!(review.valid, "setup diagnostics: {:?}", review.diagnostics);
    let review_id = review.review_id.expect("valid review ID");
    let plan = review.plan.expect("valid reviewed plan");
    let path = root.join(format!("complete-workflow{V2_BUNDLE_SUFFIX}"));
    let outcome = create_with_selection(&setup_state, active_state, &review_id, |_| {
        Ok(Some(path.clone()))
    })
    .expect("atomic setup creation");
    let CreateSessionOutcome::Created { session } = outcome else {
        panic!("deterministic selection must create the session")
    };
    assert_eq!(session.session_id, plan.session_id);
    E2eCreatedSession {
        path,
        session_id: plan.session_id,
        slot_ids: plan.slots.into_iter().map(|slot| slot.slot_id).collect(),
        antenna_labels: plan
            .antennas
            .into_iter()
            .map(|antenna| antenna.label)
            .collect(),
    }
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
            wspr_live_acquisition_enabled: false,
            signal_plan: None,
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
        assert!(!plan.wspr_live_acquisition_enabled);
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
    fn reviewed_signal_plan_creates_a_checkpointed_schema_v3_bundle() {
        let state = SetupSessionState::default();
        let active = ActiveSessionState::default();
        let mut draft = valid_draft();
        draft.signal_plan = Some(SetupSignalPlanDraft {
            mode: SignalModeV3::Cw,
            collection_profile: SignalCollectionProfileV3::ManualObservation,
            planned_power_watts: "5".into(),
            transmitted_callsign: "N1RWJ".into(),
            differing_identity_validated: false,
            message: "CQ CQ N1RWJ N1RWJ TEST".into(),
            repetition_count: "2".into(),
            key_speed_wpm: "20".into(),
            transmit_seconds: "20".into(),
            interval_seconds: "30".into(),
            frequencies_hz: "14050000, 14050300".into(),
        });

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();

        assert!(review.valid, "diagnostics: {:?}", review.diagnostics);
        let review_id = review.review_id.unwrap();
        let plan = review.plan.unwrap();
        assert_eq!(plan.schema_version, SCHEMA_VERSION_V3);
        assert!(!plan.wspr_live_acquisition_enabled);
        assert_eq!(plan.slots.len(), 8);
        assert_eq!(
            plan.signal_plan.unwrap().frequencies_hz,
            vec![14_050_000, 14_050_300]
        );
        assert_eq!(
            plan.slots[0].signal.as_ref().unwrap().frequency_hz,
            14_050_000
        );
        assert_eq!(
            plan.slots[4].signal.as_ref().unwrap().frequency_hz,
            14_050_300
        );

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(format!("signal{V2_BUNDLE_SUFFIX}"));
        let outcome =
            create_with_selection(&state, &active, &review_id, |_| Ok(Some(path.clone()))).unwrap();
        assert!(matches!(outcome, CreateSessionOutcome::Created { .. }));
        let persisted = BundleStore::new(path).read_v3_checkpointed().unwrap();
        assert_eq!(persisted.manifest.schema_version, SCHEMA_VERSION_V3);
        assert_eq!(persisted.schedule.signal_plans.len(), 1);
        assert_eq!(persisted.schedule.slots.len(), 8);
    }

    #[test]
    fn wspr_live_automatic_acquisition_choice_survives_review_and_creation() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.wspr_live_acquisition_enabled = true;

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();

        assert!(review.valid);
        assert!(review.plan.unwrap().wspr_live_acquisition_enabled);
        assert!(state
            .0
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .bundle
            .wspr_live_acquisition_enabled());
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
        assert!(!persisted.session_state.wspr_live_acquisition_enabled);
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
