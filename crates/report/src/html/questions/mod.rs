use std::fmt::Write as _;

use crate::{
    GeographicProfileAnswerability, ObservedReachAnswerability, PairedDetectabilityAnswerability,
    RepeatabilityAnswerability, ReportAzimuthSector, ReportDistanceBin, ReportLifecycleEventKind,
    ReportObservedAntennaProfile, ReportObservedProfileCell, ReportOperatorEvent,
    ReportOperatorEventKind, ReportOverviewLifecycleState, ReportOverviewLimitation,
    ReportOverviewLocationCell, ReportOverviewPathDelta, ReportOverviewReach,
    ReportOverviewStratum, ReportPathLocationAvailability, ReportRunTimelineRow,
    ReportStratumAvailability, SamePathSignalAnswerability, SessionReport,
};
use antennabench_core::AlignedSlotStatus;

use super::{audit::*, evidence::*, shared::*};

mod activity;
mod coverage;
mod location;
mod overview;
mod paths;
mod quality;

pub(super) use activity::{coverage_text, render_reporter_activity_section};
pub(super) use coverage::{render_compact_coverage_map_section, render_coverage_map_section};

pub(super) use location::{render_compact_distance_section, render_distance_section};
pub(super) use overview::{
    overview_lifecycle_label, render_answer_first_overview,
    render_answer_first_overview_with_reference, render_how_to_read, render_question_navigation,
};
pub(super) use paths::{
    plural_suffix, render_reach_bar, render_reach_section, render_same_path_section,
    render_same_path_stratum,
};
pub(super) use quality::render_run_quality_section;
