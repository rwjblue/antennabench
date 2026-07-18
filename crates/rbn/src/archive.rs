use std::{
    collections::BTreeSet,
    io::{Read, Seek},
};

use antennabench_core::{v3::SignalModeV3, Band};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip::ZipArchive;

pub const RBN_ARCHIVE_COLUMNS: [&str; 13] = [
    "callsign", "de_pfx", "de_cont", "freq", "band", "dx", "dx_pfx", "dx_cont", "mode", "db",
    "date", "speed", "tx_mode",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RbnArchiveLimits {
    pub compressed_bytes: u64,
    pub uncompressed_bytes: u64,
    pub rows: u64,
    pub row_bytes: u64,
    pub retained_rows: usize,
}

pub const RBN_ARCHIVE_LIMITS: RbnArchiveLimits = RbnArchiveLimits {
    compressed_bytes: 128 * 1024 * 1024,
    uncompressed_bytes: 2 * 1024 * 1024 * 1024,
    rows: 10_000_000,
    row_bytes: 64 * 1024,
    retained_rows: 100_000,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbnImportConfig {
    pub heard_callsign: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub selected_bands: Vec<Band>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RbnRowDisposition {
    Accepted,
    Malformed,
    Filtered,
    Unsupported,
    Duplicate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RbnRowReason {
    Accepted,
    InvalidValue,
    CallsignFiltered,
    TimeFiltered,
    BandFiltered,
    UnsupportedBand,
    UnsupportedMode,
    ExactDuplicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RbnSpot {
    pub reporter_call: String,
    pub heard_call: String,
    pub observed_at: DateTime<Utc>,
    pub frequency_hz: u64,
    pub band: Band,
    pub mode: SignalModeV3,
    pub snr_db: f32,
    pub key_speed_wpm: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RbnRowResult {
    pub row_number: u64,
    pub disposition: RbnRowDisposition,
    pub reason: RbnRowReason,
    pub raw_fields: Vec<String>,
    pub spot: Option<RbnSpot>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbnDispositionSummary {
    pub total: u64,
    pub accepted: u64,
    pub malformed: u64,
    pub filtered: u64,
    pub unsupported: u64,
    pub duplicate: u64,
    pub retained: u64,
    pub omitted: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRbnArchive {
    pub archive_member: String,
    pub rows: Vec<RbnRowResult>,
    pub summary: RbnDispositionSummary,
}

#[derive(Debug, Error)]
pub enum RbnImportError {
    #[error("invalid RBN import configuration: {0}")]
    Config(String),
    #[error("invalid RBN ZIP archive: {0}")]
    Archive(String),
    #[error("unsupported RBN CSV schema: {0}")]
    Schema(String),
    #[error("RBN archive resource limit exceeded: {0}")]
    Resource(String),
    #[error("RBN CSV read failed: {0}")]
    Csv(String),
}

pub fn parse_rbn_zip<R: Read + Seek>(
    reader: R,
    compressed_bytes: u64,
    config: &RbnImportConfig,
) -> Result<ParsedRbnArchive, RbnImportError> {
    parse_rbn_zip_with_limits(reader, compressed_bytes, config, RBN_ARCHIVE_LIMITS)
}

pub fn parse_rbn_zip_with_limits<R: Read + Seek>(
    reader: R,
    compressed_bytes: u64,
    config: &RbnImportConfig,
    limits: RbnArchiveLimits,
) -> Result<ParsedRbnArchive, RbnImportError> {
    validate_config(config)?;
    if compressed_bytes > limits.compressed_bytes {
        return Err(RbnImportError::Resource(format!(
            "compressed bytes {compressed_bytes} exceed {}",
            limits.compressed_bytes
        )));
    }
    let mut archive =
        ZipArchive::new(reader).map_err(|error| RbnImportError::Archive(error.to_string()))?;
    let members = (0..archive.len())
        .filter_map(|index| {
            let entry = archive.by_index(index).ok()?;
            (!entry.is_dir()).then(|| (index, entry.name().to_string(), entry.size()))
        })
        .collect::<Vec<_>>();
    let [(index, name, size)] = members.as_slice() else {
        return Err(RbnImportError::Archive(
            "archive must contain exactly one regular CSV member".into(),
        ));
    };
    if !name.to_ascii_lowercase().ends_with(".csv") {
        return Err(RbnImportError::Archive(
            "the single archive member must have a .csv suffix".into(),
        ));
    }
    if *size > limits.uncompressed_bytes {
        return Err(RbnImportError::Resource(format!(
            "uncompressed bytes {size} exceed {}",
            limits.uncompressed_bytes
        )));
    }
    let entry = archive
        .by_index(*index)
        .map_err(|error| RbnImportError::Archive(error.to_string()))?;
    let mut parsed = parse_rbn_csv_with_limits(entry, config, limits)?;
    parsed.archive_member = name.clone();
    Ok(parsed)
}

pub fn parse_rbn_csv<R: Read>(
    reader: R,
    config: &RbnImportConfig,
) -> Result<ParsedRbnArchive, RbnImportError> {
    parse_rbn_csv_with_limits(reader, config, RBN_ARCHIVE_LIMITS)
}

pub fn parse_rbn_csv_with_limits<R: Read>(
    reader: R,
    config: &RbnImportConfig,
    limits: RbnArchiveLimits,
) -> Result<ParsedRbnArchive, RbnImportError> {
    validate_config(config)?;
    let mut csv = csv::ReaderBuilder::new().flexible(true).from_reader(reader);
    let headers = csv
        .headers()
        .map_err(|error| RbnImportError::Csv(error.to_string()))?;
    if headers.iter().collect::<Vec<_>>() != RBN_ARCHIVE_COLUMNS {
        return Err(RbnImportError::Schema(format!(
            "expected {}, received {}",
            RBN_ARCHIVE_COLUMNS.join(","),
            headers.iter().collect::<Vec<_>>().join(",")
        )));
    }

    let heard_callsign = config.heard_callsign.trim().to_ascii_uppercase();
    let selected_bands = config.selected_bands.clone();
    let mut fingerprints = BTreeSet::new();
    let mut summary = RbnDispositionSummary::default();
    let mut rows = Vec::new();
    for record in csv.records() {
        summary.total += 1;
        if summary.total > limits.rows {
            return Err(RbnImportError::Resource(format!(
                "rows exceed {}",
                limits.rows
            )));
        }
        let record = record.map_err(|error| RbnImportError::Csv(error.to_string()))?;
        let row_bytes = record.iter().map(str::len).sum::<usize>() + record.len();
        if u64::try_from(row_bytes).unwrap_or(u64::MAX) > limits.row_bytes {
            return Err(RbnImportError::Resource(format!(
                "row {} bytes exceed {}",
                summary.total, limits.row_bytes
            )));
        }
        let raw_fields = record.iter().map(str::to_string).collect::<Vec<_>>();
        let mut result = classify_row(
            summary.total,
            raw_fields,
            &heard_callsign,
            config.window_start,
            config.window_end,
            &selected_bands,
        );
        if let Some(spot) = &result.spot {
            let fingerprint = format!(
                "{}|{}|{}|{}|{:?}|{}|{}",
                spot.reporter_call,
                spot.heard_call,
                spot.observed_at,
                spot.frequency_hz,
                spot.mode,
                spot.snr_db,
                spot.key_speed_wpm
                    .map_or_else(String::new, |value| value.to_string()),
            );
            if !fingerprints.insert(fingerprint) {
                result.disposition = RbnRowDisposition::Duplicate;
                result.reason = RbnRowReason::ExactDuplicate;
                result.spot = None;
            }
        }
        count(&mut summary, result.disposition);
        if rows.len() < limits.retained_rows {
            rows.push(result);
            summary.retained += 1;
        } else {
            summary.omitted += 1;
        }
    }
    Ok(ParsedRbnArchive {
        archive_member: String::new(),
        rows,
        summary,
    })
}

fn validate_config(config: &RbnImportConfig) -> Result<(), RbnImportError> {
    let callsign = config.heard_callsign.trim();
    if callsign.is_empty()
        || callsign.len() > 32
        || !callsign
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'/')
    {
        return Err(RbnImportError::Config(
            "heard_callsign must be a bounded ASCII callsign".into(),
        ));
    }
    if config.window_start >= config.window_end {
        return Err(RbnImportError::Config(
            "window_start must precede window_end".into(),
        ));
    }
    if config.selected_bands.is_empty() {
        return Err(RbnImportError::Config(
            "selected_bands must not be empty".into(),
        ));
    }
    Ok(())
}

fn classify_row(
    row_number: u64,
    raw_fields: Vec<String>,
    heard_callsign: &str,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    selected_bands: &[Band],
) -> RbnRowResult {
    let malformed = || RbnRowResult {
        row_number,
        disposition: RbnRowDisposition::Malformed,
        reason: RbnRowReason::InvalidValue,
        raw_fields: raw_fields.clone(),
        spot: None,
    };
    if raw_fields.len() != RBN_ARCHIVE_COLUMNS.len() {
        return malformed();
    }
    let reporter_call = raw_fields[0].trim().to_ascii_uppercase();
    let heard_call = raw_fields[5].trim().to_ascii_uppercase();
    if !valid_call(&reporter_call) || !valid_call(&heard_call) {
        return malformed();
    }
    if heard_call != heard_callsign {
        return disposition(
            row_number,
            raw_fields,
            RbnRowDisposition::Filtered,
            RbnRowReason::CallsignFiltered,
        );
    }
    let observed_at =
        match NaiveDateTime::parse_from_str(raw_fields[10].trim(), "%Y-%m-%d %H:%M:%S") {
            Ok(value) => value.and_utc(),
            Err(_) => return malformed(),
        };
    if observed_at < window_start || observed_at >= window_end {
        return disposition(
            row_number,
            raw_fields,
            RbnRowDisposition::Filtered,
            RbnRowReason::TimeFiltered,
        );
    }
    let Some(band) = parse_band(raw_fields[4].trim()) else {
        return disposition(
            row_number,
            raw_fields,
            RbnRowDisposition::Unsupported,
            RbnRowReason::UnsupportedBand,
        );
    };
    if !selected_bands.contains(&band) {
        return disposition(
            row_number,
            raw_fields,
            RbnRowDisposition::Filtered,
            RbnRowReason::BandFiltered,
        );
    }
    let mode = match raw_fields[12].trim().to_ascii_uppercase().as_str() {
        "CW" => SignalModeV3::Cw,
        "RTTY" => SignalModeV3::Rtty,
        _ => {
            return disposition(
                row_number,
                raw_fields,
                RbnRowDisposition::Unsupported,
                RbnRowReason::UnsupportedMode,
            )
        }
    };
    let Some(frequency_hz) = parse_khz(raw_fields[3].trim()) else {
        return malformed();
    };
    let Ok(snr_db) = raw_fields[9].trim().parse::<f32>() else {
        return malformed();
    };
    if !snr_db.is_finite() {
        return malformed();
    }
    let key_speed_wpm = match raw_fields[11].trim() {
        "" => None,
        value => match value.parse::<u16>() {
            Ok(0) | Err(_) => return malformed(),
            Ok(value) => Some(value),
        },
    };
    RbnRowResult {
        row_number,
        disposition: RbnRowDisposition::Accepted,
        reason: RbnRowReason::Accepted,
        raw_fields,
        spot: Some(RbnSpot {
            reporter_call,
            heard_call,
            observed_at,
            frequency_hz,
            band,
            mode,
            snr_db,
            key_speed_wpm,
        }),
    }
}

fn disposition(
    row_number: u64,
    raw_fields: Vec<String>,
    disposition: RbnRowDisposition,
    reason: RbnRowReason,
) -> RbnRowResult {
    RbnRowResult {
        row_number,
        disposition,
        reason,
        raw_fields,
        spot: None,
    }
}

fn valid_call(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-'))
}

fn parse_khz(value: &str) -> Option<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole = whole.parse::<u64>().ok()?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u64>().ok()? * 10_u64.pow(u32::try_from(3 - fraction.len()).ok()?)
    };
    whole
        .checked_mul(1000)?
        .checked_add(fraction)
        .filter(|value| *value > 0)
}

fn parse_band(value: &str) -> Option<Band> {
    Some(match value {
        "160m" => Band::M160,
        "80m" => Band::M80,
        "60m" => Band::M60,
        "40m" => Band::M40,
        "30m" => Band::M30,
        "20m" => Band::M20,
        "17m" => Band::M17,
        "15m" => Band::M15,
        "12m" => Band::M12,
        "10m" => Band::M10,
        "6m" => Band::M6,
        "2m" => Band::M2,
        _ => return None,
    })
}

fn count(summary: &mut RbnDispositionSummary, disposition: RbnRowDisposition) {
    match disposition {
        RbnRowDisposition::Accepted => summary.accepted += 1,
        RbnRowDisposition::Malformed => summary.malformed += 1,
        RbnRowDisposition::Filtered => summary.filtered += 1,
        RbnRowDisposition::Unsupported => summary.unsupported += 1,
        RbnRowDisposition::Duplicate => summary.duplicate += 1,
    }
}
