use std::collections::BTreeMap;

use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput, AdapterReasonId,
        AttachmentReference, MutationMember, NormalizedRecordKind, NormalizedRecordLink,
        Provenance, ProviderId, RecordMetaV2, SourceId,
    },
    v3::{AdapterRecordV3, ObservationRecordV3},
    ObservationKind, SCHEMA_VERSION_V3,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ParsedRbnArchive, RbnImportConfig, RbnRowDisposition, RbnRowReason, RbnRowResult, RbnSpot,
    RBN_ARCHIVE_COLUMNS,
};

pub const RBN_ADAPTER_ID: &str = "antennabench.rbn-daily-archive";
pub const RBN_PROVIDER_ID: &str = "reverse-beacon-network";
pub const RBN_SOURCE_ID: &str = "rbn-daily-archive";
pub const RBN_ACQUISITION_CHANNEL: &str = "file-import";
pub const RBN_ADAPTER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub struct RbnImportPreparationConfig {
    pub captured_at: DateTime<Utc>,
    pub source_locator: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbnPreparedSummary {
    pub total: u64,
    pub accepted: u64,
    pub malformed: u64,
    pub filtered: u64,
    pub unsupported: u64,
    pub duplicate: u64,
    pub conflict: u64,
    pub observations_created: u64,
    pub omitted: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRbnImport {
    pub mutation_id: String,
    pub adapter_records: Vec<AdapterRecordV3>,
    pub observations: Vec<ObservationRecordV3>,
    pub summary: RbnPreparedSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExistingSpot {
    fingerprint: String,
}

pub fn prepare_rbn_import(
    parsed: &ParsedRbnArchive,
    import_config: &RbnImportConfig,
    preparation: &RbnImportPreparationConfig,
    session_id: &str,
    import_id: &str,
    exact_archive: AttachmentReference,
    existing: &[AdapterRecordV3],
) -> PreparedRbnImport {
    let mutation_id = format!("rbn-import-{import_id}");
    let provenance = rbn_provenance();
    let mut replay = existing_rbn_spots(existing);
    let mut adapter_records = Vec::with_capacity(parsed.rows.len() + 2);
    let mut observations = Vec::with_capacity(parsed.summary.accepted as usize);
    let mut summary = RbnPreparedSummary {
        total: parsed.summary.total,
        omitted: parsed.summary.omitted,
        ..RbnPreparedSummary::default()
    };

    adapter_records.push(AdapterRecordV3 {
        meta: import_meta(
            session_id,
            &mutation_id,
            0,
            preparation.captured_at,
            &provenance,
        ),
        record_id: format!("rbn-capture-{import_id}"),
        source_time: Some(preparation.captured_at),
        record_type: "rbn_archive_capture".into(),
        disposition: AdapterDisposition::Accepted,
        reason: reason("rbn.capture"),
        normalized_records: Vec::new(),
        input: AdapterInput::Attachment {
            attachment: exact_archive.clone(),
        },
    });
    adapter_records.push(AdapterRecordV3 {
        meta: import_meta(
            session_id,
            &mutation_id,
            1,
            preparation.captured_at,
            &provenance,
        ),
        record_id: format!("rbn-summary-{import_id}"),
        source_time: Some(preparation.captured_at),
        record_type: "rbn_import_summary".into(),
        disposition: AdapterDisposition::Accepted,
        reason: reason("rbn.import-summary"),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: String::new(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: preparation.source_locator.clone(),
        },
    });

    for row in &parsed.rows {
        let identity = row.spot.as_ref().map(spot_identity);
        let fingerprint = row.spot.as_ref().map(spot_fingerprint);
        let replay_disposition = identity.as_ref().and_then(|identity| {
            replay.get(identity).map(|prior| {
                if fingerprint.as_deref() == Some(prior.fingerprint.as_str()) {
                    AdapterDisposition::Duplicate
                } else {
                    AdapterDisposition::Conflict
                }
            })
        });
        let disposition = replay_disposition.unwrap_or_else(|| row_disposition(row.disposition));
        let creates_observation = disposition == AdapterDisposition::Accepted && row.spot.is_some();
        let adapter_id = format!("rbn-row-{import_id}-{}", row.row_number);
        let observation_id = format!("rbn-observation-{import_id}-{}", row.row_number);
        let normalized_records = creates_observation
            .then(|| NormalizedRecordLink {
                record_kind: NormalizedRecordKind::Observation,
                record_id: observation_id.clone(),
            })
            .into_iter()
            .collect();
        let member_index = u32::try_from(adapter_records.len()).expect("bounded RBN rows fit u32");
        adapter_records.push(AdapterRecordV3 {
            meta: import_meta(
                session_id,
                &mutation_id,
                member_index,
                row.spot
                    .as_ref()
                    .map_or(preparation.captured_at, |spot| spot.observed_at),
                &provenance,
            ),
            record_id: adapter_id.clone(),
            source_time: row.spot.as_ref().map(|spot| spot.observed_at),
            record_type: "rbn_archive_row".into(),
            disposition,
            reason: disposition_reason(disposition, row.reason),
            normalized_records,
            input: AdapterInput::Inline {
                data: serde_json::to_string(&serde_json::json!({
                    "fields": row.raw_fields,
                    "spot_identity": identity,
                    "spot_fingerprint": fingerprint,
                }))
                .expect("RBN fields serialize"),
                media_type: "application/json".into(),
                encoding: None,
                source_locator: preparation.source_locator.clone(),
            },
        });

        match disposition {
            AdapterDisposition::Accepted if creates_observation => {
                let spot = row.spot.as_ref().expect("accepted RBN row has spot");
                replay.insert(
                    identity.expect("accepted RBN row has identity"),
                    ExistingSpot {
                        fingerprint: fingerprint.expect("accepted RBN row has fingerprint"),
                    },
                );
                observations.push(rbn_observation(
                    spot,
                    row,
                    session_id,
                    &mutation_id,
                    &adapter_id,
                    observation_id,
                    preparation.captured_at,
                    &provenance,
                ));
                summary.accepted += 1;
                summary.observations_created += 1;
            }
            AdapterDisposition::Malformed => summary.malformed += 1,
            AdapterDisposition::Filtered => summary.filtered += 1,
            AdapterDisposition::Unsupported => summary.unsupported += 1,
            AdapterDisposition::Duplicate => summary.duplicate += 1,
            AdapterDisposition::Conflict => summary.conflict += 1,
            AdapterDisposition::PartiallyNormalized | AdapterDisposition::Accepted => {}
        }
    }

    if let AdapterInput::Inline { data, .. } = &mut adapter_records[1].input {
        *data = serde_json::to_string(&serde_json::json!({
            "provider_id": RBN_PROVIDER_ID,
            "source_id": RBN_SOURCE_ID,
            "acquisition_channel": RBN_ACQUISITION_CHANNEL,
            "session_callsign": import_config.heard_callsign,
            "captured_at": preparation.captured_at,
            "window_start": import_config.window_start,
            "window_end": import_config.window_end,
            "selected_bands": import_config.selected_bands,
            "archive_member": parsed.archive_member,
            "expected_columns": RBN_ARCHIVE_COLUMNS,
            "exact_archive": exact_archive,
            "counts": summary,
        }))
        .expect("RBN import summary serializes");
    }

    let member_count = u32::try_from(adapter_records.len() + observations.len())
        .expect("bounded RBN import fits u32");
    for record in &mut adapter_records {
        record.meta.mutation.member_count = member_count;
    }
    for (offset, observation) in observations.iter_mut().enumerate() {
        observation.meta.mutation.member_index =
            u32::try_from(adapter_records.len() + offset).expect("bounded RBN import fits u32");
        observation.meta.mutation.member_count = member_count;
    }

    PreparedRbnImport {
        mutation_id,
        adapter_records,
        observations,
        summary,
    }
}

fn rbn_provenance() -> Provenance {
    Provenance {
        provider_id: ProviderId::new(RBN_PROVIDER_ID).expect("static RBN provider"),
        source_id: SourceId::new(RBN_SOURCE_ID).expect("static RBN source"),
        acquisition_channel: AcquisitionChannelId::new(RBN_ACQUISITION_CHANNEL)
            .expect("static RBN acquisition channel"),
        adapter_id: AdapterId::new(RBN_ADAPTER_ID).expect("static RBN adapter"),
        adapter_version: RBN_ADAPTER_VERSION.into(),
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
        schema_version: SCHEMA_VERSION_V3,
        session_id: session_id.into(),
        recorded_at,
        provenance: provenance.clone(),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count: 0,
        },
        runtime_context_id: None,
    }
}

fn existing_rbn_spots(existing: &[AdapterRecordV3]) -> BTreeMap<String, ExistingSpot> {
    existing
        .iter()
        .filter(|record| {
            record.meta.provenance.adapter_id.as_str() == RBN_ADAPTER_ID
                && record.record_type == "rbn_archive_row"
                && record.disposition == AdapterDisposition::Accepted
        })
        .filter_map(|record| {
            let AdapterInput::Inline { data, .. } = &record.input else {
                return None;
            };
            let value: serde_json::Value = serde_json::from_str(data).ok()?;
            let identity = value.get("spot_identity")?.as_str()?.to_string();
            let fingerprint = value.get("spot_fingerprint")?.as_str()?.to_string();
            Some((identity, ExistingSpot { fingerprint }))
        })
        .collect()
}

fn spot_identity(spot: &RbnSpot) -> String {
    format!(
        "{}|{}|{}|{}|{:?}",
        spot.reporter_call, spot.heard_call, spot.observed_at, spot.frequency_hz, spot.mode
    )
}

fn spot_fingerprint(spot: &RbnSpot) -> String {
    let bytes = serde_json::to_vec(spot).expect("RBN spot serializes");
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn row_disposition(value: RbnRowDisposition) -> AdapterDisposition {
    match value {
        RbnRowDisposition::Accepted => AdapterDisposition::Accepted,
        RbnRowDisposition::Malformed => AdapterDisposition::Malformed,
        RbnRowDisposition::Filtered => AdapterDisposition::Filtered,
        RbnRowDisposition::Unsupported => AdapterDisposition::Unsupported,
        RbnRowDisposition::Duplicate => AdapterDisposition::Duplicate,
    }
}

fn disposition_reason(
    disposition: AdapterDisposition,
    row_reason: RbnRowReason,
) -> AdapterReasonId {
    let value = match disposition {
        AdapterDisposition::Duplicate => "rbn.exact-duplicate-or-replay",
        AdapterDisposition::Conflict => "rbn.replay-conflict",
        _ => match row_reason {
            RbnRowReason::Accepted => "rbn.accepted",
            RbnRowReason::InvalidValue => "rbn.invalid-value",
            RbnRowReason::CallsignFiltered => "rbn.callsign-filtered",
            RbnRowReason::TimeFiltered => "rbn.time-filtered",
            RbnRowReason::BandFiltered => "rbn.band-filtered",
            RbnRowReason::UnsupportedBand => "rbn.unsupported-band",
            RbnRowReason::UnsupportedMode => "rbn.unsupported-mode",
            RbnRowReason::ExactDuplicate => "rbn.exact-duplicate-or-replay",
        },
    };
    reason(value)
}

fn reason(value: &str) -> AdapterReasonId {
    AdapterReasonId::new(value).expect("static RBN reason")
}

#[allow(clippy::too_many_arguments)]
fn rbn_observation(
    spot: &RbnSpot,
    row: &RbnRowResult,
    session_id: &str,
    mutation_id: &str,
    adapter_id: &str,
    observation_id: String,
    captured_at: DateTime<Utc>,
    provenance: &Provenance,
) -> ObservationRecordV3 {
    ObservationRecordV3 {
        meta: import_meta(session_id, mutation_id, 0, captured_at, provenance),
        observation_id,
        adapter_record_ids: vec![adapter_id.into()],
        observation_kind: ObservationKind::PublicReport,
        band: spot.band,
        frequency_hz: Some(spot.frequency_hz),
        mode: Some(
            match spot.mode {
                antennabench_core::v3::SignalModeV3::Cw => "CW",
                antennabench_core::v3::SignalModeV3::Rtty => "RTTY",
            }
            .into(),
        ),
        reporter_call: Some(spot.reporter_call.clone()),
        heard_call: Some(spot.heard_call.clone()),
        reporter_grid: None,
        heard_grid: None,
        distance_km: None,
        azimuth_degrees: None,
        snr_db: Some(spot.snr_db),
        drift_hz_per_minute: None,
        power_watts: None,
        slot_id: None,
        slot_label: None,
        slot_confidence: None,
        raw: serde_json::json!({
            "provider": RBN_PROVIDER_ID,
            "source": RBN_SOURCE_ID,
            "row_number": row.row_number,
            "key_speed_wpm": spot.key_speed_wpm,
            "fields": row.raw_fields,
        }),
    }
}
