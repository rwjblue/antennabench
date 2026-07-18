#[cfg(test)]
use std::path::{Path, PathBuf};
use std::{collections::BTreeMap, sync::Mutex};

use antennabench_core::{
    upgrade_v2_bundle_model, validate_bundle_report, validate_signal_plan_schedule_v3,
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleDiagnostic,
    BundleDiagnosticSeverity, BundleFileRole, BundleFilesV2, BundleManifestV2, BundleV2Contents,
    BundleV3Contents, BundleValidationProfile, ExperimentMode, PlanGenerationV2, PlannedSlot,
    Schedule, SessionGoal, SessionLifecycleV2, SessionStateV2, SignalCollectionProfileV3,
    SignalModeV3, Station, WsprCycleDirection, WsprCycleIntentV3, SCHEMA_VERSION_V2,
};
#[cfg(test)]
use antennabench_core::{SCHEMA_VERSION_V5, V2_BUNDLE_SUFFIX};
use antennabench_storage::{BundleStore, LivePersistenceHooks, SystemLivePersistenceHooks};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::antenna_control::{
    activate_setup_controller, controller_profiles_for_app, prepare_setup_controller,
    AntennaControllerState, ControllerProfile, PreparedSetupController, SetupControllerDraft,
    SetupControllerReview,
};
use crate::open_session::{
    ActiveSessionState, OpenedSession, SessionErrorKind, SessionErrorPayload,
};
use crate::wsjtx_session::WsjtxSessionState;

mod bundle;
mod creation;
mod preferences;

use bundle::{build_v3_setup_bundle, planned_wspr_cycles, setup_plan_review_v3, use_latest_schema};
use creation::{
    create_with_selection, creation_error, reviewed_setup_controller, reviewed_station_preferences,
};
use preferences::{
    automatic_session_destination, read_station_preferences, resolved_app_data_dir,
    write_station_preferences,
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
    bundle: BundleV3Contents,
    controller: Option<PreparedSetupController>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StationPreferences {
    callsign: String,
    grid: String,
    power_watts: Option<String>,
    operator_notes: Option<String>,
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
    #[serde(default)]
    antenna_controller: Option<SetupControllerDraft>,
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
    band: Band,
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
    schedule_review: SetupScheduleReview,
    capabilities: SetupCapabilityReview,
    slots: Vec<SetupSlotReview>,
    antenna_controller: Option<SetupControllerReview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupScheduleReview {
    period_kind: &'static str,
    period_count: usize,
    wspr_cycle_count: Option<usize>,
    ideal_minimum_minutes: Option<u64>,
    summary: String,
    counterbalance_explanation: String,
    transition_summary: String,
    transitions: Vec<SetupTransitionReview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupTransitionReview {
    from_sequence_number: u32,
    to_sequence_number: u32,
    antenna_change: bool,
    direction_change: bool,
    summary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupCapabilityReview {
    can_describe: Vec<String>,
    cannot_establish: Vec<String>,
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
    band: Band,
    antenna_label: String,
    direction: Option<WsprCycleDirection>,
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
    let transmitted_callsign = draft.transmitted_callsign.trim().to_uppercase();
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

#[cfg(test)]
fn build_review(
    state: &SetupSessionState,
    draft: SetupDraft,
    hooks: &dyn LivePersistenceHooks,
) -> Result<SetupReview, SessionErrorPayload> {
    build_review_with_profiles(state, draft, hooks, &[])
}

fn build_review_with_profiles(
    state: &SetupSessionState,
    draft: SetupDraft,
    hooks: &dyn LivePersistenceHooks,
    controller_profiles: &[ControllerProfile],
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
    let wspr_cycles = rounds
        .map(|rounds| planned_wspr_cycles(&scheduled_labels, draft.schedule.mode, rounds))
        .unwrap_or_default();
    let slot_count = if let Some(plan) = signal_plan.as_ref() {
        rounds
            .and_then(|rounds| usize::try_from(rounds).ok())
            .and_then(|rounds| rounds.checked_mul(scheduled_labels.len()))
            .and_then(|count| count.checked_mul(plan.frequencies_hz.len()))
    } else {
        Some(wspr_cycles.len())
    };
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
    let mut next_start = created_at;
    if signal_plan.is_some() {
        for _ in 0..rounds {
            for antenna_label in &scheduled_labels {
                let sequence_number =
                    u32::try_from(slots.len() + 1).expect("slot count is bounded");
                slots.push(PlannedSlot {
                    slot_id: hooks.new_id("slot"),
                    sequence_number,
                    starts_at: next_start,
                    duration_seconds: 120,
                    guard_seconds: 0,
                    band: draft.schedule.band,
                    antenna_label: (*antenna_label).clone(),
                });
                next_start += Duration::seconds(120);
            }
        }
    } else {
        for (antenna_label, _) in &wspr_cycles {
            let sequence_number = u32::try_from(slots.len() + 1).expect("slot count is bounded");
            slots.push(PlannedSlot {
                slot_id: hooks.new_id("slot"),
                sequence_number,
                starts_at: next_start,
                duration_seconds: 120,
                guard_seconds: 0,
                band: draft.schedule.band,
                antenna_label: antenna_label.clone(),
            });
            next_start += Duration::seconds(120);
        }
    }

    let station = Station {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.clone(),
        callsign: draft.station.callsign.trim().to_uppercase(),
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
    let report = validate_bundle_report(&bundle.clone().into_current().bundle);

    let mut bundle = if let Some(signal_plan) = signal_plan {
        build_v3_setup_bundle(
            bundle,
            signal_plan,
            &scheduled_label_values,
            rounds,
            draft.schedule.band,
            hooks,
        )?
    } else {
        let mut bundle = upgrade_v2_bundle_model(bundle);
        bundle.schedule.wspr_cycle_intents = bundle
            .schedule
            .slots
            .drain(..)
            .zip(wspr_cycles.iter().map(|(_, direction)| *direction))
            .map(|(slot, direction)| WsprCycleIntentV3 {
                intent_id: slot.slot_id,
                sequence_number: slot.sequence_number,
                band: slot.band,
                antenna_label: slot.antenna_label,
                direction: Some(direction),
                signal: slot.signal,
            })
            .collect();
        bundle
    };
    use_latest_schema(&mut bundle);
    let (prepared_controller, controller_review) = match draft
        .antenna_controller
        .as_ref()
        .filter(|controller| controller.enabled)
    {
        Some(controller) => {
            match prepare_setup_controller(controller, &mut bundle, controller_profiles, |prefix| {
                hooks.new_id(prefix)
            }) {
                Ok((prepared, review)) => (Some(prepared), Some(review)),
                Err(message) => {
                    *state.0.lock().map_err(|_| {
                        SessionErrorPayload::report_pipeline("setup review state is unavailable")
                    })? = None;
                    return Ok(SetupReview {
                        review_id: None,
                        valid: false,
                        diagnostics: vec![field_diagnostic(
                            "setup.antenna_controller.invalid",
                            "antennaController",
                            message,
                        )],
                        plan: None,
                    });
                }
            }
        }
        None => (None, None),
    };
    BundleStore::refresh_v3_checkpoint(&mut bundle).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The normalized schema-v5 setup plan could not be prepared.",
            error.to_string(),
        )
    })?;

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

    let plan = setup_plan_review_v3(&bundle, controller_review);
    *state
        .0
        .lock()
        .map_err(|_| SessionErrorPayload::report_pipeline("setup review state is unavailable"))? =
        Some(PendingSetup {
            review_id: review_id.clone(),
            bundle,
            controller: prepared_controller,
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

#[tauri::command]
pub(crate) fn review_session_setup(
    app: AppHandle,
    state: State<'_, SetupSessionState>,
    draft: SetupDraft,
) -> Result<SetupReview, SessionErrorPayload> {
    let profiles = controller_profiles_for_app(&app)?;
    build_review_with_profiles(state.inner(), draft, &SystemLivePersistenceHooks, &profiles)
}

#[tauri::command]
pub(crate) fn load_station_preferences(
    app: AppHandle,
) -> Result<Option<StationPreferences>, SessionErrorPayload> {
    read_station_preferences(&resolved_app_data_dir(&app)?)
}

#[tauri::command]
pub(crate) async fn create_session_from_review(
    app: AppHandle,
    setup_state: State<'_, SetupSessionState>,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    wsjtx_state: State<'_, WsjtxSessionState>,
    review_id: String,
) -> Result<CreateSessionOutcome, SessionErrorPayload> {
    let app_data_dir = resolved_app_data_dir(&app)?;
    let station_preferences = reviewed_station_preferences(setup_state.inner(), &review_id)?;
    let setup_controller = reviewed_setup_controller(setup_state.inner(), &review_id)?;
    controller_state.revoke();
    let outcome = create_with_selection(
        setup_state.inner(),
        active_state.inner(),
        &review_id,
        |bundle| automatic_session_destination(&app_data_dir, bundle).map(Some),
    )?;
    if matches!(outcome, CreateSessionOutcome::Created { .. }) {
        if let Err(error) = write_station_preferences(&app_data_dir, &station_preferences) {
            eprintln!("AntennaBench could not remember station details: {error:?}");
        }
        wsjtx_state.stop_all(
            "WSJT-X reception stopped because a different session was created and activated.",
        );
        if let Some(controller) = &setup_controller {
            let (source, _) = crate::open_session::active_session_source(active_state.inner())?;
            let session_id = BundleStore::new(&source)
                .read_v3_checkpointed()
                .map_err(creation_error)?
                .manifest
                .session_id;
            activate_setup_controller(
                &app_data_dir,
                controller_state.inner(),
                source,
                session_id,
                controller,
            )?;
        }
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
    create_e2e_session_with_signal(root, active_state, false)
}

#[cfg(test)]
pub(crate) fn create_e2e_signal_session(
    root: &Path,
    active_state: &ActiveSessionState,
) -> E2eCreatedSession {
    create_e2e_session_with_signal(root, active_state, true)
}

#[cfg(test)]
fn create_e2e_session_with_signal(
    root: &Path,
    active_state: &ActiveSessionState,
    with_signal: bool,
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
            callsign: " n1rwj ".into(),
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
            band: Band::M20,
            rounds: "2".into(),
        },
        wspr_live_acquisition_enabled: false,
        signal_plan: with_signal.then(|| SetupSignalPlanDraft {
            mode: SignalModeV3::Cw,
            collection_profile: SignalCollectionProfileV3::ManualObservation,
            planned_power_watts: "5".into(),
            transmitted_callsign: "n1rwj".into(),
            differing_identity_validated: false,
            message: "CQ CQ N1RWJ N1RWJ TEST".into(),
            repetition_count: "2".into(),
            key_speed_wpm: "20".into(),
            transmit_seconds: "20".into(),
            interval_seconds: "30".into(),
            frequencies_hz: "14050000".into(),
        }),
        antenna_controller: None,
    };
    let review = build_review(&setup_state, draft, &DeterministicHooks(Mutex::new(1)))
        .expect("deterministic setup review");
    assert!(review.valid, "setup diagnostics: {:?}", review.diagnostics);
    let review_id = review.review_id.expect("valid review ID");
    let plan = review.plan.expect("valid reviewed plan");
    if with_signal {
        assert_eq!(plan.schedule_review.period_kind, "controlled_signal_slot");
        assert!(plan.schedule_review.wspr_cycle_count.is_none());
        assert!(plan.schedule_review.ideal_minimum_minutes.is_none());
    } else {
        assert_eq!(plan.schedule_review.period_kind, "wspr_cycle");
        assert_eq!(plan.schedule_review.wspr_cycle_count, Some(8));
        assert_eq!(plan.schedule_review.ideal_minimum_minutes, Some(16));
        assert_eq!(plan.schedule_review.transitions.len(), 7);
        assert!(plan
            .capabilities
            .can_describe
            .iter()
            .any(|statement| { statement.contains("Transmit-path same-path signal differences") }));
        assert!(plan
            .capabilities
            .can_describe
            .iter()
            .any(|statement| { statement.contains("Receive-path same-path signal differences") }));
        assert!(plan.capabilities.cannot_establish.iter().any(|statement| {
            statement.contains("reduces but does not eliminate time and propagation confounding")
        }));
    }
    let stem = if with_signal {
        "signal-workflow"
    } else {
        "complete-workflow"
    };
    let path = root.join(format!("{stem}{V2_BUNDLE_SUFFIX}"));
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
                callsign: " n1rwj ".into(),
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
                band: Band::M20,
                rounds: "2".into(),
            },
            wspr_live_acquisition_enabled: false,
            signal_plan: None,
            antenna_controller: None,
        }
    }

    #[test]
    fn review_normalizes_an_untimed_operator_paced_plan() {
        let state = SetupSessionState::default();
        let review = build_review(&state, valid_draft(), &FixedHooks::new()).unwrap();

        assert!(review.valid);
        assert!(review.diagnostics.is_empty());
        assert_eq!(review.review_id.as_deref(), Some("review-0003"));
        let plan = review.plan.unwrap();
        assert_eq!(plan.session_id, "session-0001");
        assert_eq!(plan.schema_version, SCHEMA_VERSION_V5);
        assert_eq!(plan.station.callsign, "N1RWJ");
        assert_eq!(plan.station.grid, "FN42");
        assert!(!plan.wspr_live_acquisition_enabled);
        assert_eq!(plan.slots.len(), 8);
        assert_eq!(plan.slots[0].slot_id, "slot-0004");
        assert_eq!(plan.slots[0].antenna_label, "Vertical");
        assert_eq!(plan.slots[0].direction, Some(WsprCycleDirection::Receive));
        assert_eq!(plan.slots[1].antenna_label, "Dipole");
        assert_eq!(plan.slots[1].direction, Some(WsprCycleDirection::Receive));
        assert_eq!(plan.slots[2].antenna_label, "Dipole");
        assert_eq!(plan.slots[2].direction, Some(WsprCycleDirection::Transmit));
        assert_eq!(plan.slots[3].antenna_label, "Vertical");
        assert_eq!(plan.slots[3].direction, Some(WsprCycleDirection::Transmit));
        assert_eq!(plan.slots[2].sequence_number, 3);
    }

    #[test]
    fn review_normalizes_local_controller_policy_targets_and_every_invocation_preview() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.antenna_controller = Some(SetupControllerDraft {
            enabled: true,
            arm_for_session: true,
            invocation: antennabench_core::AntennaControlInvocationPolicyV5::OperatorTriggered,
            manual_review_required: true,
            profile: crate::antenna_control::ControllerProfileDraft {
                profile_id: None,
                name: "Bench switch".into(),
                switch_command: crate::antenna_control::ControllerCommandDraft {
                    one_line: "switch --target {target} --mode {mode} --direction {direction}"
                        .into(),
                    program: String::new(),
                    arguments: Vec::new(),
                },
                verification_command: Some(crate::antenna_control::ControllerCommandDraft {
                    one_line: "verify --target {target} --mode {mode}".into(),
                    program: String::new(),
                    arguments: Vec::new(),
                }),
                timeout_seconds: 10,
            },
            targets: vec![
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Vertical".into(),
                    target: "relay A;$(literal)".into(),
                },
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Dipole".into(),
                    target: "relay B".into(),
                },
            ],
        });

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();
        assert!(review.valid, "diagnostics: {:?}", review.diagnostics);
        let plan = review.plan.unwrap();
        let controller = plan.antenna_controller.unwrap();
        assert!(controller.arm_for_session);
        assert_eq!(controller.invocations.len(), plan.slots.len());
        assert_eq!(
            controller.invocations[0].mode,
            ExperimentMode::WholeStationAb
        );
        assert_eq!(
            controller.invocations[0].switch_arguments[1],
            "relay A;$(literal)"
        );
        let pending = state.0.lock().unwrap();
        let pending = pending.as_ref().unwrap();
        assert!(matches!(
            pending.bundle.schedule.antenna_control,
            Some(
                antennabench_core::AntennaControlPolicyV5::CommandControlled {
                    invocation:
                        antennabench_core::AntennaControlInvocationPolicyV5::OperatorTriggered,
                    manual_review_required: true,
                }
            )
        ));
    }

    fn controller_draft() -> SetupControllerDraft {
        SetupControllerDraft {
            enabled: true,
            arm_for_session: false,
            invocation: antennabench_core::AntennaControlInvocationPolicyV5::OperatorTriggered,
            manual_review_required: true,
            profile: crate::antenna_control::ControllerProfileDraft {
                profile_id: None,
                name: "Bench switch".into(),
                switch_command: crate::antenna_control::ControllerCommandDraft {
                    one_line: "switch {target} {mode} {direction}".into(),
                    program: String::new(),
                    arguments: Vec::new(),
                },
                verification_command: None,
                timeout_seconds: 10,
            },
            targets: vec![
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Vertical".into(),
                    target: "relay-a".into(),
                },
                crate::antenna_control::ControllerTargetDraft {
                    antenna_label: "Dipole".into(),
                    target: "relay-b".into(),
                },
            ],
        }
    }

    #[test]
    fn review_freezes_automatic_command_authority_and_requires_verification() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        let mut controller = controller_draft();
        controller.invocation = antennabench_core::AntennaControlInvocationPolicyV5::Automatic;
        controller.manual_review_required = false;
        draft.antenna_controller = Some(controller.clone());

        let rejected = build_review(&state, draft, &FixedHooks::new()).unwrap();
        assert!(!rejected.valid);
        assert!(rejected.diagnostics[0]
            .message
            .contains("independent verification command"));

        controller.profile.verification_command =
            Some(crate::antenna_control::ControllerCommandDraft {
                one_line: "verify {target} {mode} {direction}".into(),
                program: String::new(),
                arguments: Vec::new(),
            });
        let mut draft = valid_draft();
        draft.antenna_controller = Some(controller);
        let accepted = build_review(&state, draft, &FixedHooks::new()).unwrap();
        assert!(accepted.valid, "diagnostics: {:?}", accepted.diagnostics);
        let controller = accepted.plan.unwrap().antenna_controller.unwrap();
        assert_eq!(
            controller.invocation,
            antennabench_core::AntennaControlInvocationPolicyV5::Automatic
        );
        assert!(!controller.manual_review_required);
        assert!(controller
            .authority_summary
            .contains("authorize the next eligible WSPR boundary"));
        assert!(matches!(
            state
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .bundle
                .schedule
                .antenna_control,
            Some(
                antennabench_core::AntennaControlPolicyV5::CommandControlled {
                    invocation: antennabench_core::AntennaControlInvocationPolicyV5::Automatic,
                    manual_review_required: false,
                }
            )
        ));
    }

    #[test]
    fn review_rejects_every_controller_setup_failure_before_pending_creation() {
        for case in ["quoting", "placeholder", "target", "timeout", "argument"] {
            let state = SetupSessionState::default();
            let mut draft = valid_draft();
            let mut controller = controller_draft();
            match case {
                "quoting" => controller.profile.switch_command.one_line = "switch 'open".into(),
                "placeholder" => {
                    controller.profile.switch_command.one_line = "switch {unknown}".into()
                }
                "target" => {
                    controller.targets.pop();
                }
                "timeout" => controller.profile.timeout_seconds = 0,
                "argument" => {
                    controller.profile.switch_command.one_line = format!(
                        "switch {}",
                        "x".repeat(antennabench_core::COMMAND_ARGUMENT_MAX_BYTES + 1)
                    )
                }
                _ => unreachable!(),
            }
            draft.antenna_controller = Some(controller);
            let review = build_review(&state, draft, &FixedHooks::new()).unwrap();
            assert!(!review.valid, "case {case}");
            assert!(review.plan.is_none(), "case {case}");
            assert!(state.0.lock().unwrap().is_none(), "case {case}");
            assert_eq!(
                review.diagnostics[0].code, "setup.antenna_controller.invalid",
                "case {case}"
            );
        }
    }

    #[test]
    fn wspr_cycle_order_counterbalances_direction_and_antenna_order() {
        let a = "A".to_string();
        let b = "B".to_string();
        let antennas = [&a, &b];

        assert_eq!(
            planned_wspr_cycles(&antennas, ExperimentMode::WholeStationAb, 2),
            vec![
                ("A".into(), WsprCycleDirection::Receive),
                ("B".into(), WsprCycleDirection::Receive),
                ("B".into(), WsprCycleDirection::Transmit),
                ("A".into(), WsprCycleDirection::Transmit),
                ("B".into(), WsprCycleDirection::Receive),
                ("A".into(), WsprCycleDirection::Receive),
                ("A".into(), WsprCycleDirection::Transmit),
                ("B".into(), WsprCycleDirection::Transmit),
            ]
        );
        assert_eq!(
            planned_wspr_cycles(&antennas, ExperimentMode::TxFocused, 2),
            vec![
                ("A".into(), WsprCycleDirection::Transmit),
                ("B".into(), WsprCycleDirection::Transmit),
                ("B".into(), WsprCycleDirection::Transmit),
                ("A".into(), WsprCycleDirection::Transmit),
            ]
        );
        assert_eq!(
            planned_wspr_cycles(&antennas, ExperimentMode::RxFocused, 2),
            vec![
                ("A".into(), WsprCycleDirection::Receive),
                ("B".into(), WsprCycleDirection::Receive),
                ("B".into(), WsprCycleDirection::Receive),
                ("A".into(), WsprCycleDirection::Receive),
            ]
        );
    }

    #[test]
    fn normalized_review_projects_every_mode_schedule_and_capability_matrix() {
        struct Case {
            mode: ExperimentMode,
            cycle_count: usize,
            minimum_minutes: u64,
            order: &'static [(&'static str, WsprCycleDirection)],
            can_include: &'static str,
            cannot_include: Option<&'static str>,
        }
        let cases = [
            Case {
                mode: ExperimentMode::WholeStationAb,
                cycle_count: 8,
                minimum_minutes: 16,
                order: &[
                    ("Vertical", WsprCycleDirection::Receive),
                    ("Dipole", WsprCycleDirection::Receive),
                    ("Dipole", WsprCycleDirection::Transmit),
                    ("Vertical", WsprCycleDirection::Transmit),
                    ("Dipole", WsprCycleDirection::Receive),
                    ("Vertical", WsprCycleDirection::Receive),
                    ("Vertical", WsprCycleDirection::Transmit),
                    ("Dipole", WsprCycleDirection::Transmit),
                ],
                can_include: "Receive-path same-path signal differences",
                cannot_include: None,
            },
            Case {
                mode: ExperimentMode::TxFocused,
                cycle_count: 4,
                minimum_minutes: 8,
                order: &[
                    ("Vertical", WsprCycleDirection::Transmit),
                    ("Dipole", WsprCycleDirection::Transmit),
                    ("Dipole", WsprCycleDirection::Transmit),
                    ("Vertical", WsprCycleDirection::Transmit),
                ],
                can_include: "Transmit-path same-path signal differences",
                cannot_include: Some("Receive-path antenna performance"),
            },
            Case {
                mode: ExperimentMode::RxFocused,
                cycle_count: 4,
                minimum_minutes: 8,
                order: &[
                    ("Vertical", WsprCycleDirection::Receive),
                    ("Dipole", WsprCycleDirection::Receive),
                    ("Dipole", WsprCycleDirection::Receive),
                    ("Vertical", WsprCycleDirection::Receive),
                ],
                can_include: "Receive-path same-path signal differences",
                cannot_include: Some("Transmit reach or transmit-path antenna performance"),
            },
            Case {
                mode: ExperimentMode::SingleAntennaProfiling,
                cycle_count: 4,
                minimum_minutes: 8,
                order: &[
                    ("Vertical", WsprCycleDirection::Receive),
                    ("Vertical", WsprCycleDirection::Transmit),
                    ("Vertical", WsprCycleDirection::Receive),
                    ("Vertical", WsprCycleDirection::Transmit),
                ],
                can_include: "profiled antenna",
                cannot_include: Some("only one antenna is scheduled"),
            },
        ];

        for case in cases {
            let state = SetupSessionState::default();
            let mut draft = valid_draft();
            draft.schedule.mode = case.mode;
            if case.mode == ExperimentMode::SingleAntennaProfiling {
                draft.schedule.goal = SessionGoal::SingleAntennaProfiling;
            }
            draft.wspr_live_acquisition_enabled = true;
            let review = build_review(&state, draft, &FixedHooks::new()).unwrap();
            assert!(
                review.valid,
                "mode {:?}: {:?}",
                case.mode, review.diagnostics
            );
            let plan = review.plan.unwrap();
            assert_eq!(plan.mode, case.mode);
            assert_eq!(plan.schedule_review.period_kind, "wspr_cycle");
            assert_eq!(plan.schedule_review.period_count, case.cycle_count);
            assert_eq!(
                plan.schedule_review.wspr_cycle_count,
                Some(case.cycle_count)
            );
            assert_eq!(
                plan.schedule_review.ideal_minimum_minutes,
                Some(case.minimum_minutes)
            );
            assert_eq!(
                plan.slots
                    .iter()
                    .map(|slot| (slot.antenna_label.as_str(), slot.direction.unwrap()))
                    .collect::<Vec<_>>(),
                case.order,
                "mode {:?}",
                case.mode
            );
            assert!(plan
                .capabilities
                .can_describe
                .iter()
                .any(|statement| statement.contains(case.can_include)));
            if let Some(expected) = case.cannot_include {
                assert!(plan
                    .capabilities
                    .cannot_establish
                    .iter()
                    .any(|statement| statement.contains(expected)));
            }
            for expected in [
                "Universal antenna gain",
                "reduces but does not eliminate time and propagation confounding",
                "missing decode as a zero-strength measurement",
                "does not provide an independent completeness guarantee",
                "winner",
            ] {
                assert!(
                    plan.capabilities
                        .cannot_establish
                        .iter()
                        .any(|statement| statement.contains(expected)),
                    "mode {:?} missing {expected}",
                    case.mode
                );
            }
        }
    }

    #[test]
    fn normalized_review_distinguishes_antenna_and_direction_transitions() {
        let state = SetupSessionState::default();
        let review = build_review(&state, valid_draft(), &FixedHooks::new()).unwrap();
        let schedule = review.plan.unwrap().schedule_review;

        assert_eq!(schedule.transitions.len(), 7);
        assert_eq!(
            (
                schedule.transitions[0].antenna_change,
                schedule.transitions[0].direction_change
            ),
            (true, false)
        );
        assert_eq!(
            schedule.transitions[0].summary,
            "Change antenna; keep TX/RX direction"
        );
        assert_eq!(
            (
                schedule.transitions[1].antenna_change,
                schedule.transitions[1].direction_change
            ),
            (false, true)
        );
        assert_eq!(
            schedule.transitions[1].summary,
            "Keep antenna; change TX/RX direction"
        );
        assert_eq!(
            (
                schedule.transitions[3].antenna_change,
                schedule.transitions[3].direction_change
            ),
            (true, true)
        );
        assert_eq!(
            schedule.transitions[3].summary,
            "Change antenna and TX/RX direction"
        );
        assert_eq!(
            schedule.transition_summary,
            "7 transitions: 5 antenna changes, 3 direction changes, 1 requiring both."
        );
    }

    #[test]
    fn receive_only_setup_accepts_bidirectional_public_spot_collection() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.schedule.mode = ExperimentMode::RxFocused;
        draft.wspr_live_acquisition_enabled = true;

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();

        assert!(review.valid);
        assert!(review.diagnostics.is_empty());
    }

    #[test]
    fn reviewed_signal_plan_creates_a_checkpointed_schema_v4_bundle() {
        let state = SetupSessionState::default();
        let active = ActiveSessionState::default();
        let mut draft = valid_draft();
        draft.signal_plan = Some(SetupSignalPlanDraft {
            mode: SignalModeV3::Cw,
            collection_profile: SignalCollectionProfileV3::ManualObservation,
            planned_power_watts: "5".into(),
            transmitted_callsign: "n1rwj".into(),
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
        assert_eq!(plan.schema_version, SCHEMA_VERSION_V5);
        assert!(!plan.wspr_live_acquisition_enabled);
        assert_eq!(plan.slots.len(), 8);
        assert_eq!(plan.schedule_review.period_kind, "controlled_signal_slot");
        assert_eq!(plan.schedule_review.period_count, 8);
        assert!(plan.schedule_review.wspr_cycle_count.is_none());
        assert!(plan.schedule_review.ideal_minimum_minutes.is_none());
        assert!(plan
            .capabilities
            .cannot_establish
            .iter()
            .any(|statement| { statement.contains("Receive-path antenna performance") }));
        assert!(plan.capabilities.cannot_establish.iter().any(|statement| {
            statement.contains("automatic WSPR.live collection is off")
                && statement
                    .contains("direct/local receiver evidence remains separately attributed")
        }));
        let signal_plan = plan.signal_plan.unwrap();
        assert_eq!(signal_plan.transmitted_callsign, "N1RWJ");
        assert_eq!(signal_plan.frequencies_hz, vec![14_050_000, 14_050_300]);
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
        assert_eq!(persisted.manifest.schema_version, SCHEMA_VERSION_V5);
        assert_eq!(
            persisted.schedule.antenna_control,
            Some(antennabench_core::AntennaControlPolicyV5::Manual)
        );
        assert_eq!(persisted.schedule.signal_plans.len(), 1);
        assert_eq!(persisted.schedule.wspr_cycle_intents.len(), 8);
        assert!(persisted.schedule.slots.is_empty());
    }

    #[test]
    fn wspr_live_automatic_acquisition_choice_survives_review_and_creation() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.wspr_live_acquisition_enabled = true;

        let review = build_review(&state, draft, &FixedHooks::new()).unwrap();

        assert!(review.valid);
        assert!(review.plan.unwrap().wspr_live_acquisition_enabled);
        assert!(
            state
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .bundle
                .session_state
                .wspr_live_acquisition_enabled
        );
    }

    #[test]
    fn review_returns_stable_field_diagnostics_without_pending_creation() {
        let state = SetupSessionState::default();
        let mut draft = valid_draft();
        draft.station.callsign = " ".into();
        draft.antennas[1].label = "Vertical".into();

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
        assert_eq!(session.slot_count, 8);
        assert!(setup_state.0.lock().unwrap().is_none());
        let persisted = BundleStore::new(destination)
            .read_v3_checkpointed()
            .unwrap();
        assert_eq!(persisted.manifest.session_id, session.session_id);
        assert_eq!(persisted.session_state.lifecycle, SessionLifecycleV2::Ready);
        assert!(!persisted.session_state.wspr_live_acquisition_enabled);
        assert_eq!(
            persisted.schedule.wspr_cycle_intents[0].intent_id,
            "slot-0004"
        );
        assert!(persisted.schedule.slots.is_empty());
        println!(
            "desktop-e2e result=setup-created revision={} slots={}",
            persisted.session_state.revision,
            persisted.schedule.wspr_cycle_intents.len()
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

    #[test]
    fn automatic_destinations_use_app_data_callsign_time_and_collision_suffix() {
        let temp = tempfile::tempdir().unwrap();
        let state = SetupSessionState::default();
        build_review(&state, valid_draft(), &FixedHooks::new()).unwrap();
        let bundle = state.0.lock().unwrap().as_ref().unwrap().bundle.clone();

        let first = automatic_session_destination(temp.path(), &bundle).unwrap();
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            format!("n1rwj-20260715T010000Z{V2_BUNDLE_SUFFIX}")
        );
        std::fs::create_dir_all(&first).unwrap();
        let second = automatic_session_destination(temp.path(), &bundle).unwrap();
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            format!("n1rwj-20260715T010000Z-2{V2_BUNDLE_SUFFIX}")
        );
    }

    #[test]
    fn station_preferences_round_trip_outside_session_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let preferences = StationPreferences {
            callsign: "N1RWJ".into(),
            grid: "FN42".into(),
            power_watts: Some("5".into()),
            operator_notes: Some("backyard".into()),
        };

        assert_eq!(read_station_preferences(temp.path()).unwrap(), None);
        write_station_preferences(temp.path(), &preferences).unwrap();
        assert_eq!(
            read_station_preferences(temp.path()).unwrap(),
            Some(preferences)
        );
    }
}
