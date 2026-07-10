use antennabench_core::Band;
use antennabench_wsjtx::{parse_all_wspr_line, parse_all_wspr_text, AllWsprLineIssueKind};

#[test]
fn parses_all_wspr_decode_rows() {
    let input = include_str!("../../../fixtures/wsjtx/all_wspr_sample.txt");

    let parsed = parse_all_wspr_text(input);

    assert!(parsed.issues.is_empty());
    assert_eq!(parsed.decodes.len(), 3);

    let first = &parsed.decodes[0];
    assert_eq!(first.line_number, 1);
    assert_eq!(first.timestamp.to_rfc3339(), "2026-07-09T20:02:00+00:00");
    assert_eq!(first.snr_db, -18.0);
    assert_eq!(first.dt_seconds, 0.07);
    assert_eq!(first.frequency_hz, 14_095_600);
    assert_eq!(first.frequency_mhz_text, "14.095600");
    assert_eq!(first.band, Band::M20);
    assert_eq!(first.tx_call, "K1ABC");
    assert_eq!(first.tx_grid, "EM12");
    assert_eq!(first.tx_power_dbm, 37);
    assert!((first.tx_power_watts - 5.011872).abs() < 0.000001);
    assert_eq!(first.drift_hz_per_minute, 0.0);

    assert_eq!(parsed.decodes[1].extra_fields, vec!["0.19", "2"]);
    assert_eq!(parsed.decodes[2].band, Band::M40);
}

#[test]
fn parse_line_reports_too_few_fields() {
    let issue = parse_all_wspr_line(7, "260709 2002 -18").unwrap_err();

    assert_eq!(issue.line_number, 7);
    assert_eq!(issue.kind, AllWsprLineIssueKind::TooFewFields { actual: 3 });
}
