use std::collections::BTreeMap;

use chrono::Duration;

use crate::{
    project_wspr_run_v3, AdapterDisposition, AdapterRecordV2, NormalizedRecordKind,
    ObservationRecordV2, OperatorEventV3, ScheduleV3, WsprCycleDirection, SCHEMA_VERSION_V4,
    WSPR_NOMINAL_START_OFFSET_SECONDS,
};

const WSPR_LIVE_PROVIDER_ID: &str = "wspr-live";
const WSPR_LIVE_SPOT_RECORD_TYPE: &str = "wspr_live_spot";

pub(crate) fn repair_confirmed_wspr_live_observations(
    schema_version: u16,
    schedule: &ScheduleV3,
    events: &[OperatorEventV3],
    observations: &mut [ObservationRecordV2],
    adapter_records: &[AdapterRecordV2],
) {
    if schema_version < SCHEMA_VERSION_V4 {
        return;
    }

    let directions = schedule
        .wspr_cycle_intents
        .iter()
        .map(|intent| (intent.intent_id.as_str(), intent.direction))
        .collect::<BTreeMap<_, _>>();
    let projection = project_wspr_run_v3(schedule, events);
    let source_times = adapter_records
        .iter()
        .filter(|record| {
            record.disposition == AdapterDisposition::Accepted
                && record.record_type == WSPR_LIVE_SPOT_RECORD_TYPE
                && record.meta.provenance.provider_id.as_str() == WSPR_LIVE_PROVIDER_ID
        })
        .filter_map(|record| {
            let source_time = record.source_time?;
            record
                .normalized_records
                .iter()
                .filter(|link| link.record_kind == NormalizedRecordKind::Observation)
                .map(move |link| (link.record_id.as_str(), source_time))
                .next()
        })
        .collect::<BTreeMap<_, _>>();

    for observation in observations {
        let Some(source_time) = source_times
            .get(observation.observation_id.as_str())
            .copied()
        else {
            continue;
        };
        let Some(direction) = observation
            .raw
            .get("direction")
            .and_then(serde_json::Value::as_str)
            .and_then(wspr_direction)
        else {
            continue;
        };
        let Some(cycle) = projection.cycles.iter().find(|cycle| {
            cycle.occupancy_fully_covers_transmission
                && cycle.band == observation.band
                && directions.get(cycle.intent_id.as_str()).copied().flatten() == Some(direction)
                && cycle
                    .window
                    .starts_at
                    .checked_sub_signed(Duration::seconds(WSPR_NOMINAL_START_OFFSET_SECONDS))
                    .is_some_and(|canonical_start| canonical_start <= source_time)
                && source_time < cycle.window.transmission_ends_at
        }) else {
            continue;
        };

        // The durable v2+ record keeps its trusted local capture time. The
        // compatibility projection uses the confirmed AntennaBench cycle start
        // as its scientific alignment timestamp while the linked adapter record
        // retains the provider's exact even-minute source time.
        observation.meta.recorded_at = cycle.window.starts_at;
        observation.slot_id = Some(cycle.intent_id.clone());
        observation.slot_label = Some(cycle.antenna_label.clone());
        observation.slot_confidence = Some(0.95);
    }
}

fn wspr_direction(value: &str) -> Option<WsprCycleDirection> {
    match value {
        "receive" => Some(WsprCycleDirection::Receive),
        "transmit" => Some(WsprCycleDirection::Transmit),
        _ => None,
    }
}
