use std::fmt::Write as _;

use crate::{ReportLifecycleEventKind, SessionReport};
use antennabench_analysis::{
    ComparisonSide, ComparisonStratum, ComparisonTimelineRow, PairedObservationRow,
    SolarEndpointContext, SolarLightState, SolarPositionResult,
};
use antennabench_core::{
    v2::SessionLifecycleV2, v3::WsprCycleDirection, v5::AntennaControlDispositionV5,
};

use super::{evidence::evidence_summary, shared::*, ControllerEvidenceHandling};

mod activity;
mod appendix;
mod comparison;
mod eligibility;
mod location;
mod snapshot;

// Keep cross-section helpers renderer-scoped while presenting one audit façade.
pub(super) use activity::render_reporter_activity_audit;
pub(super) use appendix::render_audit_appendix;
pub(super) use comparison::{
    comparison_stat, render_comparison_blocks, render_comparison_diagnostics,
    render_comparison_timeline, render_overlap, render_paired_differences, render_paired_snr_time,
    render_stratum_summaries,
};
pub(super) use eligibility::{render_eligibility, render_exclusion_records, render_notices};
pub(super) use location::{optional_measure_f64, render_location_views, render_solar_context};
pub(super) use snapshot::{
    lifecycle, lifecycle_event, render_context, render_overall, render_snapshot,
    snapshot_has_detail,
};
