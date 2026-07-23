use super::*;

#[derive(Debug, Clone, Copy, Default, Deserialize)]
struct AutomaticCaptureCounts {
    total: usize,
    accepted: usize,
    filtered: usize,
    duplicate: usize,
    conflict: usize,
    observations_created: usize,
}

#[derive(Debug, Deserialize)]
struct AutomaticCaptureSummary {
    acquisition_channel: String,
    window_end: DateTime<Utc>,
    counts: AutomaticCaptureCounts,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct AutomaticCaptureTotals {
    pub(super) successful_windows: usize,
    pub(super) returned: usize,
    pub(super) accepted: usize,
    pub(super) filtered: usize,
    pub(super) conflicted: usize,
    pub(super) duplicated: usize,
    pub(super) created: usize,
}

fn automatic_capture_summaries(
    snapshot: &AcquisitionSnapshot,
) -> impl Iterator<Item = AutomaticCaptureSummary> + '_ {
    snapshot.adapter_records().iter().filter_map(|record| {
        if record.record_type != "wspr_live_import_summary" {
            return None;
        }
        let AdapterInput::Inline { data, .. } = &record.input else {
            return None;
        };
        serde_json::from_str::<AutomaticCaptureSummary>(data)
            .ok()
            .filter(|summary| summary.acquisition_channel == "https-query")
    })
}

pub(super) fn automatic_capture_counts(snapshot: &AcquisitionSnapshot) -> AutomaticCaptureTotals {
    automatic_capture_summaries(snapshot).fold(
        AutomaticCaptureTotals::default(),
        |mut totals, summary| {
            totals.successful_windows += 1;
            totals.returned += summary.counts.total;
            totals.accepted += summary.counts.accepted;
            totals.filtered += summary.counts.filtered;
            totals.conflicted += summary.counts.conflict;
            totals.duplicated += summary.counts.duplicate;
            totals.created += summary.counts.observations_created;
            totals
        },
    )
}

pub(super) fn automatic_capture_attempts(
    snapshot: &AcquisitionSnapshot,
    window_end: DateTime<Utc>,
) -> usize {
    automatic_capture_summaries(snapshot)
        .filter(|summary| summary.window_end == window_end)
        .count()
}

pub(super) fn zero_evidence_outcome(
    session: OpenedSession,
    revision: u64,
    plan: &WsprLiveAcquisitionPlan,
    captured_through: DateTime<Utc>,
    counts: AutomaticCaptureTotals,
    retry_available: bool,
) -> WsprLiveAcquisitionOutcome {
    WsprLiveAcquisitionOutcome::AwaitingAcknowledgement {
        session: Box::new(session),
        revision,
        completed_slot_id: plan.completed_slot_id.clone(),
        captured_through,
        retry_available,
        successful_windows: counts.successful_windows,
        returned: counts.returned,
        accepted: counts.accepted,
        filtered: counts.filtered,
        conflicted: counts.conflicted,
        duplicated: counts.duplicated,
        created: counts.created,
    }
}

pub(super) fn captured_outcome(
    plan: &WsprLiveAcquisitionPlan,
    response: &WsprLiveHttpResponse,
    committed: CommittedWsprLiveResponse,
) -> WsprLiveAcquisitionOutcome {
    WsprLiveAcquisitionOutcome::Captured {
        session: Box::new(committed.session),
        revision: committed.revision,
        completed_slot_id: plan.completed_slot_id.clone(),
        captured_through: plan.query.window_end,
        checked_at: response.received_at,
        total: committed.summary.total,
        accepted: committed.summary.accepted,
        duplicate: committed.summary.duplicate,
        conflict: committed.summary.conflict,
        observations_created: committed.summary.observations_created,
    }
}

pub(super) fn failed_outcome(
    plan: &WsprLiveAcquisitionPlan,
    error: SessionErrorPayload,
) -> WsprLiveAcquisitionOutcome {
    WsprLiveAcquisitionOutcome::Failed {
        completed_slot_id: plan.completed_slot_id.clone(),
        window_end: plan.query.window_end,
        message: error.message,
        detail: error.detail,
    }
}
