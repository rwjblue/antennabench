#[derive(Debug, Clone)]
pub(in crate::html) struct RateCellView {
    pub(in crate::html) class: String,
    pub(in crate::html) label: String,
    pub(in crate::html) rate: String,
    pub(in crate::html) count: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct RateMapRowView {
    pub(in crate::html) distance: &'static str,
    pub(in crate::html) cells: Vec<RateCellView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct RateMapView {
    pub(in crate::html) side: &'static str,
    pub(in crate::html) antenna: String,
    pub(in crate::html) group_number: usize,
    pub(in crate::html) rows: Vec<RateMapRowView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CommonCellRowView {
    pub(in crate::html) sector: &'static str,
    pub(in crate::html) distance: &'static str,
    pub(in crate::html) receivers: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
    pub(in crate::html) left_rate: String,
    pub(in crate::html) right_rate: String,
    pub(in crate::html) coverage: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CommonMarginalRowView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) receivers: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
    pub(in crate::html) left_heard: usize,
    pub(in crate::html) right_heard: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CommonBlockView {
    pub(in crate::html) block: usize,
    pub(in crate::html) order: &'static str,
    pub(in crate::html) left_slot: String,
    pub(in crate::html) right_slot: String,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) active: usize,
    pub(in crate::html) located: usize,
    pub(in crate::html) unavailable: usize,
    pub(in crate::html) populated_cells: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CommonCoverageGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) receivers: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) located_opportunities: usize,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) known_blocks: usize,
    pub(in crate::html) eligible_blocks: usize,
    pub(in crate::html) coverage_known: bool,
    pub(in crate::html) finding: Option<String>,
    pub(in crate::html) rate_maps: Vec<RateMapView>,
    pub(in crate::html) cells: Vec<CommonCellRowView>,
    pub(in crate::html) distance_rows: Vec<CommonMarginalRowView>,
    pub(in crate::html) azimuth_rows: Vec<CommonMarginalRowView>,
    pub(in crate::html) unavailable_receivers: usize,
    pub(in crate::html) unavailable_opportunities: usize,
    pub(in crate::html) blocks: Vec<CommonBlockView>,
    pub(in crate::html) include_audit: bool,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct WorldCellView {
    pub(in crate::html) x: String,
    pub(in crate::html) y: String,
    pub(in crate::html) fill: String,
    pub(in crate::html) title: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PolarDotView {
    pub(in crate::html) x: String,
    pub(in crate::html) y: String,
    pub(in crate::html) fill: String,
    pub(in crate::html) title: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LegacyPanelView {
    pub(in crate::html) side: &'static str,
    pub(in crate::html) antenna: String,
    pub(in crate::html) heard: usize,
    pub(in crate::html) active: usize,
    pub(in crate::html) mapped: usize,
    pub(in crate::html) unmapped: usize,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) grid_hatch: String,
    pub(in crate::html) polar_hatch: String,
    pub(in crate::html) polar_clip: String,
    pub(in crate::html) world_cells: Vec<WorldCellView>,
    pub(in crate::html) polar_rings: Vec<String>,
    pub(in crate::html) polar_dots: Vec<PolarDotView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LegacyCoverageGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) panels: Vec<LegacyPanelView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CoverageView {
    pub(in crate::html) summary: bool,
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) groups: Vec<CommonCoverageGroupView>,
    pub(in crate::html) world_coastline: String,
    pub(in crate::html) polar_coastline: String,
    pub(in crate::html) legacy_groups: Vec<LegacyCoverageGroupView>,
}
