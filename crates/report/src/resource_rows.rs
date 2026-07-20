use antennabench_analysis::AnalysisSummary;

use crate::{
    common_opportunity, complementarity, ReportCommonOpportunityMapGroup, ReportCoverageMapGroup,
    ReportCoverageOverlapGroup, ReportSnapshotContext,
};

pub(crate) fn required_overview_row_count(
    summary: &AnalysisSummary,
    coverage_maps: &[ReportCoverageMapGroup],
    common_opportunity_maps: &[ReportCommonOpportunityMapGroup],
    coverage_overlap: &[ReportCoverageOverlapGroup],
    snapshot: &ReportSnapshotContext,
) -> usize {
    summary.eligibility.exclusions.len()
        + summary.comparison.strata.len() * 41
        + summary.comparison.path_summaries.len()
        + summary.reporter_activity.census_cycles.len()
        + summary.reporter_activity.cycle_rates.len()
        + summary.reporter_activity.paired_rates.len()
        + summary.reporter_activity.joint_summaries.len()
        + coverage_maps
            .iter()
            .flat_map(|group| &group.panels)
            .map(|panel| panel.cells.len() + panel.polar_cells.len())
            .sum::<usize>()
        + common_opportunity::overview_row_count(common_opportunity_maps)
        + complementarity::overview_row_count(coverage_overlap)
        + summary.slots.len()
        + snapshot.operator_events.len()
}
