use std::collections::BTreeSet;

use antennabench_core::{
    v2::CurrentBundleContents,
    v3::{BundleV3Contents, ScheduleV3, WsprCycleDirection},
    ExperimentMode, ObservationKind, SCHEMA_VERSION_V4,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDirectionCoverage {
    TransmitOnly,
    ReceiveOnly,
    TransmitAndReceive,
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CatalogObservationCounts {
    pub total: usize,
    pub local_decodes: usize,
    pub public_spots: usize,
    pub imported_spots: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogExperimentSummary {
    pub planned_repetitions: Option<usize>,
    pub direction_coverage: Option<CatalogDirectionCoverage>,
    pub planned_cycle_count: Option<usize>,
    pub observation_counts: Option<CatalogObservationCounts>,
}

impl CatalogExperimentSummary {
    pub(crate) fn from_current(current: Option<&CurrentBundleContents>) -> Self {
        let Some(current) = current else {
            return Self::default();
        };
        Self {
            planned_repetitions: None,
            direction_coverage: Some(CatalogDirectionCoverage::Unknown),
            planned_cycle_count: Some(current.bundle.schedule.slots.len()),
            observation_counts: Some(count_observations(
                current
                    .bundle
                    .observations
                    .iter()
                    .map(|observation| observation.observation_kind),
            )),
        }
    }

    pub(crate) fn from_v3(bundle: &BundleV3Contents) -> Self {
        summary_from_native_schedule(
            bundle.manifest.schema_version,
            &bundle.schedule,
            bundle
                .observations
                .iter()
                .map(|observation| observation.observation_kind),
        )
    }
}

fn summary_from_native_schedule(
    schema_version: u16,
    schedule: &ScheduleV3,
    observations: impl Iterator<Item = ObservationKind>,
) -> CatalogExperimentSummary {
    let intents = &schedule.wspr_cycle_intents;
    let planned_cycle_count = if intents.is_empty() {
        schedule.slots.len()
    } else {
        intents.len()
    };
    let signal_blocks = intents
        .iter()
        .filter_map(|intent| intent.signal.as_ref())
        .map(|signal| signal.counterbalance_block_id.as_str())
        .collect::<BTreeSet<_>>();
    let planned_repetitions = if !signal_blocks.is_empty() {
        Some(signal_blocks.len())
    } else if intents.is_empty() {
        None
    } else {
        wspr_repetitions(schedule)
    };
    CatalogExperimentSummary {
        planned_repetitions,
        direction_coverage: Some(direction_coverage(schema_version, schedule)),
        planned_cycle_count: Some(planned_cycle_count),
        observation_counts: Some(count_observations(observations)),
    }
}

fn wspr_repetitions(schedule: &ScheduleV3) -> Option<usize> {
    let intents = &schedule.wspr_cycle_intents;
    let mut planned_shapes = Vec::new();
    for intent in intents {
        let shape = (intent.band, intent.antenna_label.as_str());
        if !planned_shapes.contains(&shape) {
            planned_shapes.push(shape);
        }
    }
    let directions_per_shape = match schedule.mode {
        ExperimentMode::WholeStationAb | ExperimentMode::SingleAntennaProfiling => 2,
        ExperimentMode::TxFocused | ExperimentMode::RxFocused => 1,
    };
    let cycles_per_repetition = planned_shapes.len().checked_mul(directions_per_shape)?;
    (cycles_per_repetition != 0 && intents.len().is_multiple_of(cycles_per_repetition))
        .then_some(intents.len() / cycles_per_repetition)
}

fn direction_coverage(schema_version: u16, schedule: &ScheduleV3) -> CatalogDirectionCoverage {
    if schema_version < SCHEMA_VERSION_V4 {
        return CatalogDirectionCoverage::Unknown;
    }
    let directions = schedule
        .wspr_cycle_intents
        .iter()
        .filter_map(|intent| intent.direction)
        .collect::<BTreeSet<_>>();
    match (
        directions.contains(&WsprCycleDirection::Transmit),
        directions.contains(&WsprCycleDirection::Receive),
    ) {
        (true, true) => CatalogDirectionCoverage::TransmitAndReceive,
        (true, false) => CatalogDirectionCoverage::TransmitOnly,
        (false, true) => CatalogDirectionCoverage::ReceiveOnly,
        (false, false) => CatalogDirectionCoverage::Unknown,
    }
}

fn count_observations(
    observations: impl Iterator<Item = ObservationKind>,
) -> CatalogObservationCounts {
    let mut counts = CatalogObservationCounts::default();
    for kind in observations {
        counts.total += 1;
        match kind {
            ObservationKind::LocalDecode => counts.local_decodes += 1,
            ObservationKind::PublicReport => counts.public_spots += 1,
            ObservationKind::ImportedSpot => counts.imported_spots += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use antennabench_core::{
        v3::{
            CounterbalanceBlockIdV3, ScheduleV3, SignalAllocationV3, SignalPlanIdV3,
            SignalVariantIdV3, WsprCycleIntentV3,
        },
        Band, ExperimentMode, SessionGoal, SCHEMA_VERSION_V3, SCHEMA_VERSION_V6,
    };

    use super::*;

    fn schedule(mode: ExperimentMode, directions: &[WsprCycleDirection]) -> ScheduleV3 {
        ScheduleV3 {
            schema_version: SCHEMA_VERSION_V6,
            session_id: "session".into(),
            mode,
            goal: SessionGoal::GeneralCoverage,
            antenna_control: None,
            signal_plans: Vec::new(),
            wspr_cycle_intents: directions
                .iter()
                .enumerate()
                .map(|(index, direction)| WsprCycleIntentV3 {
                    intent_id: format!("intent-{index}"),
                    sequence_number: u32::try_from(index + 1).unwrap(),
                    band: Band::M20,
                    antenna_label: "A".into(),
                    direction: Some(*direction),
                    signal: None,
                })
                .collect(),
            slots: Vec::new(),
        }
    }

    fn summary(schema_version: u16, schedule: &ScheduleV3) -> CatalogExperimentSummary {
        summary_from_native_schedule(schema_version, schedule, std::iter::empty())
    }

    #[test]
    fn current_wspr_modes_project_repetitions_and_direction_coverage() {
        let both = schedule(
            ExperimentMode::WholeStationAb,
            &[
                WsprCycleDirection::Receive,
                WsprCycleDirection::Transmit,
                WsprCycleDirection::Receive,
                WsprCycleDirection::Transmit,
            ],
        );
        let tx = schedule(
            ExperimentMode::TxFocused,
            &[WsprCycleDirection::Transmit, WsprCycleDirection::Transmit],
        );
        let rx = schedule(
            ExperimentMode::RxFocused,
            &[WsprCycleDirection::Receive, WsprCycleDirection::Receive],
        );
        let single = schedule(
            ExperimentMode::SingleAntennaProfiling,
            &[WsprCycleDirection::Receive, WsprCycleDirection::Transmit],
        );

        for (plan, repetitions, coverage) in [
            (&both, 2, CatalogDirectionCoverage::TransmitAndReceive),
            (&tx, 2, CatalogDirectionCoverage::TransmitOnly),
            (&rx, 2, CatalogDirectionCoverage::ReceiveOnly),
            (&single, 1, CatalogDirectionCoverage::TransmitAndReceive),
        ] {
            let projected = summary(SCHEMA_VERSION_V6, plan);
            assert_eq!(projected.planned_repetitions, Some(repetitions));
            assert_eq!(projected.direction_coverage, Some(coverage));
            assert_eq!(
                projected.planned_cycle_count,
                Some(plan.wspr_cycle_intents.len())
            );
        }
    }

    #[test]
    fn schema_v3_direction_is_unknown_even_when_a_value_is_present() {
        let plan = schedule(ExperimentMode::TxFocused, &[WsprCycleDirection::Transmit]);
        assert_eq!(
            summary(SCHEMA_VERSION_V3, &plan).direction_coverage,
            Some(CatalogDirectionCoverage::Unknown)
        );
    }

    #[test]
    fn controlled_signal_repetitions_are_counterbalance_blocks() {
        let mut plan = schedule(
            ExperimentMode::TxFocused,
            &[
                WsprCycleDirection::Transmit,
                WsprCycleDirection::Transmit,
                WsprCycleDirection::Transmit,
            ],
        );
        for (index, intent) in plan.wspr_cycle_intents.iter_mut().enumerate() {
            intent.signal = Some(SignalAllocationV3 {
                signal_plan_id: SignalPlanIdV3::new("primary").unwrap(),
                frequency_hz: 14_050_000,
                frequency_variant_id: SignalVariantIdV3::new("fixed").unwrap(),
                counterbalance_block_id: CounterbalanceBlockIdV3::new(format!(
                    "block-{}",
                    index.min(1) + 1
                ))
                .unwrap(),
                counterbalance_position: 0,
            });
        }
        assert_eq!(
            summary(SCHEMA_VERSION_V6, &plan).planned_repetitions,
            Some(2)
        );
    }

    #[test]
    fn observation_counts_include_zero_and_each_stored_kind() {
        assert_eq!(
            count_observations(std::iter::empty()),
            CatalogObservationCounts::default()
        );
        assert_eq!(
            count_observations(
                [
                    ObservationKind::LocalDecode,
                    ObservationKind::PublicReport,
                    ObservationKind::ImportedSpot,
                    ObservationKind::PublicReport,
                ]
                .into_iter()
            ),
            CatalogObservationCounts {
                total: 4,
                local_decodes: 1,
                public_spots: 2,
                imported_spots: 1,
            }
        );
    }
}
