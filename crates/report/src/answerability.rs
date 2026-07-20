use antennabench_analysis::{
    ComparisonAvailability, PairedComparisonAnalysis, ReporterActivityAnalysis,
    ReporterActivityCoverage, ReporterActivityUnknownReason,
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
    let geographic_profile = if comparison
        .observed_path_profiles
        .iter()
        .any(|profile| profile.located_path_count > 0)
    {
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
