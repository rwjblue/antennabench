#[derive(Debug, Clone, Copy)]
pub(super) struct FullHeaderView<'a> {
    pub(super) session_id: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OperationalHistoryView<'a> {
    pub(super) summary: &'a str,
}
mod audit;
mod comparison;
mod evidence;
mod location_audit;
mod overview;
mod snapshot;

pub(super) use audit::*;
pub(super) use comparison::*;
pub(super) use evidence::*;
pub(super) use location_audit::*;
pub(super) use overview::*;
pub(super) use snapshot::*;
