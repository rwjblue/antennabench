use antennabench_core::{
    normalize_bundle, validate_bundle, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band,
    BundleContents, BundleFiles, BundleManifest, ExperimentMode, ObservationKind,
    ObservationRecord, OperatorEvent, OperatorEventType, PlannedSlot, RecordMeta, RecordSource,
    Schedule, SessionGoal, Station, WsjtXRecord,
};
use antennabench_wsjtx::{
    append_wsjtx_import, import_all_wspr_text, import_parsed_all_wspr_text, parse_all_wspr_line,
    parse_all_wspr_text, AllWsprLineIssueKind, WsjtxImport, WsjtxImportConfig,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

const SESSION_ID: &str = "session-wsjtx-import-test";

fn import_config(import_id: &str) -> WsjtxImportConfig {
    WsjtxImportConfig {
        session_id: SESSION_ID.to_string(),
        import_id: import_id.to_string(),
        station_callsign: "N1RWJ".to_string(),
        station_grid: "FN42".to_string(),
        imported_at: Utc.with_ymd_and_hms(2026, 7, 9, 19, 59, 55).unwrap(),
    }
}

#[test]
fn parses_all_wspr_decode_rows() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_sample.txt");

    let parsed = parse_all_wspr_text(input);

    assert!(parsed.issues.is_empty());
    assert_eq!(parsed.decodes.len(), 3);

    let first = &parsed.decodes[0];
    assert_eq!(first.line_number, 1);
    assert_eq!(first.timestamp.to_rfc3339(), "2026-07-09T20:02:00+00:00");
    assert_eq!(first.snr_db, -18.0);
    assert_eq!(first.dt_seconds, 0.07);
    assert_eq!(first.frequency_hz, 14_095_600);
    assert_eq!(first.frequency_mhz_text, "14.095600");
    assert_eq!(first.band, Band::M20);
    assert_eq!(first.tx_call, "K1ABC");
    assert_eq!(first.tx_grid, "EM12");
    assert_eq!(first.tx_power_dbm, 37);
    assert!((first.tx_power_watts - 5.011872).abs() < 0.000001);
    assert_eq!(first.drift_hz_per_minute, 0.0);

    assert_eq!(parsed.decodes[1].extra_fields, vec!["0.19", "2"]);
    assert_eq!(parsed.decodes[2].band, Band::M40);
}

#[test]
fn parse_line_reports_too_few_fields() {
    let issue = parse_all_wspr_line(7, "260709 2002 -18").unwrap_err();

    assert_eq!(issue.line_number, 7);
    assert_eq!(issue.kind, AllWsprLineIssueKind::TooFewFields { actual: 3 });
}

#[test]
fn parse_text_collects_malformed_lines_without_losing_valid_decodes() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_mixed_quality.txt");

    let parsed = parse_all_wspr_text(input);

    assert_eq!(parsed.decodes.len(), 2);
    assert_eq!(parsed.decodes[0].tx_call, "K1ABC");
    assert_eq!(parsed.decodes[1].tx_call, "VE3ZZZ");

    insta::assert_debug_snapshot!(
        parsed.issues.iter().map(|issue| (&issue.line_number, &issue.kind)).collect::<Vec<_>>(),
        @r###"
    [
        (
            2,
            TooFewFields {
                actual: 5,
            },
        ),
        (
            3,
            InvalidSnr {
                value: "xx",
            },
        ),
        (
            5,
            UnsupportedBand {
                frequency_hz: 99999999,
            },
        ),
    ]
    "###
    );
}

#[test]
fn imports_valid_lines_into_raw_wsjtx_records_and_observations() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_sample.txt");

    let import = import_all_wspr_text(input, import_config("sample-all-wspr")).unwrap();

    assert!(import.issues.is_empty());
    assert_eq!(import.wsjtx_records.len(), 3);
    assert_eq!(import.observations.len(), 3);
    assert_eq!(import.wsjtx_records[0].meta.source, RecordSource::WsjtxLog);
    assert_eq!(
        import.observations[0].observation_kind,
        ObservationKind::LocalDecode
    );
    let _append_fn: fn(&mut BundleContents, WsjtxImport) = append_wsjtx_import;

    let wsjtx_records: Vec<&WsjtXRecord> = import.wsjtx_records.iter().collect();
    insta::assert_json_snapshot!(
        wsjtx_records,
        @r#"
    [
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:02:00Z",
          "source": "wsjtx_log"
        },
        "record_id": "sample-all-wspr-wsjtx-000001",
        "message_type": "all_wspr_decode",
        "raw": {
          "fields": [
            "260709",
            "2002",
            "-18",
            "0.07",
            "14.095600",
            "K1ABC",
            "EM12",
            "37",
            "0"
          ],
          "line": "260709 2002 -18 0.07 14.095600 K1ABC EM12 37 0",
          "line_number": 1
        }
      },
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:04:00Z",
          "source": "wsjtx_log"
        },
        "record_id": "sample-all-wspr-wsjtx-000002",
        "message_type": "all_wspr_decode",
        "raw": {
          "fields": [
            "260709",
            "2004",
            "-24",
            "-0.12",
            "14.095640",
            "W3AAA",
            "FM19",
            "30",
            "-1",
            "0.19",
            "2"
          ],
          "line": "260709 2004 -24 -0.12 14.095640 W3AAA FM19 30 -1 0.19 2",
          "line_number": 2
        }
      },
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:06:00Z",
          "source": "wsjtx_log"
        },
        "record_id": "sample-all-wspr-wsjtx-000003",
        "message_type": "all_wspr_decode",
        "raw": {
          "fields": [
            "260709",
            "2006",
            "-27",
            "0.00",
            "7.040047",
            "VE3ZZZ",
            "FN03",
            "23",
            "1"
          ],
          "line": "260709 2006 -27 0.00 7.040047 VE3ZZZ FN03 23 1",
          "line_number": 3
        }
      }
    ]
    "#
    );

    let observations: Vec<&ObservationRecord> = import.observations.iter().collect();
    insta::assert_json_snapshot!(
        observations,
        @r#"
    [
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:02:00Z",
          "source": "wsjtx_log"
        },
        "observation_id": "sample-all-wspr-obs-000001",
        "observation_kind": "local_decode",
        "band": "20m",
        "frequency_hz": 14095600,
        "mode": "WSPR",
        "reporter_call": "N1RWJ",
        "heard_call": "K1ABC",
        "reporter_grid": "FN42",
        "heard_grid": "EM12",
        "distance_km": null,
        "azimuth_degrees": null,
        "snr_db": -18.0,
        "drift_hz_per_minute": 0.0,
        "power_watts": 5.0118723,
        "slot_id": null,
        "slot_label": null,
        "slot_confidence": null,
        "raw": {
          "dt_seconds": 0.07000000029802322,
          "extra_fields": [],
          "fields": [
            "260709",
            "2002",
            "-18",
            "0.07",
            "14.095600",
            "K1ABC",
            "EM12",
            "37",
            "0"
          ],
          "frequency_mhz": "14.095600",
          "line": "260709 2002 -18 0.07 14.095600 K1ABC EM12 37 0",
          "line_number": 1,
          "tx_power_dbm": 37
        }
      },
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:04:00Z",
          "source": "wsjtx_log"
        },
        "observation_id": "sample-all-wspr-obs-000002",
        "observation_kind": "local_decode",
        "band": "20m",
        "frequency_hz": 14095640,
        "mode": "WSPR",
        "reporter_call": "N1RWJ",
        "heard_call": "W3AAA",
        "reporter_grid": "FN42",
        "heard_grid": "FM19",
        "distance_km": null,
        "azimuth_degrees": null,
        "snr_db": -24.0,
        "drift_hz_per_minute": -1.0,
        "power_watts": 1.0,
        "slot_id": null,
        "slot_label": null,
        "slot_confidence": null,
        "raw": {
          "dt_seconds": -0.11999999731779099,
          "extra_fields": [
            "0.19",
            "2"
          ],
          "fields": [
            "260709",
            "2004",
            "-24",
            "-0.12",
            "14.095640",
            "W3AAA",
            "FM19",
            "30",
            "-1",
            "0.19",
            "2"
          ],
          "frequency_mhz": "14.095640",
          "line": "260709 2004 -24 -0.12 14.095640 W3AAA FM19 30 -1 0.19 2",
          "line_number": 2,
          "tx_power_dbm": 30
        }
      },
      {
        "meta": {
          "schema_version": 1,
          "session_id": "session-wsjtx-import-test",
          "timestamp": "2026-07-09T20:06:00Z",
          "source": "wsjtx_log"
        },
        "observation_id": "sample-all-wspr-obs-000003",
        "observation_kind": "local_decode",
        "band": "40m",
        "frequency_hz": 7040047,
        "mode": "WSPR",
        "reporter_call": "N1RWJ",
        "heard_call": "VE3ZZZ",
        "reporter_grid": "FN42",
        "heard_grid": "FN03",
        "distance_km": null,
        "azimuth_degrees": null,
        "snr_db": -27.0,
        "drift_hz_per_minute": 1.0,
        "power_watts": 0.19952624,
        "slot_id": null,
        "slot_label": null,
        "slot_confidence": null,
        "raw": {
          "dt_seconds": 0.0,
          "extra_fields": [],
          "fields": [
            "260709",
            "2006",
            "-27",
            "0.00",
            "7.040047",
            "VE3ZZZ",
            "FN03",
            "23",
            "1"
          ],
          "frequency_mhz": "7.040047",
          "line": "260709 2006 -27 0.00 7.040047 VE3ZZZ FN03 23 1",
          "line_number": 3,
          "tx_power_dbm": 23
        }
      }
    ]
    "#
    );
}

#[test]
fn preserves_malformed_lines_as_wsjtx_records_without_observations() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_mixed_quality.txt");

    let import = import_all_wspr_text(input, import_config("mixed-quality")).unwrap();

    assert_eq!(import.wsjtx_records.len(), 5);
    assert_eq!(import.observations.len(), 2);
    assert_eq!(import.issues.len(), 3);

    let malformed_records: Vec<_> = import
        .wsjtx_records
        .iter()
        .filter(|record| record.message_type == "all_wspr_malformed")
        .map(|record| {
            json!({
                "record_id": record.record_id,
                "timestamp": record.meta.timestamp,
                "raw": record.raw,
            })
        })
        .collect();

    insta::assert_json_snapshot!(
        malformed_records,
        @r#"
    [
      {
        "raw": {
          "error": "too few fields: expected at least 9, got 5",
          "fields": [
            "bad",
            "line",
            "with",
            "too",
            "few"
          ],
          "line": "bad line with too few",
          "line_number": 2
        },
        "record_id": "mixed-quality-wsjtx-000002",
        "timestamp": "2026-07-09T19:59:55Z"
      },
      {
        "raw": {
          "error": "invalid SNR: xx",
          "fields": [
            "260709",
            "2004",
            "xx",
            "-0.12",
            "14.095640",
            "W3AAA",
            "FM19",
            "30",
            "-1"
          ],
          "line": "260709 2004 xx -0.12 14.095640 W3AAA FM19 30 -1",
          "line_number": 3
        },
        "record_id": "mixed-quality-wsjtx-000003",
        "timestamp": "2026-07-09T19:59:55Z"
      },
      {
        "raw": {
          "error": "unsupported band for frequency 99999999 Hz",
          "fields": [
            "260709",
            "2008",
            "-12",
            "0.10",
            "99.999999",
            "BADBAND",
            "FN42",
            "23",
            "0"
          ],
          "line": "260709 2008 -12 0.10 99.999999 BADBAND FN42 23 0",
          "line_number": 5
        },
        "record_id": "mixed-quality-wsjtx-000005",
        "timestamp": "2026-07-09T19:59:55Z"
      }
    ]
    "#
    );
}

#[test]
fn parsed_import_uses_parsed_raw_lines_as_source_of_truth() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_mixed_quality.txt");
    let parsed = parse_all_wspr_text(input);

    let import = import_parsed_all_wspr_text(
        parsed,
        "wrong one\nwrong two\nwrong three\nwrong four\nwrong five",
        import_config("parsed-source"),
    )
    .unwrap();

    assert_eq!(
        import.wsjtx_records[0].raw["line"],
        "260709 2002 -18 0.07 14.095600 K1ABC EM12 37 0"
    );
    assert_eq!(
        import.observations[0].raw["line"],
        "260709 2002 -18 0.07 14.095600 K1ABC EM12 37 0"
    );
    assert_eq!(import.wsjtx_records[1].raw["line"], "bad line with too few");
    assert_eq!(import.issues[0].raw_line, "bad line with too few");
}

#[test]
fn appends_imported_records_then_normalizes_and_validates_bundle() {
    let mut bundle = sample_bundle();
    let input = "\
260709 2001 -18 0.07 14.095600 K1ABC EM12 37 0
260709 2003 -24 -0.12 14.095640 W3AAA FM19 30 -1
";
    let import = import_all_wspr_text(input, import_config("normalization")).unwrap();

    append_wsjtx_import(&mut bundle, import);
    let normalized = normalize_bundle(bundle);
    validate_bundle(&normalized).unwrap();

    let imported_annotations: Vec<_> = normalized
        .observations
        .iter()
        .filter(|observation| observation.observation_id.starts_with("normalization-obs-"))
        .map(|observation| {
            assert_eq!(observation.slot_confidence, Some(0.95));
            json!({
                "observation_id": observation.observation_id,
                "slot_id": observation.slot_id,
                "slot_label": observation.slot_label,
                "slot_confidence": observation.slot_confidence.map(snapshot_confidence),
            })
        })
        .collect();

    assert_eq!(imported_annotations.len(), 2);
    assert_eq!(
        imported_annotations[0]["slot_id"],
        json!("slot-001"),
        "first imported observation should normalize to slot-001"
    );
    assert_eq!(
        imported_annotations[0]["slot_label"],
        json!("A"),
        "first imported observation should normalize to antenna A"
    );
    assert_eq!(
        imported_annotations[1]["slot_id"],
        json!("slot-002"),
        "second imported observation should normalize to slot-002"
    );
    assert_eq!(
        imported_annotations[1]["slot_label"],
        json!("B"),
        "second imported observation should normalize to antenna B"
    );

    insta::assert_json_snapshot!(
        imported_annotations,
        @r###"
        [
          {
            "observation_id": "normalization-obs-000001",
            "slot_confidence": 0.95,
            "slot_id": "slot-001",
            "slot_label": "A"
          },
          {
            "observation_id": "normalization-obs-000002",
            "slot_confidence": 0.95,
            "slot_id": "slot-002",
            "slot_label": "B"
          }
        ]
        "###
    );
}

fn sample_bundle() -> BundleContents {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 0).unwrap();

    BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            created_at: starts_at - chrono::Duration::seconds(60),
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
            antennas: vec![
                Antenna {
                    label: "A".to_string(),
                    facets: vec!["vertical".to_string()],
                    height_m: None,
                    radial_count: None,
                    radial_length_m: None,
                    orientation_degrees: None,
                    tuner: None,
                    feedline: None,
                    notes: None,
                },
                Antenna {
                    label: "B".to_string(),
                    facets: vec!["dipole".to_string()],
                    height_m: None,
                    radial_count: None,
                    radial_length_m: None,
                    orientation_degrees: None,
                    tuner: None,
                    feedline: None,
                    notes: None,
                },
            ],
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
                starts_at + chrono::Duration::seconds(3),
            ),
            operator_event(
                "event-002",
                "slot-002",
                starts_at + chrono::Duration::seconds(123),
            ),
        ],
        observations: Vec::new(),
        wsjtx: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
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
    timestamp: chrono::DateTime<Utc>,
) -> OperatorEvent {
    OperatorEvent {
        meta: RecordMeta {
            schema_version: 1,
            session_id: SESSION_ID.to_string(),
            timestamp,
            source: RecordSource::Operator,
        },
        event_id: event_id.to_string(),
        slot_id: Some(slot_id.to_string()),
        event_type: OperatorEventType::Switched,
        note: None,
    }
}

fn snapshot_confidence(confidence: f32) -> f64 {
    (f64::from(confidence) * 100.0).round() / 100.0
}
