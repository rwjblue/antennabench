use super::*;

pub(super) fn acquisition_notices(
    report: &SessionReport,
    audit_reference: &str,
) -> Vec<NoticeView> {
    let evidence = &report.snapshot.adapter_evidence;
    let mut notices = Vec::new();
    if evidence.gap_count > 0
        || evidence.workflow_status == ReportAcquisitionWorkflowStatus::Incomplete
    {
        let message = if evidence.gap_count == 1 {
            format!("1 recorded acquisition gap; inspect {audit_reference} for its durable recorded context")
        } else if evidence.gap_count > 1 {
            format!(
                "{} recorded acquisition gaps; inspect {audit_reference} for their durable recorded context",
                evidence.gap_count,
            )
        } else {
            format!("Recorded acquisition is incomplete; inspect {audit_reference} for its durable recorded context")
        };
        notices.push(NoticeView {
            critical: true,
            label: "Recorded acquisition",
            message,
        });
    }
    if evidence
        .imports
        .iter()
        .any(|import| import.provider_id == "wspr-live")
    {
        let counts = wspr_live_capture_counts(report);
        notices.push(NoticeView {
            critical: report.evidence.overall.observation_counts.usable == 0,
            label: "Captured-window outcome",
            message: format!(
                "{} successful window{}; {} row{} returned, {} accepted, {} filtered, {} conflicted, {} duplicated, and {} observation{} created",
                counts.successful_windows,
                plural_suffix(counts.successful_windows),
                counts.returned,
                plural_suffix(counts.returned),
                counts.accepted,
                counts.filtered,
                counts.conflicted,
                counts.duplicated,
                counts.created,
                plural_suffix(counts.created),
            ),
        });
        notices.push(NoticeView {
            critical: false,
            label: "Public-source boundary",
            message: "AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee".to_string(),
        });
    }
    notices
}

#[derive(Default)]
struct WsprLiveCaptureCounts {
    successful_windows: usize,
    returned: usize,
    accepted: usize,
    filtered: usize,
    conflicted: usize,
    duplicated: usize,
    created: usize,
}

fn wspr_live_capture_counts(report: &SessionReport) -> WsprLiveCaptureCounts {
    report
        .snapshot
        .adapter_evidence
        .imports
        .iter()
        .filter(|import| import.provider_id == "wspr-live")
        .fold(WsprLiveCaptureCounts::default(), |mut counts, import| {
            counts.successful_windows += 1;
            counts.returned += import.total_count;
            counts.accepted += import.accepted_count;
            counts.filtered += import.filtered_count;
            counts.conflicted += import.conflict_count;
            counts.duplicated += import.duplicate_count;
            counts.created += import.observations_created;
            counts
        })
}

pub(super) fn zero_evidence_diagnosis(report: &SessionReport) -> Option<String> {
    (report.evidence.overall.observation_counts.usable == 0).then(|| {
        let callsign = &report.overview.scope.station.callsign;
        let counts = wspr_live_capture_counts(report);
        let lead =
            format!("No usable observations were recorded for {callsign} in the captured windows.");
        if counts.successful_windows == 0 {
            return format!("{lead} No successful WSPR.live captured-window summary was recorded.");
        }
        if counts.returned == 0 {
            return format!(
                "{lead} WSPR.live completed {} successful window{} and returned zero rows.",
                counts.successful_windows,
                plural_suffix(counts.successful_windows),
            );
        }
        if counts.created == 0 {
            return format!(
                "{lead} WSPR.live completed {} successful window{}, returned {} row{}, and created no observations ({} accepted, {} filtered, {} conflicted, {} duplicated).",
                counts.successful_windows,
                plural_suffix(counts.successful_windows),
                counts.returned,
                plural_suffix(counts.returned),
                counts.accepted,
                counts.filtered,
                counts.conflicted,
                counts.duplicated,
            );
        }
        format!(
            "{lead} WSPR.live created {} observation{}, but none was usable in this report scope; the recorded acquisition counts do not identify a cause.",
            counts.created,
            plural_suffix(counts.created),
        )
    })
}
