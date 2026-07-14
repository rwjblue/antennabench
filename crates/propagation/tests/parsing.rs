use antennabench_core::RecordSource;
use antennabench_propagation::{
    parse_estimated_kp_response, parse_f107_response, retry_allowed, should_acquire, AppendOutcome,
    DiscardedItem, HttpMetadata, InvalidItemReason, ParseError, SessionAcquisitionPhase,
    SourceFreshness, SwpcProduct,
};
use chrono::{Duration, TimeZone, Utc};

const F107: &[u8] = include_bytes!("../../../fixtures/noaa-swpc/f107.json");
const ESTIMATED_KP: &[u8] = include_bytes!("../../../fixtures/noaa-swpc/estimated-kp.json");

fn captured_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 13, 21, 25, 0)
        .single()
        .unwrap()
}

fn http() -> HttpMetadata {
    HttpMetadata {
        status: 200,
        etag: Some("\"fixture-etag\"".to_string()),
        last_modified: Some("Sun, 13 Jul 2026 21:21:00 GMT".to_string()),
        date: Some("Sun, 13 Jul 2026 21:25:00 GMT".to_string()),
        content_type: Some("application/json".to_string()),
    }
}

#[test]
fn parses_captured_products_as_separate_sparse_source_attributed_records() {
    let f107 = parse_f107_response("session-test", captured_at(), F107, http()).unwrap();
    let kp =
        parse_estimated_kp_response("session-test", captured_at(), ESTIMATED_KP, http()).unwrap();

    assert_eq!(f107.product, SwpcProduct::SolarFluxF107);
    assert_eq!(f107.record.meta.schema_version, 1);
    assert_eq!(f107.record.meta.session_id, "session-test");
    assert_eq!(f107.record.meta.timestamp, captured_at());
    assert_eq!(f107.record.meta.source, RecordSource::NoaaSwpc);
    assert_eq!(f107.record.solar_flux_f107, Some(103.0));
    assert_eq!(f107.record.kp_index, None);
    assert_eq!(
        f107.record.observed_at.to_rfc3339(),
        "2026-07-13T20:00:00+00:00"
    );
    assert!(f107.record.sunspot_number.is_none());
    assert!(f107.record.a_index.is_none());
    assert!(f107.record.solar_wind_speed_kms.is_none());
    assert!(f107.record.bz_nt.is_none());
    assert!(f107.record.alerts.is_empty());
    assert!(f107.record.daylight_state.is_none());

    assert_eq!(kp.product, SwpcProduct::EstimatedPlanetaryKp);
    assert_eq!(kp.record.solar_flux_f107, None);
    assert_eq!(kp.record.kp_index, Some(0.67));
    assert_ne!(
        kp.record.kp_index,
        Some(1.0),
        "integer kp_index is not normalized"
    );
    assert_eq!(
        kp.record.observed_at.to_rfc3339(),
        "2026-07-13T21:20:00+00:00"
    );
    assert_eq!(kp.freshness, SourceFreshness::Current { age_seconds: 300 });
}

#[test]
fn preserves_selected_source_object_literal_endpoint_retrieval_and_http_metadata() {
    let parsed = parse_f107_response("session-test", captured_at(), F107, http()).unwrap();
    let raw = parsed.record.raw;

    assert_eq!(raw["provider"], "NOAA/NWS Space Weather Prediction Center");
    assert_eq!(raw["endpoint"], SwpcProduct::SolarFluxF107.endpoint());
    assert_eq!(raw["retrieved_at"], "2026-07-13T21:25:00Z");
    assert_eq!(raw["selected"]["time_tag"], "2026-07-13T20:00:00");
    assert_eq!(raw["selected"]["flux"], 103);
    assert_eq!(raw["http"]["etag"], "\"fixture-etag\"");
    assert_eq!(raw["http"]["status"], 200);
    assert_eq!(raw["value_semantics"], "observed_f10_7_solar_flux_sfu");
    assert!(raw["source_attribution"]
        .as_str()
        .unwrap()
        .contains("National Research Council Canada"));
}

#[test]
fn selection_is_latest_and_independent_of_response_order() {
    let reversed = br#"[
      {"time_tag":"2026-07-13T21:20:00","kp_index":1,"estimated_kp":0.67,"kp":"1M"},
      {"time_tag":"2026-07-13T21:18:00","kp_index":0,"estimated_kp":0.33,"kp":"0P"}
    ]"#;
    let forward =
        parse_estimated_kp_response("session-test", captured_at(), ESTIMATED_KP, http()).unwrap();
    let reversed =
        parse_estimated_kp_response("session-test", captured_at(), reversed, http()).unwrap();

    assert_eq!(forward.record, reversed.record);
    assert_eq!(forward.freshness, reversed.freshness);
}

#[test]
fn partially_valid_payload_reports_every_discarded_item_without_defaults() {
    let payload = br#"[
      null,
      {"flux":100},
      {"time_tag":"not-a-time","flux":101},
      {"time_tag":"2026-07-13T19:00:00","flux":null},
      {"time_tag":"2026-07-13T19:00:00","flux":1001},
      {"time_tag":"2026-07-13T20:00:00","flux":102}
    ]"#;
    let parsed = parse_f107_response("session-test", captured_at(), payload, http()).unwrap();

    assert_eq!(parsed.record.solar_flux_f107, Some(102.0));
    assert_eq!(
        parsed.discarded_items,
        vec![
            DiscardedItem {
                index: 0,
                reason: InvalidItemReason::NotAnObject
            },
            DiscardedItem {
                index: 1,
                reason: InvalidItemReason::MissingTimeTag
            },
            DiscardedItem {
                index: 2,
                reason: InvalidItemReason::InvalidTimeTag
            },
            DiscardedItem {
                index: 3,
                reason: InvalidItemReason::InvalidValue
            },
            DiscardedItem {
                index: 4,
                reason: InvalidItemReason::OutOfRangeValue
            },
        ]
    );
    assert_eq!(parsed.discarded_item_count(), 5);
}

#[test]
fn malformed_empty_and_out_of_range_payloads_return_typed_failures() {
    assert!(matches!(
        parse_f107_response("session-test", captured_at(), b"not json", http()),
        Err(ParseError::InvalidJson { .. })
    ));
    assert!(matches!(
        parse_f107_response("session-test", captured_at(), br#"{}"#, http()),
        Err(ParseError::ExpectedArray { .. })
    ));
    assert!(matches!(
        parse_f107_response("session-test", captured_at(), br#"[]"#, http()),
        Err(ParseError::NoValidObservation {
            product: SwpcProduct::SolarFluxF107,
            ..
        })
    ));
    let error = parse_estimated_kp_response(
        "session-test",
        captured_at(),
        br#"[{"time_tag":"2026-07-13T21:20:00","estimated_kp":9.1}]"#,
        http(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ParseError::NoValidObservation {
            product: SwpcProduct::EstimatedPlanetaryKp,
            ..
        }
    ));
}

#[test]
fn classifies_staleness_and_suppresses_only_unchanged_source_observations() {
    let stale_at = Utc
        .with_ymd_and_hms(2026, 7, 13, 21, 31, 0)
        .single()
        .unwrap();
    let parsed =
        parse_estimated_kp_response("session-test", stale_at, ESTIMATED_KP, http()).unwrap();
    assert_eq!(
        parsed.freshness,
        SourceFreshness::Stale { age_seconds: 660 }
    );
    assert!(parsed.freshness.is_stale());

    let existing = vec![parsed.record.clone()];
    assert!(matches!(
        parsed.clone().append_outcome(&existing),
        AppendOutcome::Unchanged { .. }
    ));
    let changed = parse_estimated_kp_response(
        "session-test",
        stale_at,
        br#"[{"time_tag":"2026-07-13T21:20:00","kp_index":1,"estimated_kp":1.0}]"#,
        http(),
    )
    .unwrap();
    assert!(matches!(
        changed.append_outcome(&existing),
        AppendOutcome::Append(_)
    ));
}

#[test]
fn rejects_far_future_observations_and_clamps_allowed_clock_skew() {
    let payload = br#"[
      {"time_tag":"2026-07-13T21:35:01","estimated_kp":8.0},
      {"time_tag":"2026-07-13T21:20:00","estimated_kp":0.67}
    ]"#;
    let parsed =
        parse_estimated_kp_response("session-test", captured_at(), payload, http()).unwrap();

    assert_eq!(parsed.record.kp_index, Some(0.67));
    assert_eq!(
        parsed.discarded_items,
        vec![DiscardedItem {
            index: 0,
            reason: InvalidItemReason::FutureDatedObservation,
        }]
    );

    let allowed_skew = br#"[
      {"time_tag":"2026-07-13T21:30:00","estimated_kp":1.0}
    ]"#;
    let parsed =
        parse_estimated_kp_response("session-test", captured_at(), allowed_skew, http()).unwrap();
    assert_eq!(
        parsed.freshness,
        SourceFreshness::Current { age_seconds: 0 }
    );

    let only_far_future = br#"[
      {"time_tag":"2026-07-13T21:30:01","estimated_kp":1.0}
    ]"#;
    assert!(matches!(
        parse_estimated_kp_response(
            "session-test",
            captured_at(),
            only_far_future,
            http()
        ),
        Err(ParseError::NoValidObservation {
            product: SwpcProduct::EstimatedPlanetaryKp,
            discarded_items,
        }) if discarded_items == vec![DiscardedItem {
            index: 0,
            reason: InvalidItemReason::FutureDatedObservation,
        }]
    ));
}

#[test]
fn exposes_start_active_end_polling_and_retry_policy() {
    let now = captured_at();
    assert!(should_acquire(
        SwpcProduct::EstimatedPlanetaryKp,
        SessionAcquisitionPhase::Start,
        Some(now),
        now
    ));
    assert!(should_acquire(
        SwpcProduct::EstimatedPlanetaryKp,
        SessionAcquisitionPhase::End,
        Some(now),
        now
    ));
    assert!(!should_acquire(
        SwpcProduct::EstimatedPlanetaryKp,
        SessionAcquisitionPhase::ActivePoll,
        Some(now - Duration::minutes(4)),
        now
    ));
    assert!(should_acquire(
        SwpcProduct::EstimatedPlanetaryKp,
        SessionAcquisitionPhase::ActivePoll,
        Some(now - Duration::minutes(5)),
        now
    ));
    assert!(!should_acquire(
        SwpcProduct::SolarFluxF107,
        SessionAcquisitionPhase::ActivePoll,
        Some(now - Duration::hours(5)),
        now
    ));
    assert!(should_acquire(
        SwpcProduct::SolarFluxF107,
        SessionAcquisitionPhase::ActivePoll,
        Some(now - Duration::hours(6)),
        now
    ));
    assert!(!retry_allowed(Some(now - Duration::seconds(59)), now));
    assert!(retry_allowed(Some(now - Duration::minutes(1)), now));
}
