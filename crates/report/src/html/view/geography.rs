use super::ReachBarView;

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileTotalView {
    pub(in crate::html) antenna: String,
    pub(in crate::html) unique_paths: usize,
    pub(in crate::html) located: usize,
    pub(in crate::html) missing: usize,
    pub(in crate::html) inconsistent: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileDistributionRowView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) left: String,
    pub(in crate::html) right: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileDistributionView {
    pub(in crate::html) caption: &'static str,
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) rows: Vec<ProfileDistributionRowView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileBarRowView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) left_label: String,
    pub(in crate::html) left_count: usize,
    pub(in crate::html) left_class: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) right_count: usize,
    pub(in crate::html) right_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileBarChartView {
    pub(in crate::html) heading: &'static str,
    pub(in crate::html) rows: Vec<ProfileBarRowView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CompositionRowView {
    pub(in crate::html) distance: &'static str,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) shared: usize,
    pub(in crate::html) right_only: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ProfileView {
    pub(in crate::html) totals: Vec<ProfileTotalView>,
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) distributions: Vec<ProfileDistributionView>,
    pub(in crate::html) bar_charts: Vec<ProfileBarChartView>,
    pub(in crate::html) composition: Vec<CompositionRowView>,
    pub(in crate::html) composition_unavailable: usize,
    pub(in crate::html) composition_suffix: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct FullProfileGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) dominant_summary: Option<String>,
    pub(in crate::html) profile: ProfileView,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct FootprintReachView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) left_total: usize,
    pub(in crate::html) right_total: usize,
    pub(in crate::html) bar: ReachBarView,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CompactFootprintGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) reach: FootprintReachView,
    pub(in crate::html) profile: ProfileView,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ObservedPathAuditRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) antenna: String,
    pub(in crate::html) remote_path: String,
    pub(in crate::html) location: String,
    pub(in crate::html) block_support: usize,
    pub(in crate::html) slot_support: usize,
    pub(in crate::html) blocks: String,
    pub(in crate::html) slots: String,
    pub(in crate::html) observations: usize,
    pub(in crate::html) observation_ids: String,
    pub(in crate::html) snr: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ContextCellView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) unique_paths: usize,
    pub(in crate::html) paired_rows: usize,
    pub(in crate::html) delta: String,
    pub(in crate::html) evidence: String,
    pub(in crate::html) empty: bool,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ContextSectionView {
    pub(in crate::html) heading: &'static str,
    pub(in crate::html) caption: &'static str,
    pub(in crate::html) cells: Vec<ContextCellView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LocationPathAuditView {
    pub(in crate::html) remote_path: String,
    pub(in crate::html) pairs: usize,
    pub(in crate::html) delta: String,
    pub(in crate::html) status: &'static str,
    pub(in crate::html) distance: String,
    pub(in crate::html) azimuth: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PathContextGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) located: usize,
    pub(in crate::html) located_suffix: &'static str,
    pub(in crate::html) unavailable: usize,
    pub(in crate::html) missing: usize,
    pub(in crate::html) inconsistent: usize,
    pub(in crate::html) sections: Vec<ContextSectionView>,
    pub(in crate::html) paths: Vec<LocationPathAuditView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PathContextView {
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) all_unavailable: Option<String>,
    pub(in crate::html) groups: Vec<PathContextGroupView>,
    pub(in crate::html) unavailable: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct GeographyView {
    pub(in crate::html) single_antenna: bool,
    pub(in crate::html) goal_focus: Option<String>,
    pub(in crate::html) no_profiles: bool,
    pub(in crate::html) profiles: Vec<FullProfileGroupView>,
    pub(in crate::html) audit_rows: Vec<ObservedPathAuditRowView>,
    pub(in crate::html) path_context: PathContextView,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CompactFootprintView {
    pub(in crate::html) single_antenna: bool,
    pub(in crate::html) goal_focus: Option<String>,
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) groups: Vec<CompactFootprintGroupView>,
    pub(in crate::html) unavailable: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ObservedPathAuditView {
    pub(in crate::html) rows: Vec<ObservedPathAuditRowView>,
}
