use std::fmt::Write as _;

use crate::{
    GeographicProfileAnswerability, ObservedReachAnswerability, PairedDetectabilityAnswerability,
    RepeatabilityAnswerability, ReportAzimuthSector, ReportDistanceBin, ReportLifecycleEventKind,
    ReportObservedAntennaProfile, ReportObservedProfileCell, ReportOperatorEvent,
    ReportOperatorEventKind, ReportOverviewLifecycleState, ReportOverviewLimitation,
    ReportOverviewLocationCell, ReportOverviewPathDelta, ReportOverviewReach,
    ReportOverviewStratum, ReportPathLocationAvailability, ReportQuestionFamily,
    ReportRunTimelineRow, ReportStratumAvailability, SamePathSignalAnswerability, SessionReport,
};
use antennabench_core::AlignedSlotStatus;

use super::{audit::*, evidence::*, shared::*};

mod activity;
mod coverage;
mod location;
mod overlap;
mod overview;
mod paths;
mod quality;

pub(super) use activity::{coverage_text, render_reporter_activity_section};
pub(super) use coverage::{render_compact_coverage_map_section, render_coverage_map_section};

pub(super) use location::{render_compact_observed_footprint_section, render_distance_section};
pub(super) use overlap::{
    render_compact_overlap_repeatability_section, render_overlap_repeatability_section,
};
pub(super) use overview::{
    overview_lifecycle_label, render_answer_first_overview,
    render_answer_first_overview_with_reference, render_how_to_read, render_question_navigation,
};
pub(super) use paths::{
    plural_suffix, render_reach_bar, render_reach_section, render_same_path_section,
    render_same_path_stratum,
};
pub(super) use quality::render_run_quality_section;

pub(super) fn ordered_question_families(report: &SessionReport) -> Vec<ReportQuestionFamily> {
    let priority = report
        .overview
        .goal_lens
        .as_ref()
        .map(|lens| lens.priority.as_slice())
        .unwrap_or(&[
            ReportQuestionFamily::SharedPathSignal,
            ReportQuestionFamily::CommonOpportunityDetection,
            ReportQuestionFamily::ObservedReach,
            ReportQuestionFamily::Repeatability,
            ReportQuestionFamily::GeographicProfile,
        ]);
    priority
        .iter()
        .copied()
        .filter(|family| question_family_has_content(report, *family))
        .collect()
}

pub(super) fn question_family_is_primary_available(
    report: &SessionReport,
    family: ReportQuestionFamily,
) -> bool {
    match family {
        ReportQuestionFamily::SharedPathSignal => {
            report.overview.answerability.same_path_signal == SamePathSignalAnswerability::Available
        }
        ReportQuestionFamily::CommonOpportunityDetection => {
            report.overview.answerability.paired_detectability
                == PairedDetectabilityAnswerability::Available
        }
        ReportQuestionFamily::ObservedReach => {
            report.overview.answerability.observed_reach == ObservedReachAnswerability::Available
        }
        ReportQuestionFamily::GeographicProfile => {
            report.overview.answerability.geographic_profile
                == GeographicProfileAnswerability::Available
        }
        ReportQuestionFamily::Repeatability => {
            report.overview.answerability.repeatability == RepeatabilityAnswerability::Available
        }
    }
}

fn question_family_has_content(report: &SessionReport, family: ReportQuestionFamily) -> bool {
    question_family_is_primary_available(report, family)
        || (family == ReportQuestionFamily::Repeatability && !report.coverage_overlap.is_empty())
}

pub(super) fn is_single_antenna_lens(report: &SessionReport) -> bool {
    report
        .overview
        .goal_lens
        .as_ref()
        .is_some_and(|lens| lens.goal == antennabench_core::SessionGoal::SingleAntennaProfiling)
}
