use antennabench_analysis::{
    ComparisonAvailability, PairedComparisonAnalysis, PairedObservationRow,
    ReporterActivityAnalysis, ReporterActivityCoverage, ReporterActivityUnknownReason,
};

use crate::{
    GeographicProfileAnswerability, ObservedReachAnswerability, PairedDetectabilityAnswerability,
    RepeatabilityAnswerability, ReportQuestionAnswerability, SamePathSignalAnswerability,
};

pub(crate) fn build_question_answerability(
    comparison: &PairedComparisonAnalysis,
    reporter_activity: &ReporterActivityAnalysis,
) -> ReportQuestionAnswerability {
    let same_path_signal = match comparison.availability {
        ComparisonAvailability::DescriptivePairsAvailable => SamePathSignalAnswerability::Available,
        ComparisonAvailability::NoMatchedPaths
            if comparison.overlap_rows.iter().any(|row| {
                (row.missing_snr_left_count > 0 && row.right_finite_count > 0)
                    || (row.missing_snr_right_count > 0 && row.left_finite_count > 0)
            }) =>
        {
            SamePathSignalAnswerability::NoFiniteSnr
        }
        ComparisonAvailability::NoMatchedPaths => SamePathSignalAnswerability::NoMatchedPaths,
        ComparisonAvailability::NoEligibleBlocks => SamePathSignalAnswerability::NoEligibleBlocks,
        ComparisonAvailability::NotApplicable => SamePathSignalAnswerability::NotApplicable,
        ComparisonAvailability::UnsupportedComparisonShape => {
            SamePathSignalAnswerability::UnsupportedShape
        }
    };

    let paired_detectability = match comparison.availability {
        ComparisonAvailability::NotApplicable => PairedDetectabilityAnswerability::NotApplicable,
        ComparisonAvailability::UnsupportedComparisonShape => {
            PairedDetectabilityAnswerability::UnsupportedShape
        }
        ComparisonAvailability::NoEligibleBlocks => {
            PairedDetectabilityAnswerability::NoEligibleBlocks
        }
        ComparisonAvailability::NoMatchedPaths
        | ComparisonAvailability::DescriptivePairsAvailable => {
            let paired = &reporter_activity.paired_rates;
            if paired
                .iter()
                .any(|row| row.coverage.is_known() && row.active_in_both_count > 0)
            {
                PairedDetectabilityAnswerability::Available
            } else if paired.iter().any(|row| {
                matches!(
                    row.coverage,
                    ReporterActivityCoverage::Unknown(
                        ReporterActivityUnknownReason::UnsupportedReceiveDirection
                    )
                )
            }) {
                PairedDetectabilityAnswerability::UnsupportedDirection
            } else if paired.iter().any(|row| row.coverage.is_known()) {
                PairedDetectabilityAnswerability::NoCommonActiveReporters
            } else {
                PairedDetectabilityAnswerability::ActivityCoverageUnknown
            }
        }
    };

    let observed_reach = if comparison
        .overlap_rows
        .iter()
        .any(|row| row.left_finite_count > 0 || row.right_finite_count > 0)
    {
        ObservedReachAnswerability::Available
    } else {
        ObservedReachAnswerability::default()
    };
    let geographic_profile = if comparison.paired_rows.iter().any(paired_row_has_location) {
        GeographicProfileAnswerability::Available
    } else {
        GeographicProfileAnswerability::default()
    };
    let repeatability = if comparison.diagnostics.eligible_block_count >= 2 {
        RepeatabilityAnswerability::Available
    } else {
        RepeatabilityAnswerability::default()
    };

    ReportQuestionAnswerability {
        same_path_signal,
        paired_detectability,
        observed_reach,
        geographic_profile,
        repeatability,
    }
}

fn paired_row_has_location(row: &PairedObservationRow) -> bool {
    valid_grid(row.left_remote_grid.as_deref())
        && valid_grid(row.right_remote_grid.as_deref())
        && valid_distance(row.left_distance_km)
        && valid_distance(row.right_distance_km)
        && valid_azimuth(row.left_azimuth_degrees)
        && valid_azimuth(row.right_azimuth_degrees)
}

fn valid_grid(grid: Option<&str>) -> bool {
    grid.is_some_and(|grid| !grid.trim().is_empty())
}

fn valid_distance(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && value >= 0.0)
}

fn valid_azimuth(value: Option<f64>) -> bool {
    value.is_some_and(|value| value.is_finite() && (0.0..360.0).contains(&value))
}
