use std::fmt::Write as _;

use crate::{
    ReportAzimuthSector, ReportDistanceBin, ReportLifecycleEventKind, ReportOperatorEvent,
    ReportOperatorEventKind, ReportOverviewLifecycleState, ReportOverviewLimitation,
    ReportOverviewLocationCell, ReportOverviewPathDelta, ReportOverviewStratum,
    ReportPathLocationAvailability, ReportRunTimelineRow, ReportStratumAvailability, SessionReport,
};
use antennabench_core::AlignedSlotStatus;

use super::{audit::*, evidence::*, shared::*};

mod location;
mod overview;
mod paths;
mod quality;

pub(super) use location::render_distance_section;
pub(super) use overview::{
    overview_lifecycle_label, render_answer_first_overview,
    render_answer_first_overview_with_reference, render_question_navigation,
};
pub(super) use paths::{
    plural_suffix, render_reach_section, render_same_path_section, render_same_path_stratum,
};
pub(super) use quality::render_run_quality_section;
