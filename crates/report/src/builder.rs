use antennabench_analysis::{
    summarize_bundle, AnalysisSummary, AntennaEvidenceSummary, BandEvidenceSummary,
    EvidenceSummary, ObservationKindCount, PairedComparisonAnalysis, SlotEvidenceSummary,
};
use antennabench_core::{BundleContents, ObservationKind, PlannedSlot};
use chrono::Duration;

use crate::{
    AntennaEvidenceSection, AntennaSnrRow, BandEvidenceCountRow, BandEvidenceSection,
    EvidenceSections, ReportChartData, ReportComparisonData, ReportError, ReportEvidenceSummary,
    ReportNotice, ScheduleOverview, ScheduledSlotContext, ScheduledTimeRange, SessionContext,
    SessionReport, SlotEvidenceCountRow, SlotEvidenceSection, StationContext,
    UsableObservationKindCounts,
};

pub fn build_report(bundle: &BundleContents) -> Result<SessionReport, ReportError> {
    let AnalysisSummary {
        session_id: _,
        evidence_quality,
        overall,
        antennas,
        bands,
        slots,
        comparison,
    } = summarize_bundle(bundle)?;

    let context = build_context(bundle, &bands);
    let evidence = EvidenceSections {
        evidence_quality,
        overall: project_evidence(overall),
        antennas: antennas.into_iter().map(project_antenna).collect(),
        bands: bands.into_iter().map(project_band).collect(),
        slots: slots.into_iter().map(project_slot).collect(),
    };
    let chart_data = build_chart_data(&evidence);
    let notices = build_notices(&context, &evidence);

    Ok(SessionReport {
        context,
        evidence,
        comparison: project_comparison(comparison),
        chart_data,
        notices,
    })
}

fn project_comparison(comparison: PairedComparisonAnalysis) -> ReportComparisonData {
    ReportComparisonData {
        availability: comparison.availability,
        left_label: comparison.left_label,
        right_label: comparison.right_label,
        delta_orientation: comparison.delta_orientation,
        diagnostics: comparison.diagnostics,
        blocks: comparison.blocks,
        overlap_rows: comparison.overlap_rows,
        timeline_rows: comparison.timeline_rows,
        paired_rows: comparison.paired_rows,
        path_summaries: comparison.path_summaries,
        strata: comparison.strata,
    }
}

fn build_context(
    bundle: &BundleContents,
    analyzed_bands: &[BandEvidenceSummary],
) -> SessionContext {
    let slots = bundle
        .schedule
        .slots
        .iter()
        .map(project_scheduled_slot)
        .collect::<Vec<_>>();
    let scheduled_time_range =
        slots
            .first()
            .zip(slots.last())
            .map(|(first, last)| ScheduledTimeRange {
                starts_at: first.starts_at,
                ends_at: last.ends_at,
            });
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

    SessionContext {
        session_id: bundle.manifest.session_id.clone(),
        station: StationContext {
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            power_watts: bundle.station.power_watts,
        },
        experiment_mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        scheduled_time_range,
        antennas: bundle.antennas.antennas.clone(),
        bands,
        schedule: ScheduleOverview {
            slot_count: slots.len(),
            slots,
        },
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
