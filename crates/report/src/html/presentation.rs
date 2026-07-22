use antennabench_analysis::ComparisonStratum;

use crate::{ReportOverviewReach, SessionReport};

use super::{
    geometry::geometry_class,
    shared::{band, observation_kind, path_direction, record_source},
    view::{ReachBarView, ReachSegmentView},
};

/// The two comparison labels shared by full and summary presentation builders.
/// These are display facts only; the type carries no report or audit access.
pub(super) struct AntennaLabels {
    pub(super) left: String,
    pub(super) right: String,
}

pub(super) fn antenna_labels(report: &SessionReport) -> AntennaLabels {
    AntennaLabels {
        left: report
            .comparison
            .left_label
            .clone()
            .unwrap_or_else(|| "Left".into()),
        right: report
            .comparison
            .right_label
            .clone()
            .unwrap_or_else(|| "Right".into()),
    }
}

pub(super) fn comparison_group_label(value: &ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        value.mode.as_str(),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}

/// Identical observed-reach display facts shared by the full reach section and
/// summary footprint. Consumer-specific prose and omission policy stay outside
/// this value.
pub(super) struct ReachPresentation {
    pub(super) left_only: usize,
    pub(super) both: usize,
    pub(super) right_only: usize,
    pub(super) left_total: usize,
    pub(super) right_total: usize,
    pub(super) universe: usize,
    pub(super) bar: ReachBarView,
}

pub(super) fn reach_presentation(
    reach: &ReportOverviewReach,
    bar_class: &str,
) -> ReachPresentation {
    let counts = [
        (reach.left_only_unique_path_count, "left"),
        (reach.both_unique_path_count, "both"),
        (reach.right_only_unique_path_count, "right"),
    ];
    let universe = counts.iter().map(|(count, _)| count).sum::<usize>();
    let denominator = universe.max(1) as f64;
    ReachPresentation {
        left_only: reach.left_only_unique_path_count,
        both: reach.both_unique_path_count,
        right_only: reach.right_only_unique_path_count,
        left_total: reach.left_only_unique_path_count + reach.both_unique_path_count,
        right_total: reach.right_only_unique_path_count + reach.both_unique_path_count,
        universe,
        bar: ReachBarView {
            class: bar_class.to_string(),
            segments: counts
                .into_iter()
                .filter(|(count, _)| *count > 0)
                .map(|(count, side)| ReachSegmentView {
                    side,
                    geometry_class: geometry_class(count as f64 / denominator * 100.0),
                })
                .collect(),
        },
    }
}
