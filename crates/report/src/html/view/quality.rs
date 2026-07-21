#[derive(Debug, Clone)]
pub(in crate::html) struct QualityFactView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) value: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AnswerabilityRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) availability: &'static str,
    pub(in crate::html) pairs: usize,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) unique_paths: usize,
    pub(in crate::html) left_then_right: usize,
    pub(in crate::html) right_then_left: usize,
    pub(in crate::html) unmatched_left: usize,
    pub(in crate::html) unmatched_right: usize,
    pub(in crate::html) missing_left: usize,
    pub(in crate::html) missing_right: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) duplicates: usize,
    pub(in crate::html) conflicts: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct LifecycleChipView {
    pub(in crate::html) symbol: &'static str,
    pub(in crate::html) event: &'static str,
    pub(in crate::html) occurred_at: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct TimelineEventView {
    pub(in crate::html) event_id: String,
    pub(in crate::html) occurred_at: String,
    pub(in crate::html) kind: &'static str,
    pub(in crate::html) detail: Option<String>,
    pub(in crate::html) correction_action: Option<&'static str>,
    pub(in crate::html) correction_target: Option<String>,
    pub(in crate::html) correction_reason: Option<String>,
    pub(in crate::html) correction_state: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct QualityTimelineRowView {
    pub(in crate::html) class: &'static str,
    pub(in crate::html) state: &'static str,
    pub(in crate::html) symbol: &'static str,
    pub(in crate::html) sequence: u32,
    pub(in crate::html) planned_antenna: String,
    pub(in crate::html) actual_antenna: String,
    pub(in crate::html) usable: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) facts: Vec<QualityFactView>,
    pub(in crate::html) events: Vec<TimelineEventView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AcquisitionQualityView {
    pub(in crate::html) paragraph_class: &'static str,
    pub(in crate::html) lead: Option<&'static str>,
    pub(in crate::html) body: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ExclusionSummaryRowView {
    pub(in crate::html) reason: &'static str,
    pub(in crate::html) count: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct QualityView {
    pub(in crate::html) no_strata: bool,
    pub(in crate::html) comparison_state: &'static str,
    pub(in crate::html) comparison_text: &'static str,
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) answerability: Vec<AnswerabilityRowView>,
    pub(in crate::html) lifecycle: Vec<LifecycleChipView>,
    pub(in crate::html) timeline: Vec<QualityTimelineRowView>,
    pub(in crate::html) acquisition: AcquisitionQualityView,
    pub(in crate::html) exclusions: Vec<ExclusionSummaryRowView>,
    pub(in crate::html) has_exclusion_records: bool,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CompactQualityView {
    pub(in crate::html) comparison_state: &'static str,
    pub(in crate::html) lifecycle: &'static str,
    pub(in crate::html) usable: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) acquisition: String,
    pub(in crate::html) bounded: bool,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CompactReferenceView {
    pub(in crate::html) session_id: String,
    pub(in crate::html) revision: String,
}
