#[derive(Debug, Clone, Copy)]
pub(super) struct FullHeaderView<'a> {
    pub(super) session_id: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OperationalHistoryView<'a> {
    pub(super) summary: &'a str,
}
