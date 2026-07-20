use std::collections::BTreeSet;

use antennabench_core::v2::{
    AdapterDisposition, AdapterInput, AdapterRecordV2, AttachmentReference,
};
use antennabench_core::Band;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

use crate::{
    adapter_reason, normalize_maidenhead_grid, percent_encode_query,
    wspr_live_observation::import_meta, wspr_live_provenance, WsprLiveAcquisitionChannel,
    WsprLiveImportConfig, WsprLiveImportError, WsprLiveQueryPlan, WSPR_LIVE_PROVIDER_ID,
    WSPR_LIVE_QUERY_ENDPOINT, WSPR_LIVE_SOURCE_ID, WSPR_LIVE_WSPR2_CODE,
};

pub const WSPR_LIVE_ACTIVITY_RECORD_TYPE: &str = "wspr_live_activity_census";
pub const WSPR_LIVE_ACTIVITY_CAPTURE_RECORD_TYPE: &str = "wspr_live_activity_census_capture";
pub const WSPR_LIVE_ACTIVITY_SUMMARY_RECORD_TYPE: &str = "wspr_live_activity_census_summary";
/// Eight typical cycles at the measured worst case are about 4,000 rows.
/// Keeping 10,000 rows leaves headroom for multi-band sessions while bounding
/// every immutable mutation. The query requests one extra row to durably
/// distinguish an exact-bound result from truncation.
pub const WSPR_LIVE_ACTIVITY_ROW_LIMIT: usize = 10_000;
pub const WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT: usize = WSPR_LIVE_ACTIVITY_ROW_LIMIT + 1;

pub const WSPR_LIVE_ACTIVITY_COLUMNS: [&str; 8] = [
    "time",
    "band",
    "rx_sign",
    "rx_loc",
    "spots_decoded",
    "stations_heard",
    "max_snr",
    "median_snr",
];

impl WsprLiveQueryPlan {
    pub fn activity_census_sql(&self) -> String {
        let bands = self
            .provider_bands
            .iter()
            .map(i16::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let window_start = self.window_start.format("%Y-%m-%d %H:%M:%S");
        let window_end = self.window_end.format("%Y-%m-%d %H:%M:%S");

        format!(
            "SELECT time, band, rx_sign, any(rx_loc) AS rx_loc, count() AS spots_decoded, uniqExact(tx_sign) AS stations_heard, max(snr) AS max_snr, median(snr) AS median_snr FROM wspr.rx WHERE band IN ({bands}) AND code = {} AND time >= toDateTime('{}', 'UTC') AND time < toDateTime('{}', 'UTC') GROUP BY time, band, rx_sign ORDER BY time, band, rx_sign LIMIT {} FORMAT JSON",
            WSPR_LIVE_WSPR2_CODE,
            window_start,
            window_end,
            WSPR_LIVE_ACTIVITY_QUERY_ROW_LIMIT,
        )
    }

    pub fn activity_census_query_url(&self) -> String {
        format!(
            "{WSPR_LIVE_QUERY_ENDPOINT}?query={}",
            percent_encode_query(&self.activity_census_sql())
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsprLiveActivityRow {
    pub cycle_time: DateTime<Utc>,
    pub band: Band,
    pub reporter: String,
    pub reporter_grid: Option<String>,
    pub spots_decoded: u64,
    pub stations_heard: u64,
    pub max_snr: f32,
    pub median_snr: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedWsprLiveActivity {
    pub rows: Vec<WsprLiveActivityRow>,
    pub source_rows: usize,
    pub malformed_rows: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WsprLiveActivitySummary {
    pub source_rows: usize,
    pub accepted: usize,
    pub duplicate: usize,
    pub malformed: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedWsprLiveActivity {
    pub mutation_id: String,
    pub adapter_records: Vec<AdapterRecordV2>,
    pub summary: WsprLiveActivitySummary,
}

pub fn parse_wspr_live_activity_json(
    input: &[u8],
    config: &WsprLiveImportConfig,
) -> Result<ParsedWsprLiveActivity, WsprLiveImportError> {
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
    if let Some(rows) = object.get("rows").and_then(Value::as_u64) {
        if rows != data.len() as u64 {
            return Err(WsprLiveImportError::Schema(format!(
                "rows declares {rows}, but data contains {} entries",
                data.len()
            )));
        }
    }

    let truncated = data.len() > WSPR_LIVE_ACTIVITY_ROW_LIMIT;
    let mut rows = Vec::with_capacity(data.len().min(WSPR_LIVE_ACTIVITY_ROW_LIMIT));
    let mut malformed_rows = 0;
    for raw in data.iter().take(WSPR_LIVE_ACTIVITY_ROW_LIMIT) {
        if let Some(row) = parse_row(raw, config) {
            rows.push(row);
        } else {
            malformed_rows += 1;
        }
    }
    Ok(ParsedWsprLiveActivity {
        rows,
        source_rows: data.len(),
        malformed_rows,
        truncated,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_wspr_live_activity(
    parsed: &ParsedWsprLiveActivity,
    config: &WsprLiveImportConfig,
    session_id: &str,
    capture_id: &str,
    exact_response: AttachmentReference,
    existing: &[AdapterRecordV2],
) -> PreparedWsprLiveActivity {
    let mutation_id = format!("wspr-live-activity-{capture_id}");
    let provenance = wspr_live_provenance(WsprLiveAcquisitionChannel::HttpsQuery);
    let mut known = existing_activity_keys(existing);
    let mut records = Vec::with_capacity(parsed.rows.len() + 2);
    let mut summary = WsprLiveActivitySummary {
        source_rows: parsed.source_rows,
        malformed: parsed.malformed_rows,
        truncated: parsed.truncated,
        ..WsprLiveActivitySummary::default()
    };

    records.push(AdapterRecordV2 {
        meta: import_meta(session_id, &mutation_id, 0, config.captured_at, &provenance),
        record_id: format!("wspr-live-activity-capture-{capture_id}"),
        source_time: Some(config.captured_at),
        record_type: WSPR_LIVE_ACTIVITY_CAPTURE_RECORD_TYPE.into(),
        disposition: AdapterDisposition::Accepted,
        reason: adapter_reason("wspr-live.activity-census-capture"),
        normalized_records: Vec::new(),
        input: AdapterInput::Attachment {
            attachment: exact_response.clone(),
        },
    });
    records.push(AdapterRecordV2 {
        meta: import_meta(session_id, &mutation_id, 1, config.captured_at, &provenance),
        record_id: format!("wspr-live-activity-summary-{capture_id}"),
        source_time: Some(config.captured_at),
        record_type: WSPR_LIVE_ACTIVITY_SUMMARY_RECORD_TYPE.into(),
        disposition: if parsed.truncated {
            AdapterDisposition::PartiallyNormalized
        } else {
            AdapterDisposition::Accepted
        },
        reason: adapter_reason(if parsed.truncated {
            "wspr-live.activity-census-truncated"
        } else {
            "wspr-live.activity-census-summary"
        }),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: String::new(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: config.source_locator.clone(),
        },
    });

    for row in &parsed.rows {
        let key = activity_key(row.cycle_time, row.band, row.reporter.clone());
        if !known.insert(key) {
            summary.duplicate += 1;
            continue;
        }
        summary.accepted += 1;
        let member_index = u32::try_from(records.len()).expect("activity row bound fits u32");
        records.push(AdapterRecordV2 {
            meta: import_meta(
                session_id,
                &mutation_id,
                member_index,
                config.captured_at,
                &provenance,
            ),
            record_id: format!("wspr-live-activity-{capture_id}-{}", summary.accepted),
            source_time: Some(row.cycle_time),
            record_type: WSPR_LIVE_ACTIVITY_RECORD_TYPE.into(),
            disposition: AdapterDisposition::Accepted,
            reason: adapter_reason("wspr-live.activity-census"),
            normalized_records: Vec::new(),
            input: AdapterInput::Inline {
                data: serde_json::to_string(&serde_json::json!({
                    "cycle_time": row.cycle_time,
                    "band": row.band,
                    "reporter": row.reporter,
                    "reporter_grid": row.reporter_grid,
                    "spots_decoded": row.spots_decoded,
                    "stations_heard": row.stations_heard,
                    "max_snr": row.max_snr,
                    "median_snr": row.median_snr,
                }))
                .expect("activity census row serializes"),
                media_type: "application/json".into(),
                encoding: None,
                source_locator: config.source_locator.clone(),
            },
        });
    }

    if let AdapterInput::Inline { data, .. } = &mut records[1].input {
        *data = serde_json::to_string(&serde_json::json!({
            "provider_id": WSPR_LIVE_PROVIDER_ID,
            "source_id": WSPR_LIVE_SOURCE_ID,
            "acquisition_channel": "https-query",
            "captured_at": config.captured_at,
            "window_start": config.window_start,
            "window_end": config.window_end,
            "selected_bands": config.selected_bands,
            "record_type": WSPR_LIVE_ACTIVITY_RECORD_TYPE,
            "row_limit": WSPR_LIVE_ACTIVITY_ROW_LIMIT,
            "truncated": summary.truncated,
            "exact_response": exact_response,
            "counts": {
                "source_rows": summary.source_rows,
                "accepted": summary.accepted,
                "duplicate": summary.duplicate,
                "malformed": summary.malformed,
            }
        }))
        .expect("activity census summary serializes");
    }
    let member_count = u32::try_from(records.len()).expect("activity row bound fits u32");
    for record in &mut records {
        record.meta.mutation.member_count = member_count;
    }
    PreparedWsprLiveActivity {
        mutation_id,
        adapter_records: records,
        summary,
    }
}

pub fn prepare_wspr_live_activity_failure(
    config: &WsprLiveImportConfig,
    session_id: &str,
    capture_id: &str,
    reason_code: &str,
    detail: &str,
) -> PreparedWsprLiveActivity {
    let mutation_id = format!("wspr-live-activity-failure-{capture_id}");
    let provenance = wspr_live_provenance(WsprLiveAcquisitionChannel::HttpsQuery);
    let record = AdapterRecordV2 {
        meta: import_meta(session_id, &mutation_id, 0, config.captured_at, &provenance),
        record_id: format!("wspr-live-activity-failure-{capture_id}"),
        source_time: Some(config.captured_at),
        record_type: WSPR_LIVE_ACTIVITY_SUMMARY_RECORD_TYPE.into(),
        disposition: AdapterDisposition::Unsupported,
        reason: adapter_reason(reason_code),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: serde_json::to_string(&serde_json::json!({
                "provider_id": WSPR_LIVE_PROVIDER_ID,
                "source_id": WSPR_LIVE_SOURCE_ID,
                "acquisition_channel": "https-query",
                "captured_at": config.captured_at,
                "window_start": config.window_start,
                "window_end": config.window_end,
                "selected_bands": config.selected_bands,
                "record_type": WSPR_LIVE_ACTIVITY_RECORD_TYPE,
                "status": "failed",
                "reason": reason_code,
                "detail": detail,
                "row_limit": WSPR_LIVE_ACTIVITY_ROW_LIMIT,
                "truncated": false,
                "counts": { "source_rows": 0, "accepted": 0, "duplicate": 0, "malformed": 0 }
            }))
            .expect("activity census failure serializes"),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: config.source_locator.clone(),
        },
    };
    PreparedWsprLiveActivity {
        mutation_id,
        adapter_records: vec![record],
        summary: WsprLiveActivitySummary::default(),
    }
}

fn parse_row(raw: &Value, config: &WsprLiveImportConfig) -> Option<WsprLiveActivityRow> {
    let object = raw.as_object()?;
    let cycle_time = timestamp(object.get("time")?)?;
    if cycle_time < config.window_start || cycle_time >= config.window_end {
        return None;
    }
    let band = object
        .get("band")
        .and_then(integer)
        .and_then(crate::wspr_live::band_from_wspr_live)?;
    if !config.selected_bands.contains(&band) {
        return None;
    }
    let reporter = object
        .get("rx_sign")?
        .as_str()
        .and_then(crate::wspr_live_reporter::normalize_wspr_reporter_id)?;
    let reporter_grid = object
        .get("rx_loc")
        .and_then(Value::as_str)
        .and_then(normalize_maidenhead_grid);
    let spots_decoded = unsigned(object.get("spots_decoded")?)?;
    let stations_heard = unsigned(object.get("stations_heard")?)?;
    let max_snr = number(object.get("max_snr")?)? as f32;
    let median_snr = number(object.get("median_snr")?)? as f32;
    if spots_decoded == 0
        || stations_heard == 0
        || stations_heard > spots_decoded
        || !max_snr.is_finite()
        || !median_snr.is_finite()
        || !(-100.0..=100.0).contains(&max_snr)
        || !(-100.0..=100.0).contains(&median_snr)
    {
        return None;
    }
    Some(WsprLiveActivityRow {
        cycle_time,
        band,
        reporter,
        reporter_grid,
        spots_decoded,
        stations_heard,
        max_snr,
        median_snr,
    })
}

fn existing_activity_keys(
    existing: &[AdapterRecordV2],
) -> BTreeSet<(DateTime<Utc>, String, String)> {
    existing
        .iter()
        .filter(|record| {
            record.record_type == WSPR_LIVE_ACTIVITY_RECORD_TYPE
                && record.disposition == AdapterDisposition::Accepted
        })
        .filter_map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                return None;
            };
            let value: Value = serde_json::from_str(data).ok()?;
            Some(activity_key(
                value.get("cycle_time")?.as_str()?.parse().ok()?,
                serde_json::from_value(value.get("band")?.clone()).ok()?,
                value.get("reporter")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

fn activity_key(
    cycle_time: DateTime<Utc>,
    band: Band,
    reporter: String,
) -> (DateTime<Utc>, String, String) {
    (
        cycle_time,
        serde_json::to_string(&band).expect("band serializes"),
        reporter,
    )
}

fn validate_columns(meta: Option<&Value>) -> Result<(), WsprLiveImportError> {
    let columns = meta
        .and_then(Value::as_array)
        .ok_or_else(|| WsprLiveImportError::Schema("meta must be an array".into()))?;
    let names = columns
        .iter()
        .map(|column| column.get("name").and_then(Value::as_str))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| WsprLiveImportError::Schema("meta columns must have names".into()))?;
    if names != WSPR_LIVE_ACTIVITY_COLUMNS {
        return Err(WsprLiveImportError::Schema(format!(
            "expected activity census columns {:?}, got {names:?}",
            WSPR_LIVE_ACTIVITY_COLUMNS
        )));
    }
    Ok(())
}

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    let text = value.as_str()?;
    DateTime::parse_from_rfc3339(text)
        .map(|value| value.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| value.and_utc())
        })
}

fn unsigned(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}
