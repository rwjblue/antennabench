#[derive(Debug, Clone)]
pub(in crate::html) struct PathTickView {
    pub(in crate::html) x: String,
    pub(in crate::html) label: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PathDotView {
    pub(in crate::html) detail: String,
    pub(in crate::html) class: &'static str,
    pub(in crate::html) x: String,
    pub(in crate::html) y: String,
    pub(in crate::html) radius: String,
    pub(in crate::html) fill: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ExactPathView {
    pub(in crate::html) remote_path: String,
    pub(in crate::html) pairs: usize,
    pub(in crate::html) delta: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PathDistributionView {
    pub(in crate::html) negative_label: String,
    pub(in crate::html) negative_count: usize,
    pub(in crate::html) tied_count: usize,
    pub(in crate::html) positive_label: String,
    pub(in crate::html) positive_count: usize,
    pub(in crate::html) median: String,
    pub(in crate::html) first_quartile: String,
    pub(in crate::html) third_quartile: String,
    pub(in crate::html) aria_label: String,
    pub(in crate::html) ticks: Vec<PathTickView>,
    pub(in crate::html) dots: Vec<PathDotView>,
    pub(in crate::html) orientation_text: String,
    pub(in crate::html) exact_paths: Vec<ExactPathView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PathStratumView {
    pub(in crate::html) label: String,
    pub(in crate::html) matched_paths: usize,
    pub(in crate::html) path_suffix: &'static str,
    pub(in crate::html) matched_pairs: usize,
    pub(in crate::html) pair_suffix: &'static str,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) block_suffix: &'static str,
    pub(in crate::html) empty_message: Option<String>,
    pub(in crate::html) distribution: Option<PathDistributionView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SamePathView {
    pub(in crate::html) compact: bool,
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) all_unavailable: Option<String>,
    pub(in crate::html) orientation: Option<String>,
    pub(in crate::html) strata: Vec<PathStratumView>,
    pub(in crate::html) unavailable: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReachSegmentView {
    pub(in crate::html) side: &'static str,
    pub(in crate::html) geometry_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReachBarView {
    pub(in crate::html) class: String,
    pub(in crate::html) segments: Vec<ReachSegmentView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReachRowView {
    pub(in crate::html) label: String,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) left_total: usize,
    pub(in crate::html) right_total: usize,
    pub(in crate::html) universe: usize,
    pub(in crate::html) universe_suffix: &'static str,
    pub(in crate::html) missing_left: usize,
    pub(in crate::html) missing_right: usize,
    pub(in crate::html) duplicates: usize,
    pub(in crate::html) conflicts: usize,
    pub(in crate::html) bar: ReachBarView,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReachView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) rows: Vec<ReachRowView>,
    pub(in crate::html) unavailable: Option<String>,
}
