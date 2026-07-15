use std::collections::BTreeSet;

use antennabench_core::{
    AdapterDisposition, AttachmentReference, Band, ObservationKind, PlannedSlot,
};
use antennabench_wsjtx::{
    derive_wspr_live_query_scope, latest_due_wspr_live_acquisition,
    parse_wspr_live_json_with_limits, plan_wspr_live_acquisition_for_completed_slot,
    plan_wspr_live_acquisitions_for_confirmed_slots, plan_wspr_live_query,
    prepare_wspr_live_acquisition, prepare_wspr_live_import, AdapterCancellationToken,
    WsprLiveAcquisitionChannel, WsprLiveImportConfig, WsprLiveImportError, WsprLiveImportLimits,
    WsprLiveQueryScope, WsprLiveRowDisposition, WsprLiveRowReason, WSPR_LIVE_COLUMNS,
    WSPR_LIVE_INGESTION_GRACE_SECONDS, WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS,
    WSPR_LIVE_QUERY_ENDPOINT,
};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

fn config() -> WsprLiveImportConfig {
    WsprLiveImportConfig {
        session_callsign: "N1RWJ".into(),
        window_start: Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap(),
        window_end: Utc.with_ymd_and_hms(2026, 7, 15, 21, 0, 0).unwrap(),
        selected_bands: vec![Band::M20, Band::M40],
        captured_at: Utc.with_ymd_and_hms(2026, 7, 15, 21, 5, 0).unwrap(),
        source_locator: Some("operator-selected.json".into()),
    }
}

fn row(id: u64) -> Value {
    json!({
        "id": id.to_string(),
        "time": "2026-07-15 20:12:00",
        "band": 14,
        "rx_sign": "K1ABC",
        "rx_loc": "EM12",
        "tx_sign": "N1RWJ",
        "tx_loc": "FN42",
        "distance": 2450,
        "azimuth": 252,
        "rx_azimuth": 65,
        "frequency": "14095600",
        "power": 37,
        "snr": -18,
        "drift": 1,
        "version": "2.6.1",
        "code": 1
    })
}

fn document(rows: Vec<Value>) -> Vec<u8> {
    let row_count = rows.len();
    serde_json::to_vec(&json!({
        "meta": WSPR_LIVE_COLUMNS.map(|name| json!({"name": name, "type": "Synthetic"})),
        "data": rows,
        "rows": row_count
    }))
    .unwrap()
}

fn attachment() -> AttachmentReference {
    AttachmentReference {
        sha256: "a".repeat(64),
        byte_size: 123,
        media_type: "application/json".into(),
        encoding: None,
        container: None,
        source_locator: Some("operator-selected.json".into()),
    }
}

#[test]
fn plans_an_exact_stable_bounded_query_and_encoded_url() {
    let mut scope = WsprLiveQueryScope::from(&config());
    scope.session_callsign = " n1rwj ".into();
    scope.selected_bands = vec![Band::M20, Band::M40, Band::M20];
    let plan = plan_wspr_live_query(&scope).unwrap();

    assert_eq!(plan.session_callsign, "N1RWJ");
    assert_eq!(plan.provider_bands, [7, 14]);
    assert_eq!(
        plan.sql(),
        "SELECT id, time, band, rx_sign, rx_loc, tx_sign, tx_loc, distance, azimuth, rx_azimuth, frequency, power, snr, drift, version, code FROM wspr.rx WHERE tx_sign = 'N1RWJ' AND time >= toDateTime('2026-07-15 20:00:00', 'UTC') AND time < toDateTime('2026-07-15 21:00:00', 'UTC') AND band IN (7, 14) AND code = 1 ORDER BY time, id FORMAT JSON"
    );
    assert_eq!(
        plan.query_url(),
        "https://db1.wspr.live/?query=SELECT%20id%2C%20time%2C%20band%2C%20rx_sign%2C%20rx_loc%2C%20tx_sign%2C%20tx_loc%2C%20distance%2C%20azimuth%2C%20rx_azimuth%2C%20frequency%2C%20power%2C%20snr%2C%20drift%2C%20version%2C%20code%20FROM%20wspr.rx%20WHERE%20tx_sign%20%3D%20%27N1RWJ%27%20AND%20time%20%3E%3D%20toDateTime%28%272026-07-15%2020%3A00%3A00%27%2C%20%27UTC%27%29%20AND%20time%20%3C%20toDateTime%28%272026-07-15%2021%3A00%3A00%27%2C%20%27UTC%27%29%20AND%20band%20IN%20%287%2C%2014%29%20AND%20code%20%3D%201%20ORDER%20BY%20time%2C%20id%20FORMAT%20JSON"
    );
    assert_eq!(WSPR_LIVE_QUERY_ENDPOINT, "https://db1.wspr.live/");
}

#[test]
fn query_planning_rejects_untrusted_callsigns_and_invalid_bounds() {
    let mut scope = WsprLiveQueryScope::from(&config());
    scope.session_callsign = "N1RWJ' OR 1=1".into();
    assert!(matches!(
        plan_wspr_live_query(&scope),
        Err(WsprLiveImportError::Config(_))
    ));

    let mut scope = WsprLiveQueryScope::from(&config());
    scope.window_end = scope.window_start;
    assert!(matches!(
        plan_wspr_live_query(&scope),
        Err(WsprLiveImportError::Config(_))
    ));
}

#[test]
fn query_planning_maps_every_supported_band_in_provider_order() {
    let scope = WsprLiveQueryScope {
        session_callsign: "N1RWJ".into(),
        window_start: config().window_start,
        window_end: config().window_end,
        selected_bands: vec![
            Band::M2,
            Band::M10,
            Band::M160,
            Band::M12,
            Band::M80,
            Band::M15,
            Band::M60,
            Band::M17,
            Band::M40,
            Band::M20,
            Band::M30,
            Band::M6,
        ],
    };

    assert_eq!(
        plan_wspr_live_query(&scope).unwrap().provider_bands,
        [1, 3, 5, 7, 10, 14, 18, 21, 24, 28, 50, 144]
    );
}

#[test]
fn derives_the_query_scope_from_the_complete_schedule_not_slot_order() {
    let slots = vec![
        PlannedSlot {
            slot_id: "late".into(),
            sequence_number: 2,
            starts_at: Utc.with_ymd_and_hms(2026, 7, 15, 20, 10, 0).unwrap(),
            duration_seconds: 120,
            guard_seconds: 10,
            band: Band::M20,
            antenna_label: "B".into(),
        },
        PlannedSlot {
            slot_id: "early".into(),
            sequence_number: 1,
            starts_at: Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap(),
            duration_seconds: 120,
            guard_seconds: 10,
            band: Band::M40,
            antenna_label: "A".into(),
        },
        PlannedSlot {
            slot_id: "latest-end".into(),
            sequence_number: 3,
            starts_at: Utc.with_ymd_and_hms(2026, 7, 15, 20, 11, 0).unwrap(),
            duration_seconds: 300,
            guard_seconds: 10,
            band: Band::M20,
            antenna_label: "A".into(),
        },
    ];

    let scope = derive_wspr_live_query_scope("n1rwj", &slots).unwrap();
    assert_eq!(scope.session_callsign, "n1rwj");
    assert_eq!(
        scope.window_start,
        Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap()
    );
    assert_eq!(
        scope.window_end,
        Utc.with_ymd_and_hms(2026, 7, 15, 20, 16, 0).unwrap()
    );
    assert_eq!(scope.selected_bands, [Band::M20, Band::M40]);
    assert_eq!(
        plan_wspr_live_query(&scope).unwrap().provider_bands,
        [7, 14]
    );

    assert!(matches!(
        derive_wspr_live_query_scope("N1RWJ", &[]),
        Err(WsprLiveImportError::Config(_))
    ));
}

#[test]
fn completed_segments_plan_delayed_cumulative_queries_and_coalesce_when_due() {
    let started_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap();
    let slots = [
        planned_slot("slot-1", 1, started_at, Band::M40),
        planned_slot(
            "slot-2",
            2,
            started_at + chrono::Duration::seconds(120),
            Band::M20,
        ),
        planned_slot(
            "slot-3",
            3,
            started_at + chrono::Duration::seconds(240),
            Band::M40,
        ),
    ];
    let first = plan_wspr_live_acquisition_for_completed_slot("n1rwj", &slots, "slot-1").unwrap();
    let second = plan_wspr_live_acquisition_for_completed_slot("n1rwj", &slots, "slot-2").unwrap();

    assert_eq!(first.query.window_start, started_at);
    assert_eq!(
        first.query.window_end,
        started_at + chrono::Duration::seconds(120)
    );
    assert_eq!(first.query.provider_bands, [7]);
    assert_eq!(
        first.not_before,
        first.segment_ended_at + chrono::Duration::seconds(WSPR_LIVE_INGESTION_GRACE_SECONDS)
    );
    assert_eq!(second.query.window_start, started_at);
    assert_eq!(
        second.query.window_end,
        started_at + chrono::Duration::seconds(240)
    );
    assert_eq!(second.query.provider_bands, [7, 14]);

    let plans = [first, second];
    assert_eq!(
        latest_due_wspr_live_acquisition(
            &plans,
            started_at + chrono::Duration::seconds(421),
            None,
        )
        .unwrap()
        .completed_slot_id,
        "slot-1"
    );
    assert_eq!(
        latest_due_wspr_live_acquisition(
            &plans,
            started_at + chrono::Duration::seconds(540),
            None,
        )
        .unwrap()
        .completed_slot_id,
        "slot-2"
    );
    assert!(latest_due_wspr_live_acquisition(
        &plans,
        started_at + chrono::Duration::seconds(540),
        Some(started_at + chrono::Duration::seconds(535)),
    )
    .is_none());
    assert_eq!(
        latest_due_wspr_live_acquisition(
            &plans,
            started_at + chrono::Duration::seconds(535 + WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS),
            Some(started_at + chrono::Duration::seconds(535)),
        )
        .unwrap()
        .completed_slot_id,
        "slot-2"
    );
}

#[test]
fn acquisition_planning_rejects_unknown_completed_slots() {
    let started_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap();
    let slots = [planned_slot("slot-1", 1, started_at, Band::M40)];

    assert!(matches!(
        plan_wspr_live_acquisition_for_completed_slot("N1RWJ", &slots, "missing"),
        Err(WsprLiveImportError::Config(_))
    ));
}

#[test]
fn durable_antenna_confirmations_authorize_prior_and_final_segments() {
    let started_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap();
    let slots = [
        planned_slot("slot-1", 1, started_at, Band::M40),
        planned_slot(
            "slot-2",
            2,
            started_at + chrono::Duration::seconds(120),
            Band::M20,
        ),
        planned_slot(
            "slot-3",
            3,
            started_at + chrono::Duration::seconds(240),
            Band::M40,
        ),
    ];

    let first_only = BTreeSet::from(["slot-1".to_string()]);
    assert!(
        plan_wspr_live_acquisitions_for_confirmed_slots("N1RWJ", &slots, &first_only)
            .unwrap()
            .is_empty()
    );

    let second = BTreeSet::from(["slot-1".to_string(), "slot-2".to_string()]);
    let plans = plan_wspr_live_acquisitions_for_confirmed_slots("N1RWJ", &slots, &second).unwrap();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].completed_slot_id, "slot-1");

    let final_confirmation = BTreeSet::from([
        "slot-1".to_string(),
        "slot-2".to_string(),
        "slot-3".to_string(),
    ]);
    let plans =
        plan_wspr_live_acquisitions_for_confirmed_slots("N1RWJ", &slots, &final_confirmation)
            .unwrap();
    assert_eq!(
        plans
            .iter()
            .map(|plan| plan.completed_slot_id.as_str())
            .collect::<Vec<_>>(),
        ["slot-1", "slot-2", "slot-3"]
    );
}

fn planned_slot(
    slot_id: &str,
    sequence_number: u32,
    starts_at: chrono::DateTime<Utc>,
    band: Band,
) -> PlannedSlot {
    PlannedSlot {
        slot_id: slot_id.into(),
        sequence_number,
        starts_at,
        duration_seconds: 120,
        guard_seconds: 10,
        band,
        antenna_label: if sequence_number.is_multiple_of(2) {
            "B"
        } else {
            "A"
        }
        .into(),
    }
}

#[test]
fn accepts_a_synthetic_tx_report_with_exact_direction_and_units() {
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![row(7001)]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();

    assert_eq!(parsed.summary.total, 1);
    assert_eq!(parsed.summary.accepted, 1);
    let result = &parsed.rows[0];
    assert_eq!(result.disposition, WsprLiveRowDisposition::Accepted);
    let spot = result.spot.as_ref().unwrap();
    assert_eq!(spot.provider_spot_id, 7001);
    assert_eq!(spot.band, Band::M20);
    assert_eq!(spot.reporter_call, "K1ABC");
    assert_eq!(spot.transmitter_call, "N1RWJ");
    assert_eq!(spot.reporter_grid.as_deref(), Some("EM12"));
    assert_eq!(spot.transmitter_grid.as_deref(), Some("FN42"));
    assert_eq!(spot.frequency_hz, 14_095_600);
    assert_eq!(spot.power_dbm, 37);
    assert_eq!(spot.snr_db, -18.0);
    assert_eq!(spot.drift_hz_per_minute, 1.0);
    assert_eq!(spot.distance_km, Some(2450.0));
    assert_eq!(spot.azimuth_degrees, Some(252.0));
}

#[test]
fn maps_every_supported_wspr_live_frequency_prefix_band() {
    let cases = [
        (1, Band::M160, 1_836_600),
        (3, Band::M80, 3_568_600),
        (5, Band::M60, 5_287_200),
        (7, Band::M40, 7_038_600),
        (10, Band::M30, 10_138_700),
        (14, Band::M20, 14_095_600),
        (18, Band::M17, 18_104_600),
        (21, Band::M15, 21_094_600),
        (24, Band::M12, 24_924_600),
        (28, Band::M10, 28_124_600),
        (50, Band::M6, 50_293_000),
        (144, Band::M2, 144_489_000),
    ];

    for (provider_band, band, frequency_hz) in cases {
        let mut value = row(provider_band as u64);
        value["band"] = json!(provider_band);
        value["frequency"] = json!(frequency_hz.to_string());
        let mut config = config();
        config.selected_bands = vec![band];
        let parsed = parse_wspr_live_json_with_limits(
            &document(vec![value]),
            &config,
            &AdapterCancellationToken::default(),
            WsprLiveImportLimits::testing(1024),
        )
        .unwrap();

        assert_eq!(parsed.summary.accepted, 1, "provider band {provider_band}");
        assert_eq!(parsed.rows[0].spot.as_ref().unwrap().band, band);
    }
}

#[test]
fn accepts_documented_missing_optional_location_and_receiver_version() {
    let mut value = row(7002);
    value["rx_loc"] = Value::Null;
    value["tx_loc"] = json!("");
    value["distance"] = Value::Null;
    value["azimuth"] = Value::Null;
    value["rx_azimuth"] = Value::Null;
    value["version"] = Value::Null;
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![value]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();

    assert_eq!(parsed.summary.accepted, 1);
    let spot = parsed.rows[0].spot.as_ref().unwrap();
    assert_eq!(spot.reporter_grid, None);
    assert_eq!(spot.transmitter_grid, None);
    assert_eq!(spot.distance_km, None);
    assert_eq!(spot.azimuth_degrees, None);
    assert_eq!(spot.receiver_version, None);
}

#[test]
fn repeats_callsign_time_band_and_wspr2_filters_locally() {
    let mut callsign = row(1);
    callsign["tx_sign"] = json!("N0CALL");
    let mut time = row(2);
    time["time"] = json!("2026-07-15 21:00:00");
    let mut band = row(3);
    band["band"] = json!(28);
    band["frequency"] = json!("28124600");
    let mut mode = row(4);
    mode["code"] = json!(15);
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![callsign, time, band, mode]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();

    assert_eq!(parsed.summary.filtered, 3);
    assert_eq!(parsed.summary.unsupported, 1);
    assert_eq!(
        parsed.rows.iter().map(|row| row.reason).collect::<Vec<_>>(),
        [
            WsprLiveRowReason::CallsignFiltered,
            WsprLiveRowReason::TimeFiltered,
            WsprLiveRowReason::BandFiltered,
            WsprLiveRowReason::UnsupportedMode,
        ]
    );
    assert!(parsed.rows.iter().all(|row| row.spot.is_none()));
}

#[test]
fn structural_schema_resource_and_cancellation_fail_the_complete_import() {
    let mut wrong_columns = WSPR_LIVE_COLUMNS;
    wrong_columns[0] = "spot_id";
    let schema = serde_json::to_vec(&json!({
        "meta": wrong_columns.map(|name| json!({"name": name})),
        "data": []
    }))
    .unwrap();
    assert!(matches!(
        parse_wspr_live_json_with_limits(
            &schema,
            &config(),
            &AdapterCancellationToken::default(),
            WsprLiveImportLimits::testing(1024),
        ),
        Err(WsprLiveImportError::Schema(_))
    ));
    let oversized = parse_wspr_live_json_with_limits(
        &document(vec![row(1), row(2)]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1),
    )
    .unwrap_err();
    assert!(matches!(oversized, WsprLiveImportError::Resource(_)));
    let cancellation = AdapterCancellationToken::default();
    cancellation.cancel();
    let cancelled = parse_wspr_live_json_with_limits(
        &document(vec![row(1)]),
        &config(),
        &cancellation,
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap_err();
    assert!(matches!(cancelled, WsprLiveImportError::Resource(_)));
}

#[test]
fn invalid_rows_remain_bounded_malformed_dispositions() {
    let mut invalid_grid = row(1);
    invalid_grid["rx_loc"] = json!("ZZ99");
    let mut mismatched_frequency = row(2);
    mismatched_frequency["frequency"] = json!("7038600");
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![invalid_grid, mismatched_frequency]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();

    assert_eq!(parsed.summary.malformed, 2);
    assert!(parsed.rows.iter().all(|row| {
        row.disposition == WsprLiveRowDisposition::Malformed
            && row.reason == WsprLiveRowReason::InvalidValue
            && row.spot.is_none()
    }));
}

#[test]
fn prepares_provenance_linked_imported_spot_evidence() {
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![row(7001)]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();
    let prepared =
        prepare_wspr_live_import(&parsed, &config(), "session-1", "one", attachment(), &[]);

    assert_eq!(prepared.adapter_records.len(), 3);
    assert_eq!(prepared.observations.len(), 1);
    assert_eq!(prepared.summary.observations_created, 1);
    let capture = &prepared.adapter_records[0];
    assert_eq!(capture.record_type, "wspr_live_import_capture");
    assert_eq!(capture.meta.provenance.provider_id.as_str(), "wspr-live");
    assert_eq!(
        capture.meta.provenance.source_id.as_str(),
        "wsprnet-spots-mirror"
    );
    assert_eq!(
        capture.meta.provenance.acquisition_channel.as_str(),
        "file-import"
    );
    assert_eq!(
        prepared.adapter_records[1].record_type,
        "wspr_live_import_summary"
    );
    let adapter = &prepared.adapter_records[2];
    let observation = &prepared.observations[0];
    assert_eq!(adapter.disposition, AdapterDisposition::Accepted);
    assert_eq!(observation.observation_kind, ObservationKind::ImportedSpot);
    assert_eq!(observation.reporter_call.as_deref(), Some("K1ABC"));
    assert_eq!(observation.heard_call.as_deref(), Some("N1RWJ"));
    assert_eq!(observation.azimuth_degrees, Some(252.0));
    assert_eq!(
        observation.adapter_record_ids[0].as_str(),
        adapter.record_id.as_str()
    );
    assert_eq!(
        adapter.normalized_records[0].record_id,
        observation.observation_id
    );
}

#[test]
fn automatic_acquisition_uses_https_provenance_without_changing_normalization() {
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![row(7001)]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();
    let prepared = prepare_wspr_live_acquisition(
        &parsed,
        &config(),
        "session-1",
        "automatic-one",
        attachment(),
        &[],
        WsprLiveAcquisitionChannel::HttpsQuery,
    );

    assert_eq!(prepared.summary.observations_created, 1);
    assert_eq!(
        prepared.adapter_records[0]
            .meta
            .provenance
            .acquisition_channel
            .as_str(),
        "https-query"
    );
    assert_eq!(
        prepared.observations[0].observation_kind,
        ObservationKind::ImportedSpot
    );
    let summary = match &prepared.adapter_records[1].input {
        antennabench_core::AdapterInput::Inline { data, .. } => {
            serde_json::from_str::<Value>(data).unwrap()
        }
        _ => panic!("summary must be inline"),
    };
    assert_eq!(summary["acquisition_channel"], "https-query");
}

#[test]
fn provider_ids_make_exact_replays_duplicates_and_changed_payloads_conflicts() {
    let parsed = parse_wspr_live_json_with_limits(
        &document(vec![row(7001)]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();
    let first = prepare_wspr_live_import(&parsed, &config(), "session-1", "one", attachment(), &[]);
    let replay = prepare_wspr_live_import(
        &parsed,
        &config(),
        "session-1",
        "two",
        attachment(),
        &first.adapter_records,
    );
    assert_eq!(replay.summary.duplicate, 1);
    assert!(replay.observations.is_empty());
    assert_eq!(
        replay.adapter_records[2].disposition,
        AdapterDisposition::Duplicate
    );

    let mut changed = row(7001);
    changed["snr"] = json!(-9);
    let changed = parse_wspr_live_json_with_limits(
        &document(vec![changed]),
        &config(),
        &AdapterCancellationToken::default(),
        WsprLiveImportLimits::testing(1024),
    )
    .unwrap();
    let conflict = prepare_wspr_live_import(
        &changed,
        &config(),
        "session-1",
        "three",
        attachment(),
        &first.adapter_records,
    );
    assert_eq!(conflict.summary.conflict, 1);
    assert!(conflict.observations.is_empty());
    assert_eq!(
        conflict.adapter_records[2].disposition,
        AdapterDisposition::Conflict
    );
}
