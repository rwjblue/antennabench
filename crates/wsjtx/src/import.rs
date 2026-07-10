use antennabench_core::{
    BundleContents, ObservationKind, ObservationRecord, RecordMeta, RecordSource, WsjtXRecord,
    SCHEMA_VERSION,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use thiserror::Error;

use crate::{parse_all_wspr_text, AllWsprDecode, AllWsprLineIssue, ParsedAllWsprText};

#[derive(Debug, Clone, PartialEq)]
pub struct WsjtxImportConfig {
    pub session_id: String,
    pub import_id: String,
    pub station_callsign: String,
    pub station_grid: String,
    pub imported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsjtxImport {
    pub wsjtx_records: Vec<WsjtXRecord>,
    pub observations: Vec<ObservationRecord>,
    pub issues: Vec<WsjtxImportIssue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WsjtxImportIssue {
    pub line_number: usize,
    pub raw_line: String,
    pub source: AllWsprLineIssue,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum WsjtxImportError {
    #[error("session_id must not be empty")]
    EmptySessionId,
    #[error("import_id must not be empty")]
    EmptyImportId,
    #[error("station_callsign must not be empty")]
    EmptyStationCallsign,
    #[error("station_grid must not be empty")]
    EmptyStationGrid,
}

pub fn import_all_wspr_text(
    input: &str,
    config: WsjtxImportConfig,
) -> Result<WsjtxImport, WsjtxImportError> {
    let parsed = parse_all_wspr_text(input);

    import_parsed_all_wspr_text(parsed, input, config)
}

pub fn import_parsed_all_wspr_text(
    parsed: ParsedAllWsprText,
    _original_input: &str,
    config: WsjtxImportConfig,
) -> Result<WsjtxImport, WsjtxImportError> {
    validate_config(&config)?;

    let reporter_call = config.station_callsign.to_ascii_uppercase();
    let reporter_grid = config.station_grid.to_ascii_uppercase();
    let mut wsjtx_records = Vec::new();
    let mut observations = Vec::new();
    let mut issues = Vec::new();
    let mut decodes = parsed.decodes.into_iter().peekable();
    let mut line_issues = parsed.issues.into_iter().peekable();

    while decodes.peek().is_some() || line_issues.peek().is_some() {
        let next_decode_line = decodes.peek().map(|decode| decode.line_number);
        let next_issue_line = line_issues.peek().map(|issue| issue.line_number);

        match (next_decode_line, next_issue_line) {
            (Some(decode_line), Some(issue_line)) if decode_line <= issue_line => {
                let decode = decodes.next().expect("peeked decode exists");
                wsjtx_records.push(wsjtx_decode_record(&decode, &config));
                observations.push(observation_record(
                    &decode,
                    &config,
                    &reporter_call,
                    &reporter_grid,
                ));
            }
            (Some(_), None) => {
                let decode = decodes.next().expect("peeked decode exists");
                wsjtx_records.push(wsjtx_decode_record(&decode, &config));
                observations.push(observation_record(
                    &decode,
                    &config,
                    &reporter_call,
                    &reporter_grid,
                ));
            }
            (_, Some(_)) => {
                let issue = line_issues.next().expect("peeked issue exists");
                wsjtx_records.push(wsjtx_malformed_record(&issue, &config));
                issues.push(WsjtxImportIssue {
                    line_number: issue.line_number,
                    raw_line: issue.raw_line.clone(),
                    source: issue,
                });
            }
            (None, None) => break,
        }
    }

    Ok(WsjtxImport {
        wsjtx_records,
        observations,
        issues,
    })
}

pub fn append_wsjtx_import(bundle: &mut BundleContents, import: WsjtxImport) {
    bundle.wsjtx.extend(import.wsjtx_records);
    bundle.observations.extend(import.observations);
}

fn validate_config(config: &WsjtxImportConfig) -> Result<(), WsjtxImportError> {
    if config.session_id.trim().is_empty() {
        return Err(WsjtxImportError::EmptySessionId);
    }
    if config.import_id.trim().is_empty() {
        return Err(WsjtxImportError::EmptyImportId);
    }
    if config.station_callsign.trim().is_empty() {
        return Err(WsjtxImportError::EmptyStationCallsign);
    }
    if config.station_grid.trim().is_empty() {
        return Err(WsjtxImportError::EmptyStationGrid);
    }

    Ok(())
}

fn wsjtx_decode_record(decode: &AllWsprDecode, config: &WsjtxImportConfig) -> WsjtXRecord {
    WsjtXRecord {
        meta: record_meta(config.session_id.clone(), decode.timestamp),
        record_id: wsjtx_record_id(&config.import_id, decode.line_number),
        message_type: "all_wspr_decode".to_string(),
        raw: json!({
            "line_number": decode.line_number,
            "line": decode.raw_line,
            "fields": fields(&decode.raw_line),
        }),
    }
}

fn wsjtx_malformed_record(issue: &AllWsprLineIssue, config: &WsjtxImportConfig) -> WsjtXRecord {
    WsjtXRecord {
        meta: record_meta(config.session_id.clone(), config.imported_at),
        record_id: wsjtx_record_id(&config.import_id, issue.line_number),
        message_type: "all_wspr_malformed".to_string(),
        raw: json!({
            "line_number": issue.line_number,
            "line": issue.raw_line,
            "fields": fields(&issue.raw_line),
            "error": issue.kind.to_string(),
        }),
    }
}

fn observation_record(
    decode: &AllWsprDecode,
    config: &WsjtxImportConfig,
    reporter_call: &str,
    reporter_grid: &str,
) -> ObservationRecord {
    ObservationRecord {
        meta: record_meta(config.session_id.clone(), decode.timestamp),
        observation_id: observation_id(&config.import_id, decode.line_number),
        observation_kind: ObservationKind::LocalDecode,
        band: decode.band,
        frequency_hz: Some(decode.frequency_hz),
        mode: Some("WSPR".to_string()),
        reporter_call: Some(reporter_call.to_string()),
        heard_call: Some(decode.tx_call.clone()),
        reporter_grid: Some(reporter_grid.to_string()),
        heard_grid: Some(decode.tx_grid.clone()),
        distance_km: None,
        azimuth_degrees: None,
        snr_db: Some(decode.snr_db),
        drift_hz_per_minute: Some(decode.drift_hz_per_minute),
        power_watts: Some(decode.tx_power_watts),
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: json!({
            "line_number": decode.line_number,
            "line": decode.raw_line,
            "fields": fields(&decode.raw_line),
            "dt_seconds": decode.dt_seconds,
            "frequency_mhz": decode.frequency_mhz_text,
            "tx_power_dbm": decode.tx_power_dbm,
            "extra_fields": decode.extra_fields,
        }),
    }
}

fn record_meta(session_id: String, timestamp: DateTime<Utc>) -> RecordMeta {
    RecordMeta {
        schema_version: SCHEMA_VERSION,
        session_id,
        timestamp,
        source: RecordSource::WsjtxLog,
    }
}

fn wsjtx_record_id(import_id: &str, line_number: usize) -> String {
    format!("{import_id}-wsjtx-{line_number:06}")
}

fn observation_id(import_id: &str, line_number: usize) -> String {
    format!("{import_id}-obs-{line_number:06}")
}

fn fields(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}
