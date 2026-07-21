#[derive(Debug, Clone, Copy)]
pub(super) struct FullHeaderView<'a> {
    pub(super) session_id: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OperationalHistoryView<'a> {
    pub(super) summary: &'a str,
}
mod activity;
mod audit;
mod comparison;
mod coverage;
mod evidence;
mod geography;
mod location_audit;
mod overlap_question;
mod overview;
mod paths;
mod quality;
mod snapshot;

pub(super) use activity::*;
pub(super) use audit::*;
pub(super) use comparison::*;
pub(super) use coverage::*;
pub(super) use evidence::*;
pub(super) use geography::*;
pub(super) use location_audit::*;
pub(super) use overlap_question::*;
pub(super) use overview::*;
pub(super) use paths::*;
pub(super) use quality::*;
pub(super) use snapshot::*;
