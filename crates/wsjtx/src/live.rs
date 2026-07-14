use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration as StdDuration;

use antennabench_core::{
    BundleContents, ObservationKind, ObservationRecord, RecordMeta, RecordSource, WsjtXRecord,
    SCHEMA_VERSION,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    band_from_frequency_hz, dbm_to_watts, normalize_maidenhead_grid, normalize_wspr_callsign,
    parse_wsjtx_datagram, DatagramParseError, HeartbeatMessage, ParsedDatagram, StatusMessage,
    UnsupportedMessage, WsjtxMessage, WsprDecodeMessage, MAX_SUPPORTED_SCHEMA,
};
use crate::{
    diagnostic, AdapterResourceDiagnostic, AdapterResourceError, AdapterResourceStage,
    AdapterResourceUnit, WsjtxAdapterLimits, WSJTX_ADAPTER_LIMITS,
};

const CLIENT_RESTART_TIMEOUT_SECONDS: i64 = 45;
pub const MAX_UDP_DATAGRAM_BYTES: usize = 65_535;

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
    #[error(transparent)]
    Resource(#[from] AdapterResourceError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveAcquisitionGap {
    pub stopped_at: DateTime<Utc>,
    pub diagnostic: AdapterResourceDiagnostic,
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
    limits: WsjtxAdapterLimits,
    admission: TokenBucket,
    gap: Option<LiveAcquisitionGap>,
}

#[derive(Debug)]
struct ClientRuntime {
    state: LiveClientState,
    seen_wspr_datagrams: VecDeque<([u8; 32], DateTime<Utc>)>,
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: DateTime<Utc>,
}

impl LiveWsjtxIngest {
    pub fn new(config: LiveIngestConfig) -> Result<Self, LiveIngestConfigError> {
        Self::new_with_limits(config, WSJTX_ADAPTER_LIMITS)
    }

    #[doc(hidden)]
    pub fn new_with_limits(
        config: LiveIngestConfig,
        limits: WsjtxAdapterLimits,
    ) -> Result<Self, LiveIngestConfigError> {
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

        let session_started_at = config.session_started_at;
        Ok(Self {
            config,
            station_callsign,
            station_grid,
            next_sequence: 1,
            clients: HashMap::new(),
            limits,
            admission: TokenBucket {
                tokens: limits.udp_rate_burst as f64,
                last_refill: session_started_at,
            },
            gap: None,
        })
    }

    pub fn client_state(&self, client_id: &str) -> Option<&LiveClientState> {
        self.clients.get(client_id).map(|client| &client.state)
    }

    pub fn acquisition_gap(&self) -> Option<&LiveAcquisitionGap> {
        self.gap.as_ref()
    }

    pub fn is_stopped(&self) -> bool {
        self.gap.is_some()
    }

    pub fn ingest_datagram(
        &mut self,
        datagram: &[u8],
        received_at: DateTime<Utc>,
    ) -> Result<LiveIngestOutcome, LiveIngestError> {
        if let Some(gap) = &self.gap {
            return Err(AdapterResourceError {
                diagnostic: gap.diagnostic.clone(),
            }
            .into());
        }
        if datagram.len() as u64 > self.limits.udp_datagram_bytes {
            return Err(self.stop(
                "resource.adapter.udp.datagram_bytes",
                AdapterResourceStage::Admission,
                self.limits.udp_datagram_bytes,
                Some(datagram.len() as u64),
                AdapterResourceUnit::Bytes,
                received_at,
            ));
        }
        self.admit_rate(received_at)?;
        let parsed = parse_wsjtx_datagram(datagram)?;
        let client_id = parsed.message.client_id().to_string();
        if client_id.len() as u64 > self.limits.udp_client_id_bytes {
            return Err(self.stop(
                "resource.adapter.udp.client_id_bytes",
                AdapterResourceStage::Admission,
                self.limits.udp_client_id_bytes,
                Some(client_id.len() as u64),
                AdapterResourceUnit::Bytes,
                received_at,
            ));
        }
        if let WsjtxMessage::Unsupported(message) = parsed.message {
            return Ok(LiveIngestOutcome::IgnoredUnsupported {
                schema: parsed.schema,
                message,
            });
        }
        self.touch_client(&client_id, parsed.schema, received_at)?;
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
                )?;
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

    fn admit_rate(&mut self, received_at: DateTime<Utc>) -> Result<(), LiveIngestError> {
        let elapsed = received_at
            .signed_duration_since(self.admission.last_refill)
            .num_microseconds()
            .unwrap_or_default()
            .max(0) as f64
            / 1_000_000.0;
        self.admission.tokens = (self.admission.tokens
            + elapsed * self.limits.udp_rate_per_second as f64)
            .min(self.limits.udp_rate_burst as f64);
        if received_at > self.admission.last_refill {
            self.admission.last_refill = received_at;
        }
        if self.admission.tokens < 1.0 {
            return Err(self.stop(
                "resource.adapter.udp.rate",
                AdapterResourceStage::Admission,
                self.limits.udp_rate_burst,
                Some(self.limits.udp_rate_burst + 1),
                AdapterResourceUnit::Datagrams,
                received_at,
            ));
        }
        self.admission.tokens -= 1.0;
        Ok(())
    }

    fn stop(
        &mut self,
        code: &'static str,
        stage: AdapterResourceStage,
        limit: u64,
        observed: Option<u64>,
        unit: AdapterResourceUnit,
        stopped_at: DateTime<Utc>,
    ) -> LiveIngestError {
        let error = diagnostic(
            code,
            "wsjtx.live_udp",
            &self.config.receiver_id,
            stage,
            limit,
            observed,
            unit,
            true,
        );
        self.gap.get_or_insert_with(|| LiveAcquisitionGap {
            stopped_at,
            diagnostic: error.diagnostic.clone(),
        });
        LiveIngestError::Resource(error)
    }

    fn touch_client(
        &mut self,
        client_id: &str,
        schema: u32,
        received_at: DateTime<Utc>,
    ) -> Result<(), LiveIngestError> {
        if !self.clients.contains_key(client_id)
            && self.clients.len() as u64 >= self.limits.udp_clients
        {
            let eviction_cutoff =
                received_at - Duration::seconds(self.limits.udp_idle_eviction_seconds);
            let evict = self
                .clients
                .iter()
                .filter(|(_, client)| client.state.last_seen_at <= eviction_cutoff)
                .min_by(|(left_id, left), (right_id, right)| {
                    left.state
                        .last_seen_at
                        .cmp(&right.state.last_seen_at)
                        .then_with(|| left_id.cmp(right_id))
                })
                .map(|(id, _)| id.clone());
            if let Some(evict) = evict {
                self.clients.remove(&evict);
            } else {
                return Err(self.stop(
                    "resource.adapter.udp.clients",
                    AdapterResourceStage::Admission,
                    self.limits.udp_clients,
                    Some(self.clients.len() as u64 + 1),
                    AdapterResourceUnit::Clients,
                    received_at,
                ));
            }
        }
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
                        seen_wspr_datagrams: VecDeque::new(),
                    },
                );
            }
        }
        Ok(())
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
    ) -> Result<(Option<ObservationRecord>, WsprDecodeDisposition), LiveIngestError> {
        let fingerprint: [u8; 32] = Sha256::digest(datagram).into();
        let dedup_cutoff = received_at - Duration::seconds(self.limits.udp_dedup_window_seconds);
        let (duplicate, dedup_overflow) = {
            let client = self
                .clients
                .get_mut(&message.client_id)
                .expect("client touched before decode");
            while client
                .seen_wspr_datagrams
                .front()
                .is_some_and(|(_, seen_at)| *seen_at < dedup_cutoff)
            {
                client.seen_wspr_datagrams.pop_front();
            }
            let duplicate = client
                .seen_wspr_datagrams
                .iter()
                .any(|(seen, _)| *seen == fingerprint);
            let overflow = (!duplicate
                && client.seen_wspr_datagrams.len() as u64
                    >= self.limits.udp_dedup_entries_per_client)
                .then_some(client.seen_wspr_datagrams.len() as u64 + 1);
            if !duplicate && overflow.is_none() {
                client
                    .seen_wspr_datagrams
                    .push_back((fingerprint, received_at));
            }
            (duplicate, overflow)
        };
        if let Some(observed) = dedup_overflow {
            return Err(self.stop(
                "resource.adapter.udp.dedup_entries",
                AdapterResourceStage::Admission,
                self.limits.udp_dedup_entries_per_client,
                Some(observed),
                AdapterResourceUnit::Entries,
                received_at,
            ));
        }

        if !message.is_new {
            return Ok((None, WsprDecodeDisposition::Replay));
        }
        if message.off_air {
            return Ok((None, WsprDecodeDisposition::OffAir));
        }
        if duplicate {
            return Ok((None, WsprDecodeDisposition::Duplicate));
        }

        let client = self
            .clients
            .get(&message.client_id)
            .expect("client touched before decode");
        let Some(status) = &client.state.status else {
            return Ok((None, WsprDecodeDisposition::MissingStatus));
        };
        if !status.mode.eq_ignore_ascii_case("WSPR") {
            return Ok((None, WsprDecodeDisposition::NonWsprMode));
        }
        if !status.de_call.eq_ignore_ascii_case(&self.station_callsign)
            || !status.de_grid.eq_ignore_ascii_case(&self.station_grid)
        {
            return Ok((None, WsprDecodeDisposition::StationIdentityMismatch));
        }

        let Some(band) = band_from_frequency_hz(message.frequency_hz) else {
            return Ok((None, WsprDecodeDisposition::UnsupportedBand));
        };
        let Some(callsign) = normalize_wspr_callsign(&message.callsign) else {
            return Ok((None, WsprDecodeDisposition::InvalidCallsign));
        };
        let Some(grid) = normalize_maidenhead_grid(&message.grid) else {
            return Ok((None, WsprDecodeDisposition::InvalidGrid));
        };
        let Ok(power_dbm) = i16::try_from(message.power_dbm) else {
            return Ok((None, WsprDecodeDisposition::InvalidPower));
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

        Ok((
            Some(observation),
            WsprDecodeDisposition::ObservationProduced,
        ))
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

#[derive(Debug)]
pub struct BoundedUdpQueue {
    receiver_id: String,
    limits: WsjtxAdapterLimits,
    queued_bytes: u64,
    datagrams: VecDeque<ReceivedUdpDatagram>,
    gap: Option<LiveAcquisitionGap>,
}

impl BoundedUdpQueue {
    pub fn new(receiver_id: impl Into<String>) -> Self {
        Self::with_limits(receiver_id, WSJTX_ADAPTER_LIMITS)
    }

    #[doc(hidden)]
    pub fn with_limits(receiver_id: impl Into<String>, limits: WsjtxAdapterLimits) -> Self {
        Self {
            receiver_id: receiver_id.into(),
            limits,
            queued_bytes: 0,
            datagrams: VecDeque::new(),
            gap: None,
        }
    }

    pub fn push(&mut self, datagram: ReceivedUdpDatagram) -> Result<(), AdapterResourceError> {
        if let Some(gap) = &self.gap {
            return Err(AdapterResourceError {
                diagnostic: gap.diagnostic.clone(),
            });
        }
        let observed_items = self.datagrams.len() as u64 + 1;
        let observed_bytes = self.queued_bytes + datagram.bytes.len() as u64;
        let (code, limit, observed, unit) = if observed_items > self.limits.udp_queue_datagrams {
            (
                "resource.adapter.udp.queue_datagrams",
                self.limits.udp_queue_datagrams,
                observed_items,
                AdapterResourceUnit::Datagrams,
            )
        } else if observed_bytes > self.limits.udp_queue_bytes {
            (
                "resource.adapter.udp.queue_bytes",
                self.limits.udp_queue_bytes,
                observed_bytes,
                AdapterResourceUnit::Bytes,
            )
        } else {
            self.queued_bytes = observed_bytes;
            self.datagrams.push_back(datagram);
            return Ok(());
        };
        let failure = diagnostic(
            code,
            "wsjtx.live_udp",
            &self.receiver_id,
            AdapterResourceStage::Queue,
            limit,
            Some(observed),
            unit,
            true,
        );
        self.gap = Some(LiveAcquisitionGap {
            stopped_at: datagram.received_at,
            diagnostic: failure.diagnostic.clone(),
        });
        Err(failure)
    }

    pub fn pop(&mut self) -> Option<ReceivedUdpDatagram> {
        self.datagrams.pop_front().inspect(|datagram| {
            self.queued_bytes -= datagram.bytes.len() as u64;
        })
    }

    pub fn len(&self) -> usize {
        self.datagrams.len()
    }

    pub fn is_empty(&self) -> bool {
        self.datagrams.is_empty()
    }

    pub fn queued_bytes(&self) -> u64 {
        self.queued_bytes
    }

    pub fn acquisition_gap(&self) -> Option<&LiveAcquisitionGap> {
        self.gap.as_ref()
    }
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
