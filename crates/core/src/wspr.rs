use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Band, OperatorEventPayloadV3, OperatorEventV3, ScheduleV3};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntennaOccupancyIntervalV3 {
    pub antenna_label: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub ready_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmedWsprCycleV3 {
    pub intent_id: String,
    pub sequence_number: u32,
    pub band: Band,
    pub antenna_label: String,
    pub ready_at: DateTime<Utc>,
    pub ready_event_id: String,
    pub window: WsprCycleWindow,
    pub occupancy_fully_covers_transmission: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprRunDiagnosticV3 {
    pub code: &'static str,
    pub event_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WsprRunProjectionV3 {
    pub occupancies: Vec<AntennaOccupancyIntervalV3>,
    pub cycles: Vec<ArmedWsprCycleV3>,
    pub diagnostics: Vec<WsprRunDiagnosticV3>,
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

pub fn project_wspr_run_v3(
    schedule: &ScheduleV3,
    events: &[OperatorEventV3],
) -> WsprRunProjectionV3 {
    let mut projection = WsprRunProjectionV3::default();
    let mut open_occupancy: Option<AntennaOccupancyIntervalV3> = None;
    let mut seen_intents = std::collections::BTreeSet::new();

    for event in events {
        let closes_occupancy = matches!(
            event.payload,
            OperatorEventPayloadV3::AntennaSwitchStarted { .. }
                | OperatorEventPayloadV3::SessionInterrupted { .. }
                | OperatorEventPayloadV3::InterruptionDetected { .. }
                | OperatorEventPayloadV3::SessionEnded { .. }
                | OperatorEventPayloadV3::SessionAbandoned { .. }
        );
        if closes_occupancy {
            close_occupancy(
                &mut open_occupancy,
                event.occurred_at,
                &event.event_id,
                &mut projection,
            );
            continue;
        }

        let OperatorEventPayloadV3::WsprCycleArmed {
            antenna_label,
            cycle_starts_at,
        } = &event.payload
        else {
            continue;
        };
        close_occupancy(
            &mut open_occupancy,
            event.occurred_at,
            &event.event_id,
            &mut projection,
        );
        open_occupancy = Some(AntennaOccupancyIntervalV3 {
            antenna_label: antenna_label.clone(),
            starts_at: event.occurred_at,
            ends_at: None,
            ready_event_id: event.event_id.clone(),
        });

        let Some(intent_id) = event.slot_id.as_deref() else {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.missing_intent",
                event_id: event.event_id.clone(),
                message: "armed WSPR cycle does not identify a cycle intent".into(),
            });
            continue;
        };
        let Some(intent) = schedule
            .wspr_cycle_intents
            .iter()
            .find(|intent| intent.intent_id == intent_id)
        else {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.unknown_intent",
                event_id: event.event_id.clone(),
                message: format!("armed WSPR cycle references unknown intent {intent_id:?}"),
            });
            continue;
        };
        if seen_intents.contains(intent_id) {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.intent_reused",
                event_id: event.event_id.clone(),
                message: format!("cycle intent {intent_id:?} was armed more than once"),
            });
            continue;
        }
        if schedule
            .wspr_cycle_intents
            .get(projection.cycles.len())
            .is_some_and(|expected| expected.intent_id != intent_id)
        {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.intent_out_of_order",
                event_id: event.event_id.clone(),
                message: format!(
                    "cycle intent {intent_id:?} was armed before the preceding intended cycles"
                ),
            });
            continue;
        }
        seen_intents.insert(intent_id.to_string());
        let Ok(window) = WsprCycleWindow::from_start(*cycle_starts_at) else {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.invalid_cycle_start",
                event_id: event.event_id.clone(),
                message: "armed cycle is not aligned to the WSPR protocol".into(),
            });
            continue;
        };
        if window.starts_at < event.occurred_at {
            projection.diagnostics.push(WsprRunDiagnosticV3 {
                code: "wspr_run.cycle_before_ready",
                event_id: event.event_id.clone(),
                message: "armed cycle starts before the antenna readiness action".into(),
            });
            continue;
        }
        projection.cycles.push(ArmedWsprCycleV3 {
            intent_id: intent.intent_id.clone(),
            sequence_number: intent.sequence_number,
            band: intent.band,
            antenna_label: antenna_label.clone(),
            ready_at: event.occurred_at,
            ready_event_id: event.event_id.clone(),
            window,
            occupancy_fully_covers_transmission: false,
        });
    }
    if let Some(interval) = open_occupancy {
        projection.occupancies.push(interval);
    }

    for cycle in &mut projection.cycles {
        cycle.occupancy_fully_covers_transmission = projection.occupancies.iter().any(|interval| {
            interval.ready_event_id == cycle.ready_event_id
                && interval.antenna_label == cycle.antenna_label
                && interval.starts_at <= cycle.window.starts_at
                && interval
                    .ends_at
                    .is_none_or(|ends_at| ends_at >= cycle.window.transmission_ends_at)
        });
    }
    projection
}

fn close_occupancy(
    open: &mut Option<AntennaOccupancyIntervalV3>,
    ends_at: DateTime<Utc>,
    event_id: &str,
    projection: &mut WsprRunProjectionV3,
) {
    let Some(mut interval) = open.take() else {
        return;
    };
    if ends_at < interval.starts_at {
        projection.diagnostics.push(WsprRunDiagnosticV3 {
            code: "wspr_run.non_monotonic_action",
            event_id: event_id.to_string(),
            message: "operator action time moved backwards across an antenna interval".into(),
        });
        interval.ends_at = Some(interval.starts_at);
    } else {
        interval.ends_at = Some(ends_at);
    }
    projection.occupancies.push(interval);
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::{
        ExperimentMode, MutationMember, OperatorEventV3, Provenance, RecordMetaV3, RecordSource,
        ScheduleV3, SessionGoal, WsprCycleIntentV3, SCHEMA_VERSION_V3,
    };

    fn utc(hour: u32, minute: u32, second: u32, millis: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 15, hour, minute, second)
            .single()
            .unwrap()
            + Duration::milliseconds(i64::from(millis))
    }

    fn event(
        id: &str,
        occurred_at: DateTime<Utc>,
        intent_id: Option<&str>,
        payload: OperatorEventPayloadV3,
    ) -> OperatorEventV3 {
        OperatorEventV3 {
            meta: RecordMetaV3 {
                schema_version: SCHEMA_VERSION_V3,
                session_id: "session".into(),
                recorded_at: occurred_at,
                provenance: Provenance::from_legacy(RecordSource::Operator, "test"),
                mutation: MutationMember {
                    mutation_id: format!("mutation-{id}"),
                    member_index: 0,
                    member_count: 1,
                },
            },
            event_id: id.into(),
            occurred_at,
            time_basis: crate::EventTimeBasisV2::ObservedNow,
            uncertainty_seconds: None,
            slot_id: intent_id.map(str::to_string),
            payload,
        }
    }

    fn run_schedule() -> ScheduleV3 {
        ScheduleV3 {
            schema_version: SCHEMA_VERSION_V3,
            session_id: "session".into(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            signal_plans: Vec::new(),
            wspr_cycle_intents: vec![
                WsprCycleIntentV3 {
                    intent_id: "intent-a".into(),
                    sequence_number: 1,
                    band: Band::M20,
                    antenna_label: "A".into(),
                    signal: None,
                },
                WsprCycleIntentV3 {
                    intent_id: "intent-b".into(),
                    sequence_number: 2,
                    band: Band::M20,
                    antenna_label: "B".into(),
                    signal: None,
                },
            ],
            slots: Vec::new(),
        }
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

    #[test]
    fn derives_half_open_occupancy_and_only_accepts_fully_covered_transmissions() {
        let events = vec![
            event(
                "ready-a",
                utc(12, 0, 0, 0),
                Some("intent-a"),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "A".into(),
                    cycle_starts_at: utc(12, 0, 1, 0),
                },
            ),
            event(
                "switch-a",
                utc(12, 1, 51, 592),
                None,
                OperatorEventPayloadV3::AntennaSwitchStarted { note: None },
            ),
            event(
                "ready-b",
                utc(12, 2, 0, 0),
                Some("intent-b"),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "B".into(),
                    cycle_starts_at: utc(12, 2, 1, 0),
                },
            ),
            event(
                "switch-b-early",
                utc(12, 3, 0, 0),
                None,
                OperatorEventPayloadV3::AntennaSwitchStarted { note: None },
            ),
        ];

        let projection = project_wspr_run_v3(&run_schedule(), &events);
        assert!(projection.diagnostics.is_empty());
        assert_eq!(projection.occupancies.len(), 2);
        assert_eq!(projection.occupancies[0].ends_at, Some(utc(12, 1, 51, 592)));
        assert!(projection.cycles[0].occupancy_fully_covers_transmission);
        assert!(!projection.cycles[1].occupancy_fully_covers_transmission);
    }

    #[test]
    fn interruption_closes_occupancy_and_reused_intents_are_rejected() {
        let events = vec![
            event(
                "ready-a",
                utc(12, 0, 0, 0),
                Some("intent-a"),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "A".into(),
                    cycle_starts_at: utc(12, 0, 1, 0),
                },
            ),
            event(
                "interrupted",
                utc(12, 0, 30, 0),
                None,
                OperatorEventPayloadV3::SessionInterrupted { reason: None },
            ),
            event(
                "ready-a-again",
                utc(12, 2, 0, 0),
                Some("intent-a"),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "A".into(),
                    cycle_starts_at: utc(12, 2, 1, 0),
                },
            ),
        ];

        let projection = project_wspr_run_v3(&run_schedule(), &events);
        assert!(!projection.cycles[0].occupancy_fully_covers_transmission);
        assert!(projection
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "wspr_run.intent_reused"));
    }
}
