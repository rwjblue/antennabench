use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

use antennabench_core::{normalize_bundle, validate_bundle, ObservationKind, RecordSource};
use antennabench_storage::BundleStore;
use antennabench_wsjtx::{
    append_live_wsjtx_message, parse_wsjtx_datagram, BoundedUdpQueue, DatagramParseError,
    LiveIngestConfig, LiveIngestError, LiveIngestOutcome, LiveMessageDisposition,
    LiveRecordedMessage, LiveWsjtxIngest, ReceivedUdpDatagram, UdpReceiverError,
    WsjtxAdapterLimits, WsjtxMessage, WsjtxUdpReceiver, WsprDecodeDisposition,
    MAX_SUPPORTED_SCHEMA, MIN_SUPPORTED_SCHEMA, WSJTX_ADAPTER_LIMITS, WSJTX_MAGIC,
};
use chrono::{DateTime, TimeZone, Utc};

const CLIENT_ID: &str = "WSJT-X";
const SESSION_ID: &str = "session-live-wsjtx-test";

#[test]
fn live_admission_rate_breach_records_one_gap_and_stops_only_the_receiver() {
    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_rate_per_second = 1;
    limits.udp_rate_burst = 2;
    let mut ingest = live_ingest_with_limits(limits);
    let at = utc(2026, 7, 9, 20, 1, 0);
    ingest
        .ingest_datagram(&heartbeat_datagram(3, "a", "a"), at)
        .unwrap();
    ingest
        .ingest_datagram(&heartbeat_datagram(3, "b", "b"), at)
        .unwrap();
    let error = ingest
        .ingest_datagram(&heartbeat_datagram(3, "c", "c"), at)
        .unwrap_err();
    assert!(matches!(error, LiveIngestError::Resource(_)));
    assert_eq!(
        ingest.acquisition_gap().unwrap().diagnostic.code,
        "resource.adapter.udp.rate"
    );
    assert!(ingest.is_stopped());
    let repeated = ingest
        .ingest_datagram(&heartbeat_datagram(3, "d", "d"), at)
        .unwrap_err();
    assert_eq!(error.to_string(), repeated.to_string());
}

#[test]
fn live_client_and_dedup_state_are_bounded_without_storing_datagram_copies() {
    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_clients = 1;
    let mut clients = live_ingest_with_limits(limits);
    let at = utc(2026, 7, 9, 20, 1, 0);
    clients.ingest_datagram(&heartbeat_for("one"), at).unwrap();
    let error = clients
        .ingest_datagram(&heartbeat_for("two"), at)
        .unwrap_err();
    assert!(error.to_string().contains("resource.adapter.udp.clients"));

    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_dedup_entries_per_client = 2;
    let mut dedup = live_ingest_with_limits(limits);
    dedup
        .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), at)
        .unwrap();
    dedup
        .ingest_datagram(&wspr_decode_datagram(true, 72_110_000, false), at)
        .unwrap();
    dedup
        .ingest_datagram(&wspr_decode_datagram(true, 72_111_000, false), at)
        .unwrap();
    let error = dedup
        .ingest_datagram(&wspr_decode_datagram(true, 72_112_000, false), at)
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("resource.adapter.udp.dedup_entries"));
}

#[test]
fn bounded_udp_queue_stops_at_item_or_byte_limit_without_silent_drop() {
    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_queue_datagrams = 2;
    limits.udp_queue_bytes = 4;
    let mut queue = BoundedUdpQueue::with_limits("queue-test", limits);
    let at = utc(2026, 7, 9, 20, 1, 0);
    let source = "127.0.0.1:1234".parse().unwrap();
    for bytes in [vec![1, 2], vec![3, 4]] {
        queue
            .push(ReceivedUdpDatagram {
                bytes,
                source,
                received_at: at,
            })
            .unwrap();
    }
    let failure = queue
        .push(ReceivedUdpDatagram {
            bytes: vec![5],
            source,
            received_at: at,
        })
        .unwrap_err();
    assert_eq!(
        failure.diagnostic.code,
        "resource.adapter.udp.queue_datagrams"
    );
    assert_eq!(queue.len(), 2);
    assert!(queue.acquisition_gap().is_some());

    let mut byte_limits = WSJTX_ADAPTER_LIMITS;
    byte_limits.udp_queue_datagrams = 10;
    byte_limits.udp_queue_bytes = 3;
    let mut queue = BoundedUdpQueue::with_limits("byte-queue", byte_limits);
    queue
        .push(ReceivedUdpDatagram {
            bytes: vec![1, 2],
            source,
            received_at: at,
        })
        .unwrap();
    let failure = queue
        .push(ReceivedUdpDatagram {
            bytes: vec![3, 4],
            source,
            received_at: at,
        })
        .unwrap_err();
    assert_eq!(failure.diagnostic.code, "resource.adapter.udp.queue_bytes");
}

#[test]
fn datagram_client_id_idle_eviction_and_dedup_window_use_fixed_boundaries() {
    let at = utc(2026, 7, 9, 20, 1, 0);
    let mut exact = heartbeat_datagram(3, "2.6.1", "test");
    exact.resize(65_535, 0);
    live_ingest().ingest_datagram(&exact, at).unwrap();
    let mut over = exact;
    over.push(0);
    let mut ingest = live_ingest();
    let error = ingest.ingest_datagram(&over, at).unwrap_err();
    assert!(error
        .to_string()
        .contains("resource.adapter.udp.datagram_bytes"));

    let mut ingest = live_ingest();
    let error = ingest
        .ingest_datagram(&heartbeat_for(&"x".repeat(129)), at)
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("resource.adapter.udp.client_id_bytes"));

    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_clients = 1;
    limits.udp_idle_eviction_seconds = 5;
    let mut ingest = live_ingest_with_limits(limits);
    ingest.ingest_datagram(&heartbeat_for("one"), at).unwrap();
    ingest
        .ingest_datagram(&heartbeat_for("two"), at + chrono::Duration::seconds(5))
        .unwrap();
    assert!(ingest.client_state("one").is_none());
    assert!(ingest.client_state("two").is_some());

    let mut limits = WSJTX_ADAPTER_LIMITS;
    limits.udp_dedup_entries_per_client = 1;
    limits.udp_dedup_window_seconds = 10;
    let mut ingest = live_ingest_with_limits(limits);
    ingest
        .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), at)
        .unwrap();
    ingest
        .ingest_datagram(&wspr_decode_datagram(true, 72_110_000, false), at)
        .unwrap();
    ingest
        .ingest_datagram(
            &wspr_decode_datagram(true, 72_121_000, false),
            at + chrono::Duration::seconds(11),
        )
        .unwrap();
}

#[test]
fn parses_synthetic_schema_three_fixture_and_ignores_trailing_fields() {
    let fixture = synthetic_fixture();

    let heartbeat = parse_wsjtx_datagram(&fixture.heartbeat).unwrap();
    assert_eq!(heartbeat.schema, 3);
    assert_eq!(heartbeat.trailing_bytes, 0);
    let WsjtxMessage::Heartbeat(heartbeat) = heartbeat.message else {
        panic!("expected heartbeat");
    };
    assert_eq!(heartbeat.client_id, CLIENT_ID);
    assert_eq!(heartbeat.maximum_schema, 3);
    assert_eq!(heartbeat.version, "2.6.1");
    assert_eq!(heartbeat.revision, "r123");

    let status = parse_wsjtx_datagram(&fixture.status).unwrap();
    assert_eq!(status.trailing_bytes, 2);
    let WsjtxMessage::Status(status) = status.message else {
        panic!("expected status");
    };
    assert_eq!(status.dial_frequency_hz, 14_095_600);
    assert_eq!(status.mode, "WSPR");
    assert!(status.tx_enabled);
    assert!(!status.transmitting);
    assert!(status.decoding);
    assert_eq!(status.de_call, "N1RWJ");
    assert_eq!(status.de_grid, "FN42");

    let decode = parse_wsjtx_datagram(&fixture.decode).unwrap();
    assert_eq!(decode.trailing_bytes, 1);
    let WsjtxMessage::WsprDecode(decode) = decode.message else {
        panic!("expected WSPR decode");
    };
    assert!(decode.is_new);
    assert_eq!(decode.time_millis, 72_110_000);
    assert_eq!(decode.snr_db, -18);
    assert_eq!(decode.delta_time_seconds, 0.07);
    assert_eq!(decode.frequency_hz, 14_095_600);
    assert_eq!(decode.callsign, "K1ABC");
    assert_eq!(decode.grid, "EM12");
    assert_eq!(decode.power_dbm, 37);
    assert!(!decode.off_air);
}

#[test]
fn parses_schema_two_heartbeat_without_maximum_schema_field() {
    let datagram = heartbeat_datagram(2, "2.5.4", "legacy");

    let parsed = parse_wsjtx_datagram(&datagram).unwrap();
    let WsjtxMessage::Heartbeat(heartbeat) = parsed.message else {
        panic!("expected heartbeat");
    };

    assert_eq!(heartbeat.maximum_schema, 2);
    assert_eq!(heartbeat.version, "2.5.4");
    assert_eq!(heartbeat.revision, "legacy");
}

#[test]
fn heartbeat_tracks_client_identity_and_negotiated_schema() {
    let mut ingest = live_ingest();
    let received_at = utc(2026, 7, 9, 20, 1, 0);
    let message = recorded(
        ingest
            .ingest_datagram(&heartbeat_datagram(3, "2.6.1", "r123"), received_at)
            .unwrap(),
    );

    assert_eq!(message.disposition, LiveMessageDisposition::Heartbeat);
    let client = ingest.client_state(CLIENT_ID).unwrap();
    assert_eq!(client.schema, 3);
    assert_eq!(client.maximum_schema, Some(3));
    assert_eq!(client.negotiated_schema, Some(3));
    assert_eq!(client.version.as_deref(), Some("2.6.1"));
    assert_eq!(client.revision.as_deref(), Some("r123"));
}

#[test]
fn safely_reports_unknown_types_and_malformed_datagrams() {
    let unknown = unsupported_datagram(3, 99);
    let parsed = parse_wsjtx_datagram(&unknown).unwrap();
    let WsjtxMessage::Unsupported(message) = parsed.message else {
        panic!("expected unsupported message");
    };
    assert_eq!(message.message_type, 99);
    assert_eq!(message.client_id, CLIENT_ID);

    let mut invalid_magic = unknown.clone();
    invalid_magic[0] = 0;
    assert!(matches!(
        parse_wsjtx_datagram(&invalid_magic),
        Err(DatagramParseError::InvalidMagic { .. })
    ));

    for schema in [1, 4] {
        let datagram = unsupported_datagram(schema, 99);
        assert_eq!(
            parse_wsjtx_datagram(&datagram),
            Err(DatagramParseError::UnsupportedSchema { actual: schema })
        );
    }

    let truncated = &status_datagram("WSPR", "N1RWJ", "FN42")[..24];
    assert!(matches!(
        parse_wsjtx_datagram(truncated),
        Err(DatagramParseError::Truncated { .. })
    ));

    let invalid_time = wspr_decode_datagram(true, 86_400_000, false);
    assert_eq!(
        parse_wsjtx_datagram(&invalid_time),
        Err(DatagramParseError::InvalidTime { actual: 86_400_000 })
    );

    let mut invalid_delta_time = wspr_decode_datagram(true, 72_110_000, false);
    invalid_delta_time[31..39].copy_from_slice(&f64::NAN.to_be_bytes());
    assert_eq!(
        parse_wsjtx_datagram(&invalid_delta_time),
        Err(DatagramParseError::NonFiniteFloat {
            field: "WSPR decode delta time"
        })
    );

    assert_eq!(MIN_SUPPORTED_SCHEMA, 2);
    assert_eq!(MAX_SUPPORTED_SCHEMA, 3);
}

#[test]
fn new_on_air_decode_is_preserved_and_becomes_a_local_observation() {
    let fixture = synthetic_fixture();
    let mut ingest = live_ingest();
    let status_at = utc(2026, 7, 9, 20, 1, 45);
    recorded(ingest.ingest_datagram(&fixture.status, status_at).unwrap());

    let message = recorded(
        ingest
            .ingest_datagram(&fixture.decode, utc(2026, 7, 9, 20, 1, 51))
            .unwrap(),
    );

    assert_eq!(
        message.disposition,
        LiveMessageDisposition::WsprDecode(WsprDecodeDisposition::ObservationProduced)
    );
    assert_eq!(message.wsjtx_record.meta.source, RecordSource::WsjtxUdp);
    assert_eq!(message.wsjtx_record.message_type, "udp_wspr_decode");
    assert_eq!(
        message.wsjtx_record.meta.timestamp,
        utc(2026, 7, 9, 20, 1, 50)
    );
    assert_eq!(message.wsjtx_record.raw["trailing_bytes"], 1);
    assert_eq!(
        message.wsjtx_record.raw["fields"]["observation_disposition"],
        "observation_produced"
    );

    let observation = message.observation.expect("eligible observation");
    assert_eq!(observation.observation_kind, ObservationKind::LocalDecode);
    assert_eq!(observation.observation_id, "live-test-obs-000002");
    assert_eq!(observation.reporter_call.as_deref(), Some("N1RWJ"));
    assert_eq!(observation.reporter_grid.as_deref(), Some("FN42"));
    assert_eq!(observation.heard_call.as_deref(), Some("K1ABC"));
    assert_eq!(observation.heard_grid.as_deref(), Some("EM12"));
    assert_eq!(observation.frequency_hz, Some(14_095_600));
    assert_eq!(observation.snr_db, Some(-18.0));
    assert!((observation.power_watts.unwrap() - 5.011_872).abs() < 0.000_001);
}

#[test]
fn replay_off_air_duplicate_and_identity_policies_are_explicit() {
    let mut ingest = live_ingest();
    let received_at = utc(2026, 7, 9, 20, 1, 51);
    recorded(
        ingest
            .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), received_at)
            .unwrap(),
    );

    let decode = wspr_decode_datagram(true, 72_110_000, false);
    let first = recorded(ingest.ingest_datagram(&decode, received_at).unwrap());
    assert!(first.observation.is_some());
    let duplicate = recorded(ingest.ingest_datagram(&decode, received_at).unwrap());
    assert_policy(duplicate, WsprDecodeDisposition::Duplicate);

    let replay = recorded(
        ingest
            .ingest_datagram(&wspr_decode_datagram(false, 72_120_000, false), received_at)
            .unwrap(),
    );
    assert_policy(replay, WsprDecodeDisposition::Replay);

    let off_air = recorded(
        ingest
            .ingest_datagram(&wspr_decode_datagram(true, 72_130_000, true), received_at)
            .unwrap(),
    );
    assert_policy(off_air, WsprDecodeDisposition::OffAir);

    recorded(
        ingest
            .ingest_datagram(&status_datagram("WSPR", "N0CALL", "FN42"), received_at)
            .unwrap(),
    );
    let mismatch = recorded(
        ingest
            .ingest_datagram(&wspr_decode_datagram(true, 72_140_000, false), received_at)
            .unwrap(),
    );
    assert_policy(mismatch, WsprDecodeDisposition::StationIdentityMismatch);
}

#[test]
fn timestamp_reconstruction_handles_midnight_rollover() {
    let mut ingest = live_ingest();
    let received_at = utc(2026, 7, 10, 0, 0, 2);
    recorded(
        ingest
            .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), received_at)
            .unwrap(),
    );

    let message = recorded(
        ingest
            .ingest_datagram(&wspr_decode_datagram(true, 86_399_000, false), received_at)
            .unwrap(),
    );

    assert_eq!(
        message.wsjtx_record.meta.timestamp,
        utc(2026, 7, 9, 23, 59, 59)
    );
    assert_eq!(
        message.observation.unwrap().meta.timestamp,
        utc(2026, 7, 9, 23, 59, 59)
    );
}

#[test]
fn malformed_and_unsupported_datagrams_do_not_poison_receiver_state() {
    let mut ingest = live_ingest();
    let received_at = utc(2026, 7, 9, 20, 1, 51);
    let status = status_datagram("WSPR", "N1RWJ", "FN42");

    assert!(ingest.ingest_datagram(&status[..24], received_at).is_err());
    let ignored = ingest
        .ingest_datagram(&unsupported_datagram(3, 99), received_at)
        .unwrap();
    assert!(matches!(
        ignored,
        LiveIngestOutcome::IgnoredUnsupported { .. }
    ));
    assert!(ingest.client_state(CLIENT_ID).is_none());

    let status = recorded(ingest.ingest_datagram(&status, received_at).unwrap());
    assert_eq!(status.wsjtx_record.record_id, "live-test-wsjtx-000001");

    let decode = recorded(
        ingest
            .ingest_datagram(&wspr_decode_datagram(true, 72_110_000, false), received_at)
            .unwrap(),
    );
    assert!(decode.observation.is_some());
}

#[test]
fn timeout_and_close_create_deterministic_client_generations() {
    let mut ingest = live_ingest();
    let first_at = utc(2026, 7, 9, 20, 1, 0);
    let decode = wspr_decode_datagram(true, 72_110_000, false);
    recorded(
        ingest
            .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), first_at)
            .unwrap(),
    );
    assert!(recorded(ingest.ingest_datagram(&decode, first_at).unwrap())
        .observation
        .is_some());

    let after_timeout = utc(2026, 7, 9, 20, 1, 46);
    let missing_status = recorded(ingest.ingest_datagram(&decode, after_timeout).unwrap());
    assert_policy(missing_status, WsprDecodeDisposition::MissingStatus);
    assert_eq!(ingest.client_state(CLIENT_ID).unwrap().generation, 2);

    let close = recorded(
        ingest
            .ingest_datagram(&close_datagram(), after_timeout)
            .unwrap(),
    );
    assert_eq!(close.disposition, LiveMessageDisposition::Close);
    assert!(ingest.client_state(CLIENT_ID).is_none());

    recorded(
        ingest
            .ingest_datagram(&status_datagram("WSPR", "N1RWJ", "FN42"), after_timeout)
            .unwrap(),
    );
    let after_close = recorded(ingest.ingest_datagram(&decode, after_timeout).unwrap());
    assert!(after_close.observation.is_some());
    assert_eq!(ingest.client_state(CLIENT_ID).unwrap().generation, 1);
}

#[test]
fn live_observations_normalize_and_validate_in_the_existing_bundle_pipeline() {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let mut bundle = BundleStore::new(fixture_root).read_validated().unwrap();
    let config = LiveIngestConfig {
        session_id: bundle.manifest.session_id.clone(),
        receiver_id: "fixture-live".to_string(),
        station_callsign: bundle.station.callsign.clone(),
        station_grid: bundle.station.grid.clone(),
        session_started_at: bundle.manifest.created_at,
    };
    let mut ingest = LiveWsjtxIngest::new(config).unwrap();
    let fixture = synthetic_fixture();

    let status = recorded(
        ingest
            .ingest_datagram(&fixture.status, utc(2026, 7, 9, 20, 1, 45))
            .unwrap(),
    );
    append_live_wsjtx_message(&mut bundle, status);
    let decode = recorded(
        ingest
            .ingest_datagram(&fixture.decode, utc(2026, 7, 9, 20, 1, 51))
            .unwrap(),
    );
    append_live_wsjtx_message(&mut bundle, decode);

    let normalized = normalize_bundle(bundle);
    validate_bundle(&normalized).unwrap();
    let observation = normalized.observations.last().unwrap();
    assert_eq!(observation.slot_id.as_deref(), Some("slot-001"));
    assert_eq!(observation.meta.source, RecordSource::WsjtxUdp);
}

#[test]
fn udp_receiver_has_a_minimal_receive_and_shutdown_boundary() {
    let mut receiver = WsjtxUdpReceiver::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(StdDuration::from_secs(2)))
        .unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    let datagram = heartbeat_datagram(3, "2.6.1", "test");
    sender
        .send_to(&datagram, receiver.local_addr().unwrap())
        .unwrap();

    let received = receiver.receive().unwrap();
    assert_eq!(received.bytes, datagram);
    assert_eq!(received.source, sender.local_addr().unwrap());

    receiver.shutdown();
    assert!(receiver.is_shutdown());
    assert!(matches!(
        receiver.receive(),
        Err(UdpReceiverError::Shutdown)
    ));
}

fn live_ingest() -> LiveWsjtxIngest {
    live_ingest_with_limits(WSJTX_ADAPTER_LIMITS)
}

fn live_ingest_with_limits(limits: WsjtxAdapterLimits) -> LiveWsjtxIngest {
    LiveWsjtxIngest::new_with_limits(
        LiveIngestConfig {
            session_id: SESSION_ID.to_string(),
            receiver_id: "live-test".to_string(),
            station_callsign: "n1rwj".to_string(),
            station_grid: "fn42".to_string(),
            session_started_at: utc(2026, 7, 9, 19, 58, 0),
        },
        limits,
    )
    .unwrap()
}

fn recorded(outcome: LiveIngestOutcome) -> LiveRecordedMessage {
    let LiveIngestOutcome::Recorded(message) = outcome else {
        panic!("expected a preserved supported message");
    };
    *message
}

fn assert_policy(message: LiveRecordedMessage, expected: WsprDecodeDisposition) {
    assert!(message.observation.is_none());
    assert_eq!(
        message.disposition,
        LiveMessageDisposition::WsprDecode(expected)
    );
    assert_eq!(message.wsjtx_record.message_type, "udp_wspr_decode");
}

fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .unwrap()
}

struct SyntheticFixture {
    heartbeat: Vec<u8>,
    status: Vec<u8>,
    decode: Vec<u8>,
}

fn synthetic_fixture() -> SyntheticFixture {
    let mut values = include_str!("../../../fixtures/wsjtx/udp/schema3-live-sequence.hex")
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name, decode_hex(value)));
    let (_, heartbeat) = values.next().unwrap();
    let (_, status) = values.next().unwrap();
    let (_, decode) = values.next().unwrap();
    SyntheticFixture {
        heartbeat,
        status,
        decode,
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let digits = std::str::from_utf8(chunk).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}

fn heartbeat_datagram(schema: u32, version: &str, revision: &str) -> Vec<u8> {
    let mut bytes = header(schema, 0);
    if schema >= 3 {
        put_u32(&mut bytes, 3);
    }
    put_utf8(&mut bytes, version);
    put_utf8(&mut bytes, revision);
    bytes
}

fn heartbeat_for(client_id: &str) -> Vec<u8> {
    let mut bytes = header_for(3, 0, client_id);
    put_u32(&mut bytes, 3);
    put_utf8(&mut bytes, "2.6.1");
    put_utf8(&mut bytes, "test");
    bytes
}

fn status_datagram(mode: &str, de_call: &str, de_grid: &str) -> Vec<u8> {
    let mut bytes = header(3, 1);
    put_u64(&mut bytes, 14_095_600);
    put_utf8(&mut bytes, mode);
    put_utf8(&mut bytes, "");
    put_utf8(&mut bytes, "");
    put_utf8(&mut bytes, mode);
    put_bool(&mut bytes, true);
    put_bool(&mut bytes, false);
    put_bool(&mut bytes, true);
    put_u32(&mut bytes, 1_500);
    put_u32(&mut bytes, 1_500);
    put_utf8(&mut bytes, de_call);
    put_utf8(&mut bytes, de_grid);
    bytes
}

fn wspr_decode_datagram(is_new: bool, time_millis: u32, off_air: bool) -> Vec<u8> {
    let mut bytes = header(3, 10);
    put_bool(&mut bytes, is_new);
    put_u32(&mut bytes, time_millis);
    put_i32(&mut bytes, -18);
    put_f64(&mut bytes, 0.07);
    put_u64(&mut bytes, 14_095_600);
    put_i32(&mut bytes, 0);
    put_utf8(&mut bytes, "K1ABC");
    put_utf8(&mut bytes, "EM12");
    put_i32(&mut bytes, 37);
    put_bool(&mut bytes, off_air);
    bytes
}

fn close_datagram() -> Vec<u8> {
    header(3, 6)
}

fn unsupported_datagram(schema: u32, message_type: u32) -> Vec<u8> {
    let mut bytes = header(schema, message_type);
    bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    bytes
}

fn header(schema: u32, message_type: u32) -> Vec<u8> {
    header_for(schema, message_type, CLIENT_ID)
}

fn header_for(schema: u32, message_type: u32, client_id: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_u32(&mut bytes, WSJTX_MAGIC);
    put_u32(&mut bytes, schema);
    put_u32(&mut bytes, message_type);
    put_utf8(&mut bytes, client_id);
    bytes
}

fn put_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}

fn put_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_f64(bytes: &mut Vec<u8>, value: f64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn put_utf8(bytes: &mut Vec<u8>, value: &str) {
    put_u32(bytes, value.len() as u32);
    bytes.extend_from_slice(value.as_bytes());
}
