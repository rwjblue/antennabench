use std::io::{self, BufRead};

use antennabench_core::Band;
use chrono::{DateTime, NaiveDate, Utc};
use thiserror::Error;

use crate::{
    diagnostic, AdapterCancellationToken, AdapterResourceError, AdapterResourceStage,
    AdapterResourceUnit, WsjtxAdapterLimits, WSJTX_ADAPTER_LIMITS,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AllWsprDecode {
    pub line_number: usize,
    pub raw_line: String,
    pub timestamp: DateTime<Utc>,
    pub snr_db: f32,
    pub dt_seconds: f32,
    pub frequency_hz: u64,
    pub frequency_mhz_text: String,
    pub band: Band,
    pub tx_call: String,
    pub tx_grid: String,
    pub tx_power_dbm: i16,
    pub tx_power_watts: f32,
    pub drift_hz_per_minute: f32,
    pub extra_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAllWsprText {
    pub decodes: Vec<AllWsprDecode>,
    pub issues: Vec<AllWsprLineIssue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IncompleteAllWsprText {
    pub retained_prefix: Vec<u8>,
    pub retained_bytes: u64,
    pub failure: AdapterResourceError,
}

#[derive(Debug, Error)]
pub enum AllWsprStreamError {
    #[error("failed to read offline WSJT-X source")]
    Io(#[source] io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AllWsprStreamOutcome {
    Complete(ParsedAllWsprText),
    Quarantined(IncompleteAllWsprText),
}

#[derive(Error, Debug, Clone, PartialEq)]
#[error("line {line_number}: {kind}")]
pub struct AllWsprLineIssue {
    pub line_number: usize,
    pub raw_line: String,
    pub kind: AllWsprLineIssueKind,
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum AllWsprLineIssueKind {
    #[error("too few fields: expected at least 9, got {actual}")]
    TooFewFields { actual: usize },
    #[error("invalid date: {value}")]
    InvalidDate { value: String },
    #[error("invalid time: {value}")]
    InvalidTime { value: String },
    #[error("invalid SNR: {value}")]
    InvalidSnr { value: String },
    #[error("invalid DT: {value}")]
    InvalidDt { value: String },
    #[error("invalid frequency: {value}")]
    InvalidFrequency { value: String },
    #[error("invalid callsign: {value}")]
    InvalidCallsign { value: String },
    #[error("invalid grid: {value}")]
    InvalidGrid { value: String },
    #[error("unsupported band for frequency {frequency_hz} Hz")]
    UnsupportedBand { frequency_hz: u64 },
    #[error("invalid power: {value}")]
    InvalidPower { value: String },
    #[error("invalid drift: {value}")]
    InvalidDrift { value: String },
}

pub fn parse_all_wspr_text(input: &str) -> ParsedAllWsprText {
    let mut decodes = Vec::new();
    let mut issues = Vec::new();

    for (index, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        match parse_all_wspr_line(index + 1, line) {
            Ok(decode) => decodes.push(decode),
            Err(issue) => issues.push(issue),
        }
    }

    ParsedAllWsprText { decodes, issues }
}

pub fn parse_all_wspr_reader(
    reader: impl BufRead,
    source: impl Into<String>,
    cancellation: &AdapterCancellationToken,
) -> Result<AllWsprStreamOutcome, AllWsprStreamError> {
    parse_all_wspr_reader_with_limits(reader, source.into(), cancellation, WSJTX_ADAPTER_LIMITS)
}

#[doc(hidden)]
pub fn parse_all_wspr_reader_with_limits(
    mut reader: impl BufRead,
    source: String,
    cancellation: &AdapterCancellationToken,
    limits: WsjtxAdapterLimits,
) -> Result<AllWsprStreamOutcome, AllWsprStreamError> {
    let mut retained_prefix = Vec::new();
    let mut current_line = Vec::new();
    let mut total_bytes = 0_u64;
    let mut physical_lines = 0_u64;
    let mut nonblank_lines = 0_u64;
    let mut parsed = ParsedAllWsprText {
        decodes: Vec::new(),
        issues: Vec::new(),
    };

    loop {
        if cancellation.is_cancelled() {
            return Ok(quarantined(
                retained_prefix,
                diagnostic(
                    "resource.operation.cancelled",
                    "wsjtx.offline",
                    &source,
                    AdapterResourceStage::Stream,
                    0,
                    Some(total_bytes),
                    AdapterResourceUnit::Checkpoints,
                    false,
                ),
            ));
        }

        let available = reader.fill_buf().map_err(AllWsprStreamError::Io)?;
        if available.is_empty() {
            if !current_line.is_empty() {
                physical_lines += 1;
                if let Err(failure) = process_streamed_line(
                    &mut parsed,
                    &current_line,
                    physical_lines as usize,
                    &mut nonblank_lines,
                    &source,
                    limits,
                ) {
                    return Ok(quarantined(retained_prefix, failure));
                }
            }
            return Ok(AllWsprStreamOutcome::Complete(parsed));
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(available.len(), |index| index + 1);
        let chunk = &available[..consumed];
        let observed_total = total_bytes.saturating_add(chunk.len() as u64);
        if observed_total > limits.offline_source_bytes {
            return Ok(quarantined(
                retained_prefix,
                diagnostic(
                    "resource.adapter.offline_source_bytes",
                    "wsjtx.offline",
                    &source,
                    AdapterResourceStage::Stream,
                    limits.offline_source_bytes,
                    Some(observed_total),
                    AdapterResourceUnit::Bytes,
                    false,
                ),
            ));
        }
        let observed_line = current_line.len() as u64 + chunk.len() as u64;
        if observed_line > limits.offline_line_bytes {
            return Ok(quarantined(
                retained_prefix,
                diagnostic(
                    "resource.adapter.offline_line_bytes",
                    "wsjtx.offline",
                    &source,
                    AdapterResourceStage::Stream,
                    limits.offline_line_bytes,
                    Some(observed_line),
                    AdapterResourceUnit::Bytes,
                    false,
                ),
            ));
        }

        retained_prefix.extend_from_slice(chunk);
        current_line.extend_from_slice(chunk);
        total_bytes = observed_total;
        reader.consume(consumed);
        if newline.is_some() {
            physical_lines += 1;
            if let Err(failure) = process_streamed_line(
                &mut parsed,
                &current_line,
                physical_lines as usize,
                &mut nonblank_lines,
                &source,
                limits,
            ) {
                return Ok(quarantined(retained_prefix, failure));
            }
            current_line.clear();
        }
    }
}

fn process_streamed_line(
    parsed: &mut ParsedAllWsprText,
    bytes: &[u8],
    line_number: usize,
    nonblank_lines: &mut u64,
    source: &str,
    limits: WsjtxAdapterLimits,
) -> Result<(), AdapterResourceError> {
    let line = String::from_utf8_lossy(bytes)
        .trim_end_matches(['\r', '\n'])
        .to_string();
    if line.trim().is_empty() {
        return Ok(());
    }
    *nonblank_lines += 1;
    if *nonblank_lines > limits.offline_nonblank_lines {
        return Err(diagnostic(
            "resource.adapter.offline_source_lines",
            "wsjtx.offline",
            source,
            AdapterResourceStage::Stream,
            limits.offline_nonblank_lines,
            Some(*nonblank_lines),
            AdapterResourceUnit::Lines,
            false,
        ));
    }
    match parse_all_wspr_line(line_number, &line) {
        Ok(decode) => parsed.decodes.push(decode),
        Err(issue) => parsed.issues.push(issue),
    }
    Ok(())
}

fn quarantined(prefix: Vec<u8>, failure: AdapterResourceError) -> AllWsprStreamOutcome {
    AllWsprStreamOutcome::Quarantined(IncompleteAllWsprText {
        retained_bytes: prefix.len() as u64,
        retained_prefix: prefix,
        failure,
    })
}

pub fn parse_all_wspr_line(
    line_number: usize,
    line: &str,
) -> Result<AllWsprDecode, AllWsprLineIssue> {
    let raw_line = line.to_string();
    let fields: Vec<&str> = line.split_whitespace().collect();

    if fields.len() < 9 {
        return Err(issue(
            line_number,
            raw_line,
            AllWsprLineIssueKind::TooFewFields {
                actual: fields.len(),
            },
        ));
    }

    let timestamp = parse_timestamp(fields[0], fields[1])
        .map_err(|kind| issue(line_number, raw_line.clone(), kind))?;
    let snr_db = parse_f32(fields[2], |value| AllWsprLineIssueKind::InvalidSnr {
        value,
    })
    .map_err(|kind| issue(line_number, raw_line.clone(), kind))?;
    let dt_seconds = parse_f32(fields[3], |value| AllWsprLineIssueKind::InvalidDt { value })
        .map_err(|kind| issue(line_number, raw_line.clone(), kind))?;
    let frequency_mhz = parse_f64(fields[4]).ok_or_else(|| {
        issue(
            line_number,
            raw_line.clone(),
            AllWsprLineIssueKind::InvalidFrequency {
                value: fields[4].to_string(),
            },
        )
    })?;
    let frequency_hz = (frequency_mhz * 1_000_000.0).round() as u64;
    let band = band_from_frequency_hz(frequency_hz).ok_or_else(|| {
        issue(
            line_number,
            raw_line.clone(),
            AllWsprLineIssueKind::UnsupportedBand { frequency_hz },
        )
    })?;
    let tx_power_dbm = fields[7].parse::<i16>().map_err(|_| {
        issue(
            line_number,
            raw_line.clone(),
            AllWsprLineIssueKind::InvalidPower {
                value: fields[7].to_string(),
            },
        )
    })?;
    let drift_hz_per_minute = parse_f32(fields[8], |value| AllWsprLineIssueKind::InvalidDrift {
        value,
    })
    .map_err(|kind| issue(line_number, raw_line.clone(), kind))?;
    let tx_call = parse_callsign(line_number, &raw_line, fields[5])?;
    let tx_grid = parse_grid(line_number, &raw_line, fields[6])?;

    Ok(AllWsprDecode {
        line_number,
        raw_line,
        timestamp,
        snr_db,
        dt_seconds,
        frequency_hz,
        frequency_mhz_text: fields[4].to_string(),
        band,
        tx_call,
        tx_grid,
        tx_power_dbm,
        tx_power_watts: dbm_to_watts(tx_power_dbm),
        drift_hz_per_minute,
        extra_fields: fields[9..]
            .iter()
            .map(|field| (*field).to_string())
            .collect(),
    })
}

fn issue(line_number: usize, raw_line: String, kind: AllWsprLineIssueKind) -> AllWsprLineIssue {
    AllWsprLineIssue {
        line_number,
        raw_line,
        kind,
    }
}

fn parse_timestamp(date: &str, time: &str) -> Result<DateTime<Utc>, AllWsprLineIssueKind> {
    if date.len() != 6 || !date.chars().all(|c| c.is_ascii_digit()) {
        return Err(AllWsprLineIssueKind::InvalidDate {
            value: date.to_string(),
        });
    }
    if time.len() != 4 || !time.chars().all(|c| c.is_ascii_digit()) {
        return Err(AllWsprLineIssueKind::InvalidTime {
            value: time.to_string(),
        });
    }

    let year = 2000 + date[0..2].parse::<i32>().expect("date digits checked");
    let month = date[2..4].parse::<u32>().expect("date digits checked");
    let day = date[4..6].parse::<u32>().expect("date digits checked");
    let hour = time[0..2].parse::<u32>().expect("time digits checked");
    let minute = time[2..4].parse::<u32>().expect("time digits checked");

    let date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        AllWsprLineIssueKind::InvalidDate {
            value: date.to_string(),
        }
    })?;
    let timestamp =
        date.and_hms_opt(hour, minute, 0)
            .ok_or_else(|| AllWsprLineIssueKind::InvalidTime {
                value: time.to_string(),
            })?;

    Ok(DateTime::from_naive_utc_and_offset(timestamp, Utc))
}

fn parse_f32(
    value: &str,
    make_issue: impl FnOnce(String) -> AllWsprLineIssueKind,
) -> Result<f32, AllWsprLineIssueKind> {
    value
        .parse::<f32>()
        .ok()
        .filter(|parsed| parsed.is_finite())
        .ok_or_else(|| make_issue(value.to_string()))
}

fn parse_f64(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|parsed| parsed.is_finite())
}

fn parse_callsign(
    line_number: usize,
    raw_line: &str,
    value: &str,
) -> Result<String, AllWsprLineIssue> {
    normalize_wspr_callsign(value).ok_or_else(|| {
        issue(
            line_number,
            raw_line.to_string(),
            AllWsprLineIssueKind::InvalidCallsign {
                value: value.to_string(),
            },
        )
    })
}

fn parse_grid(line_number: usize, raw_line: &str, value: &str) -> Result<String, AllWsprLineIssue> {
    normalize_maidenhead_grid(value).ok_or_else(|| {
        issue(
            line_number,
            raw_line.to_string(),
            AllWsprLineIssueKind::InvalidGrid {
                value: value.to_string(),
            },
        )
    })
}

pub fn normalize_wspr_callsign(value: &str) -> Option<String> {
    let callsign = value.to_ascii_uppercase();
    let valid_length = (3..=12).contains(&callsign.len());
    let valid_chars = callsign.bytes().all(|byte| byte.is_ascii_alphanumeric());
    let has_letter = callsign.bytes().any(|byte| byte.is_ascii_alphabetic());
    let has_digit = callsign.bytes().any(|byte| byte.is_ascii_digit());

    (valid_length && valid_chars && has_letter && has_digit).then_some(callsign)
}

pub fn normalize_maidenhead_grid(value: &str) -> Option<String> {
    let grid = value.to_ascii_uppercase();
    let bytes = grid.as_bytes();
    let is_valid = matches!(bytes.len(), 4 | 6)
        && matches!(bytes[0], b'A'..=b'R')
        && matches!(bytes[1], b'A'..=b'R')
        && bytes[2].is_ascii_digit()
        && bytes[3].is_ascii_digit()
        && (bytes.len() == 4
            || (matches!(bytes[4], b'A'..=b'X') && matches!(bytes[5], b'A'..=b'X')));

    is_valid.then_some(grid)
}

pub fn dbm_to_watts(dbm: i16) -> f32 {
    10.0_f32.powf((dbm as f32 - 30.0) / 10.0)
}

pub fn band_from_frequency_hz(frequency_hz: u64) -> Option<Band> {
    match frequency_hz {
        1_800_000..=2_000_000 => Some(Band::M160),
        3_500_000..=4_000_000 => Some(Band::M80),
        5_000_000..=5_500_000 => Some(Band::M60),
        7_000_000..=7_300_000 => Some(Band::M40),
        10_100_000..=10_150_000 => Some(Band::M30),
        14_000_000..=14_350_000 => Some(Band::M20),
        18_068_000..=18_168_000 => Some(Band::M17),
        21_000_000..=21_450_000 => Some(Band::M15),
        24_890_000..=24_990_000 => Some(Band::M12),
        28_000_000..=29_700_000 => Some(Band::M10),
        50_000_000..=54_000_000 => Some(Band::M6),
        144_000_000..=148_000_000 => Some(Band::M2),
        _ => None,
    }
}
