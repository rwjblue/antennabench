#[derive(Debug, Clone)]
pub(in crate::html) struct DetectionRateRowView {
    pub(in crate::html) label: String,
    pub(in crate::html) heard: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) geometry_class: String,
    pub(in crate::html) rate: String,
    pub(in crate::html) side: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct DetectionRateView {
    pub(in crate::html) takeaway_lead: String,
    pub(in crate::html) takeaway_detail: String,
    pub(in crate::html) rows: Vec<DetectionRateRowView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityOutcomeView {
    pub(in crate::html) label: String,
    pub(in crate::html) count: usize,
    pub(in crate::html) class: &'static str,
    pub(in crate::html) geometry_class: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) label: String,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) known_blocks: usize,
    pub(in crate::html) eligible_blocks: usize,
    pub(in crate::html) unique_receivers: usize,
    pub(in crate::html) rates: Option<DetectionRateView>,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) outcomes: Vec<ActivityOutcomeView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityJointSummaryRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) unique_receivers: usize,
    pub(in crate::html) eligible_blocks: usize,
    pub(in crate::html) left_then_right: usize,
    pub(in crate::html) right_then_left: usize,
    pub(in crate::html) opportunities: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
    pub(in crate::html) left_rate: String,
    pub(in crate::html) right_rate: String,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) known_blocks: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityPairedRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) order: String,
    pub(in crate::html) left_slot: String,
    pub(in crate::html) right_slot: String,
    pub(in crate::html) active: usize,
    pub(in crate::html) both: usize,
    pub(in crate::html) left_only: usize,
    pub(in crate::html) right_only: usize,
    pub(in crate::html) neither: usize,
    pub(in crate::html) left_rate: String,
    pub(in crate::html) right_rate: String,
    pub(in crate::html) coverage: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityCycleRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) antenna: String,
    pub(in crate::html) starts: String,
    pub(in crate::html) slot: String,
    pub(in crate::html) rate: String,
    pub(in crate::html) coverage: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ActivityReceiverRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) receiver: String,
    pub(in crate::html) locator: String,
    pub(in crate::html) outcome: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReporterActivityView {
    pub(in crate::html) no_activity: bool,
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) groups: Vec<ActivityGroupView>,
    pub(in crate::html) summaries: Vec<ActivityJointSummaryRowView>,
    pub(in crate::html) paired_rows: Vec<ActivityPairedRowView>,
    pub(in crate::html) cycle_rows: Vec<ActivityCycleRowView>,
    pub(in crate::html) receiver_rows: Vec<ActivityReceiverRowView>,
}
