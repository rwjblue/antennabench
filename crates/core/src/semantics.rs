use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{
    codes, AnalysisStatus, BundleContents, BundleDiagnostic, BundleDiagnosticCategory,
    BundleDiagnosticLocation, BundleDiagnosticSeverity, BundleFileRole, BundleRecordKind,
    BundleValidationProfile, ExperimentMode, SessionGoal, ANALYSIS_AND_WRITE_OPERATIONS,
    WRITE_OPERATIONS,
};

pub const MACHINE_ID_MAX_BYTES: usize = 128;
pub const ANTENNA_LABEL_MAX_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MachineIdentityError {
    #[error("identity must not be empty")]
    Empty,
    #[error("identity exceeds {MACHINE_ID_MAX_BYTES} bytes")]
    TooLong,
    #[error("identity must contain only ASCII bytes")]
    NonAscii,
}

pub fn validate_machine_identity(value: &str) -> Result<(), MachineIdentityError> {
    if value.is_empty() {
        Err(MachineIdentityError::Empty)
    } else if value.len() > MACHINE_ID_MAX_BYTES {
        Err(MachineIdentityError::TooLong)
    } else if !value.is_ascii() {
        Err(MachineIdentityError::NonAscii)
    } else {
        Ok(())
    }
}

pub(crate) fn semantic_diagnostics(bundle: &BundleContents) -> Vec<BundleDiagnostic> {
    let mut diagnostics = Vec::new();
    validate_identities(bundle, &mut diagnostics);
    validate_station(bundle, &mut diagnostics);
    validate_antennas(bundle, &mut diagnostics);
    validate_schedule(bundle, &mut diagnostics);
    validate_observations(bundle, &mut diagnostics);
    validate_rig(bundle, &mut diagnostics);
    validate_propagation(bundle, &mut diagnostics);
    validate_analysis(bundle, &mut diagnostics);
    diagnostics
}

fn validate_identities(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    validate_identity(
        &bundle.manifest.session_id,
        BundleDiagnosticLocation::file(BundleFileRole::Manifest),
        "/session_id",
        "session",
        diagnostics,
    );
    for (index, slot) in bundle.schedule.slots.iter().enumerate() {
        validate_identity(
            &slot.slot_id,
            location(
                BundleFileRole::Schedule,
                Some(BundleRecordKind::Slot),
                Some(&slot.slot_id),
                Some(index),
                Some(format!("/slots/{index}/slot_id")),
            ),
            "/slot_id",
            "slot",
            diagnostics,
        );
    }
    for (file, kind, values) in [
        (
            BundleFileRole::Events,
            BundleRecordKind::OperatorEvent,
            bundle
                .events
                .iter()
                .map(|record| record.event_id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            BundleFileRole::Observations,
            BundleRecordKind::Observation,
            bundle
                .observations
                .iter()
                .map(|record| record.observation_id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            BundleFileRole::WsjtX,
            BundleRecordKind::WsjtXRecord,
            bundle
                .wsjtx
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            BundleFileRole::Rig,
            BundleRecordKind::RigRecord,
            bundle
                .rig
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            BundleFileRole::Propagation,
            BundleRecordKind::PropagationRecord,
            bundle
                .propagation
                .iter()
                .map(|record| record.record_id.as_str())
                .collect::<Vec<_>>(),
        ),
    ] {
        for (index, id) in values.into_iter().enumerate() {
            validate_identity(
                id,
                location(file, Some(kind), Some(id), Some(index), None),
                if kind == BundleRecordKind::OperatorEvent {
                    "/event_id"
                } else if kind == BundleRecordKind::Observation {
                    "/observation_id"
                } else {
                    "/record_id"
                },
                "record",
                diagnostics,
            );
        }
    }
}

fn validate_identity(
    value: &str,
    mut identity_location: BundleDiagnosticLocation,
    field_path: &str,
    identity_kind: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    let Err(error) = validate_machine_identity(value) else {
        return;
    };
    if identity_location.field_path.is_none() {
        identity_location.field_path = Some(field_path.into());
    }
    diagnostics.push(BundleDiagnostic {
        code: match error {
            MachineIdentityError::Empty => codes::EMPTY_IDENTITY,
            MachineIdentityError::TooLong | MachineIdentityError::NonAscii => {
                codes::INVALID_IDENTITY
            }
        }
        .into(),
        category: BundleDiagnosticCategory::Semantic,
        severity: BundleDiagnosticSeverity::Warning,
        blocked_operations: WRITE_OPERATIONS.to_vec(),
        location: identity_location,
        message: format!("{identity_kind} identity {value:?} is invalid: {error}"),
        related_locations: Vec::new(),
    });
}

fn validate_station(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    for (field, value) in [
        ("/callsign", bundle.station.callsign.as_str()),
        ("/grid", bundle.station.grid.as_str()),
    ] {
        if value.is_empty() || value.trim() != value {
            diagnostics.push(semantic_warning(
                codes::INVALID_REQUIRED_TEXT,
                BundleFileRole::Station,
                field,
                "new station identity text must be trimmed and nonempty".into(),
            ));
        }
    }
    if let Some(value) = bundle.station.power_watts {
        validate_number(
            f64::from(value),
            |value| value > 0.0,
            number_location(BundleFileRole::Station, None, None, "/power_watts"),
            "station power must be finite and greater than zero",
            diagnostics,
        );
    }
}

fn validate_antennas(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    let mut labels = HashMap::<&str, usize>::new();
    for (index, antenna) in bundle.antennas.antennas.iter().enumerate() {
        let normalized = antenna.label.trim();
        let label_location = location(
            BundleFileRole::Antennas,
            Some(BundleRecordKind::Antenna),
            Some(&antenna.label),
            Some(index),
            Some(format!("/antennas/{index}/label")),
        );
        if normalized.is_empty()
            || normalized != antenna.label
            || antenna.label.len() > ANTENNA_LABEL_MAX_BYTES
            || antenna.label.chars().any(char::is_control)
        {
            diagnostics.push(BundleDiagnostic {
                code: codes::INVALID_ANTENNA_LABEL.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Warning,
                blocked_operations: WRITE_OPERATIONS.to_vec(),
                location: label_location.clone(),
                message: format!(
                    "antenna label must be trimmed, nonempty, control-free, and at most {ANTENNA_LABEL_MAX_BYTES} UTF-8 bytes"
                ),
                related_locations: Vec::new(),
            });
        }
        if let Some(first_index) = labels.insert(normalized, index) {
            diagnostics.push(BundleDiagnostic {
                code: codes::DUPLICATE_ANTENNA_LABEL.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
                location: label_location,
                message: format!("antenna label {normalized:?} is ambiguous after trimming"),
                related_locations: vec![location(
                    BundleFileRole::Antennas,
                    Some(BundleRecordKind::Antenna),
                    Some(&bundle.antennas.antennas[first_index].label),
                    Some(first_index),
                    Some(format!("/antennas/{first_index}/label")),
                )],
            });
        }

        for (field, value) in [
            ("height_m", antenna.height_m),
            ("radial_length_m", antenna.radial_length_m),
        ] {
            if let Some(value) = value {
                validate_number(
                    f64::from(value),
                    |value| value >= 0.0,
                    number_location(
                        BundleFileRole::Antennas,
                        Some(BundleRecordKind::Antenna),
                        Some((&antenna.label, index)),
                        &format!("/antennas/{index}/{field}"),
                    ),
                    "antenna dimension must be finite and nonnegative",
                    diagnostics,
                );
            }
        }
        if let Some(value) = antenna.orientation_degrees {
            validate_number(
                f64::from(value),
                |value| (0.0..360.0).contains(&value),
                number_location(
                    BundleFileRole::Antennas,
                    Some(BundleRecordKind::Antenna),
                    Some((&antenna.label, index)),
                    &format!("/antennas/{index}/orientation_degrees"),
                ),
                "antenna orientation must be finite and in [0, 360) degrees",
                diagnostics,
            );
        }
    }
}

fn validate_schedule(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    if bundle.schedule.slots.is_empty() {
        diagnostics.push(semantic_warning(
            codes::EMPTY_SCHEDULE,
            BundleFileRole::Schedule,
            "/slots",
            "newly authored schedules require at least one slot".into(),
        ));
    }

    let mut sequence_numbers = HashMap::<u32, usize>::new();
    let mut previous_sequence = None;
    for (index, slot) in bundle.schedule.slots.iter().enumerate() {
        let slot_location = |field: &str| {
            location(
                BundleFileRole::Schedule,
                Some(BundleRecordKind::Slot),
                Some(&slot.slot_id),
                Some(index),
                Some(format!("/slots/{index}/{field}")),
            )
        };
        if let Some(first_index) = sequence_numbers.insert(slot.sequence_number, index) {
            diagnostics.push(BundleDiagnostic {
                code: codes::DUPLICATE_SEQUENCE_NUMBER.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: WRITE_OPERATIONS.to_vec(),
                location: slot_location("sequence_number"),
                message: format!(
                    "slot sequence number {} is duplicated",
                    slot.sequence_number
                ),
                related_locations: vec![location(
                    BundleFileRole::Schedule,
                    Some(BundleRecordKind::Slot),
                    Some(&bundle.schedule.slots[first_index].slot_id),
                    Some(first_index),
                    Some(format!("/slots/{first_index}/sequence_number")),
                )],
            });
        }
        if previous_sequence.is_some_and(|previous| slot.sequence_number < previous) {
            diagnostics.push(BundleDiagnostic {
                code: codes::SLOT_SEQUENCE_OUT_OF_ORDER.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
                location: slot_location("sequence_number"),
                message: "slot sequence numbers must be strictly increasing in persisted order"
                    .into(),
                related_locations: Vec::new(),
            });
        }
        previous_sequence = Some(slot.sequence_number);

        if slot.duration_seconds == 0 {
            diagnostics.push(BundleDiagnostic {
                code: codes::INVALID_SLOT_DURATION.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
                location: slot_location("duration_seconds"),
                message: "slot duration must be greater than zero".into(),
                related_locations: Vec::new(),
            });
        }
        if slot.guard_seconds >= slot.duration_seconds {
            diagnostics.push(BundleDiagnostic {
                code: codes::INVALID_SLOT_GUARD.into(),
                category: BundleDiagnosticCategory::Semantic,
                severity: BundleDiagnosticSeverity::Error,
                blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
                location: slot_location("guard_seconds"),
                message: "slot guard time must be less than slot duration".into(),
                related_locations: Vec::new(),
            });
        }
    }

    let distinct_labels = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| slot.antenna_label.as_str())
        .collect::<HashSet<_>>()
        .len();
    let shape_matches = match bundle.schedule.mode {
        ExperimentMode::SingleAntennaProfiling => {
            distinct_labels == 1 && bundle.schedule.goal == SessionGoal::SingleAntennaProfiling
        }
        ExperimentMode::WholeStationAb | ExperimentMode::TxFocused | ExperimentMode::RxFocused => {
            distinct_labels >= 2 && bundle.schedule.goal != SessionGoal::SingleAntennaProfiling
        }
    };
    if !shape_matches {
        diagnostics.push(semantic_warning(
            codes::EXPERIMENT_SHAPE_MISMATCH,
            BundleFileRole::Schedule,
            "/mode",
            "experiment mode, goal, and distinct scheduled antennas do not form a supported shape"
                .into(),
        ));
    }
}

fn validate_observations(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    for (index, observation) in bundle.observations.iter().enumerate() {
        let record = Some((observation.observation_id.as_str(), index));
        if observation.frequency_hz == Some(0) {
            invalid_range(
                number_location(
                    BundleFileRole::Observations,
                    Some(BundleRecordKind::Observation),
                    record,
                    "/frequency_hz",
                ),
                "observation frequency must be greater than zero",
                diagnostics,
            );
        }
        for (field, value, rule, predicate) in [
            (
                "/distance_km",
                observation.distance_km,
                "observation distance must be finite and nonnegative",
                nonnegative as fn(f64) -> bool,
            ),
            (
                "/azimuth_degrees",
                observation.azimuth_degrees,
                "observation azimuth must be finite and in [0, 360) degrees",
                degrees,
            ),
        ] {
            if let Some(value) = value {
                validate_number(
                    value,
                    predicate,
                    number_location(
                        BundleFileRole::Observations,
                        Some(BundleRecordKind::Observation),
                        record,
                        field,
                    ),
                    rule,
                    diagnostics,
                );
            }
        }
        if let Some(value) = observation.snr_db {
            validate_write_only_finite(
                f64::from(value),
                number_location(
                    BundleFileRole::Observations,
                    Some(BundleRecordKind::Observation),
                    record,
                    "/snr_db",
                ),
                "observation SNR must be finite",
                diagnostics,
            );
        }
        if let Some(value) = observation.drift_hz_per_minute {
            validate_number(
                f64::from(value),
                |_| true,
                number_location(
                    BundleFileRole::Observations,
                    Some(BundleRecordKind::Observation),
                    record,
                    "/drift_hz_per_minute",
                ),
                "observation drift must be finite",
                diagnostics,
            );
        }
        if let Some(value) = observation.power_watts {
            validate_number(
                f64::from(value),
                positive,
                number_location(
                    BundleFileRole::Observations,
                    Some(BundleRecordKind::Observation),
                    record,
                    "/power_watts",
                ),
                "observation power must be finite and greater than zero",
                diagnostics,
            );
        }
        if observation
            .slot_confidence
            .is_some_and(|value| !value.is_finite())
        {
            diagnostics.push(number_diagnostic(
                codes::NON_FINITE_NUMBER,
                number_location(
                    BundleFileRole::Observations,
                    Some(BundleRecordKind::Observation),
                    record,
                    "/slot_confidence",
                ),
                "slot confidence must be finite and in [0, 1]".into(),
            ));
        }
    }
}

fn validate_rig(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    for (index, record) in bundle.rig.iter().enumerate() {
        if record.frequency_hz == Some(0) {
            invalid_range(
                number_location(
                    BundleFileRole::Rig,
                    Some(BundleRecordKind::RigRecord),
                    Some((&record.record_id, index)),
                    "/frequency_hz",
                ),
                "rig frequency must be greater than zero",
                diagnostics,
            );
        }
    }
}

fn validate_propagation(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    for (index, record) in bundle.propagation.iter().enumerate() {
        let identity = Some((record.record_id.as_str(), index));
        for (field, value, predicate, rule) in [
            (
                "/solar_flux_f107",
                record.solar_flux_f107,
                nonnegative as fn(f64) -> bool,
                "solar flux must be finite and nonnegative",
            ),
            (
                "/kp_index",
                record.kp_index,
                kp,
                "planetary Kp must be finite and in [0, 9]",
            ),
            (
                "/solar_wind_speed_kms",
                record.solar_wind_speed_kms,
                nonnegative,
                "solar wind speed must be finite and nonnegative",
            ),
            ("/bz_nt", record.bz_nt, any_finite, "Bz must be finite"),
        ] {
            if let Some(value) = value {
                validate_number(
                    f64::from(value),
                    predicate,
                    number_location(
                        BundleFileRole::Propagation,
                        Some(BundleRecordKind::PropagationRecord),
                        identity,
                        field,
                    ),
                    rule,
                    diagnostics,
                );
            }
        }
    }
}

fn validate_analysis(bundle: &BundleContents, diagnostics: &mut Vec<BundleDiagnostic>) {
    if bundle.analysis.status == AnalysisStatus::Generated && bundle.analysis.generated_at.is_none()
    {
        diagnostics.push(semantic_warning(
            codes::ANALYSIS_METADATA_MISMATCH,
            BundleFileRole::Analysis,
            "/generated_at",
            "generated analysis metadata requires a generation timestamp".into(),
        ));
    }
}

fn validate_number(
    value: f64,
    range: impl Fn(f64) -> bool,
    location: BundleDiagnosticLocation,
    rule: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    if !value.is_finite() {
        diagnostics.push(number_diagnostic(
            codes::NON_FINITE_NUMBER,
            location,
            format!("{rule}; found non-finite value {value}"),
        ));
    } else if !range(value) {
        diagnostics.push(number_diagnostic(
            codes::INVALID_RANGE,
            location,
            format!("{rule}; found {value}"),
        ));
    }
}

fn invalid_range(
    location: BundleDiagnosticLocation,
    message: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    diagnostics.push(number_diagnostic(
        codes::INVALID_RANGE,
        location,
        message.into(),
    ));
}

fn validate_write_only_finite(
    value: f64,
    location: BundleDiagnosticLocation,
    rule: &str,
    diagnostics: &mut Vec<BundleDiagnostic>,
) {
    if !value.is_finite() {
        diagnostics.push(BundleDiagnostic {
            code: codes::NON_FINITE_NUMBER.into(),
            category: BundleDiagnosticCategory::Semantic,
            severity: BundleDiagnosticSeverity::Error,
            blocked_operations: WRITE_OPERATIONS.to_vec(),
            location,
            message: format!("{rule}; found non-finite value {value}"),
            related_locations: Vec::new(),
        });
    }
}

fn number_diagnostic(
    code: &str,
    location: BundleDiagnosticLocation,
    message: String,
) -> BundleDiagnostic {
    BundleDiagnostic {
        code: code.into(),
        category: BundleDiagnosticCategory::Semantic,
        severity: BundleDiagnosticSeverity::Error,
        blocked_operations: ANALYSIS_AND_WRITE_OPERATIONS.to_vec(),
        location,
        message,
        related_locations: Vec::new(),
    }
}

fn number_location(
    file: BundleFileRole,
    kind: Option<BundleRecordKind>,
    record: Option<(&str, usize)>,
    field_path: &str,
) -> BundleDiagnosticLocation {
    location(
        file,
        kind,
        record.map(|(id, _)| id),
        record.map(|(_, index)| index),
        Some(field_path.into()),
    )
}

fn semantic_warning(
    code: &str,
    file: BundleFileRole,
    field_path: &str,
    message: String,
) -> BundleDiagnostic {
    BundleDiagnostic {
        code: code.into(),
        category: BundleDiagnosticCategory::Semantic,
        severity: BundleDiagnosticSeverity::Warning,
        blocked_operations: vec![BundleValidationProfile::StrictCreation],
        location: location(file, None, None, None, Some(field_path.into())),
        message,
        related_locations: Vec::new(),
    }
}

fn location(
    file: BundleFileRole,
    record_kind: Option<BundleRecordKind>,
    record_id: Option<&str>,
    record_index: Option<usize>,
    field_path: Option<String>,
) -> BundleDiagnosticLocation {
    BundleDiagnosticLocation {
        file,
        record_kind,
        record_id: record_id.map(str::to_string),
        record_index,
        physical_line: matches!(
            file,
            BundleFileRole::Events
                | BundleFileRole::Observations
                | BundleFileRole::WsjtX
                | BundleFileRole::Rig
                | BundleFileRole::Propagation
        )
        .then(|| record_index.map(|index| index + 1))
        .flatten(),
        field_path,
    }
}

fn positive(value: f64) -> bool {
    value > 0.0
}

fn nonnegative(value: f64) -> bool {
    value >= 0.0
}

fn degrees(value: f64) -> bool {
    (0.0..360.0).contains(&value)
}

fn kp(value: f64) -> bool {
    (0.0..=9.0).contains(&value)
}

fn any_finite(_: f64) -> bool {
    true
}
