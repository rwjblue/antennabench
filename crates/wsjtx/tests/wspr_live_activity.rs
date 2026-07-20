use antennabench_core::{
    v2::{AdapterDisposition, AdapterInput, AttachmentReference},
    Band,
};
use antennabench_wsjtx::{
    parse_wspr_live_activity_json, plan_wspr_live_query, prepare_wspr_live_activity,
    prepare_wspr_live_activity_failure, WsprLiveImportConfig, WsprLiveQueryScope,
    WSPR_LIVE_ACTIVITY_COLUMNS, WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT, WSPR_LIVE_ACTIVITY_RECORD_TYPE,
    WSPR_LIVE_ACTIVITY_ROW_LIMIT, WSPR_LIVE_ACTIVITY_SUMMARY_RECORD_TYPE,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn config() -> WsprLiveImportConfig {
    WsprLiveImportConfig {
        session_callsign: "N1RWJ".into(),
        window_start: Utc.with_ymd_and_hms(2026, 7, 19, 2, 0, 0).unwrap(),
        window_end: Utc.with_ymd_and_hms(2026, 7, 19, 3, 0, 0).unwrap(),
        selected_bands: vec![Band::M20],
        captured_at: Utc.with_ymd_and_hms(2026, 7, 19, 3, 5, 0).unwrap(),
        source_locator: Some("https://db1.wspr.live/".into()),
        confirmed_cycles: None,
    }
}

fn attachment() -> AttachmentReference {
    AttachmentReference {
        sha256: "b".repeat(64),
        byte_size: 512,
        media_type: "application/json".into(),
        encoding: None,
        container: Some("clickhouse-format-json".into()),
        source_locator: Some("https://db1.wspr.live/".into()),
    }
}

fn row(time: &str, reporter: &str, grid: &str) -> Value {
    row_on_band(time, 14, reporter, grid)
}

fn row_on_band(time: &str, band: i64, reporter: &str, grid: &str) -> Value {
    json!({
        "time": time,
        "band": band,
        "rx_sign": reporter,
        "rx_loc": grid,
        "spots_decoded": "37",
        "stations_heard": 12,
        "max_snr": -4,
        "median_snr": "-17.5",
    })
}

fn document(rows: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "meta": WSPR_LIVE_ACTIVITY_COLUMNS.map(|name| json!({
            "name": name,
            "type": "Synthetic",
        })),
        "rows": rows.len(),
        "data": rows,
    }))
    .unwrap()
}

#[test]
fn census_query_is_aggregated_stable_and_bounded_by_one_sentinel_row() {
    let plan = plan_wspr_live_query(&WsprLiveQueryScope::from(&config())).unwrap();
    let sql = plan.activity_census_sql();

    assert!(sql.starts_with("SELECT time, band, rx_sign, any(rx_loc) AS rx_loc, count() AS spots_decoded, uniqExact(tx_sign) AS stations_heard, max(snr) AS max_snr, median(snr) AS median_snr FROM wspr.rx"));
    assert!(sql.contains("band IN (14) AND code = 1"));
    assert!(sql.contains("GROUP BY time, band, rx_sign ORDER BY time, band, rx_sign"));
    assert!(sql.contains(&format!(
        "LIMIT {WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT} FORMAT JSON"
    )));
    assert!(plan
        .activity_census_query_url()
        .starts_with("https://db1.wspr.live/?query=SELECT%20time%2C"));
}

#[test]
fn parsing_normalizes_reporters_and_drops_only_invalid_grids() {
    let parsed = parse_wspr_live_activity_json(
        &document(vec![
            row("2026-07-19 02:00:00", "k1abc-7", "not-a-grid"),
            row("2026-07-19 02:02:00", "bad reporter!", "FN42"),
        ]),
        &config(),
    )
    .unwrap();

    assert_eq!(parsed.source_rows, 2);
    assert_eq!(parsed.rows.len(), 1);
    assert_eq!(parsed.malformed_rows, 1);
    assert_eq!(parsed.rows[0].reporter, "K1ABC-7");
    assert_eq!(parsed.rows[0].band, Band::M20);
    assert_eq!(parsed.rows[0].reporter_grid, None);
    assert_eq!(parsed.rows[0].spots_decoded, 37);
    assert_eq!(parsed.rows[0].stations_heard, 12);
}

#[test]
fn deterministic_row_bound_sets_a_durable_truncation_marker() {
    let rows = (0..WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT)
        .map(|index| row("2026-07-19 02:00:00", &format!("K{index}ABC"), "FN42"))
        .collect();
    let parsed = parse_wspr_live_activity_json(&document(rows), &config()).unwrap();

    assert!(parsed.truncated);
    assert_eq!(parsed.source_rows, WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT);
    assert_eq!(parsed.rows.len(), WSPR_LIVE_ACTIVITY_ROW_LIMIT);
    let prepared = prepare_wspr_live_activity(
        &parsed,
        &config(),
        "session-1",
        "capture-1",
        attachment(),
        &[],
    );
    assert!(prepared.summary.truncated);
    assert_eq!(
        prepared.adapter_records[1].disposition,
        AdapterDisposition::PartiallyNormalized
    );
    let AdapterInput::Inline { data, .. } = &prepared.adapter_records[1].input else {
        panic!("summary must be inline")
    };
    let summary: Value = serde_json::from_str(data).unwrap();
    assert_eq!(summary["row_limit"], WSPR_LIVE_ACTIVITY_ROW_LIMIT);
    assert_eq!(summary["truncated"], true);
}

#[test]
fn overlapping_windows_do_not_append_a_second_cycle_reporter_record() {
    let parsed = parse_wspr_live_activity_json(
        &document(vec![
            row("2026-07-19 02:00:00", "K1ABC", "FN42"),
            row("2026-07-19 02:02:00", "W1XYZ", "EM12"),
        ]),
        &config(),
    )
    .unwrap();
    let first = prepare_wspr_live_activity(
        &parsed,
        &config(),
        "session-1",
        "capture-1",
        attachment(),
        &[],
    );
    let second = prepare_wspr_live_activity(
        &parsed,
        &config(),
        "session-1",
        "capture-2",
        attachment(),
        &first.adapter_records,
    );

    assert_eq!(first.summary.accepted, 2);
    assert_eq!(second.summary.accepted, 0);
    assert_eq!(second.summary.duplicate, 2);
    assert_eq!(second.adapter_records.len(), 2);
    assert_eq!(
        first
            .adapter_records
            .iter()
            .filter(|record| record.record_type == WSPR_LIVE_ACTIVITY_RECORD_TYPE)
            .count(),
        2
    );
    assert!(first
        .adapter_records
        .iter()
        .all(|record| record.normalized_records.is_empty()));
}

#[test]
fn same_cycle_reporter_is_retained_once_per_band_and_deduped_per_band() {
    let mut multi_band_config = config();
    multi_band_config.selected_bands = vec![Band::M20, Band::M40];
    let parsed = parse_wspr_live_activity_json(
        &document(vec![
            row_on_band("2026-07-19 02:00:00", 14, "K1ABC", "FN42"),
            row_on_band("2026-07-19 02:00:00", 7, "K1ABC", "FN42"),
        ]),
        &multi_band_config,
    )
    .unwrap();
    let first = prepare_wspr_live_activity(
        &parsed,
        &multi_band_config,
        "session-1",
        "capture-1",
        attachment(),
        &[],
    );
    let second = prepare_wspr_live_activity(
        &parsed,
        &multi_band_config,
        "session-1",
        "capture-2",
        attachment(),
        &first.adapter_records,
    );

    assert_eq!(first.summary.accepted, 2);
    assert_eq!(second.summary.accepted, 0);
    assert_eq!(second.summary.duplicate, 2);
    let bands = first
        .adapter_records
        .iter()
        .filter(|record| record.record_type == WSPR_LIVE_ACTIVITY_RECORD_TYPE)
        .map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                panic!("activity row must be inline")
            };
            serde_json::from_str::<Value>(data).unwrap()["band"].clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(bands, vec![json!("20m"), json!("40m")]);
}

#[test]
fn malformed_missing_and_unselected_bands_are_rejected() {
    let parsed = parse_wspr_live_activity_json(
        &document(vec![
            row_on_band("2026-07-19 02:00:00", 999, "K1ABC", "FN42"),
            row_on_band("2026-07-19 02:02:00", 7, "W1XYZ", "EM12"),
            {
                let mut missing = row("2026-07-19 02:04:00", "N1TEST", "FN31");
                missing.as_object_mut().unwrap().remove("band");
                missing
            },
        ]),
        &config(),
    )
    .unwrap();

    assert!(parsed.rows.is_empty());
    assert_eq!(parsed.source_rows, 3);
    assert_eq!(parsed.malformed_rows, 3);
}

#[test]
fn query_failure_is_a_typed_adapter_disposition_without_observations() {
    let prepared = prepare_wspr_live_activity_failure(
        &config(),
        "session-1",
        "capture-1",
        "wspr-live.activity-census-query-failed",
        "transport unavailable",
    );

    assert_eq!(prepared.adapter_records.len(), 1);
    let failure = &prepared.adapter_records[0];
    assert_eq!(failure.record_type, WSPR_LIVE_ACTIVITY_SUMMARY_RECORD_TYPE);
    assert_eq!(failure.disposition, AdapterDisposition::Unsupported);
    assert_eq!(
        failure.reason.as_str(),
        "wspr-live.activity-census-query-failed"
    );
    assert!(failure.normalized_records.is_empty());
}
