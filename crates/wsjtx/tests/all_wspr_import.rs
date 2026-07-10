use antennabench_core::{
    Band, BundleContents, ObservationKind, ObservationRecord, RecordSource, WsjtXRecord,
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
