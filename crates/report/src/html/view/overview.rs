#[derive(Debug, Clone)]
pub(in crate::html) struct NavigationLinkView {
    pub(in crate::html) href: &'static str,
    pub(in crate::html) label: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct NavigationView {
    pub(in crate::html) links: Vec<NavigationLinkView>,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::html) struct ReadingGuideView {
    pub(in crate::html) summary: bool,
    pub(in crate::html) single_antenna: bool,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct HeadlineFactView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) value: String,
    pub(in crate::html) detail: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct HeadlineGroupView {
    pub(in crate::html) index: usize,
    pub(in crate::html) title: Option<String>,
    pub(in crate::html) answer: Option<String>,
    pub(in crate::html) facts: Vec<HeadlineFactView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SummaryFindingView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) status: &'static str,
    pub(in crate::html) status_class: &'static str,
    pub(in crate::html) population: &'static str,
    pub(in crate::html) result: String,
    pub(in crate::html) support: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SummaryOverviewView {
    pub(in crate::html) context: String,
    pub(in crate::html) interpretation: String,
    pub(in crate::html) findings: Vec<SummaryFindingView>,
    pub(in crate::html) principal_limitation: String,
    pub(in crate::html) goal_lens: Option<GoalLensView>,
    pub(in crate::html) headline_groups: Vec<HeadlineGroupView>,
    pub(in crate::html) condition_label: String,
    pub(in crate::html) rows: Vec<OverviewResultRowView>,
    pub(in crate::html) unavailable_groups: Option<String>,
    pub(in crate::html) notices: Vec<NoticeView>,
    pub(in crate::html) availability: Vec<AvailabilityFactView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct GoalLensView {
    pub(in crate::html) label: &'static str,
    pub(in crate::html) practical_meaning: String,
    pub(in crate::html) distance_focus: Option<String>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverviewResultRowView {
    pub(in crate::html) group: String,
    pub(in crate::html) delta: String,
    pub(in crate::html) paths: usize,
    pub(in crate::html) pairs: usize,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) coverage: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AvailabilityFactView {
    pub(in crate::html) question: &'static str,
    pub(in crate::html) status: &'static str,
    pub(in crate::html) status_class: String,
    pub(in crate::html) availability: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct NoticeView {
    pub(in crate::html) critical: bool,
    pub(in crate::html) label: &'static str,
    pub(in crate::html) message: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverviewView {
    pub(in crate::html) summary: bool,
    pub(in crate::html) answerability_headline: String,
    pub(in crate::html) plain_answer: String,
    pub(in crate::html) headline_groups: Vec<HeadlineGroupView>,
    pub(in crate::html) callsign: String,
    pub(in crate::html) grid: String,
    pub(in crate::html) goal: &'static str,
    pub(in crate::html) goal_lens: Option<GoalLensView>,
    pub(in crate::html) antennas: String,
    pub(in crate::html) bands: String,
    pub(in crate::html) direction_mode: String,
    pub(in crate::html) lifecycle: &'static str,
    pub(in crate::html) orientation_label: &'static str,
    pub(in crate::html) orientation: String,
    pub(in crate::html) show_delta_scale: bool,
    pub(in crate::html) rows: Vec<OverviewResultRowView>,
    pub(in crate::html) unavailable_groups: Option<String>,
    pub(in crate::html) comparison_state: &'static str,
    pub(in crate::html) support: String,
    pub(in crate::html) limitations: Vec<String>,
    pub(in crate::html) notices: Vec<NoticeView>,
    pub(in crate::html) availability: Vec<AvailabilityFactView>,
}
