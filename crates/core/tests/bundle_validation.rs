use antennabench_core::{
    codes, validate_bundle, validate_bundle_report, AnalysisFile, AnalysisStatus, Antenna,
    AntennasFile, Band, BundleContents, BundleFiles, BundleManifest, BundleValidationIssue,
    BundleValidationProfile, ExperimentMode, ObservationKind, ObservationRecord, OperatorEvent,
    OperatorEventType, PlannedSlot, PropagationRecord, RecordMeta, RecordSource, RigRecord,
    Schedule, SessionGoal, Station, WsjtXRecord,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-validation-test";

#[test]
fn accepts_a_valid_bundle() {
    let bundle = valid_bundle();

    validate_bundle(&bundle).unwrap();
}

#[test]
fn reports_schema_version_and_session_id_mismatches() {
    let mut bundle = valid_bundle();
    bundle.station.schema_version = 2;
    bundle.schedule.session_id = "other-session".to_string();
    bundle.events[0].meta.schema_version = 2;
    bundle.observations[0].meta.session_id = "other-session".to_string();
    bundle.wsjtx[0].meta.session_id = "other-session".to_string();
    bundle.analysis.schema_version = 2;

    let error = validate_bundle(&bundle).unwrap_err();
    let non_alignment_issues = error
        .issues()
        .iter()
        .filter(|issue| {
            !matches!(
                issue,
                BundleValidationIssue::AlignmentAnnotationMismatch { .. }
            )
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(
        non_alignment_issues,
        @r###"
    [
        UnexpectedSchemaVersion {
            file: Station,
            record_id: None,
            expected: 1,
            actual: 2,
        },
        SessionIdMismatch {
            file: Schedule,
            record_id: None,
            expected: "session-validation-test",
            actual: "other-session",
        },
        UnexpectedSchemaVersion {
            file: Analysis,
            record_id: None,
            expected: 1,
            actual: 2,
        },
        UnexpectedSchemaVersion {
            file: Events,
            record_id: Some(
                "event-001",
            ),
            expected: 1,
            actual: 2,
        },
        SessionIdMismatch {
            file: Observations,
            record_id: Some(
                "obs-001",
            ),
            expected: "session-validation-test",
            actual: "other-session",
        },
        SessionIdMismatch {
            file: WsjtX,
            record_id: Some(
                "wsjtx-001",
            ),
            expected: "session-validation-test",
            actual: "other-session",
        },
    ]
    "###
    );
}

#[test]
fn reports_duplicate_ids_unknown_references_bad_windows_and_invalid_confidence() {
    let mut bundle = valid_bundle();
    let starts_at = bundle.schedule.slots[0].starts_at;
    bundle.schedule.slots.push(planned_slot(
        "slot-002",
        3,
        starts_at + chrono::Duration::seconds(60),
        "missing-antenna",
    ));
    bundle.events.push(operator_event(
        "event-001",
        "missing-slot",
        OperatorEventType::Switched,
        starts_at + chrono::Duration::seconds(10),
    ));
    bundle.observations.push(observation(
        "obs-001",
        starts_at + chrono::Duration::seconds(90),
        Some("missing-slot"),
        Some("A"),
        Some(1.5),
    ));
    bundle.wsjtx.push(WsjtXRecord {
        meta: record_meta(starts_at, RecordSource::WsjtxLog),
        record_id: "wsjtx-001".to_string(),
        message_type: "status_snapshot".to_string(),
        raw: json!({}),
    });
    bundle.rig.push(RigRecord {
        meta: record_meta(starts_at, RecordSource::RigAdapter),
        record_id: "rig-001".to_string(),
        status: "duplicate".to_string(),
        frequency_hz: None,
        mode: None,
        raw: json!({}),
    });
    bundle.propagation.push(PropagationRecord {
        meta: record_meta(starts_at, RecordSource::ImportedFile),
        record_id: "prop-001".to_string(),
        observed_at: starts_at,
        solar_flux_f107: None,
        sunspot_number: None,
        kp_index: None,
        a_index: None,
        solar_wind_speed_kms: None,
        bz_nt: None,
        alerts: Vec::new(),
        daylight_state: None,
        raw: json!({}),
    });

    let error = validate_bundle(&bundle).unwrap_err();
    let non_alignment_issues = error
        .issues()
        .iter()
        .filter(|issue| {
            !matches!(
                issue,
                BundleValidationIssue::AlignmentAnnotationMismatch { .. }
            )
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(
        non_alignment_issues,
        @r###"
    [
        DuplicateId {
            kind: Slot,
            id: "slot-002",
        },
        DuplicateId {
            kind: OperatorEvent,
            id: "event-001",
        },
        DuplicateId {
            kind: Observation,
            id: "obs-001",
        },
        DuplicateId {
            kind: WsjtXRecord,
            id: "wsjtx-001",
        },
        DuplicateId {
            kind: RigRecord,
            id: "rig-001",
        },
        DuplicateId {
            kind: PropagationRecord,
            id: "prop-001",
        },
        UnknownAntennaLabel {
            slot_id: "slot-002",
            antenna_label: "missing-antenna",
        },
        SlotWindowOutOfOrder {
            previous_slot_id: "slot-002",
            slot_id: "slot-002",
        },
        SlotWindowOverlap {
            previous_slot_id: "slot-002",
            previous_ends_at: 2026-07-10T20:04:00Z,
            slot_id: "slot-002",
            starts_at: 2026-07-10T20:01:00Z,
        },
        UnknownEventSlot {
            event_id: "event-001",
            slot_id: "missing-slot",
        },
        UnknownObservationSlot {
            observation_id: "obs-001",
            slot_id: "missing-slot",
        },
        InvalidSlotConfidence {
            observation_id: "obs-001",
            slot_confidence: 1.5,
        },
    ]
    "###
    );
}

#[test]
fn out_of_order_non_overlapping_slots_do_not_report_overlap() {
    let mut bundle = valid_bundle();
    bundle.schedule.slots[1].starts_at =
        bundle.schedule.slots[0].starts_at - chrono::Duration::seconds(180);
    bundle.schedule.slots[1].duration_seconds = 60;

    let error = validate_bundle(&bundle).unwrap_err();
    let window_issues = error
        .issues()
        .iter()
        .filter(|issue| {
            matches!(
                issue,
                BundleValidationIssue::SlotWindowOutOfOrder { .. }
                    | BundleValidationIssue::SlotWindowOverlap { .. }
            )
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(
        window_issues,
        @r###"
    [
        SlotWindowOutOfOrder {
            previous_slot_id: "slot-001",
            slot_id: "slot-002",
        },
    ]
    "###
    );
}

#[test]
fn reports_persisted_alignment_annotation_mismatches() {
    let mut bundle = valid_bundle();
    let observation = &mut bundle.observations[0];
    observation.slot_id = Some("slot-002".to_string());
    observation.slot_label = Some("B".to_string());
    observation.slot_confidence = Some(0.25);

    let error = validate_bundle(&bundle).unwrap_err();

    insta::assert_debug_snapshot!(
        error.issues(),
        @r###"
    [
        AlignmentAnnotationMismatch {
            observation_id: "obs-001",
            field: SlotId,
            expected: "Some(\"slot-001\")",
            actual: "Some(\"slot-002\")",
        },
        AlignmentAnnotationMismatch {
            observation_id: "obs-001",
            field: SlotLabel,
            expected: "Some(\"A\")",
            actual: "Some(\"B\")",
        },
        AlignmentAnnotationMismatch {
            observation_id: "obs-001",
            field: SlotConfidence,
            expected: "Some(0.95)",
            actual: "Some(0.25)",
        },
    ]
    "###
    );
}

#[test]
fn validation_profiles_distinguish_compatibility_analysis_and_writes() {
    let mut stale = valid_bundle();
    stale.observations[0].slot_label = Some("stale".to_string());
    let stale_report = validate_bundle_report(&stale);
    assert!(stale_report.allows(BundleValidationProfile::CompatibilityRead));
    assert!(stale_report.allows(BundleValidationProfile::Analysis));
    assert!(!stale_report.allows(BundleValidationProfile::StrictCreation));
    assert!(stale_report.allows(BundleValidationProfile::Upgrade));

    let mut semantically_invalid = valid_bundle();
    semantically_invalid.observations[0].slot_confidence = Some(1.5);
    let semantic_report = validate_bundle_report(&semantically_invalid);
    assert!(semantic_report.allows(BundleValidationProfile::CompatibilityRead));
    assert!(!semantic_report.allows(BundleValidationProfile::Analysis));
    assert!(!semantic_report.allows(BundleValidationProfile::StrictCreation));
    assert!(!semantic_report.allows(BundleValidationProfile::Upgrade));

    let mut structurally_invalid = valid_bundle();
    structurally_invalid.events[0].slot_id = Some("missing-slot".to_string());
    let structural_report = validate_bundle_report(&structurally_invalid);
    for profile in [
        BundleValidationProfile::CompatibilityRead,
        BundleValidationProfile::Analysis,
        BundleValidationProfile::StrictCreation,
        BundleValidationProfile::Upgrade,
    ] {
        assert!(!structural_report.allows(profile));
    }
}

#[test]
fn diagnostic_records_have_stable_codes_locations_and_ordering() {
    let mut bundle = valid_bundle();
    bundle.events[0].slot_id = Some("missing-slot".to_string());
    bundle.observations[0].slot_confidence = Some(1.5);

    let report = validate_bundle_report(&bundle);
    let summary = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == codes::UNKNOWN_EVENT_SLOT
                || diagnostic.code == codes::INVALID_SLOT_CONFIDENCE
        })
        .map(|diagnostic| {
            (
                diagnostic.code.as_str(),
                diagnostic.location.file,
                diagnostic.location.record_id.as_deref(),
                diagnostic.location.record_index,
                diagnostic.location.physical_line,
                diagnostic.location.field_path.as_deref(),
                diagnostic.blocked_operations.clone(),
            )
        })
        .collect::<Vec<_>>();

    insta::assert_debug_snapshot!(summary, @r###"
    [
        (
            "bundle.structure.unknown_event_slot",
            Events,
            Some(
                "event-001",
            ),
            Some(
                0,
            ),
            None,
            Some(
                "/slot_id",
            ),
            [
                CompatibilityRead,
                Analysis,
                StrictCreation,
                Upgrade,
            ],
        ),
        (
            "bundle.semantic.invalid_slot_confidence",
            Observations,
            Some(
                "obs-001",
            ),
            Some(
                0,
            ),
            None,
            Some(
                "/slot_confidence",
            ),
            [
                Analysis,
                StrictCreation,
                Upgrade,
            ],
        ),
    ]
    "###);
}

fn valid_bundle() -> BundleContents {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 10, 20, 0, 0).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 7, 10, 19, 58, 0).unwrap(),
            app_version: "0.1.0".to_string(),
            files: BundleFiles::default(),
        },
        station: Station {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            callsign: "N1RWJ".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            antennas: vec![antenna("A", "vertical"), antenna("B", "dipole")],
        },
        schedule: Schedule {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            slots: vec![
                planned_slot("slot-001", 1, starts_at, "A"),
                planned_slot(
                    "slot-002",
                    2,
                    starts_at + chrono::Duration::seconds(120),
                    "B",
                ),
            ],
        },
        events: vec![
            operator_event(
                "event-001",
                "slot-001",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(3),
            ),
            operator_event(
                "event-002",
                "slot-002",
                OperatorEventType::Switched,
                starts_at + chrono::Duration::seconds(123),
            ),
        ],
        observations: vec![
            observation(
                "obs-001",
                starts_at + chrono::Duration::seconds(60),
                Some("slot-001"),
                Some("A"),
                Some(0.95),
            ),
            observation(
                "obs-002",
                starts_at + chrono::Duration::seconds(180),
                Some("slot-002"),
                Some("B"),
                Some(0.95),
            ),
        ],
        wsjtx: vec![WsjtXRecord {
            meta: record_meta(
                starts_at - chrono::Duration::seconds(5),
                RecordSource::WsjtxLog,
            ),
            record_id: "wsjtx-001".to_string(),
            message_type: "status_snapshot".to_string(),
            raw: json!({"mode": "WSPR"}),
        }],
        rig: vec![RigRecord {
            meta: record_meta(
                starts_at - chrono::Duration::seconds(4),
                RecordSource::RigAdapter,
            ),
            record_id: "rig-001".to_string(),
            status: "manual_confirmation".to_string(),
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".to_string()),
            raw: json!({"operator_confirmed": true}),
        }],
        propagation: vec![PropagationRecord {
            meta: record_meta(starts_at, RecordSource::ImportedFile),
            record_id: "prop-001".to_string(),
            observed_at: starts_at,
            solar_flux_f107: Some(145.0),
            sunspot_number: Some(88),
            kp_index: Some(2.0),
            a_index: Some(8),
            solar_wind_speed_kms: None,
            bz_nt: None,
            alerts: Vec::new(),
            daylight_state: Some("mixed_path".to_string()),
            raw: json!({"fixture": true}),
        }],
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    }
}

fn antenna(label: &str, facet: &str) -> Antenna {
    Antenna {
        label: label.to_string(),
        facets: vec![facet.to_string()],
        height_m: None,
        radial_count: None,
        radial_length_m: None,
        orientation_degrees: None,
        tuner: None,
        feedline: None,
        notes: None,
    }
}

fn planned_slot(
    slot_id: &str,
    sequence_number: u32,
    starts_at: chrono::DateTime<Utc>,
    antenna_label: &str,
) -> PlannedSlot {
    PlannedSlot {
        slot_id: slot_id.to_string(),
        sequence_number,
        starts_at,
        duration_seconds: 120,
        guard_seconds: 15,
        band: Band::M20,
        antenna_label: antenna_label.to_string(),
    }
}

fn operator_event(
    event_id: &str,
    slot_id: &str,
    event_type: OperatorEventType,
    timestamp: chrono::DateTime<Utc>,
) -> OperatorEvent {
    OperatorEvent {
        meta: record_meta(timestamp, RecordSource::Operator),
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type,
        note: None,
    }
}

fn observation(
    observation_id: &str,
    timestamp: chrono::DateTime<Utc>,
    slot_id: Option<&str>,
    slot_label: Option<&str>,
    slot_confidence: Option<f32>,
) -> ObservationRecord {
    ObservationRecord {
        meta: record_meta(timestamp, RecordSource::Wsprnet),
        observation_id: observation_id.to_string(),
        observation_kind: ObservationKind::PublicReport,
        band: Band::M20,
        frequency_hz: Some(14_095_600),
        mode: Some("WSPR".to_string()),
        reporter_call: Some("K1ABC".to_string()),
        heard_call: Some("N1RWJ".to_string()),
        reporter_grid: Some("FN31".to_string()),
        heard_grid: Some("FN42".to_string()),
        distance_km: Some(150.0),
        azimuth_degrees: Some(240.0),
        snr_db: Some(-18.0),
        drift_hz_per_minute: Some(0.0),
        power_watts: Some(5.0),
        slot_id: slot_id.map(str::to_string),
        slot_label: slot_label.map(str::to_string),
        slot_confidence,
        raw: json!({}),
    }
}

fn record_meta(timestamp: chrono::DateTime<Utc>, source: RecordSource) -> RecordMeta {
    RecordMeta {
        schema_version: 1,
        session_id: SESSION_ID.to_string(),
        timestamp,
        source,
    }
}
