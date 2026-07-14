use antennabench_core::{
    AcquisitionChannelId, AdapterId, Provenance, ProviderId, RecordSource, SourceId,
    IDENTITY_MAX_BYTES,
};

#[test]
fn provider_neutral_identities_are_bounded_lowercase_ascii_segments() {
    for valid in ["wsjt-x", "noaa-swpc", "antennabench.core", "source_2"] {
        assert!(ProviderId::new(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "WSJT-X",
        "-leading",
        "trailing-",
        "two..dots",
        "not ascii",
    ] {
        assert!(ProviderId::new(invalid).is_err(), "{invalid}");
    }
    assert!(ProviderId::new("a".repeat(IDENTITY_MAX_BYTES)).is_ok());
    assert!(ProviderId::new("a".repeat(IDENTITY_MAX_BYTES + 1)).is_err());

    let provenance: Provenance = serde_json::from_value(serde_json::json!({
        "provider_id": "new-provider",
        "source_id": "new-source",
        "acquisition_channel": "offline-import",
        "adapter_id": "antennabench.future-adapter",
        "adapter_version": "7.2.1"
    }))
    .unwrap();
    assert_eq!(provenance.provider_id.as_str(), "new-provider");
    assert_eq!(provenance.source_id, SourceId::new("new-source").unwrap());
    assert_eq!(
        provenance.acquisition_channel,
        AcquisitionChannelId::new("offline-import").unwrap()
    );
    assert_eq!(
        provenance.adapter_id,
        AdapterId::new("antennabench.future-adapter").unwrap()
    );
}

#[test]
fn every_legacy_source_has_a_deterministic_v2_mapping() {
    let sources = [
        RecordSource::Operator,
        RecordSource::WsjtxUdp,
        RecordSource::WsjtxLog,
        RecordSource::Wsprnet,
        RecordSource::WsprLive,
        RecordSource::ImportedFile,
        RecordSource::RigAdapter,
        RecordSource::NoaaSwpc,
        RecordSource::Derived,
    ];

    for source in sources {
        let provenance = Provenance::from_legacy(source, "legacy-v1");
        assert_eq!(provenance.legacy_source(), source);
        assert_eq!(provenance.adapter_version, "legacy-v1");
    }
}
