use askama::Template;

use super::super::view::{
    ComparisonBlockView, ContextView, EligibilityView, ExclusionRecordsView, LocationViewsView,
    NoticesView, OverlapView, PairedRowsView, ReporterActivityAuditView, SnapshotView,
    SolarContextView, StatView, StratumSummariesView, TimelineView,
};

#[derive(Template)]
#[template(path = "report/audit/location_views.html")]
pub(in crate::html) struct LocationViewsTemplate {
    pub(in crate::html) view: LocationViewsView,
}

#[derive(Template)]
#[template(path = "report/audit/solar_context.html")]
pub(in crate::html) struct SolarContextTemplate {
    pub(in crate::html) view: SolarContextView,
}

#[derive(Template)]
#[template(path = "report/audit/comparison_diagnostics.html")]
pub(in crate::html) struct ComparisonDiagnosticsTemplate {
    pub(in crate::html) stats: Vec<StatView>,
}

#[derive(Template)]
#[template(path = "report/audit/overlap.html")]
pub(in crate::html) struct OverlapTemplate {
    pub(in crate::html) view: OverlapView,
}

#[derive(Template)]
#[template(path = "report/audit/timeline.html")]
pub(in crate::html) struct TimelineTemplate {
    pub(in crate::html) view: TimelineView,
}

#[derive(Template)]
#[template(path = "report/audit/comparison_blocks.html")]
pub(in crate::html) struct ComparisonBlocksTemplate {
    pub(in crate::html) rows: Vec<ComparisonBlockView>,
}

#[derive(Template)]
#[template(path = "report/audit/paired_differences.html")]
pub(in crate::html) struct PairedDifferencesTemplate {
    pub(in crate::html) view: PairedRowsView,
}

#[derive(Template)]
#[template(path = "report/audit/paired_snr_time.html")]
pub(in crate::html) struct PairedSnrTimeTemplate {
    pub(in crate::html) view: PairedRowsView,
}

#[derive(Template)]
#[template(path = "report/audit/stratum_summaries.html")]
pub(in crate::html) struct StratumSummariesTemplate {
    pub(in crate::html) view: StratumSummariesView,
}

#[derive(Template)]
#[template(path = "report/audit/snapshot.html")]
pub(in crate::html) struct SnapshotTemplate {
    pub(in crate::html) view: SnapshotView,
}

#[derive(Template)]
#[template(path = "report/audit/context.html")]
pub(in crate::html) struct ContextTemplate {
    pub(in crate::html) view: ContextView,
}

#[derive(Template)]
#[template(path = "report/audit/evidence_overview_start.html")]
pub(in crate::html) struct EvidenceOverviewStartTemplate {
    pub(in crate::html) coverage: &'static str,
}

#[derive(Template)]
#[template(path = "report/audit/evidence_overview_end.html")]
pub(in crate::html) struct EvidenceOverviewEndTemplate;

#[derive(Template)]
#[template(path = "report/audit/reporter_activity.html")]
pub(in crate::html) struct ReporterActivityAuditTemplate {
    pub(in crate::html) view: ReporterActivityAuditView,
}

#[derive(Template)]
#[template(path = "report/audit/eligibility.html")]
pub(in crate::html) struct EligibilityTemplate {
    pub(in crate::html) view: EligibilityView,
}

#[derive(Template)]
#[template(path = "report/audit/exclusion_records.html")]
pub(in crate::html) struct ExclusionRecordsTemplate {
    pub(in crate::html) view: ExclusionRecordsView,
}

#[derive(Template)]
#[template(path = "report/audit/notices.html")]
pub(in crate::html) struct NoticesTemplate {
    pub(in crate::html) view: NoticesView,
}

#[derive(Template)]
#[template(path = "report/audit/appendix_start.html")]
pub(in crate::html) struct AuditAppendixStartTemplate {
    pub(in crate::html) controller_details_omitted: bool,
}

#[derive(Template)]
#[template(path = "report/audit/snapshot_disclosure_start.html")]
pub(in crate::html) struct AuditSnapshotDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/context_disclosure_start.html")]
pub(in crate::html) struct AuditContextDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/comparison_disclosure_start.html")]
pub(in crate::html) struct AuditComparisonDisclosureStartTemplate;

#[derive(Template)]
#[template(path = "report/audit/disclosure_end.html")]
pub(in crate::html) struct AuditDisclosureEndTemplate;

#[derive(Template)]
#[template(path = "report/audit/appendix_end.html")]
pub(in crate::html) struct AuditAppendixEndTemplate;
