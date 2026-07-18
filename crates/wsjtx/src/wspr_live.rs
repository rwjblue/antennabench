use std::collections::{BTreeMap, BTreeSet};

use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput, AdapterReasonId,
        AdapterRecordV2, AttachmentReference, MutationMember, NormalizedRecordKind,
        NormalizedRecordLink, ObservationRecordV2, Provenance, ProviderId, RecordMetaV2, SourceId,
    },
    v3::WsprCycleDirection,
    Band, ObservationKind, PlannedSlot, SCHEMA_VERSION_V2, WSPR_NOMINAL_START_OFFSET_SECONDS,
};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::wspr_live_alignment::{matching_confirmed_cycle, wspr_live_query_window_start};
use crate::{
    diagnostic, normalize_maidenhead_grid, normalize_wspr_callsign, AdapterCancellationToken,
    AdapterResourceError, AdapterResourceStage, AdapterResourceUnit,
};

pub const WSPR_LIVE_ADAPTER_ID: &str = "antennabench.wspr-live-json";
pub const WSPR_LIVE_PROVIDER_ID: &str = "wspr-live";
pub const WSPR_LIVE_SOURCE_ID: &str = "wsprnet-spots-mirror";
pub const WSPR_LIVE_ACQUISITION_CHANNEL: &str = "file-import";
pub const WSPR_LIVE_HTTPS_ACQUISITION_CHANNEL: &str = "https-query";
pub const WSPR_LIVE_ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const WSPR_LIVE_WSPR2_CODE: i16 = 1;
pub const WSPR_LIVE_QUERY_ENDPOINT: &str = "https://db1.wspr.live/";
pub const WSPR_LIVE_INGESTION_GRACE_SECONDS: i64 = 5 * 60;
pub const WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS: i64 = 10;
/// WSPR.live names a WSPR-2 slot by its even-minute boundary, while
/// AntennaBench records the transmission start one second into that slot.
pub const WSPR_LIVE_SLOT_TIMESTAMP_OFFSET_SECONDS: i64 = WSPR_NOMINAL_START_OFFSET_SECONDS;

pub const WSPR_LIVE_COLUMNS: [&str; 16] = [
    "id",
    "time",
    "band",
    "rx_sign",
    "rx_loc",
    "tx_sign",
    "tx_loc",
    "distance",
    "azimuth",
    "rx_azimuth",
    "frequency",
    "power",
    "snr",
    "drift",
    "version",
    "code",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsprLiveImportLimits {
    pub source_bytes: u64,
    pub rows: u64,
    pub row_bytes: u64,
}

pub const WSPR_LIVE_IMPORT_LIMITS: WsprLiveImportLimits = WsprLiveImportLimits {
    source_bytes: 32 * 1024 * 1024,
    rows: 100_000,
    row_bytes: 64 * 1024,
};

impl WsprLiveImportLimits {
    #[doc(hidden)]
    pub fn testing(limit: u64) -> Self {
        Self {
            source_bytes: limit * 8,
            rows: limit,
            row_bytes: limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsprLiveImportConfig {
    pub session_callsign: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub selected_bands: Vec<Band>,
    pub captured_at: DateTime<Utc>,
    pub source_locator: Option<String>,
    /// `None` for historical schedules whose cycle direction was not durably known.
    pub confirmed_cycles: Option<Vec<WsprLiveConfirmedCycle>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprLiveConfirmedCycle {
    pub starts_at: DateTime<Utc>,
    pub transmission_ends_at: DateTime<Utc>,
    pub band: Band,
    pub direction: Option<WsprCycleDirection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprLiveQueryScope {
    pub session_callsign: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub selected_bands: Vec<Band>,
}

impl From<&WsprLiveImportConfig> for WsprLiveQueryScope {
    fn from(config: &WsprLiveImportConfig) -> Self {
        Self {
            session_callsign: config.session_callsign.clone(),
            window_start: config.window_start,
            window_end: config.window_end,
            selected_bands: config.selected_bands.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprLiveQueryPlan {
    pub session_callsign: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub provider_bands: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprLiveAcquisitionPlan {
    pub completed_slot_id: String,
    pub segment_ended_at: DateTime<Utc>,
    pub not_before: DateTime<Utc>,
    pub query: WsprLiveQueryPlan,
}

impl WsprLiveQueryPlan {
    pub fn sql(&self) -> String {
        let bands = self
            .provider_bands
            .iter()
            .map(i16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let window_start = self.window_start.format("%Y-%m-%d %H:%M:%S");
        let window_end = self.window_end.format("%Y-%m-%d %H:%M:%S");

        format!(
            "SELECT {} FROM wspr.rx WHERE (tx_sign = '{}' OR rx_sign = '{}') AND time >= toDateTime('{}', 'UTC') AND time < toDateTime('{}', 'UTC') AND band IN ({bands}) AND code = {} ORDER BY time, id FORMAT JSON",
            WSPR_LIVE_COLUMNS.join(", "),
            self.session_callsign,
            self.session_callsign,
            window_start,
            window_end,
            WSPR_LIVE_WSPR2_CODE,
        )
    }

    pub fn query_url(&self) -> String {
        format!(
            "{WSPR_LIVE_QUERY_ENDPOINT}?query={}",
            percent_encode_query(&self.sql())
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWsprLiveJson {
    pub rows: Vec<WsprLiveRowResult>,
    pub summary: WsprLiveDispositionSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsprLiveRowDisposition {
    Accepted,
    Malformed,
    Filtered,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsprLiveRowReason {
    Accepted,
    InvalidValue,
    CallsignFiltered,
    AmbiguousCallsign,
    DirectionFiltered,
    TimeFiltered,
    BandFiltered,
    UnsupportedBand,
    UnsupportedMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsprLiveRowResult {
    pub row_number: usize,
    pub provider_spot_id: Option<u64>,
    pub raw: Value,
    pub disposition: WsprLiveRowDisposition,
    pub reason: WsprLiveRowReason,
    pub spot: Option<WsprLiveSpot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsprLiveSpot {
    pub provider_spot_id: u64,
    pub observed_at: DateTime<Utc>,
    pub band: Band,
    pub reporter_call: String,
    pub reporter_grid: Option<String>,
    pub transmitter_call: String,
    pub transmitter_grid: Option<String>,
    pub distance_km: Option<f64>,
    pub azimuth_degrees: Option<f64>,
    pub receiver_azimuth_degrees: Option<f64>,
    pub frequency_hz: u64,
    pub power_dbm: i16,
    pub snr_db: f32,
    pub drift_hz_per_minute: f32,
    pub receiver_version: Option<String>,
    pub mode_code: i16,
    pub direction: WsprLiveSpotDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsprLiveSpotDirection {
    Receive,
    Transmit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WsprLiveDispositionSummary {
    pub total: usize,
    pub accepted: usize,
    pub malformed: usize,
    pub filtered: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WsprLiveImportSummary {
    pub total: usize,
    pub accepted: usize,
    pub malformed: usize,
    pub filtered: usize,
    pub unsupported: usize,
    pub duplicate: usize,
    pub conflict: usize,
    pub observations_created: usize,
    pub evidence_completeness_known: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedWsprLiveImport {
    pub mutation_id: String,
    pub adapter_records: Vec<AdapterRecordV2>,
    pub observations: Vec<ObservationRecordV2>,
    pub summary: WsprLiveImportSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsprLiveAcquisitionChannel {
    FileImport,
    HttpsQuery,
}

impl WsprLiveAcquisitionChannel {
    fn id(self) -> &'static str {
        match self {
            Self::FileImport => WSPR_LIVE_ACQUISITION_CHANNEL,
            Self::HttpsQuery => WSPR_LIVE_HTTPS_ACQUISITION_CHANNEL,
        }
    }
}

#[derive(Debug, Error)]
pub enum WsprLiveImportError {
    #[error(transparent)]
    Resource(#[from] AdapterResourceError),
    #[error("invalid WSPR.live import configuration: {0}")]
    Config(String),
    #[error("invalid WSPR.live JSON document: {0}")]
    Json(String),
    #[error("unsupported WSPR.live JSON schema: {0}")]
    Schema(String),
}

pub fn plan_wspr_live_query(
    scope: &WsprLiveQueryScope,
) -> Result<WsprLiveQueryPlan, WsprLiveImportError> {
    validate_query_scope(scope)?;
    let session_callsign =
        normalize_wspr_callsign(scope.session_callsign.trim()).expect("validated query scope");
    let provider_bands = scope
        .selected_bands
        .iter()
        .copied()
        .map(band_to_wspr_live)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    Ok(WsprLiveQueryPlan {
        session_callsign,
        window_start: scope.window_start,
        window_end: scope.window_end,
        provider_bands,
    })
}

pub fn derive_wspr_live_query_scope(
    session_callsign: &str,
    slots: &[PlannedSlot],
) -> Result<WsprLiveQueryScope, WsprLiveImportError> {
    let first_slot = slots.first().ok_or_else(|| {
        WsprLiveImportError::Config("schedule must contain at least one slot".into())
    })?;
    let mut window_start = wspr_live_query_window_start(first_slot.starts_at);
    let mut window_end = slot_end(first_slot)?;
    let mut selected_bands = vec![first_slot.band];

    for slot in &slots[1..] {
        window_start = window_start.min(wspr_live_query_window_start(slot.starts_at));
        window_end = window_end.max(slot_end(slot)?);
        if !selected_bands.contains(&slot.band) {
            selected_bands.push(slot.band);
        }
    }

    let scope = WsprLiveQueryScope {
        session_callsign: session_callsign.into(),
        window_start,
        window_end,
        selected_bands,
    };
    validate_query_scope(&scope)?;
    Ok(scope)
}

pub fn plan_wspr_live_acquisition_for_completed_slot(
    session_callsign: &str,
    slots: &[PlannedSlot],
    completed_slot_id: &str,
) -> Result<WsprLiveAcquisitionPlan, WsprLiveImportError> {
    let completed_slot = slots
        .iter()
        .find(|slot| slot.slot_id == completed_slot_id)
        .ok_or_else(|| {
            WsprLiveImportError::Config(format!(
                "completed slot {completed_slot_id} is not in the schedule"
            ))
        })?;
    let segment_ended_at = slot_end(completed_slot)?;
    let not_before = segment_ended_at
        .checked_add_signed(Duration::seconds(WSPR_LIVE_INGESTION_GRACE_SECONDS))
        .ok_or_else(|| {
            WsprLiveImportError::Config("WSPR.live grace deadline exceeds UTC range".into())
        })?;
    let mut window_start = wspr_live_query_window_start(completed_slot.starts_at);
    let mut selected_bands = Vec::new();
    for slot in slots {
        if slot_end(slot)? <= segment_ended_at {
            window_start = window_start.min(wspr_live_query_window_start(slot.starts_at));
            if !selected_bands.contains(&slot.band) {
                selected_bands.push(slot.band);
            }
        }
    }
    let scope = WsprLiveQueryScope {
        session_callsign: session_callsign.into(),
        window_start,
        window_end: segment_ended_at,
        selected_bands,
    };

    Ok(WsprLiveAcquisitionPlan {
        completed_slot_id: completed_slot_id.into(),
        segment_ended_at,
        not_before,
        query: plan_wspr_live_query(&scope)?,
    })
}

pub fn plan_wspr_live_acquisitions_for_confirmed_slots(
    session_callsign: &str,
    slots: &[PlannedSlot],
    confirmed_slot_ids: &BTreeSet<String>,
) -> Result<Vec<WsprLiveAcquisitionPlan>, WsprLiveImportError> {
    if slots.is_empty() {
        return Err(WsprLiveImportError::Config(
            "schedule must contain at least one slot".into(),
        ));
    }
    let mut authorized_completed_slots = BTreeSet::<usize>::new();
    for (index, slot) in slots.iter().enumerate() {
        if !confirmed_slot_ids.contains(&slot.slot_id) {
            continue;
        }
        if index > 0 {
            authorized_completed_slots.insert(index - 1);
        }
        if index + 1 == slots.len() {
            authorized_completed_slots.insert(index);
        }
    }

    authorized_completed_slots
        .into_iter()
        .map(|index| {
            plan_wspr_live_acquisition_for_completed_slot(
                session_callsign,
                slots,
                &slots[index].slot_id,
            )
        })
        .collect()
}

pub fn latest_due_wspr_live_acquisition(
    plans: &[WsprLiveAcquisitionPlan],
    now: DateTime<Utc>,
    last_request_started_at: Option<DateTime<Utc>>,
) -> Option<&WsprLiveAcquisitionPlan> {
    let interval_allows_request = last_request_started_at.is_none_or(|last_request| {
        last_request
            .checked_add_signed(Duration::seconds(WSPR_LIVE_MIN_REQUEST_INTERVAL_SECONDS))
            .is_some_and(|next_request| now >= next_request)
    });
    interval_allows_request.then(|| {
        plans
            .iter()
            .filter(|plan| plan.not_before <= now)
            .max_by_key(|plan| (plan.segment_ended_at, plan.completed_slot_id.as_str()))
    })?
}

fn slot_end(slot: &PlannedSlot) -> Result<DateTime<Utc>, WsprLiveImportError> {
    slot.starts_at
        .checked_add_signed(Duration::seconds(i64::from(slot.duration_seconds)))
        .ok_or_else(|| WsprLiveImportError::Config("scheduled slot end exceeds UTC range".into()))
}

pub fn parse_wspr_live_json(
    input: &[u8],
    config: &WsprLiveImportConfig,
    cancellation: &AdapterCancellationToken,
) -> Result<ParsedWsprLiveJson, WsprLiveImportError> {
    parse_wspr_live_json_with_limits(input, config, cancellation, WSPR_LIVE_IMPORT_LIMITS)
}

pub fn parse_wspr_live_json_with_limits(
    input: &[u8],
    config: &WsprLiveImportConfig,
    cancellation: &AdapterCancellationToken,
    limits: WsprLiveImportLimits,
) -> Result<ParsedWsprLiveJson, WsprLiveImportError> {
    validate_config(config)?;
    check_cancelled(cancellation, input.len() as u64, limits.source_bytes)?;
    if input.len() as u64 > limits.source_bytes {
        return Err(resource_error(
            "resource.adapter.source_bytes",
            config,
            AdapterResourceStage::Admission,
            limits.source_bytes,
            Some(input.len() as u64),
            AdapterResourceUnit::Bytes,
        )
        .into());
    }
    let document: Value = serde_json::from_slice(input)
        .map_err(|error| WsprLiveImportError::Json(error.to_string()))?;
    let object = document
        .as_object()
        .ok_or_else(|| WsprLiveImportError::Schema("top level must be an object".into()))?;
    validate_columns(object.get("meta"))?;
    let data = object
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| WsprLiveImportError::Schema("data must be an array".into()))?;
    if data.len() as u64 > limits.rows {
        return Err(resource_error(
            "resource.adapter.rows",
            config,
            AdapterResourceStage::Admission,
            limits.rows,
            Some(data.len() as u64),
            AdapterResourceUnit::Entries,
        )
        .into());
    }
    if let Some(rows) = object.get("rows").and_then(Value::as_u64) {
        if rows != data.len() as u64 {
            return Err(WsprLiveImportError::Schema(format!(
                "rows declares {rows}, but data contains {} entries",
                data.len()
            )));
        }
    }

    let callsign = normalize_wspr_callsign(config.session_callsign.trim()).expect("validated");
    let mut rows = Vec::with_capacity(data.len());
    let mut summary = WsprLiveDispositionSummary::default();
    for (index, raw) in data.iter().enumerate() {
        check_cancelled(cancellation, index as u64, limits.rows)?;
        let row_bytes = serde_json::to_vec(raw)
            .map_err(|error| WsprLiveImportError::Json(error.to_string()))?
            .len() as u64;
        if row_bytes > limits.row_bytes {
            return Err(resource_error(
                "resource.adapter.row_bytes",
                config,
                AdapterResourceStage::Stream,
                limits.row_bytes,
                Some(row_bytes),
                AdapterResourceUnit::Bytes,
            )
            .into());
        }
        let row = classify_row(
            index + 1,
            raw.clone(),
            &callsign,
            config.window_start,
            config.window_end,
            &config.selected_bands,
            config.confirmed_cycles.as_deref(),
        );
        summary.total += 1;
        match row.disposition {
            WsprLiveRowDisposition::Accepted => summary.accepted += 1,
            WsprLiveRowDisposition::Malformed => summary.malformed += 1,
            WsprLiveRowDisposition::Filtered => summary.filtered += 1,
            WsprLiveRowDisposition::Unsupported => summary.unsupported += 1,
        }
        rows.push(row);
    }
    Ok(ParsedWsprLiveJson { rows, summary })
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_wspr_live_import(
    parsed: &ParsedWsprLiveJson,
    config: &WsprLiveImportConfig,
    session_id: &str,
    import_id: &str,
    exact_response: AttachmentReference,
    existing: &[AdapterRecordV2],
) -> PreparedWsprLiveImport {
    prepare_wspr_live_acquisition(
        parsed,
        config,
        session_id,
        import_id,
        exact_response,
        existing,
        WsprLiveAcquisitionChannel::FileImport,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_wspr_live_acquisition(
    parsed: &ParsedWsprLiveJson,
    config: &WsprLiveImportConfig,
    session_id: &str,
    import_id: &str,
    exact_response: AttachmentReference,
    existing: &[AdapterRecordV2],
    channel: WsprLiveAcquisitionChannel,
) -> PreparedWsprLiveImport {
    let mutation_id = format!("wspr-live-import-{import_id}");
    let provenance = wspr_live_provenance(channel);
    let mut replay = existing_wspr_live_spots(existing, config);
    let mut adapter_records = Vec::with_capacity(parsed.rows.len() + 2);
    let mut observations = Vec::with_capacity(parsed.summary.accepted);
    let mut summary = WsprLiveImportSummary {
        total: parsed.summary.total,
        malformed: parsed.summary.malformed,
        filtered: parsed.summary.filtered,
        unsupported: parsed.summary.unsupported,
        evidence_completeness_known: false,
        ..WsprLiveImportSummary::default()
    };

    adapter_records.push(AdapterRecordV2 {
        meta: import_meta(session_id, &mutation_id, 0, config.captured_at, &provenance),
        record_id: format!("wspr-live-capture-{import_id}"),
        source_time: Some(config.captured_at),
        record_type: "wspr_live_import_capture".into(),
        disposition: AdapterDisposition::Accepted,
        reason: adapter_reason("wspr-live.capture"),
        normalized_records: Vec::new(),
        input: AdapterInput::Attachment {
            attachment: exact_response.clone(),
        },
    });
    adapter_records.push(AdapterRecordV2 {
        meta: import_meta(session_id, &mutation_id, 1, config.captured_at, &provenance),
        record_id: format!("wspr-live-summary-{import_id}"),
        source_time: Some(config.captured_at),
        record_type: "wspr_live_import_summary".into(),
        disposition: AdapterDisposition::Accepted,
        reason: adapter_reason("wspr-live.import-summary"),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: String::new(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: config.source_locator.clone(),
        },
    });

    for row in &parsed.rows {
        let fingerprint = row
            .spot
            .as_ref()
            .map_or_else(|| row_fingerprint(&row.raw), spot_fingerprint);
        let replay_disposition = (row.disposition == WsprLiveRowDisposition::Accepted)
            .then_some(row.provider_spot_id)
            .flatten()
            .and_then(|provider_id| {
                replay.get(&provider_id).map(|prior| {
                    if prior == &fingerprint {
                        AdapterDisposition::Duplicate
                    } else {
                        AdapterDisposition::Conflict
                    }
                })
            });
        let disposition =
            replay_disposition.unwrap_or_else(|| adapter_disposition(row.disposition));
        let observation_id = row
            .provider_spot_id
            .map(|provider_id| format!("wspr-live-spot-{provider_id}"));
        let creates_observation = disposition == AdapterDisposition::Accepted && row.spot.is_some();
        let normalized_records = creates_observation
            .then(|| NormalizedRecordLink {
                record_kind: NormalizedRecordKind::Observation,
                record_id: observation_id.clone().expect("accepted spot identity"),
            })
            .into_iter()
            .collect();
        let adapter_id = format!("wspr-live-row-{import_id}-{}", row.row_number);
        let member_index =
            u32::try_from(adapter_records.len()).expect("bounded row count fits u32");
        adapter_records.push(AdapterRecordV2 {
            meta: import_meta(
                session_id,
                &mutation_id,
                member_index,
                row.spot
                    .as_ref()
                    .map_or(config.captured_at, |spot| spot.observed_at),
                &provenance,
            ),
            record_id: adapter_id.clone(),
            source_time: row.spot.as_ref().map(|spot| spot.observed_at),
            record_type: "wspr_live_spot".into(),
            disposition,
            reason: adapter_reason(disposition_reason(disposition, row.reason)),
            normalized_records,
            input: AdapterInput::Inline {
                data: serde_json::to_string(&row.raw).expect("JSON value serializes"),
                media_type: "application/json".into(),
                encoding: None,
                source_locator: config.source_locator.clone(),
            },
        });

        match disposition {
            AdapterDisposition::Accepted if creates_observation => {
                let spot = row.spot.as_ref().expect("accepted spot");
                replay.insert(spot.provider_spot_id, fingerprint);
                observations.push(wspr_live_observation(
                    spot,
                    session_id,
                    &mutation_id,
                    &adapter_id,
                    observation_id.expect("accepted spot identity"),
                    config.captured_at,
                    &provenance,
                ));
                summary.accepted += 1;
                summary.observations_created += 1;
            }
            AdapterDisposition::Duplicate => summary.duplicate += 1,
            AdapterDisposition::Conflict => summary.conflict += 1,
            _ => {}
        }
    }

    if let AdapterInput::Inline { data, .. } = &mut adapter_records[1].input {
        *data = serde_json::to_string(&serde_json::json!({
            "provider_id": WSPR_LIVE_PROVIDER_ID,
            "source_id": WSPR_LIVE_SOURCE_ID,
            "acquisition_channel": channel.id(),
            "session_callsign": config.session_callsign,
            "captured_at": config.captured_at,
            "window_start": config.window_start,
            "window_end": config.window_end,
            "selected_bands": config.selected_bands,
            "mode": "WSPR-2",
            "station_roles": ["tx_sign", "rx_sign"],
            "direction_filter": if config.confirmed_cycles.is_none() {
                "historical-direction-unknown"
            } else {
                "confirmed-cycle-at-source-time"
            },
            "expected_columns": WSPR_LIVE_COLUMNS,
            "exact_response": exact_response,
            "completeness": "unknown",
            "counts": {
                "total": summary.total,
                "accepted": summary.accepted,
                "malformed": summary.malformed,
                "filtered": summary.filtered,
                "unsupported": summary.unsupported,
                "duplicate": summary.duplicate,
                "conflict": summary.conflict,
                "observations_created": summary.observations_created,
            }
        }))
        .expect("WSPR.live import summary serializes");
    }

    let member_count = adapter_records.len() + observations.len();
    for (offset, observation) in observations.iter_mut().enumerate() {
        observation.meta.mutation.member_index =
            u32::try_from(adapter_records.len() + offset).expect("bounded import fits u32");
    }
    let member_count = u32::try_from(member_count).expect("bounded import fits u32");
    for record in &mut adapter_records {
        record.meta.mutation.member_count = member_count;
    }
    for observation in &mut observations {
        observation.meta.mutation.member_count = member_count;
    }

    PreparedWsprLiveImport {
        mutation_id,
        adapter_records,
        observations,
        summary,
    }
}

fn validate_config(config: &WsprLiveImportConfig) -> Result<(), WsprLiveImportError> {
    validate_query_scope(&WsprLiveQueryScope::from(config))?;
    if config
        .source_locator
        .as_ref()
        .is_some_and(|locator| locator.len() > 2048)
    {
        return Err(WsprLiveImportError::Config(
            "source_locator exceeds 2048 bytes".into(),
        ));
    }
    Ok(())
}

fn validate_query_scope(scope: &WsprLiveQueryScope) -> Result<(), WsprLiveImportError> {
    if normalize_wspr_callsign(scope.session_callsign.trim()).is_none() {
        return Err(WsprLiveImportError::Config(
            "session_callsign must be a normalized WSPR callsign".into(),
        ));
    }
    if scope.window_start >= scope.window_end {
        return Err(WsprLiveImportError::Config(
            "window_start must precede window_end".into(),
        ));
    }
    if scope.selected_bands.is_empty() {
        return Err(WsprLiveImportError::Config(
            "selected_bands must not be empty".into(),
        ));
    }
    Ok(())
}

fn wspr_live_provenance(channel: WsprLiveAcquisitionChannel) -> Provenance {
    Provenance {
        provider_id: ProviderId::new(WSPR_LIVE_PROVIDER_ID).expect("static provider identity"),
        source_id: SourceId::new(WSPR_LIVE_SOURCE_ID).expect("static source identity"),
        acquisition_channel: AcquisitionChannelId::new(channel.id())
            .expect("static acquisition identity"),
        adapter_id: AdapterId::new(WSPR_LIVE_ADAPTER_ID).expect("static adapter identity"),
        adapter_version: WSPR_LIVE_ADAPTER_VERSION.into(),
    }
}

fn import_meta(
    session_id: &str,
    mutation_id: &str,
    member_index: u32,
    recorded_at: DateTime<Utc>,
    provenance: &Provenance,
) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V2,
        session_id: session_id.into(),
        recorded_at,
        provenance: provenance.clone(),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count: 0,
        },
    }
}

fn existing_wspr_live_spots(
    existing: &[AdapterRecordV2],
    config: &WsprLiveImportConfig,
) -> BTreeMap<u64, String> {
    let callsign =
        normalize_wspr_callsign(config.session_callsign.trim()).expect("validated config");
    existing
        .iter()
        .filter(|record| {
            record.meta.provenance.adapter_id.as_str() == WSPR_LIVE_ADAPTER_ID
                && record.record_type == "wspr_live_spot"
                && record.disposition == AdapterDisposition::Accepted
        })
        .filter_map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                return None;
            };
            let raw: Value = serde_json::from_str(data).ok()?;
            let row = classify_row(
                0,
                raw,
                &callsign,
                config.window_start,
                config.window_end,
                &config.selected_bands,
                config.confirmed_cycles.as_deref(),
            );
            let spot = row.spot?;
            Some((spot.provider_spot_id, spot_fingerprint(&spot)))
        })
        .collect()
}

fn spot_fingerprint(spot: &WsprLiveSpot) -> String {
    row_fingerprint(&serde_json::json!({
        "provider_spot_id": spot.provider_spot_id,
        "observed_at": spot.observed_at,
        "band": spot.band,
        "reporter_call": spot.reporter_call,
        "reporter_grid": spot.reporter_grid,
        "transmitter_call": spot.transmitter_call,
        "transmitter_grid": spot.transmitter_grid,
        "distance_km": spot.distance_km,
        "azimuth_degrees": spot.azimuth_degrees,
        "receiver_azimuth_degrees": spot.receiver_azimuth_degrees,
        "frequency_hz": spot.frequency_hz,
        "power_dbm": spot.power_dbm,
        "snr_db": spot.snr_db,
        "drift_hz_per_minute": spot.drift_hz_per_minute,
        "receiver_version": spot.receiver_version,
        "mode_code": spot.mode_code,
        "direction": match spot.direction {
            WsprLiveSpotDirection::Receive => "receive",
            WsprLiveSpotDirection::Transmit => "transmit",
        },
    }))
}

fn row_fingerprint(raw: &Value) -> String {
    let bytes = serde_json::to_vec(raw).expect("JSON value serializes");
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn adapter_disposition(disposition: WsprLiveRowDisposition) -> AdapterDisposition {
    match disposition {
        WsprLiveRowDisposition::Accepted => AdapterDisposition::Accepted,
        WsprLiveRowDisposition::Malformed => AdapterDisposition::Malformed,
        WsprLiveRowDisposition::Filtered => AdapterDisposition::Filtered,
        WsprLiveRowDisposition::Unsupported => AdapterDisposition::Unsupported,
    }
}

fn disposition_reason(
    disposition: AdapterDisposition,
    row_reason: WsprLiveRowReason,
) -> &'static str {
    match disposition {
        AdapterDisposition::Duplicate => "wspr-live.provider-id-duplicate",
        AdapterDisposition::Conflict => "wspr-live.provider-id-conflict",
        _ => match row_reason {
            WsprLiveRowReason::Accepted => "wspr-live.accepted",
            WsprLiveRowReason::InvalidValue => "wspr-live.invalid-value",
            WsprLiveRowReason::CallsignFiltered => "wspr-live.callsign-filtered",
            WsprLiveRowReason::AmbiguousCallsign => "wspr-live.ambiguous-callsign",
            WsprLiveRowReason::DirectionFiltered => "wspr-live.direction-filtered",
            WsprLiveRowReason::TimeFiltered => "wspr-live.time-filtered",
            WsprLiveRowReason::BandFiltered => "wspr-live.band-filtered",
            WsprLiveRowReason::UnsupportedBand => "wspr-live.unsupported-band",
            WsprLiveRowReason::UnsupportedMode => "wspr-live.unsupported-mode",
        },
    }
}

fn adapter_reason(value: &str) -> AdapterReasonId {
    AdapterReasonId::new(value).expect("static WSPR.live reason identity")
}

#[allow(clippy::too_many_arguments)]
fn wspr_live_observation(
    spot: &WsprLiveSpot,
    session_id: &str,
    mutation_id: &str,
    adapter_id: &str,
    observation_id: String,
    captured_at: DateTime<Utc>,
    provenance: &Provenance,
) -> ObservationRecordV2 {
    ObservationRecordV2 {
        meta: import_meta(session_id, mutation_id, 0, captured_at, provenance),
        observation_id,
        adapter_record_ids: vec![adapter_id.into()],
        observation_kind: ObservationKind::ImportedSpot,
        band: spot.band,
        frequency_hz: Some(spot.frequency_hz),
        mode: Some("WSPR".into()),
        reporter_call: Some(spot.reporter_call.clone()),
        heard_call: Some(spot.transmitter_call.clone()),
        reporter_grid: spot.reporter_grid.clone(),
        heard_grid: spot.transmitter_grid.clone(),
        distance_km: spot.distance_km,
        azimuth_degrees: match spot.direction {
            WsprLiveSpotDirection::Receive => spot.receiver_azimuth_degrees,
            WsprLiveSpotDirection::Transmit => spot.azimuth_degrees,
        },
        snr_db: Some(spot.snr_db),
        drift_hz_per_minute: Some(spot.drift_hz_per_minute),
        power_watts: Some(crate::dbm_to_watts(spot.power_dbm)),
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: serde_json::json!({
            "provider_spot_id": spot.provider_spot_id,
            "provider": WSPR_LIVE_PROVIDER_ID,
            "source": WSPR_LIVE_SOURCE_ID,
            "tx_azimuth_degrees": spot.azimuth_degrees,
            "rx_azimuth_degrees": spot.receiver_azimuth_degrees,
            "receiver_version": spot.receiver_version,
            "mode_code": spot.mode_code,
            "direction": match spot.direction {
                WsprLiveSpotDirection::Receive => "receive",
                WsprLiveSpotDirection::Transmit => "transmit",
            },
        }),
    }
}

fn validate_columns(meta: Option<&Value>) -> Result<(), WsprLiveImportError> {
    let meta = meta
        .and_then(Value::as_array)
        .ok_or_else(|| WsprLiveImportError::Schema("meta must be an array".into()))?;
    let mut names = Vec::with_capacity(meta.len());
    let mut unique = BTreeSet::new();
    for column in meta {
        let name = column
            .as_object()
            .and_then(|column| column.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| WsprLiveImportError::Schema("meta entries require name".into()))?;
        if !unique.insert(name) {
            return Err(WsprLiveImportError::Schema(format!(
                "duplicate meta column {name}"
            )));
        }
        names.push(name);
    }
    if names != WSPR_LIVE_COLUMNS {
        return Err(WsprLiveImportError::Schema(format!(
            "expected projection {}, received {}",
            WSPR_LIVE_COLUMNS.join(","),
            names.join(",")
        )));
    }
    Ok(())
}

fn classify_row(
    row_number: usize,
    raw: Value,
    session_callsign: &str,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    selected_bands: &[Band],
    confirmed_cycles: Option<&[WsprLiveConfirmedCycle]>,
) -> WsprLiveRowResult {
    let Some(row) = raw.as_object() else {
        return row_result(
            row_number,
            None,
            raw,
            WsprLiveRowDisposition::Malformed,
            WsprLiveRowReason::InvalidValue,
            None,
        );
    };
    let provider_spot_id = row.get("id").and_then(unsigned);
    let observed_at = row.get("time").and_then(timestamp);
    let band_number = row.get("band").and_then(integer);
    let band = band_number.and_then(band_from_wspr_live);
    let reporter_call = row
        .get("rx_sign")
        .and_then(Value::as_str)
        .and_then(normalize_wspr_callsign);
    let transmitter_call = row
        .get("tx_sign")
        .and_then(Value::as_str)
        .and_then(normalize_wspr_callsign);
    let reporter_grid = optional_grid(row.get("rx_loc"));
    let transmitter_grid = optional_grid(row.get("tx_loc"));
    let distance_km = optional_number(row.get("distance"), 0.0, 40_100.0);
    let azimuth_degrees = optional_number(row.get("azimuth"), 0.0, 360.0);
    let receiver_azimuth_degrees = optional_number(row.get("rx_azimuth"), 0.0, 360.0);
    let frequency_hz = row.get("frequency").and_then(unsigned);
    let power_dbm = row
        .get("power")
        .and_then(integer)
        .and_then(|value| i16::try_from(value).ok());
    let snr_db = row.get("snr").and_then(number).map(|value| value as f32);
    let drift = row.get("drift").and_then(number).map(|value| value as f32);
    let receiver_version = optional_string(row.get("version"));
    let mode_code = row
        .get("code")
        .and_then(integer)
        .and_then(|value| i16::try_from(value).ok());

    let required_valid = provider_spot_id.is_some()
        && observed_at.is_some()
        && band_number.is_some()
        && reporter_call.is_some()
        && transmitter_call.is_some()
        && reporter_grid.is_some()
        && transmitter_grid.is_some()
        && distance_km.is_some()
        && azimuth_degrees.is_some()
        && receiver_azimuth_degrees.is_some()
        && frequency_hz.is_some()
        && power_dbm.is_some_and(|value| (-100..=100).contains(&value))
        && snr_db.is_some_and(|value| value.is_finite() && (-100.0..=100.0).contains(&value))
        && drift.is_some_and(|value| value.is_finite() && (-1000.0..=1000.0).contains(&value))
        && receiver_version.is_some()
        && mode_code.is_some();
    if !required_valid {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Malformed,
            WsprLiveRowReason::InvalidValue,
            None,
        );
    }
    let Some(band) = band else {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Unsupported,
            WsprLiveRowReason::UnsupportedBand,
            None,
        );
    };
    if mode_code != Some(WSPR_LIVE_WSPR2_CODE) {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Unsupported,
            WsprLiveRowReason::UnsupportedMode,
            None,
        );
    }
    let observed_at = observed_at.unwrap();
    let reporter_call = reporter_call.unwrap();
    let transmitter_call = transmitter_call.unwrap();
    let direction = match (
        transmitter_call == session_callsign,
        reporter_call == session_callsign,
    ) {
        (true, false) => WsprLiveSpotDirection::Transmit,
        (false, true) => WsprLiveSpotDirection::Receive,
        (true, true) => {
            return row_result(
                row_number,
                provider_spot_id,
                raw,
                WsprLiveRowDisposition::Filtered,
                WsprLiveRowReason::AmbiguousCallsign,
                None,
            );
        }
        (false, false) => {
            return row_result(
                row_number,
                provider_spot_id,
                raw,
                WsprLiveRowDisposition::Filtered,
                WsprLiveRowReason::CallsignFiltered,
                None,
            );
        }
    };
    if observed_at < window_start || observed_at >= window_end {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Filtered,
            WsprLiveRowReason::TimeFiltered,
            None,
        );
    }
    if !selected_bands.contains(&band) {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Filtered,
            WsprLiveRowReason::BandFiltered,
            None,
        );
    }
    let frequency_hz = frequency_hz.unwrap();
    if crate::band_from_frequency_hz(frequency_hz) != Some(band) {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Malformed,
            WsprLiveRowReason::InvalidValue,
            None,
        );
    }
    let spot = WsprLiveSpot {
        provider_spot_id: provider_spot_id.unwrap(),
        observed_at,
        band,
        reporter_call,
        reporter_grid: reporter_grid.unwrap(),
        transmitter_call,
        transmitter_grid: transmitter_grid.unwrap(),
        distance_km: distance_km.unwrap(),
        azimuth_degrees: azimuth_degrees.unwrap(),
        receiver_azimuth_degrees: receiver_azimuth_degrees.unwrap(),
        frequency_hz,
        power_dbm: power_dbm.unwrap(),
        snr_db: snr_db.unwrap(),
        drift_hz_per_minute: drift.unwrap(),
        receiver_version: receiver_version.unwrap(),
        mode_code: mode_code.unwrap(),
        direction,
    };
    let expected_direction = match direction {
        WsprLiveSpotDirection::Receive => WsprCycleDirection::Receive,
        WsprLiveSpotDirection::Transmit => WsprCycleDirection::Transmit,
    };
    if confirmed_cycles.is_some_and(|cycles| {
        matching_confirmed_cycle(cycles, observed_at, band, expected_direction).is_none()
    }) {
        return row_result(
            row_number,
            provider_spot_id,
            raw,
            WsprLiveRowDisposition::Filtered,
            WsprLiveRowReason::DirectionFiltered,
            Some(spot),
        );
    }
    row_result(
        row_number,
        provider_spot_id,
        raw,
        WsprLiveRowDisposition::Accepted,
        WsprLiveRowReason::Accepted,
        Some(spot),
    )
}

fn row_result(
    row_number: usize,
    provider_spot_id: Option<u64>,
    raw: Value,
    disposition: WsprLiveRowDisposition,
    reason: WsprLiveRowReason,
    spot: Option<WsprLiveSpot>,
) -> WsprLiveRowResult {
    WsprLiveRowResult {
        row_number,
        provider_spot_id,
        raw,
        disposition,
        reason,
        spot,
    }
}

fn unsigned(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

fn integer(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str()?.parse().ok())
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str()?.parse().ok())
}

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let value = value.as_str()?;
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| value.and_utc())
        })
}

fn band_from_wspr_live(value: i64) -> Option<Band> {
    Some(match value {
        1 => Band::M160,
        3 => Band::M80,
        5 => Band::M60,
        7 => Band::M40,
        10 => Band::M30,
        14 => Band::M20,
        18 => Band::M17,
        21 => Band::M15,
        24 => Band::M12,
        28 => Band::M10,
        50 => Band::M6,
        144 => Band::M2,
        _ => return None,
    })
}

fn band_to_wspr_live(band: Band) -> i16 {
    match band {
        Band::M160 => 1,
        Band::M80 => 3,
        Band::M60 => 5,
        Band::M40 => 7,
        Band::M30 => 10,
        Band::M20 => 14,
        Band::M17 => 18,
        Band::M15 => 21,
        Band::M12 => 24,
        Band::M10 => 28,
        Band::M6 => 50,
        Band::M2 => 144,
    }
}

fn percent_encode_query(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[(byte >> 4) as usize]));
            encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    encoded
}

fn optional_grid(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        Some(Value::Null) => Some(None),
        Some(Value::String(value)) if value.trim().is_empty() => Some(None),
        Some(Value::String(value)) => normalize_maidenhead_grid(value.trim()).map(Some),
        _ => None,
    }
}

fn optional_number(value: Option<&Value>, minimum: f64, maximum: f64) -> Option<Option<f64>> {
    match value {
        Some(Value::Null) => Some(None),
        Some(value) => number(value)
            .filter(|value| value.is_finite() && (*value >= minimum && *value <= maximum))
            .map(Some),
        None => None,
    }
}

fn optional_string(value: Option<&Value>) -> Option<Option<String>> {
    match value {
        Some(Value::Null) => Some(None),
        Some(Value::String(value)) if value.len() <= 256 => {
            Some((!value.is_empty()).then(|| value.clone()))
        }
        _ => None,
    }
}

fn check_cancelled(
    cancellation: &AdapterCancellationToken,
    observed: u64,
    limit: u64,
) -> Result<(), WsprLiveImportError> {
    if cancellation.is_cancelled() {
        Err(diagnostic(
            "resource.operation.cancelled",
            WSPR_LIVE_ADAPTER_ID,
            "wspr-live-json",
            AdapterResourceStage::Stream,
            limit,
            Some(observed),
            AdapterResourceUnit::Checkpoints,
            false,
        )
        .into())
    } else {
        Ok(())
    }
}

fn resource_error(
    code: &'static str,
    config: &WsprLiveImportConfig,
    stage: AdapterResourceStage,
    limit: u64,
    observed: Option<u64>,
    unit: AdapterResourceUnit,
) -> AdapterResourceError {
    diagnostic(
        code,
        WSPR_LIVE_ADAPTER_ID,
        config.source_locator.as_deref().unwrap_or("wspr-live-json"),
        stage,
        limit,
        observed,
        unit,
        false,
    )
}
