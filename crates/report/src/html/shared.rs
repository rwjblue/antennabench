use std::fmt::{self, Write};

use crate::{
    report_resource_error, ReportCancellationToken, ReportDetailFamily, ReportImportedEvidence,
    ReportNotice, ReportOperatorEventKind, ReportOverviewStratum, ReportProviderCompleteness,
    ReportResourceStage,
};
use antennabench_analysis::{
    ComparisonAvailability, ComparisonBlockEligibility, ComparisonOrder, EvidenceQuality,
    ObservationExclusionReason, PathDirection,
};
use antennabench_core::{
    v3::WsprCycleDirection, AlignedSlotStatus, Band, ExperimentMode, ObservationKind, RecordSource,
    SessionGoal,
};
use chrono::{SecondsFormat, Utc};

pub(super) struct CheckedHtmlWriter<'a> {
    output: String,
    limit: u64,
    observed: u64,
    failure: Option<crate::ReportResourceError>,
    cancellation: &'a ReportCancellationToken,
    last_cancellation_check: u64,
}

impl<'a> CheckedHtmlWriter<'a> {
    pub(super) fn new(limit: u64, cancellation: &'a ReportCancellationToken) -> Self {
        Self {
            output: String::with_capacity(32_768.min(limit as usize)),
            limit,
            observed: 0,
            failure: None,
            cancellation,
            last_cancellation_check: 0,
        }
    }

    pub(super) fn push_str(&mut self, value: &str) {
        if self.failure.is_some() {
            return;
        }
        let observed = self.output.len() as u64 + value.len() as u64;
        self.observed = observed;
        if observed > self.limit {
            self.failure = Some(report_resource_error(
                "resource.report.html_bytes",
                ReportResourceStage::Render,
                "standalone_html",
                self.limit,
                Some(observed),
                "bytes",
            ));
            return;
        }
        if observed.saturating_sub(self.last_cancellation_check) >= 64 * 1024 {
            self.last_cancellation_check = observed;
            if self.cancellation.is_cancelled() {
                self.failure = Some(report_resource_error(
                    "resource.operation.cancelled",
                    ReportResourceStage::Render,
                    "standalone_html",
                    0,
                    Some(observed),
                    "checkpoints",
                ));
                return;
            }
        }
        self.output.push_str(value);
    }

    pub(super) fn finish(self) -> Result<String, crate::ReportResourceError> {
        if let Some(failure) = self.failure {
            Err(failure)
        } else if self.cancellation.is_cancelled() {
            Err(report_resource_error(
                "resource.operation.cancelled",
                ReportResourceStage::Render,
                "standalone_html",
                0,
                Some(self.observed),
                "checkpoints",
            ))
        } else {
            Ok(self.output)
        }
    }
}

impl fmt::Write for CheckedHtmlWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.push_str(value);
        Ok(())
    }
}

pub(super) fn fact(out: &mut CheckedHtmlWriter<'_>, label: &str, value: &str) {
    write_html!(
        out,
        "<div class=\"fact\"><dt>{}</dt><dd>{}</dd></div>",
        label,
        escape_html(value)
    );
}

pub(super) fn detail(out: &mut CheckedHtmlWriter<'_>, label: &str, value: &str) {
    write_html!(out, "<dt>{}</dt><dd>{}</dd>", label, escape_html(value));
}

pub(super) fn optional_join(values: &[String]) -> String {
    if values.is_empty() {
        not_recorded()
    } else {
        values.join(", ")
    }
}

pub(super) fn optional_measure(value: Option<f32>, unit: &str) -> String {
    value
        .map(|value| format!("{} {unit}", format_number(f64::from(value))))
        .unwrap_or_else(not_recorded)
}

pub(super) fn not_recorded() -> String {
    "Not recorded".to_string()
}
pub(super) fn not_available() -> String {
    "Not available".to_string()
}

pub(super) fn timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(super) fn format_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

pub(super) fn band(value: Band) -> &'static str {
    match value {
        Band::M160 => "160 m",
        Band::M80 => "80 m",
        Band::M60 => "60 m",
        Band::M40 => "40 m",
        Band::M30 => "30 m",
        Band::M20 => "20 m",
        Band::M17 => "17 m",
        Band::M15 => "15 m",
        Band::M12 => "12 m",
        Band::M10 => "10 m",
        Band::M6 => "6 m",
        Band::M2 => "2 m",
    }
}
pub(super) fn experiment_mode(value: ExperimentMode) -> &'static str {
    match value {
        ExperimentMode::WholeStationAb => "Whole-station A/B",
        ExperimentMode::TxFocused => "Transmit-focused",
        ExperimentMode::RxFocused => "Receive-focused",
        ExperimentMode::SingleAntennaProfiling => "Single-antenna profiling",
    }
}
pub(super) fn session_goal(value: SessionGoal) -> &'static str {
    match value {
        SessionGoal::Dx => "DX",
        SessionGoal::Regional => "Regional",
        SessionGoal::NvisLocal => "NVIS / local",
        SessionGoal::GeneralCoverage => "General coverage",
        SessionGoal::WeakSignalReliability => "Weak-signal reliability",
        SessionGoal::SingleAntennaProfiling => "Single-antenna profiling",
    }
}
pub(super) fn evidence_coverage(value: EvidenceQuality) -> &'static str {
    match value {
        EvidenceQuality::Insufficient => "Insufficient",
        EvidenceQuality::Weak => "Weak",
        EvidenceQuality::Moderate => "Moderate",
    }
}
pub(super) fn comparison_availability_label(value: ComparisonAvailability) -> &'static str {
    match value {
        ComparisonAvailability::NotApplicable => "Not applicable",
        ComparisonAvailability::UnsupportedComparisonShape => "Unsupported comparison shape",
        ComparisonAvailability::NoEligibleBlocks => "No eligible blocks",
        ComparisonAvailability::NoMatchedPaths => "No matched paths",
        ComparisonAvailability::DescriptivePairsAvailable => "Descriptive pairs available",
    }
}
pub(super) fn comparison_availability_text(value: ComparisonAvailability) -> &'static str {
    match value {
        ComparisonAvailability::NotApplicable => {
            "Single-antenna profiling does not create an A/B comparison."
        }
        ComparisonAvailability::UnsupportedComparisonShape => {
            "A paired comparison requires exactly two scheduled antenna labels."
        }
        ComparisonAvailability::NoEligibleBlocks => {
            "No adjacent same-band block contained one usable actual slot for each label."
        }
        ComparisonAvailability::NoMatchedPaths => {
            "Eligible blocks exist, but no remote path had usable signal reports on both antennas within one comparison group."
        }
        ComparisonAvailability::DescriptivePairsAvailable => {
            "Usable same-path matched pairs are available for descriptive display only."
        }
    }
}
pub(super) fn comparison_stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        escape_html(value.mode.as_str()),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}
pub(super) fn comparison_strata_list(rows: &[&ReportOverviewStratum]) -> String {
    rows.iter()
        .map(|row| comparison_stratum(&row.stratum))
        .collect::<Vec<_>>()
        .join("; ")
}
pub(super) fn comparison_groups_label(count: usize) -> String {
    format!(
        "{count} comparison {}",
        if count == 1 { "group" } else { "groups" }
    )
}
pub(super) fn report_antenna_labels(report: &crate::SessionReport) -> (String, String) {
    (
        escape_html(report.comparison.left_label.as_deref().unwrap_or("Left")),
        escape_html(report.comparison.right_label.as_deref().unwrap_or("Right")),
    )
}
pub(super) fn orientation_antenna_labels(
    orientation: &antennabench_analysis::DeltaOrientation,
) -> (String, String) {
    (
        escape_html(&orientation.subtrahend_label),
        escape_html(&orientation.minuend_label),
    )
}
pub(super) fn labeled_comparison_order(
    value: ComparisonOrder,
    left_label: &str,
    right_label: &str,
) -> String {
    match value {
        ComparisonOrder::LeftThenRight => format!("{left_label} then {right_label}"),
        ComparisonOrder::RightThenLeft => format!("{right_label} then {left_label}"),
    }
}
pub(super) fn path_direction(value: PathDirection) -> &'static str {
    match value {
        PathDirection::Transmit => "TX path",
        PathDirection::Receive => "RX path",
    }
}
pub(super) fn observation_kind(value: ObservationKind) -> &'static str {
    match value {
        ObservationKind::LocalDecode => "Local decode",
        ObservationKind::PublicReport => "Public report",
        ObservationKind::ImportedSpot => "Imported spot",
    }
}
pub(super) fn record_source(value: RecordSource) -> &'static str {
    match value {
        RecordSource::Operator => "Operator",
        RecordSource::WsjtxUdp => "WSJT-X UDP (direct/local)",
        RecordSource::WsjtxLog => "WSJT-X log",
        RecordSource::Wsprnet => "WSPRnet",
        RecordSource::WsprLive => "WSPR.live (delayed/public)",
        RecordSource::ImportedFile => "Imported file",
        RecordSource::RigAdapter => "Rig adapter",
        RecordSource::NoaaSwpc => "NOAA SWPC",
        RecordSource::Derived => "Derived",
    }
}

pub(super) fn block_eligibility(value: ComparisonBlockEligibility) -> &'static str {
    match value {
        ComparisonBlockEligibility::Eligible => "Eligible",
        ComparisonBlockEligibility::AmbiguousSequenceOrder => "Ambiguous sequence order",
        ComparisonBlockEligibility::IncompleteSameBandRun => "Incomplete same-band run",
        ComparisonBlockEligibility::MissingActualLabel => "Missing actual antenna",
        ComparisonBlockEligibility::RepeatedLabel => "Repeated actual antenna",
        ComparisonBlockEligibility::UnsupportedLabel => "Unsupported actual antenna",
    }
}

pub(super) fn wspr_direction(value: WsprCycleDirection) -> &'static str {
    match value {
        WsprCycleDirection::Receive => "Receive",
        WsprCycleDirection::Transmit => "Transmit",
    }
}

pub(super) fn wspr_readiness(value: crate::ReportWsprReadinessBasis) -> &'static str {
    match value {
        crate::ReportWsprReadinessBasis::OperatorConfirmed => "Operator confirmed",
        crate::ReportWsprReadinessBasis::CommandVerified => "Command verified",
    }
}

pub(super) fn wspr_attribution(value: crate::ReportWsprAttribution) -> &'static str {
    match value {
        crate::ReportWsprAttribution::Pending => "Not yet run",
        crate::ReportWsprAttribution::Skipped => "Skipped by operator",
        crate::ReportWsprAttribution::Attributable => "Full antenna occupancy recorded",
        crate::ReportWsprAttribution::UnknownAntennaOccupancy => "Unknown antenna occupancy",
    }
}

pub(super) fn operator_event_kind(value: ReportOperatorEventKind) -> &'static str {
    match value {
        ReportOperatorEventKind::SessionStarted => "Session started",
        ReportOperatorEventKind::SessionInterrupted => "Session interrupted",
        ReportOperatorEventKind::InterruptionDetected => "Interruption detected",
        ReportOperatorEventKind::SessionResumed => "Session resumed",
        ReportOperatorEventKind::SessionEnded => "Session ended",
        ReportOperatorEventKind::SessionAbandoned => "Session abandoned",
        ReportOperatorEventKind::AntennaSwitchStarted => "Antenna switch started",
        ReportOperatorEventKind::WsprCycleArmed => "WSPR cycle armed",
        ReportOperatorEventKind::AntennaStateConfirmed => "Antenna state confirmed",
        ReportOperatorEventKind::SignalStateConfirmed => "Signal state confirmed",
        ReportOperatorEventKind::SlotMissed => "Slot missed",
        ReportOperatorEventKind::SlotBad => "Slot bad",
        ReportOperatorEventKind::NoteAdded => "Note added",
        ReportOperatorEventKind::EventCorrected => "Event corrected",
        ReportOperatorEventKind::Switched => "Switched",
    }
}

pub(super) fn correction_action(value: crate::ReportEventCorrectionAction) -> &'static str {
    match value {
        crate::ReportEventCorrectionAction::Retracted => "Retracted",
        crate::ReportEventCorrectionAction::Replaced => "Replaced",
    }
}

pub(super) fn import_source_boundary(import: &ReportImportedEvidence) -> &'static str {
    if import.provider_id == "wspr-live" {
        "best-effort WSPR.live request-window collection; upstream mirror has no independent completeness guarantee"
    } else {
        match import.provider_completeness {
            ReportProviderCompleteness::Known => "upstream completeness guarantee recorded",
            ReportProviderCompleteness::Unknown => {
                "upstream completeness guarantee not independently recorded"
            }
            ReportProviderCompleteness::Unsupported => {
                "provider completeness assertion unsupported"
            }
        }
    }
}

pub(super) fn provider_completeness_sentence(
    completeness: ReportProviderCompleteness,
) -> &'static str {
    match completeness {
        ReportProviderCompleteness::Known => "Provider completeness is recorded as known.",
        ReportProviderCompleteness::Unknown => {
            "Upstream completeness is not independently guaranteed."
        }
        ReportProviderCompleteness::Unsupported => {
            "Provider completeness is unsupported for this acquisition type."
        }
    }
}

pub(super) fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}
pub(super) fn format_signed(value: f64) -> String {
    if value > 0.0 {
        format!("+{}", format_number(value))
    } else {
        format_number(value)
    }
}
pub(super) fn slot_status(value: AlignedSlotStatus) -> &'static str {
    match value {
        AlignedSlotStatus::PlannedNoSwitchEvent => "Planned; no switch event",
        AlignedSlotStatus::UnknownActualState => "Actual antenna unknown",
        AlignedSlotStatus::Switched => "Switched",
        AlignedSlotStatus::LateSwitch => "Late switch",
        AlignedSlotStatus::Missed => "Missed",
        AlignedSlotStatus::Bad => "Bad",
        AlignedSlotStatus::ConflictingEvidence => "Conflicting operator evidence",
    }
}
pub(super) fn exclusion_reason(value: ObservationExclusionReason) -> &'static str {
    match value {
        ObservationExclusionReason::GuardTime => "Guard time",
        ObservationExclusionReason::NearBoundary => "Near boundary",
        ObservationExclusionReason::BeforeObservedSwitch => "Before observed switch",
        ObservationExclusionReason::MissedSlot => "Missed slot",
        ObservationExclusionReason::BadSlot => "Bad slot",
        ObservationExclusionReason::BandMismatch => "Band mismatch",
        ObservationExclusionReason::OutsideSchedule => "Outside schedule",
        ObservationExclusionReason::MissingEvidence => "Missing evidence",
        ObservationExclusionReason::MalformedEvidence => "Malformed evidence",
        ObservationExclusionReason::ContradictoryEvidence => "Contradictory evidence",
        ObservationExclusionReason::UnsupportedEvidence => "Unsupported evidence",
        ObservationExclusionReason::DuplicateEvidence => "Duplicate evidence",
    }
}
pub(super) fn notice_text(value: &ReportNotice) -> String {
    match value {
        ReportNotice::NoScheduledSlots => {
            "No scheduled slots are available; schedule comparisons cannot be shown.".to_string()
        }
        ReportNotice::NoUsableObservations => {
            "No observations met the current evidence rules; no usable counts are inferred."
                .to_string()
        }
        ReportNotice::NoUsableSnrSamples => {
            "No usable SNR samples are available; SNR statistics are shown as unavailable."
                .to_string()
        }
        ReportNotice::DetailOmitted { family, row_count } => {
            format!(
                "Bounded overview: full {} detail is omitted ({} rows); no rows were sampled.",
                detail_family(*family),
                row_count
            )
        }
    }
}

pub(super) fn detail_family(value: ReportDetailFamily) -> &'static str {
    match value {
        ReportDetailFamily::LifecycleHistory => "lifecycle history",
        ReportDetailFamily::Schedule => "schedule",
        ReportDetailFamily::AntennaContext => "antenna context",
        ReportDetailFamily::AntennaEvidence => "antenna evidence",
        ReportDetailFamily::BandEvidence => "band evidence",
        ReportDetailFamily::SlotEvidence => "slot evidence",
        ReportDetailFamily::ExclusionRecords => "excluded observation",
        ReportDetailFamily::OperatorEvents => "operator-event audit",
        ReportDetailFamily::ComparisonBlocks => "comparison block",
        ReportDetailFamily::PathOverlap => "path overlap",
        ReportDetailFamily::ComparisonTimeline => "comparison timeline",
        ReportDetailFamily::PairedObservations => "paired observation",
        ReportDetailFamily::SolarContext => "solar-context",
        ReportDetailFamily::PathSummaries => "path summary",
        ReportDetailFamily::Strata => "comparison group",
        ReportDetailFamily::ObservedPathProfileRows => "observed-path profile",
        ReportDetailFamily::ReporterActivityAudit => "reporter-activity audit",
        ReportDetailFamily::CoverageMapReporters => "coverage-map reporter",
        ReportDetailFamily::Charts => "chart",
    }
}
