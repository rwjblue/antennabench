use std::collections::{BTreeMap, BTreeSet};

use antennabench_analysis::{
    ComparisonBlockEligibility, ComparisonOrder, ComparisonSide, ComparisonStratum,
    ObservedAntennaPathProfile, PairedComparisonAnalysis, ReporterActivityAnalysis,
    ReporterActivityJointOutcome,
};

use crate::{
    ReportAntennaRepeatability, ReportBlockCountFrequency, ReportCoverageOverlapGroup,
    ReportDetailFamily, ReportObservedComplementarity, ReportOpportunityComplementarity,
    ReportOpportunityOrderSummary, ReportPathRepeatability, ReportReceiverDetectionFrequency,
};

pub(crate) fn project_coverage_overlap(
    comparison: &PairedComparisonAnalysis,
    activity: &ReporterActivityAnalysis,
) -> Vec<ReportCoverageOverlapGroup> {
    let mut strata = Vec::<ComparisonStratum>::new();
    for stratum in comparison
        .observed_path_profiles
        .iter()
        .map(|profile| &profile.stratum)
        .chain(
            activity
                .joint_summaries
                .iter()
                .map(|summary| &summary.stratum),
        )
    {
        if !strata.contains(stratum) {
            strata.push(stratum.clone());
        }
    }
    strata
        .into_iter()
        .map(|stratum| ReportCoverageOverlapGroup {
            observed: project_observed(&stratum, comparison),
            common_opportunity: project_opportunities(&stratum, activity),
            stratum,
        })
        .collect()
}

pub(crate) fn overview_row_count(groups: &[ReportCoverageOverlapGroup]) -> usize {
    groups
        .iter()
        .map(|group| {
            1 + group
                .observed
                .iter()
                .flat_map(|observed| [&observed.left, &observed.right])
                .flatten()
                .map(|profile| 1 + profile.block_count_distribution.len())
                .sum::<usize>()
                + group
                    .common_opportunity
                    .as_ref()
                    .map_or(0, |common| 1 + common.order_summaries.len())
        })
        .sum()
}

pub(crate) fn audit_row_count(groups: &[ReportCoverageOverlapGroup]) -> usize {
    groups
        .iter()
        .map(|group| {
            group
                .observed
                .iter()
                .flat_map(|observed| [&observed.left, &observed.right])
                .flatten()
                .map(|profile| profile.paths.len())
                .sum::<usize>()
                + group
                    .common_opportunity
                    .as_ref()
                    .map_or(0, |common| common.receiver_frequencies.len())
        })
        .sum()
}

pub(crate) fn audit_detail(groups: &[ReportCoverageOverlapGroup]) -> (ReportDetailFamily, usize) {
    (
        ReportDetailFamily::CoverageOverlapAudit,
        audit_row_count(groups),
    )
}

pub(crate) fn clear_audit(groups: &mut [ReportCoverageOverlapGroup]) {
    for group in groups {
        if let Some(observed) = &mut group.observed {
            for profile in [&mut observed.left, &mut observed.right]
                .into_iter()
                .flatten()
            {
                profile.paths.clear();
            }
        }
        if let Some(common) = &mut group.common_opportunity {
            common.receiver_frequencies.clear();
        }
    }
}

fn project_observed(
    stratum: &ComparisonStratum,
    comparison: &PairedComparisonAnalysis,
) -> Option<ReportObservedComplementarity> {
    let left = profile(stratum, ComparisonSide::Left, comparison);
    let right = profile(stratum, ComparisonSide::Right, comparison);
    if left.is_none() && right.is_none() {
        return None;
    }
    let left_paths = path_identities(left);
    let right_paths = path_identities(right);
    let left_only = left_paths.difference(&right_paths).count();
    let shared = left_paths.intersection(&right_paths).count();
    let right_only = right_paths.difference(&left_paths).count();
    let eligible_blocks = comparison
        .blocks
        .iter()
        .filter(|block| {
            block.band == stratum.band && block.eligibility == ComparisonBlockEligibility::Eligible
        })
        .count();
    Some(ReportObservedComplementarity {
        eligible_block_count: eligible_blocks,
        left_only_unique_path_count: left_only,
        shared_unique_path_count: shared,
        right_only_unique_path_count: right_only,
        total_system_unique_path_count: left_only + shared + right_only,
        incremental_left_path_count: left_only,
        incremental_right_path_count: right_only,
        left: left.map(|profile| project_repeatability(profile, comparison)),
        right: right.map(|profile| project_repeatability(profile, comparison)),
    })
}

fn profile<'a>(
    stratum: &ComparisonStratum,
    side: ComparisonSide,
    comparison: &'a PairedComparisonAnalysis,
) -> Option<&'a ObservedAntennaPathProfile> {
    comparison
        .observed_path_profiles
        .iter()
        .find(|profile| profile.stratum == *stratum && profile.side == side)
}

fn path_identities(profile: Option<&ObservedAntennaPathProfile>) -> BTreeSet<&str> {
    profile
        .into_iter()
        .flat_map(|profile| &profile.paths)
        .map(|path| path.remote_path.as_str())
        .collect()
}

fn project_repeatability(
    profile: &ObservedAntennaPathProfile,
    comparison: &PairedComparisonAnalysis,
) -> ReportAntennaRepeatability {
    let mut distribution = BTreeMap::<usize, usize>::new();
    let paths = profile
        .paths
        .iter()
        .map(|path| {
            *distribution.entry(path.block_support_count).or_default() += 1;
            let (left_then_right, right_then_left) = order_counts(&path.block_indices, comparison);
            ReportPathRepeatability {
                remote_path: path.remote_path.clone(),
                observed_block_count: path.block_support_count,
                observation_count: path.observation_count,
                left_then_right_block_count: left_then_right,
                right_then_left_block_count: right_then_left,
                block_indices: path.block_indices.clone(),
            }
        })
        .collect::<Vec<_>>();
    ReportAntennaRepeatability {
        side: profile.side,
        antenna_label: profile.antenna_label.clone(),
        unique_endpoint_count: paths.len(),
        path_block_observation_count: paths.iter().map(|path| path.observed_block_count).sum(),
        observed_once_path_count: paths
            .iter()
            .filter(|path| path.observed_block_count == 1)
            .count(),
        repeated_path_count: paths
            .iter()
            .filter(|path| path.observed_block_count > 1)
            .count(),
        block_count_distribution: distribution
            .into_iter()
            .map(
                |(observed_block_count, unique_path_count)| ReportBlockCountFrequency {
                    observed_block_count,
                    unique_path_count,
                },
            )
            .collect(),
        paths,
    }
}

fn order_counts(block_indices: &[usize], comparison: &PairedComparisonAnalysis) -> (usize, usize) {
    block_indices.iter().fold((0, 0), |mut counts, index| {
        match comparison
            .blocks
            .iter()
            .find(|block| block.block_index == *index)
            .and_then(|block| block.order)
        {
            Some(ComparisonOrder::LeftThenRight) => counts.0 += 1,
            Some(ComparisonOrder::RightThenLeft) => counts.1 += 1,
            None => {}
        }
        counts
    })
}

#[derive(Default)]
struct ReceiverFrequency {
    opportunities: usize,
    left: usize,
    right: usize,
    both: usize,
    left_only: usize,
    right_only: usize,
    neither: usize,
    left_then_right: usize,
    right_then_left: usize,
}

fn project_opportunities(
    stratum: &ComparisonStratum,
    activity: &ReporterActivityAnalysis,
) -> Option<ReportOpportunityComplementarity> {
    let summary = activity
        .joint_summaries
        .iter()
        .find(|summary| summary.stratum == *stratum)?;
    let mut orders = BTreeMap::<ComparisonOrder, ReportOpportunityOrderSummary>::new();
    let mut receivers = BTreeMap::<String, ReceiverFrequency>::new();
    for row in activity
        .paired_rates
        .iter()
        .filter(|row| row.stratum == *stratum)
    {
        let order = orders
            .entry(row.order)
            .or_insert(ReportOpportunityOrderSummary {
                order: row.order,
                block_count: 0,
                receiver_block_opportunity_count: 0,
                heard_both_count: 0,
                left_only_count: 0,
                right_only_count: 0,
                heard_neither_count: 0,
            });
        order.block_count += 1;
        order.receiver_block_opportunity_count += row.active_in_both_count;
        order.heard_both_count += row.heard_both_count;
        order.left_only_count += row.left_only_count;
        order.right_only_count += row.right_only_count;
        order.heard_neither_count += row.heard_neither_count;
        for receiver in &row.receivers {
            let frequency = receivers.entry(receiver.receiver.clone()).or_default();
            frequency.opportunities += 1;
            match row.order {
                ComparisonOrder::LeftThenRight => frequency.left_then_right += 1,
                ComparisonOrder::RightThenLeft => frequency.right_then_left += 1,
            }
            match receiver.outcome {
                ReporterActivityJointOutcome::HeardBoth => {
                    frequency.both += 1;
                    frequency.left += 1;
                    frequency.right += 1;
                }
                ReporterActivityJointOutcome::LeftOnly => {
                    frequency.left_only += 1;
                    frequency.left += 1;
                }
                ReporterActivityJointOutcome::RightOnly => {
                    frequency.right_only += 1;
                    frequency.right += 1;
                }
                ReporterActivityJointOutcome::HeardNeither => frequency.neither += 1,
            }
        }
    }
    Some(ReportOpportunityComplementarity {
        coverage: summary.coverage,
        eligible_block_count: summary.eligible_block_count,
        known_coverage_block_count: summary.known_coverage_block_count,
        unique_common_active_receiver_count: summary.unique_active_receiver_count,
        receiver_block_opportunity_count: summary.receiver_block_opportunity_count,
        heard_both_count: summary.heard_both_count,
        left_only_count: summary.left_only_count,
        right_only_count: summary.right_only_count,
        heard_neither_count: summary.heard_neither_count,
        order_summaries: orders.into_values().collect(),
        receiver_frequencies: receivers
            .into_iter()
            .map(|(receiver, row)| ReportReceiverDetectionFrequency {
                receiver,
                opportunity_count: row.opportunities,
                left_detection_count: row.left,
                right_detection_count: row.right,
                heard_both_count: row.both,
                left_only_count: row.left_only,
                right_only_count: row.right_only,
                heard_neither_count: row.neither,
                left_then_right_opportunity_count: row.left_then_right,
                right_then_left_opportunity_count: row.right_then_left,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests;
