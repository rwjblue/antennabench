#[derive(Debug, Clone)]
pub(in crate::html) struct RepeatabilityDistributionView {
    pub(in crate::html) blocks: usize,
    pub(in crate::html) paths: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct RepeatabilityPathView {
    pub(in crate::html) remote_path: String,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) observations: usize,
    pub(in crate::html) left_then_right: usize,
    pub(in crate::html) right_then_left: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct RepeatabilityView {
    pub(in crate::html) antenna: String,
    pub(in crate::html) unique_paths: usize,
    pub(in crate::html) path_suffix: &'static str,
    pub(in crate::html) path_blocks: usize,
    pub(in crate::html) observation_suffix: &'static str,
    pub(in crate::html) once: usize,
    pub(in crate::html) repeated: usize,
    pub(in crate::html) distribution: Vec<RepeatabilityDistributionView>,
    pub(in crate::html) paths: Vec<RepeatabilityPathView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ObservedOverlapView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) total: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) shared: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) incremental_left: usize,
    pub(in crate::html) incremental_right: usize,
    pub(in crate::html) eligible_blocks: usize,
    pub(in crate::html) block_suffix: &'static str,
    pub(in crate::html) repeatability: Vec<RepeatabilityView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OpportunityOrderView {
    pub(in crate::html) order: &'static str,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReceiverFrequencyView {
    pub(in crate::html) receiver: String,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) left_detections: usize,
    pub(in crate::html) right_detections: usize,
    pub(in crate::html) left_then_right: usize,
    pub(in crate::html) right_then_left: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OpportunityOverlapView {
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) known_blocks: usize,
    pub(in crate::html) eligible_blocks: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) receivers: usize,
    pub(in crate::html) coverage_known: bool,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
    pub(in crate::html) orders: Vec<OpportunityOrderView>,
    pub(in crate::html) receiver_frequencies: Vec<ReceiverFrequencyView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverlapGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) observed: Option<ObservedOverlapView>,
    pub(in crate::html) common: Option<OpportunityOverlapView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverlapQuestionView {
    pub(in crate::html) summary: bool,
    pub(in crate::html) render: bool,
    pub(in crate::html) no_groups: bool,
    pub(in crate::html) groups: Vec<OverlapGroupView>,
}
