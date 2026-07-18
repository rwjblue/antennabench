use antennabench_core::{
    is_wspr_cycle_start, v3::WsprCycleDirection, Band, WSPR_NOMINAL_START_OFFSET_SECONDS,
};
use chrono::{DateTime, Duration, Utc};

use crate::WsprLiveConfirmedCycle;

pub(crate) fn wspr_live_query_window_start(starts_at: DateTime<Utc>) -> DateTime<Utc> {
    if !is_wspr_cycle_start(starts_at) {
        return starts_at;
    }
    starts_at
        .checked_sub_signed(Duration::seconds(WSPR_NOMINAL_START_OFFSET_SECONDS))
        .unwrap_or(starts_at)
}

pub(crate) fn matching_confirmed_cycle(
    cycles: &[WsprLiveConfirmedCycle],
    observed_at: DateTime<Utc>,
    band: Band,
    expected_direction: WsprCycleDirection,
) -> Option<&WsprLiveConfirmedCycle> {
    cycles.iter().find(|cycle| {
        cycle.band == band
            && wspr_live_query_window_start(cycle.starts_at) <= observed_at
            && observed_at < cycle.transmission_ends_at
            && cycle.direction == Some(expected_direction)
    })
}
