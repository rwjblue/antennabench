use super::*;
use crate::{
    html::{
        templates::{render_template, OverlapQuestionTemplate},
        view::{
            ObservedOverlapView, OpportunityOrderView, OpportunityOverlapView, OverlapGroupView,
            OverlapQuestionView, ReceiverFrequencyView, RepeatabilityDistributionView,
            RepeatabilityPathView, RepeatabilityView,
        },
    },
    ReportAntennaRepeatability, ReportCoverageOverlapGroup, ReportObservedComplementarity,
};

pub(in super::super) fn render_overlap_repeatability_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &OverlapQuestionTemplate {
            view: overlap_view(report, false),
        },
    )
}

pub(in super::super) fn render_compact_repeatability_disclosure(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &OverlapQuestionTemplate {
            view: overlap_view(report, true),
        },
    )
}

fn overlap_view(report: &SessionReport, compact: bool) -> OverlapQuestionView {
    let render = !compact
        || report
            .coverage_overlap
            .iter()
            .any(|group| group.observed.is_some());
    OverlapQuestionView {
        compact,
        render,
        no_groups: report.coverage_overlap.is_empty(),
        groups: report
            .coverage_overlap
            .iter()
            .enumerate()
            .filter(|(_, group)| !compact || group.observed.is_some())
            .map(|(index, group)| group_view(group, index, !compact))
            .collect(),
    }
}

fn group_view(
    group: &ReportCoverageOverlapGroup,
    index: usize,
    include_audit: bool,
) -> OverlapGroupView {
    OverlapGroupView {
        index,
        label: comparison_group_label(&group.stratum),
        observed: group
            .observed
            .as_ref()
            .map(|observed| observed_view(observed, true)),
        common: include_audit
            .then(|| group.common_opportunity.as_ref().map(common_view))
            .flatten(),
    }
}

fn observed_view(
    observed: &ReportObservedComplementarity,
    include_audit: bool,
) -> ObservedOverlapView {
    let left_label = observed.left.as_ref().map_or_else(
        || "First antenna".into(),
        |profile| profile.antenna_label.clone(),
    );
    let right_label = observed.right.as_ref().map_or_else(
        || "Second antenna".into(),
        |profile| profile.antenna_label.clone(),
    );
    ObservedOverlapView {
        left_label,
        right_label,
        total: observed.total_system_unique_path_count,
        left_only: observed.left_only_unique_path_count,
        shared: observed.shared_unique_path_count,
        right_only: observed.right_only_unique_path_count,
        incremental_left: observed.incremental_left_path_count,
        incremental_right: observed.incremental_right_path_count,
        eligible_blocks: observed.eligible_block_count,
        block_suffix: plural_suffix(observed.eligible_block_count),
        repeatability: [&observed.left, &observed.right]
            .into_iter()
            .flatten()
            .map(|profile| repeatability_view(profile, include_audit))
            .collect(),
    }
}

fn repeatability_view(
    profile: &ReportAntennaRepeatability,
    include_audit: bool,
) -> RepeatabilityView {
    RepeatabilityView {
        antenna: profile.antenna_label.clone(),
        unique_paths: profile.unique_endpoint_count,
        path_suffix: plural_suffix(profile.unique_endpoint_count),
        path_blocks: profile.path_block_observation_count,
        observation_suffix: plural_suffix(profile.path_block_observation_count),
        once: profile.observed_once_path_count,
        repeated: profile.repeated_path_count,
        distribution: profile
            .block_count_distribution
            .iter()
            .map(|row| RepeatabilityDistributionView {
                blocks: row.observed_block_count,
                paths: row.unique_path_count,
            })
            .collect(),
        paths: if include_audit {
            profile
                .paths
                .iter()
                .map(|path| RepeatabilityPathView {
                    remote_path: path.remote_path.clone(),
                    blocks: path.observed_block_count,
                    observations: path.observation_count,
                    left_then_right: path.left_then_right_block_count,
                    right_then_left: path.right_then_left_block_count,
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

fn common_view(common: &crate::ReportOpportunityComplementarity) -> OpportunityOverlapView {
    OpportunityOverlapView {
        coverage: coverage_text(common.coverage),
        known_blocks: common.known_coverage_block_count,
        eligible_blocks: common.eligible_block_count,
        opportunities: common.receiver_block_opportunity_count,
        receivers: common.unique_common_active_receiver_count,
        coverage_known: common.coverage.is_known(),
        left_only: common.left_only_count,
        both: common.heard_both_count,
        right_only: common.right_only_count,
        neither: common.heard_neither_count,
        orders: common
            .order_summaries
            .iter()
            .map(|row| OpportunityOrderView {
                order: order_label(row.order),
                blocks: row.block_count,
                opportunities: row.receiver_block_opportunity_count,
                left_only: row.left_only_count,
                both: row.heard_both_count,
                right_only: row.right_only_count,
                neither: row.heard_neither_count,
            })
            .collect(),
        receiver_frequencies: common
            .receiver_frequencies
            .iter()
            .map(|row| ReceiverFrequencyView {
                receiver: row.receiver.clone(),
                opportunities: row.opportunity_count,
                left_detections: row.left_detection_count,
                right_detections: row.right_detection_count,
                left_then_right: row.left_then_right_opportunity_count,
                right_then_left: row.right_then_left_opportunity_count,
            })
            .collect(),
    }
}

fn order_label(order: antennabench_analysis::ComparisonOrder) -> &'static str {
    match order {
        antennabench_analysis::ComparisonOrder::LeftThenRight => "First → second",
        antennabench_analysis::ComparisonOrder::RightThenLeft => "Second → first",
    }
}
