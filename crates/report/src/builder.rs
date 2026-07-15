use antennabench_analysis::{
    summarize_bundle_with_resources, AnalysisSummary, AntennaEvidenceSummary, BandEvidenceSummary,
    EvidenceSummary, ObservationKindCount, PairedComparisonAnalysis, SlotEvidenceSummary,
};
use antennabench_core::{
    codes, validate_bundle_report, BundleContents, BundleFileRole, BundleRecordKind,
    BundleValidationReport, ObservationKind, PlannedSlot,
};
use chrono::Duration;

use crate::{
    check_cancelled, report_resource_error, AntennaEvidenceSection, AntennaSnrRow,
    BandEvidenceCountRow, BandEvidenceSection, CountingWriter, EvidenceSections,
    ReportCancellationToken, ReportChartData, ReportComparisonData, ReportCompleteness,
    ReportDetailFamily, ReportError, ReportEvidenceSummary, ReportNotice, ReportResourceLimits,
    ReportResourceStage, ReportSnapshotContext, ScheduleOverview, ScheduledSlotContext,
    ScheduledTimeRange, SessionContext, SessionReport, SlotEvidenceCountRow, SlotEvidenceSection,
    StationContext, UsableObservationKindCounts, REPORT_RESOURCE_LIMITS,
};

pub fn build_report(bundle: &BundleContents) -> Result<SessionReport, ReportError> {
    let validation = validate_bundle_report(bundle);
    build_report_with_validation(bundle, &validation)
}

pub fn build_report_with_validation(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
) -> Result<SessionReport, ReportError> {
    build_report_with_snapshot(bundle, validation, ReportSnapshotContext::default())
}

pub fn build_report_with_snapshot(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot(
        bundle,
        validation,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        snapshot,
    )
}

pub fn build_report_with_resources(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot(
        bundle,
        validation,
        limits,
        cancellation,
        ReportSnapshotContext::default(),
    )
}

fn build_report_with_resources_and_snapshot(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    check_cancelled(
        cancellation,
        ReportResourceStage::Projection,
        "report_projection",
    )?;
    let summary =
        summarize_bundle_with_resources(bundle, validation, limits.analysis, cancellation)?;
    let detail_counts = DetailCounts::new(bundle, &summary, &snapshot);
    if summary.eligibility.exclusions.len() as u64 > limits.rows {
        return Err(report_resource_error(
            "resource.report.rows",
            ReportResourceStage::Projection,
            "required_overview_rows",
            limits.rows,
            Some(summary.eligibility.exclusions.len() as u64),
            "rows",
        )
        .into());
    }
    let full_detail = detail_counts.total_rows() <= limits.rows;
    let AnalysisSummary {
        session_id: _,
        evidence_quality,
        overall,
        antennas,
        bands,
        slots,
        comparison,
        mut solar_context,
        eligibility,
    } = summary;

    let context = build_context(bundle, &bands, validation, full_detail);
    let (antenna_evidence, band_evidence, slot_evidence) = if full_detail {
        (
            antennas.into_iter().map(project_antenna).collect(),
            bands.into_iter().map(project_band).collect(),
            slots.into_iter().map(project_slot).collect(),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let evidence = EvidenceSections {
        evidence_quality,
        overall: project_evidence(overall),
        antennas: antenna_evidence,
        bands: band_evidence,
        slots: slot_evidence,
    };
    let chart_data = if full_detail {
        build_chart_data(&evidence)
    } else {
        ReportChartData::default()
    };
    let mut notices = build_notices(&context, &evidence);
    if !full_detail {
        solar_context.rows.clear();
        detail_counts.append_notices(&mut notices);
    }

    let mut report = SessionReport {
        completeness: if full_detail {
            ReportCompleteness::FullDetail
        } else {
            ReportCompleteness::BoundedOverview
        },
        context,
        evidence,
        comparison: project_comparison(comparison, full_detail),
        solar_context,
        chart_data,
        notices,
        snapshot,
        eligibility_exclusions: eligibility.exclusions,
    };
    check_cancelled(cancellation, ReportResourceStage::Serialize, "report_model")?;
    if let Err(error) = check_model_size(&report, limits.model_bytes) {
        if report.completeness == ReportCompleteness::FullDetail {
            make_overview(&mut report, &detail_counts);
            check_model_size(&report, limits.model_bytes)?;
        } else {
            return Err(error);
        }
    }
    Ok(report)
}

fn project_comparison(
    comparison: PairedComparisonAnalysis,
    full_detail: bool,
) -> ReportComparisonData {
    let (blocks, overlap_rows, timeline_rows, paired_rows, path_summaries, strata) = if full_detail
    {
        (
            comparison.blocks,
            comparison.overlap_rows,
            comparison.timeline_rows,
            comparison.paired_rows,
            comparison.path_summaries,
            comparison.strata,
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    ReportComparisonData {
        availability: comparison.availability,
        left_label: comparison.left_label,
        right_label: comparison.right_label,
        delta_orientation: comparison.delta_orientation,
        diagnostics: comparison.diagnostics,
        blocks,
        overlap_rows,
        timeline_rows,
        paired_rows,
        path_summaries,
        strata,
    }
}

fn build_context(
    bundle: &BundleContents,
    analyzed_bands: &[BandEvidenceSummary],
    validation: &BundleValidationReport,
    full_detail: bool,
) -> SessionContext {
    let scheduled_time_range = bundle
        .schedule
        .slots
        .first()
        .zip(bundle.schedule.slots.last())
        .map(|(first, last)| ScheduledTimeRange {
            starts_at: first.starts_at,
            ends_at: last.starts_at + Duration::seconds(i64::from(last.duration_seconds)),
        });
    let slots = if full_detail {
        bundle
            .schedule
            .slots
            .iter()
            .map(project_scheduled_slot)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let bands = analyzed_bands
        .iter()
        .filter(|summary| {
            bundle
                .schedule
                .slots
                .iter()
                .any(|slot| slot.band == summary.band)
        })
        .map(|summary| summary.band)
        .collect();

    let mut antennas = if full_detail {
        bundle.antennas.antennas.clone()
    } else {
        Vec::new()
    };
    for diagnostic in validation.diagnostics() {
        if diagnostic.location.record_kind != Some(BundleRecordKind::Antenna)
            || !matches!(
                diagnostic.code.as_str(),
                codes::NON_FINITE_NUMBER | codes::INVALID_RANGE
            )
        {
            continue;
        }
        let Some(antenna) = diagnostic
            .location
            .record_index
            .and_then(|index| antennas.get_mut(index))
        else {
            continue;
        };
        match diagnostic.location.field_path.as_deref() {
            Some(path) if path.ends_with("/height_m") => antenna.height_m = None,
            Some(path) if path.ends_with("/radial_length_m") => antenna.radial_length_m = None,
            Some(path) if path.ends_with("/orientation_degrees") => {
                antenna.orientation_degrees = None;
            }
            _ => {}
        }
    }
    let station_power_excluded = validation.diagnostics().iter().any(|diagnostic| {
        diagnostic.location.file == BundleFileRole::Station
            && diagnostic.location.field_path.as_deref() == Some("/power_watts")
            && matches!(
                diagnostic.code.as_str(),
                codes::NON_FINITE_NUMBER | codes::INVALID_RANGE
            )
    });

    SessionContext {
        session_id: bundle.manifest.session_id.clone(),
        station: StationContext {
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            power_watts: (!station_power_excluded)
                .then_some(bundle.station.power_watts)
                .flatten(),
        },
        experiment_mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        scheduled_time_range,
        antennas,
        bands,
        schedule: ScheduleOverview {
            slot_count: bundle.schedule.slots.len(),
            slots,
        },
    }
}

#[derive(Debug)]
struct DetailCounts {
    families: Vec<(ReportDetailFamily, usize)>,
    eligibility_rows: usize,
}

impl DetailCounts {
    fn new(
        bundle: &BundleContents,
        summary: &AnalysisSummary,
        snapshot: &ReportSnapshotContext,
    ) -> Self {
        let comparison = &summary.comparison;
        let chart_rows = summary.antennas.len() + summary.bands.len() + summary.slots.len();
        Self {
            families: vec![
                (
                    ReportDetailFamily::LifecycleHistory,
                    snapshot.lifecycle_events.len(),
                ),
                (ReportDetailFamily::Schedule, bundle.schedule.slots.len()),
                (
                    ReportDetailFamily::AntennaContext,
                    bundle.antennas.antennas.len(),
                ),
                (ReportDetailFamily::AntennaEvidence, summary.antennas.len()),
                (ReportDetailFamily::BandEvidence, summary.bands.len()),
                (ReportDetailFamily::SlotEvidence, summary.slots.len()),
                (
                    ReportDetailFamily::ComparisonBlocks,
                    comparison.blocks.len(),
                ),
                (
                    ReportDetailFamily::PathOverlap,
                    comparison.overlap_rows.len(),
                ),
                (
                    ReportDetailFamily::ComparisonTimeline,
                    comparison.timeline_rows.len(),
                ),
                (
                    ReportDetailFamily::PairedObservations,
                    comparison.paired_rows.len(),
                ),
                (
                    ReportDetailFamily::SolarContext,
                    summary.solar_context.rows.len(),
                ),
                (
                    ReportDetailFamily::PathSummaries,
                    comparison.path_summaries.len(),
                ),
                (ReportDetailFamily::Strata, comparison.strata.len()),
                (ReportDetailFamily::Charts, chart_rows),
            ],
            eligibility_rows: summary.eligibility.exclusions.len(),
        }
    }

    fn total_rows(&self) -> u64 {
        (self.eligibility_rows + self.families.iter().map(|(_, count)| *count).sum::<usize>())
            as u64
    }

    fn append_notices(&self, notices: &mut Vec<ReportNotice>) {
        for (family, row_count) in &self.families {
            if *row_count > 0 {
                notices.push(ReportNotice::DetailOmitted {
                    family: *family,
                    row_count: *row_count,
                });
            }
        }
    }
}

fn make_overview(report: &mut SessionReport, counts: &DetailCounts) {
    report.completeness = ReportCompleteness::BoundedOverview;
    report.context.antennas.clear();
    report.context.schedule.slots.clear();
    report.evidence.antennas.clear();
    report.evidence.bands.clear();
    report.evidence.slots.clear();
    report.comparison.blocks.clear();
    report.comparison.overlap_rows.clear();
    report.comparison.timeline_rows.clear();
    report.comparison.paired_rows.clear();
    report.solar_context.rows.clear();
    report.comparison.path_summaries.clear();
    report.comparison.strata.clear();
    report.chart_data = ReportChartData::default();
    report.snapshot.lifecycle_events.clear();
    report
        .notices
        .retain(|notice| !matches!(notice, ReportNotice::DetailOmitted { .. }));
    counts.append_notices(&mut report.notices);
}

fn check_model_size(report: &SessionReport, limit: u64) -> Result<(), ReportError> {
    let mut writer = CountingWriter::new(limit);
    match serde_json::to_writer(&mut writer, report) {
        Ok(()) => Ok(()),
        Err(_error) if writer.observed() > limit => Err(report_resource_error(
            "resource.report.model_bytes",
            ReportResourceStage::Serialize,
            "renderer_neutral_model",
            limit,
            Some(writer.observed()),
            "bytes",
        )
        .into()),
        Err(error) => Err(ReportError::Serialization {
            message: error.to_string(),
        }),
    }
}

fn project_scheduled_slot(slot: &PlannedSlot) -> ScheduledSlotContext {
    ScheduledSlotContext {
        slot_id: slot.slot_id.clone(),
        sequence_number: slot.sequence_number,
        starts_at: slot.starts_at,
        ends_at: slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds)),
        guard_seconds: slot.guard_seconds,
        band: slot.band,
        planned_label: slot.antenna_label.clone(),
    }
}

fn project_antenna(summary: AntennaEvidenceSummary) -> AntennaEvidenceSection {
    AntennaEvidenceSection {
        antenna_label: summary.antenna_label,
        contributing_slot_count: summary.contributing_slot_count,
        evidence_quality: summary.evidence_quality,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_band(summary: BandEvidenceSummary) -> BandEvidenceSection {
    BandEvidenceSection {
        band: summary.band,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_slot(summary: SlotEvidenceSummary) -> SlotEvidenceSection {
    SlotEvidenceSection {
        slot_id: summary.slot_id,
        sequence_number: summary.sequence_number,
        band: summary.band,
        planned_label: summary.planned_label,
        actual_label: summary.actual_label,
        status: summary.status,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_evidence(summary: EvidenceSummary) -> ReportEvidenceSummary {
    ReportEvidenceSummary {
        observation_counts: summary.observation_counts,
        exclusions: summary.exclusions,
        usable_observation_kinds: project_observation_kinds(summary.usable_observation_kinds),
        snr: summary.snr,
    }
}

fn project_observation_kinds(
    kinds: impl IntoIterator<Item = ObservationKindCount>,
) -> UsableObservationKindCounts {
    let mut counts = UsableObservationKindCounts::default();

    for kind in kinds {
        match kind.kind {
            ObservationKind::LocalDecode => counts.local_decode += kind.count,
            ObservationKind::PublicReport => counts.public_report += kind.count,
            ObservationKind::ImportedSpot => counts.imported_spot += kind.count,
        }
    }

    counts
}

fn build_chart_data(evidence: &EvidenceSections) -> ReportChartData {
    ReportChartData {
        antenna_snr: evidence
            .antennas
            .iter()
            .map(|antenna| AntennaSnrRow {
                antenna_label: antenna.antenna_label.clone(),
                usable_observation_count: antenna.evidence.observation_counts.usable,
                snr: antenna.evidence.snr,
            })
            .collect(),
        band_evidence_counts: evidence
            .bands
            .iter()
            .map(|band| BandEvidenceCountRow {
                band: band.band,
                observation_counts: band.evidence.observation_counts,
                usable_observation_kinds: band.evidence.usable_observation_kinds,
            })
            .collect(),
        slot_evidence_counts: evidence
            .slots
            .iter()
            .map(|slot| SlotEvidenceCountRow {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                band: slot.band,
                planned_label: slot.planned_label.clone(),
                actual_label: slot.actual_label.clone(),
                status: slot.status,
                observation_counts: slot.evidence.observation_counts,
            })
            .collect(),
    }
}

fn build_notices(context: &SessionContext, evidence: &EvidenceSections) -> Vec<ReportNotice> {
    let mut notices = Vec::new();

    if context.schedule.slots.is_empty() {
        notices.push(ReportNotice::NoScheduledSlots);
    }
    if evidence.overall.observation_counts.usable == 0 {
        notices.push(ReportNotice::NoUsableObservations);
    }
    if evidence.overall.snr.is_none() {
        notices.push(ReportNotice::NoUsableSnrSamples);
    }

    notices
}
