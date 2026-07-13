use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration as StdDuration;

use antennabench_core::{
    BundleContents, ObservationKind, ObservationRecord, RecordMeta, RecordSource, WsjtXRecord,
    SCHEMA_VERSION,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    band_from_frequency_hz, dbm_to_watts, normalize_maidenhead_grid, normalize_wspr_callsign,
    parse_wsjtx_datagram, DatagramParseError, HeartbeatMessage, ParsedDatagram, StatusMessage,
    UnsupportedMessage, WsjtxMessage, WsprDecodeMessage, MAX_SUPPORTED_SCHEMA,
};

const CLIENT_RESTART_TIMEOUT_SECONDS: i64 = 45;
const MAX_UDP_DATAGRAM_BYTES: usize = 65_535;

#[derive(Debug, Clone, PartialEq)]
pub struct LiveIngestConfig {
    pub session_id: String,
    pub receiver_id: String,
    pub station_callsign: String,
    pub station_grid: String,
    pub session_started_at: DateTime<Utc>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LiveIngestConfigError {
    #[error("session_id must not be empty")]
    EmptySessionId,
    #[error("receiver_id must not be empty")]
    EmptyReceiverId,
    #[error("invalid WSPR station callsign: {value}")]
    InvalidStationCallsign { value: String },
    #[error("invalid Maidenhead station grid: {value}")]
    InvalidStationGrid { value: String },
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum LiveIngestError {
    #[error(transparent)]
    Parse(#[from] DatagramParseError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveClientState {
    pub client_id: String,
    pub generation: u32,
    pub schema: u32,
    pub maximum_schema: Option<u32>,
    pub negotiated_schema: Option<u32>,
    pub version: Option<String>,
    pub revision: Option<String>,
    pub status: Option<StatusMessage>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiveIngestOutcome {
    IgnoredUnsupported {
        schema: u32,
        message: UnsupportedMessage,
    },
    Recorded(Box<LiveRecordedMessage>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveRecordedMessage {
    pub wsjtx_record: WsjtXRecord,
    pub observation: Option<ObservationRecord>,
    pub disposition: LiveMessageDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveMessageDisposition {
    Heartbeat,
    Status,
    Close,
    WsprDecode(WsprDecodeDisposition),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsprDecodeDisposition {
    ObservationProduced,
    Replay,
    OffAir,
    Duplicate,
    MissingStatus,
    NonWsprMode,
    StationIdentityMismatch,
    UnsupportedBand,
    InvalidCallsign,
    InvalidGrid,
    InvalidPower,
}

impl WsprDecodeDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObservationProduced => "observation_produced",
            Self::Replay => "replay",
            Self::OffAir => "off_air",
            Self::Duplicate => "duplicate",
            Self::MissingStatus => "missing_status",
            Self::NonWsprMode => "non_wspr_mode",
            Self::StationIdentityMismatch => "station_identity_mismatch",
            Self::UnsupportedBand => "unsupported_band",
            Self::InvalidCallsign => "invalid_callsign",
            Self::InvalidGrid => "invalid_grid",
            Self::InvalidPower => "invalid_power",
        }
    }
}

#[derive(Debug)]
pub struct LiveWsjtxIngest {
    config: LiveIngestConfig,
    station_callsign: String,
    station_grid: String,
    next_sequence: u64,
    clients: HashMap<String, ClientRuntime>,
}

#[derive(Debug)]
struct ClientRuntime {
    state: LiveClientState,
    seen_wspr_datagrams: HashSet<Vec<u8>>,
}

impl LiveWsjtxIngest {
    pub fn new(config: LiveIngestConfig) -> Result<Self, LiveIngestConfigError> {
        if config.session_id.trim().is_empty() {
            return Err(LiveIngestConfigError::EmptySessionId);
        }
        if config.receiver_id.trim().is_empty() {
            return Err(LiveIngestConfigError::EmptyReceiverId);
        }
        let station_callsign =
            normalize_wspr_callsign(&config.station_callsign).ok_or_else(|| {
                LiveIngestConfigError::InvalidStationCallsign {
                    value: config.station_callsign.clone(),
                }
            })?;
        let station_grid = normalize_maidenhead_grid(&config.station_grid).ok_or_else(|| {
            LiveIngestConfigError::InvalidStationGrid {
                value: config.station_grid.clone(),
            }
        })?;

        Ok(Self {
            config,
            station_callsign,
            station_grid,
            next_sequence: 1,
            clients: HashMap::new(),
        })
    }

    pub fn client_state(&self, client_id: &str) -> Option<&LiveClientState> {
        self.clients.get(client_id).map(|client| &client.state)
    }

    pub fn ingest_datagram(
        &mut self,
        datagram: &[u8],
        received_at: DateTime<Utc>,
    ) -> Result<LiveIngestOutcome, LiveIngestError> {
        let parsed = parse_wsjtx_datagram(datagram)?;

        if let WsjtxMessage::Unsupported(message) = parsed.message {
            return Ok(LiveIngestOutcome::IgnoredUnsupported {
                schema: parsed.schema,
                message,
            });
        }

        let client_id = parsed.message.client_id().to_string();
        self.touch_client(&client_id, parsed.schema, received_at);
        let sequence = self.next_sequence;
        self.next_sequence += 1;

        let result = match &parsed.message {
            WsjtxMessage::Heartbeat(message) => {
                self.apply_heartbeat(message, parsed.schema, received_at);
                self.recorded_message(
                    sequence,
                    received_at,
                    datagram,
                    &parsed,
                    heartbeat_raw(message),
                    "udp_heartbeat",
                    LiveMessageDisposition::Heartbeat,
                    None,
                )
            }
            WsjtxMessage::Status(message) => {
                self.apply_status(message, parsed.schema, received_at);
                self.recorded_message(
                    sequence,
                    received_at,
                    datagram,
                    &parsed,
                    status_raw(message),
                    "udp_status",
                    LiveMessageDisposition::Status,
                    None,
                )
            }
            WsjtxMessage::WsprDecode(message) => {
                let source_timestamp = reconstruct_timestamp(
                    message.time_millis,
                    received_at,
                    self.config.session_started_at,
                );
                let (observation, disposition) = self.process_wspr_decode(
                    message,
                    datagram,
                    source_timestamp,
                    received_at,
                    sequence,
                );
                self.recorded_message(
                    sequence,
                    source_timestamp,
                    datagram,
                    &parsed,
                    wspr_decode_raw(message, received_at, disposition),
                    "udp_wspr_decode",
                    LiveMessageDisposition::WsprDecode(disposition),
                    observation,
                )
            }
            WsjtxMessage::Close(_) => {
                let recorded = self.recorded_message(
                    sequence,
                    received_at,
                    datagram,
                    &parsed,
                    json!({}),
                    "udp_close",
                    LiveMessageDisposition::Close,
                    None,
                );
                self.clients.remove(&client_id);
                recorded
            }
            WsjtxMessage::Unsupported(_) => unreachable!("handled before state mutation"),
        };

        Ok(LiveIngestOutcome::Recorded(Box::new(result)))
    }

    fn touch_client(&mut self, client_id: &str, schema: u32, received_at: DateTime<Utc>) {
        match self.clients.get_mut(client_id) {
            Some(client) => {
                let elapsed = received_at.signed_duration_since(client.state.last_seen_at);
                if elapsed > Duration::seconds(CLIENT_RESTART_TIMEOUT_SECONDS) {
                    client.state.generation += 1;
                    client.state.maximum_schema = None;
                    client.state.negotiated_schema = None;
                    client.state.version = None;
                    client.state.revision = None;
                    client.state.status = None;
                    client.seen_wspr_datagrams.clear();
                }
                client.state.schema = schema;
                client.state.last_seen_at = received_at;
            }
            None => {
                self.clients.insert(
                    client_id.to_string(),
                    ClientRuntime {
                        state: LiveClientState {
                            client_id: client_id.to_string(),
                            generation: 1,
                            schema,
                            maximum_schema: None,
                            negotiated_schema: None,
                            version: None,
                            revision: None,
                            status: None,
                            last_seen_at: received_at,
                        },
                        seen_wspr_datagrams: HashSet::new(),
                    },
                );
            }
        }
    }

    fn apply_heartbeat(
        &mut self,
        message: &HeartbeatMessage,
        schema: u32,
        received_at: DateTime<Utc>,
    ) {
        let client = self
            .clients
            .get_mut(&message.client_id)
            .expect("client touched before heartbeat");
        client.state.schema = schema;
        client.state.maximum_schema = Some(message.maximum_schema);
        client.state.negotiated_schema = Some(message.maximum_schema.min(MAX_SUPPORTED_SCHEMA));
        client.state.version = Some(message.version.clone());
        client.state.revision = Some(message.revision.clone());
        client.state.last_seen_at = received_at;
    }

    fn apply_status(&mut self, message: &StatusMessage, schema: u32, received_at: DateTime<Utc>) {
        let client = self
            .clients
            .get_mut(&message.client_id)
            .expect("client touched before status");
        client.state.schema = schema;
        client.state.status = Some(message.clone());
        client.state.last_seen_at = received_at;
    }

    fn process_wspr_decode(
        &mut self,
        message: &WsprDecodeMessage,
        datagram: &[u8],
        source_timestamp: DateTime<Utc>,
        received_at: DateTime<Utc>,
        sequence: u64,
    ) -> (Option<ObservationRecord>, WsprDecodeDisposition) {
        let client = self
            .clients
            .get_mut(&message.client_id)
            .expect("client touched before decode");
        let duplicate = !client.seen_wspr_datagrams.insert(datagram.to_vec());

        if !message.is_new {
            return (None, WsprDecodeDisposition::Replay);
        }
        if message.off_air {
            return (None, WsprDecodeDisposition::OffAir);
        }
        if duplicate {
            return (None, WsprDecodeDisposition::Duplicate);
        }

        let Some(status) = &client.state.status else {
            return (None, WsprDecodeDisposition::MissingStatus);
        };
        if !status.mode.eq_ignore_ascii_case("WSPR") {
            return (None, WsprDecodeDisposition::NonWsprMode);
        }
        if !status.de_call.eq_ignore_ascii_case(&self.station_callsign)
            || !status.de_grid.eq_ignore_ascii_case(&self.station_grid)
        {
            return (None, WsprDecodeDisposition::StationIdentityMismatch);
        }

        let Some(band) = band_from_frequency_hz(message.frequency_hz) else {
            return (None, WsprDecodeDisposition::UnsupportedBand);
        };
        let Some(callsign) = normalize_wspr_callsign(&message.callsign) else {
            return (None, WsprDecodeDisposition::InvalidCallsign);
        };
        let Some(grid) = normalize_maidenhead_grid(&message.grid) else {
            return (None, WsprDecodeDisposition::InvalidGrid);
        };
        let Ok(power_dbm) = i16::try_from(message.power_dbm) else {
            return (None, WsprDecodeDisposition::InvalidPower);
        };

        let hex = encode_hex(datagram);
        let observation = ObservationRecord {
            meta: RecordMeta {
                schema_version: SCHEMA_VERSION,
                session_id: self.config.session_id.clone(),
                timestamp: source_timestamp,
                source: RecordSource::WsjtxUdp,
            },
            observation_id: format!("{}-obs-{sequence:06}", self.config.receiver_id),
            observation_kind: ObservationKind::LocalDecode,
            band,
            frequency_hz: Some(message.frequency_hz),
            mode: Some("WSPR".to_string()),
            reporter_call: Some(self.station_callsign.clone()),
            heard_call: Some(callsign),
            reporter_grid: Some(self.station_grid.clone()),
            heard_grid: Some(grid),
            distance_km: None,
            azimuth_degrees: None,
            snr_db: Some(message.snr_db as f32),
            drift_hz_per_minute: Some(message.drift_hz_per_minute as f32),
            power_watts: Some(dbm_to_watts(power_dbm)),
            slot_id: None,
            slot_label: None,
            slot_confidence: None,
            raw: json!({
                "client_id": message.client_id,
                "datagram_hex": hex,
                "received_at": received_at,
                "source_time_millis": message.time_millis,
                "delta_time_seconds": message.delta_time_seconds,
                "power_dbm": message.power_dbm,
                "new": message.is_new,
                "off_air": message.off_air,
            }),
        };

        (
            Some(observation),
            WsprDecodeDisposition::ObservationProduced,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn recorded_message(
        &self,
        sequence: u64,
        timestamp: DateTime<Utc>,
        datagram: &[u8],
        parsed: &ParsedDatagram,
        fields: Value,
        message_type: &str,
        disposition: LiveMessageDisposition,
        observation: Option<ObservationRecord>,
    ) -> LiveRecordedMessage {
        LiveRecordedMessage {
            wsjtx_record: WsjtXRecord {
                meta: RecordMeta {
                    schema_version: SCHEMA_VERSION,
                    session_id: self.config.session_id.clone(),
                    timestamp,
                    source: RecordSource::WsjtxUdp,
                },
                record_id: format!("{}-wsjtx-{sequence:06}", self.config.receiver_id),
                message_type: message_type.to_string(),
                raw: json!({
                    "client_id": parsed.message.client_id(),
                    "datagram_hex": encode_hex(datagram),
                    "fields": fields,
                    "message_type_id": parsed.message.message_type(),
                    "schema": parsed.schema,
                    "trailing_bytes": parsed.trailing_bytes,
                }),
            },
            observation,
            disposition,
        }
    }
}

pub fn append_live_wsjtx_message(bundle: &mut BundleContents, message: LiveRecordedMessage) {
    bundle.wsjtx.push(message.wsjtx_record);
    if let Some(observation) = message.observation {
        bundle.observations.push(observation);
    }
}

fn heartbeat_raw(message: &HeartbeatMessage) -> Value {
    json!({
        "maximum_schema": message.maximum_schema,
        "revision": message.revision,
        "version": message.version,
    })
}

fn status_raw(message: &StatusMessage) -> Value {
    json!({
        "dial_frequency_hz": message.dial_frequency_hz,
        "mode": message.mode,
        "dx_call": message.dx_call,
        "report": message.report,
        "tx_mode": message.tx_mode,
        "tx_enabled": message.tx_enabled,
        "transmitting": message.transmitting,
        "decoding": message.decoding,
        "rx_df_hz": message.rx_df_hz,
        "tx_df_hz": message.tx_df_hz,
        "de_call": message.de_call,
        "de_grid": message.de_grid,
    })
}

fn wspr_decode_raw(
    message: &WsprDecodeMessage,
    received_at: DateTime<Utc>,
    disposition: WsprDecodeDisposition,
) -> Value {
    json!({
        "new": message.is_new,
        "time_millis": message.time_millis,
        "snr_db": message.snr_db,
        "delta_time_seconds": message.delta_time_seconds,
        "frequency_hz": message.frequency_hz,
        "drift_hz_per_minute": message.drift_hz_per_minute,
        "callsign": message.callsign,
        "grid": message.grid,
        "power_dbm": message.power_dbm,
        "off_air": message.off_air,
        "observation_disposition": disposition.as_str(),
        "received_at": received_at,
    })
}

fn reconstruct_timestamp(
    time_millis: u32,
    received_at: DateTime<Utc>,
    session_started_at: DateTime<Utc>,
) -> DateTime<Utc> {
    let seconds = time_millis / 1_000;
    let nanos = (time_millis % 1_000) * 1_000_000;
    let time = NaiveTime::from_num_seconds_from_midnight_opt(seconds, nanos)
        .expect("parser validated QTime milliseconds");
    let receipt_date = received_at.date_naive();
    let dates = [
        receipt_date.pred_opt(),
        Some(receipt_date),
        receipt_date.succ_opt(),
    ];

    dates
        .into_iter()
        .flatten()
        .map(|date| datetime(date, time))
        .min_by_key(|candidate| {
            let distance = candidate
                .signed_duration_since(received_at)
                .num_milliseconds()
                .unsigned_abs();
            let before_session = *candidate < session_started_at;
            (distance, before_session, *candidate)
        })
        .expect("receipt date always has at least one candidate")
}

fn datetime(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(date.and_time(time), Utc)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[derive(Debug)]
pub struct WsjtxUdpReceiver {
    socket: UdpSocket,
    shutdown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedUdpDatagram {
    pub bytes: Vec<u8>,
    pub source: SocketAddr,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum UdpReceiverError {
    #[error("WSJT-X UDP receiver has been shut down")]
    Shutdown,
    #[error("failed to receive WSJT-X UDP datagram")]
    Receive(#[source] io::Error),
}

impl WsjtxUdpReceiver {
    pub fn bind(address: impl ToSocketAddrs) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(address)?,
            shutdown: false,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn set_read_timeout(&self, timeout: Option<StdDuration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    pub fn receive(&mut self) -> Result<ReceivedUdpDatagram, UdpReceiverError> {
        if self.shutdown {
            return Err(UdpReceiverError::Shutdown);
        }

        let mut buffer = vec![0; MAX_UDP_DATAGRAM_BYTES];
        let (length, source) = self
            .socket
            .recv_from(&mut buffer)
            .map_err(UdpReceiverError::Receive)?;
        buffer.truncate(length);

        Ok(ReceivedUdpDatagram {
            bytes: buffer,
            source,
            received_at: Utc::now(),
        })
    }

    pub fn shutdown(&mut self) {
        self.shutdown = true;
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }
}
