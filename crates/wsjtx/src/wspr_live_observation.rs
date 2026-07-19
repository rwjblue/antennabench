use antennabench_core::{
    v2::{MutationMember, ObservationRecordV2, Provenance, RecordMetaV2},
    v3::WsprCycleDirection,
    ObservationKind, SCHEMA_VERSION_V2,
};
use chrono::{DateTime, Utc};

use crate::{
    wspr_live_alignment::matching_confirmed_cycle, WsprLiveConfirmedCycle, WsprLiveSpot,
    WsprLiveSpotDirection, WSPR_LIVE_PROVIDER_ID, WSPR_LIVE_SOURCE_ID,
};

pub(super) fn import_meta(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn wspr_live_observation(
    spot: &WsprLiveSpot,
    session_id: &str,
    mutation_id: &str,
    adapter_id: &str,
    observation_id: String,
    captured_at: DateTime<Utc>,
    provenance: &Provenance,
    confirmed_cycle: Option<&WsprLiveConfirmedCycle>,
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
        slot_id: confirmed_cycle.map(|cycle| cycle.slot_id.clone()),
        slot_label: confirmed_cycle.map(|cycle| cycle.antenna_label.clone()),
        slot_confidence: confirmed_cycle.map(|_| 0.95),
        raw: serde_json::json!({
            "provider_spot_id": spot.provider_spot_id,
            "provider": WSPR_LIVE_PROVIDER_ID,
            "source": WSPR_LIVE_SOURCE_ID,
            "tx_azimuth_degrees": spot.azimuth_degrees,
            "rx_azimuth_degrees": spot.receiver_azimuth_degrees,
            "receiver_version": spot.receiver_version,
            "mode_code": spot.mode_code,
            "source_observed_at": spot.observed_at,
            "captured_at": captured_at,
            "direction": match spot.direction {
                WsprLiveSpotDirection::Receive => "receive",
                WsprLiveSpotDirection::Transmit => "transmit",
            },
        }),
    }
}

pub(super) fn matching_cycle<'a>(
    confirmed_cycles: Option<&'a [WsprLiveConfirmedCycle]>,
    spot: &WsprLiveSpot,
) -> Option<&'a WsprLiveConfirmedCycle> {
    let direction = match spot.direction {
        WsprLiveSpotDirection::Receive => WsprCycleDirection::Receive,
        WsprLiveSpotDirection::Transmit => WsprCycleDirection::Transmit,
    };
    confirmed_cycles
        .and_then(|cycles| matching_confirmed_cycle(cycles, spot.observed_at, spot.band, direction))
}
