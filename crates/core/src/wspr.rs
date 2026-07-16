use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const WSPR_CYCLE_SECONDS: i64 = 120;
pub const WSPR_NOMINAL_START_OFFSET_SECONDS: i64 = 1;
pub const WSPR_SYMBOL_COUNT: i64 = 162;
pub const WSPR_SYMBOL_DURATION_NUMERATOR: i64 = 8_192;
pub const WSPR_SYMBOL_DURATION_DENOMINATOR: i64 = 12_000;
pub const WSPR_TRANSMISSION_MILLISECONDS: i64 =
    WSPR_SYMBOL_COUNT * WSPR_SYMBOL_DURATION_NUMERATOR * 1_000 / WSPR_SYMBOL_DURATION_DENOMINATOR;

const WSPR_CYCLE_MILLISECONDS: i64 = WSPR_CYCLE_SECONDS * 1_000;
const WSPR_START_OFFSET_MILLISECONDS: i64 = WSPR_NOMINAL_START_OFFSET_SECONDS * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsprCycleWindow {
    pub starts_at: DateTime<Utc>,
    pub transmission_ends_at: DateTime<Utc>,
    pub next_cycle_starts_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WsprCycleTimingError {
    #[error("WSPR cycle start must be exactly one second into an even UTC minute")]
    MisalignedStart,
    #[error("minimum lead time cannot be negative")]
    NegativeLeadTime,
    #[error("WSPR cycle calculation exceeded the supported timestamp range")]
    TimestampOverflow,
}

impl WsprCycleWindow {
    pub fn from_start(starts_at: DateTime<Utc>) -> Result<Self, WsprCycleTimingError> {
        if !is_wspr_cycle_start(starts_at) {
            return Err(WsprCycleTimingError::MisalignedStart);
        }
        let transmission_ends_at = starts_at
            .checked_add_signed(Duration::milliseconds(WSPR_TRANSMISSION_MILLISECONDS))
            .ok_or(WsprCycleTimingError::TimestampOverflow)?;
        let next_cycle_starts_at = starts_at
            .checked_add_signed(Duration::seconds(WSPR_CYCLE_SECONDS))
            .ok_or(WsprCycleTimingError::TimestampOverflow)?;
        Ok(Self {
            starts_at,
            transmission_ends_at,
            next_cycle_starts_at,
        })
    }

    pub fn switching_time(&self) -> Duration {
        self.next_cycle_starts_at - self.transmission_ends_at
    }
}

pub fn is_wspr_cycle_start(timestamp: DateTime<Utc>) -> bool {
    timestamp.nanosecond() == 0
        && timestamp.timestamp().rem_euclid(WSPR_CYCLE_SECONDS) == WSPR_NOMINAL_START_OFFSET_SECONDS
}

pub fn next_wspr_cycle_at_or_after(
    not_before: DateTime<Utc>,
) -> Result<WsprCycleWindow, WsprCycleTimingError> {
    let timestamp_millis = not_before.timestamp_millis();
    let shifted = timestamp_millis
        .checked_sub(WSPR_START_OFFSET_MILLISECONDS)
        .ok_or(WsprCycleTimingError::TimestampOverflow)?;
    let cycle_index = shifted.div_euclid(WSPR_CYCLE_MILLISECONDS);
    let mut candidate_millis = cycle_index
        .checked_mul(WSPR_CYCLE_MILLISECONDS)
        .and_then(|value| value.checked_add(WSPR_START_OFFSET_MILLISECONDS))
        .ok_or(WsprCycleTimingError::TimestampOverflow)?;
    if candidate_millis < timestamp_millis {
        candidate_millis = candidate_millis
            .checked_add(WSPR_CYCLE_MILLISECONDS)
            .ok_or(WsprCycleTimingError::TimestampOverflow)?;
    }
    let starts_at = DateTime::from_timestamp_millis(candidate_millis)
        .ok_or(WsprCycleTimingError::TimestampOverflow)?;
    WsprCycleWindow::from_start(starts_at)
}

pub fn next_wspr_cycle_after_ready(
    ready_at: DateTime<Utc>,
    minimum_lead_time: Duration,
) -> Result<WsprCycleWindow, WsprCycleTimingError> {
    if minimum_lead_time < Duration::zero() {
        return Err(WsprCycleTimingError::NegativeLeadTime);
    }
    let not_before = ready_at
        .checked_add_signed(minimum_lead_time)
        .ok_or(WsprCycleTimingError::TimestampOverflow)?;
    next_wspr_cycle_at_or_after(not_before)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn utc(hour: u32, minute: u32, second: u32, millis: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 15, hour, minute, second)
            .single()
            .unwrap()
            + Duration::milliseconds(i64::from(millis))
    }

    #[test]
    fn models_the_exact_wspr_transmission_and_switching_window() {
        assert_eq!(WSPR_TRANSMISSION_MILLISECONDS, 110_592);
        let cycle = WsprCycleWindow::from_start(utc(12, 0, 1, 0)).unwrap();
        assert_eq!(cycle.transmission_ends_at, utc(12, 1, 51, 592));
        assert_eq!(cycle.next_cycle_starts_at, utc(12, 2, 1, 0));
        assert_eq!(cycle.switching_time(), Duration::milliseconds(9_408));
    }

    #[test]
    fn recognizes_only_nominal_even_minute_starts() {
        assert!(is_wspr_cycle_start(utc(12, 0, 1, 0)));
        assert!(is_wspr_cycle_start(utc(12, 2, 1, 0)));
        assert!(!is_wspr_cycle_start(utc(12, 1, 1, 0)));
        assert!(!is_wspr_cycle_start(utc(12, 0, 0, 0)));
        assert!(!is_wspr_cycle_start(utc(12, 0, 1, 1)));
    }

    #[test]
    fn selects_the_first_cycle_at_or_after_the_boundary() {
        assert_eq!(
            next_wspr_cycle_at_or_after(utc(12, 0, 0, 999))
                .unwrap()
                .starts_at,
            utc(12, 0, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_at_or_after(utc(12, 0, 1, 0))
                .unwrap()
                .starts_at,
            utc(12, 0, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_at_or_after(utc(12, 0, 1, 1))
                .unwrap()
                .starts_at,
            utc(12, 2, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_at_or_after(utc(12, 1, 59, 999))
                .unwrap()
                .starts_at,
            utc(12, 2, 1, 0)
        );
    }

    #[test]
    fn readiness_lead_time_safely_rolls_to_a_later_cycle() {
        assert_eq!(
            next_wspr_cycle_after_ready(utc(12, 0, 0, 500), Duration::milliseconds(250))
                .unwrap()
                .starts_at,
            utc(12, 0, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_after_ready(utc(12, 0, 0, 500), Duration::seconds(1))
                .unwrap()
                .starts_at,
            utc(12, 2, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_after_ready(utc(12, 0, 59, 0), Duration::seconds(15))
                .unwrap()
                .starts_at,
            utc(12, 2, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_after_ready(utc(12, 2, 1, 1), Duration::zero())
                .unwrap()
                .starts_at,
            utc(12, 4, 1, 0)
        );
        assert_eq!(
            next_wspr_cycle_after_ready(utc(12, 0, 0, 0), Duration::milliseconds(-1)),
            Err(WsprCycleTimingError::NegativeLeadTime)
        );
    }
}
