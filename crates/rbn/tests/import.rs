use antennabench_core::{
    v2::{AdapterDisposition, AdapterInput, AttachmentReference},
    Band, ObservationKind,
};
use antennabench_rbn::{
    parse_rbn_csv, prepare_rbn_import, RbnImportConfig, RbnImportPreparationConfig, RBN_ADAPTER_ID,
};
use chrono::{TimeZone, Utc};

const FIXTURE: &[u8] = include_bytes!("fixtures/synthetic-current.csv");

fn import_config() -> RbnImportConfig {
    RbnImportConfig {
        heard_callsign: "n1rwj".into(),
        window_start: Utc.with_ymd_and_hms(2026, 7, 14, 12, 0, 0).unwrap(),
        window_end: Utc.with_ymd_and_hms(2026, 7, 14, 12, 30, 0).unwrap(),
        selected_bands: vec![Band::M20],
    }
}

fn preparation() -> RbnImportPreparationConfig {
    RbnImportPreparationConfig {
        captured_at: Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap(),
        source_locator: Some("20260714.zip".into()),
    }
}

fn attachment() -> AttachmentReference {
    AttachmentReference {
        sha256: "a".repeat(64),
        byte_size: 1234,
        media_type: "application/zip".into(),
        encoding: None,
        container: Some("zip-single-csv".into()),
        source_locator: Some("20260714.zip".into()),
    }
}

#[test]
fn prepares_provenance_linked_public_reports_and_preserves_every_disposition() {
    let config = import_config();
    let parsed = parse_rbn_csv(FIXTURE, &config).unwrap();
    let prepared = prepare_rbn_import(
        &parsed,
        &config,
        &preparation(),
        "session-1",
        "one",
        attachment(),
        &[],
    );

    assert_eq!(prepared.summary.total, 8);
    assert_eq!(prepared.summary.accepted, 2);
    assert_eq!(prepared.summary.malformed, 1);
    assert_eq!(prepared.summary.filtered, 2);
    assert_eq!(prepared.summary.unsupported, 2);
    assert_eq!(prepared.summary.duplicate, 1);
    assert_eq!(prepared.summary.conflict, 0);
    assert_eq!(prepared.summary.observations_created, 2);
    assert_eq!(prepared.adapter_records.len(), 10);
    assert_eq!(prepared.observations.len(), 2);

    let capture = &prepared.adapter_records[0];
    assert_eq!(capture.meta.schema_version, 3);
    assert_eq!(capture.meta.provenance.adapter_id.as_str(), RBN_ADAPTER_ID);
    assert!(matches!(
        &capture.input,
        AdapterInput::Attachment { attachment: value } if value == &attachment()
    ));

    let cw = &prepared.observations[0];
    let rtty = &prepared.observations[1];
    assert_eq!(cw.observation_kind, ObservationKind::PublicReport);
    assert_eq!(cw.mode.as_deref(), Some("CW"));
    assert_eq!(rtty.mode.as_deref(), Some("RTTY"));
    assert_eq!(cw.reporter_call.as_deref(), Some("K1ABC-1"));
    assert_eq!(cw.heard_call.as_deref(), Some("N1RWJ"));
    assert_eq!(cw.frequency_hz, Some(14_050_000));
    assert_eq!(cw.snr_db, Some(18.0));
    assert_eq!(cw.adapter_record_ids.len(), 1);
    assert!(cw.reporter_grid.is_none());
    assert!(cw.heard_grid.is_none());
    assert!(cw.distance_km.is_none());
    assert!(cw.azimuth_degrees.is_none());
    assert!(cw.drift_hz_per_minute.is_none());
    assert!(cw.power_watts.is_none());
    assert!(prepared.adapter_records.iter().any(|record| {
        record.disposition == AdapterDisposition::Malformed
            && record.reason.as_str() == "rbn.invalid-value"
    }));
    assert!(prepared.adapter_records.iter().any(|record| {
        record.disposition == AdapterDisposition::Filtered
            && record.reason.as_str() == "rbn.callsign-filtered"
    }));
    assert!(prepared.adapter_records.iter().any(|record| {
        record.disposition == AdapterDisposition::Unsupported
            && record.reason.as_str() == "rbn.unsupported-band"
    }));
}

#[test]
fn replay_is_duplicate_and_same_natural_identity_with_changed_content_is_conflict() {
    let config = import_config();
    let parsed = parse_rbn_csv(FIXTURE, &config).unwrap();
    let first = prepare_rbn_import(
        &parsed,
        &config,
        &preparation(),
        "session-1",
        "one",
        attachment(),
        &[],
    );
    let replay = prepare_rbn_import(
        &parsed,
        &config,
        &preparation(),
        "session-1",
        "two",
        attachment(),
        &first.adapter_records,
    );
    assert_eq!(replay.summary.accepted, 0);
    assert_eq!(replay.summary.duplicate, 3);
    assert_eq!(replay.summary.observations_created, 0);

    let changed = String::from_utf8(FIXTURE.to_vec()).unwrap().replacen(
        ",18,2026-07-14 12:00:01,",
        ",17,2026-07-14 12:00:01,",
        1,
    );
    let changed = parse_rbn_csv(changed.as_bytes(), &config).unwrap();
    let conflict = prepare_rbn_import(
        &changed,
        &config,
        &preparation(),
        "session-1",
        "three",
        attachment(),
        &first.adapter_records,
    );
    assert_eq!(conflict.summary.conflict, 1);
    assert_eq!(conflict.summary.duplicate, 2);
    assert_eq!(conflict.summary.observations_created, 0);
    assert!(conflict.adapter_records.iter().any(|record| {
        record.disposition == AdapterDisposition::Conflict
            && record.reason.as_str() == "rbn.replay-conflict"
    }));
}
