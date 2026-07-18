use std::io::{Cursor, Write};

use antennabench_core::{v3::SignalModeV3, Band};
use antennabench_rbn::{
    parse_rbn_csv, parse_rbn_csv_with_limits, parse_rbn_zip, RbnArchiveLimits, RbnImportConfig,
    RbnImportError, RbnRowDisposition, RBN_ARCHIVE_LIMITS,
};
use chrono::{TimeZone, Utc};
use zip::{write::SimpleFileOptions, ZipWriter};

const FIXTURE: &[u8] = include_bytes!("fixtures/synthetic-current.csv");

fn config() -> RbnImportConfig {
    RbnImportConfig {
        heard_callsign: "n1rwj".into(),
        window_start: Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap(),
        window_end: Utc.with_ymd_and_hms(2026, 7, 14, 12, 30, 0).unwrap(),
        selected_bands: vec![Band::M20],
    }
}

#[test]
fn current_header_streams_cw_and_rtty_without_inventing_missing_facts() {
    let parsed = parse_rbn_csv(FIXTURE, &config()).unwrap();

    assert_eq!(parsed.summary.total, 8);
    assert_eq!(parsed.summary.accepted, 2);
    assert_eq!(parsed.summary.filtered, 2);
    assert_eq!(parsed.summary.unsupported, 2);
    assert_eq!(parsed.summary.malformed, 1);
    assert_eq!(parsed.summary.duplicate, 1);
    let spots = parsed
        .rows
        .iter()
        .filter_map(|row| row.spot.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(spots.len(), 2);
    assert_eq!(spots[0].mode, SignalModeV3::Cw);
    assert_eq!(spots[0].frequency_hz, 14_050_000);
    assert_eq!(spots[1].mode, SignalModeV3::Rtty);
    assert_eq!(spots[1].frequency_hz, 14_051_250);
    assert!(parsed.rows.iter().any(|row| {
        row.disposition == RbnRowDisposition::Filtered && row.raw_fields[5] == "K2OTHER"
    }));
}

#[test]
fn header_drift_and_resource_boundaries_fail_closed() {
    let drifted = FIXTURE.replacen(b"callsign", b"reporter", 1);
    assert!(matches!(
        parse_rbn_csv(drifted.as_slice(), &config()),
        Err(RbnImportError::Schema(_))
    ));

    let limits = RbnArchiveLimits {
        retained_rows: 2,
        ..RBN_ARCHIVE_LIMITS
    };
    let parsed = parse_rbn_csv_with_limits(FIXTURE, &config(), limits).unwrap();
    assert_eq!(parsed.rows.len(), 2);
    assert_eq!(parsed.summary.retained, 2);
    assert_eq!(parsed.summary.omitted, 6);
    assert_eq!(parsed.summary.total, 8);

    let limits = RbnArchiveLimits {
        rows: 7,
        ..RBN_ARCHIVE_LIMITS
    };
    assert!(matches!(
        parse_rbn_csv_with_limits(FIXTURE, &config(), limits),
        Err(RbnImportError::Resource(_))
    ));
}

#[test]
fn zip_archive_requires_one_csv_member_and_streams_that_member() {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    writer
        .start_file("20260714.csv", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(FIXTURE).unwrap();
    let cursor = writer.finish().unwrap();
    let bytes = cursor.into_inner();

    let parsed = parse_rbn_zip(Cursor::new(&bytes), bytes.len() as u64, &config()).unwrap();

    assert_eq!(parsed.archive_member, "20260714.csv");
    assert_eq!(parsed.summary.accepted, 2);
}

trait ReplaceBytes {
    fn replacen(&self, from: &[u8], to: &[u8], count: usize) -> Vec<u8>;
}

impl ReplaceBytes for [u8] {
    fn replacen(&self, from: &[u8], to: &[u8], count: usize) -> Vec<u8> {
        let text = String::from_utf8(self.to_vec()).unwrap();
        text.replacen(
            std::str::from_utf8(from).unwrap(),
            std::str::from_utf8(to).unwrap(),
            count,
        )
        .into_bytes()
    }
}
