use crate::{ReportEvidenceSummary, SessionReport};
use antennabench_analysis::{ObservationCounts, SnrStatistics};

use super::super::{geometry::geometry_class, shared::*};

#[derive(Debug, Clone)]
pub(in crate::html) struct EvidenceSummaryView {
    pub(in crate::html) total: usize,
    pub(in crate::html) usable: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) kinds: String,
    pub(in crate::html) exclusions: String,
    pub(in crate::html) snr: String,
}

impl EvidenceSummaryView {
    pub(in crate::html) fn new(evidence: &ReportEvidenceSummary) -> Self {
        Self {
            total: evidence.observation_counts.total,
            usable: evidence.observation_counts.usable,
            excluded: evidence.observation_counts.excluded,
            kinds: kinds_text(evidence.usable_observation_kinds),
            exclusions: exclusions_text(evidence),
            snr: snr_text(evidence.snr),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SnrView {
    pub(in crate::html) sample_count: usize,
    pub(in crate::html) minimum: String,
    pub(in crate::html) median: String,
    pub(in crate::html) mean: String,
    pub(in crate::html) maximum: String,
    pub(in crate::html) left_class: String,
    pub(in crate::html) width_class: String,
    pub(in crate::html) median_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SnrRowView {
    pub(in crate::html) label: String,
    pub(in crate::html) usable_observation_count: usize,
    pub(in crate::html) snr: Option<SnrView>,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct CountChartRowView {
    pub(in crate::html) label: String,
    pub(in crate::html) total: usize,
    pub(in crate::html) usable: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) local: usize,
    pub(in crate::html) public: usize,
    pub(in crate::html) imported: usize,
    pub(in crate::html) usable_class: String,
    pub(in crate::html) excluded_class: String,
}

fn count_chart_row(
    label: String,
    counts: ObservationCounts,
    kinds: crate::UsableObservationKindCounts,
) -> CountChartRowView {
    let denominator = counts.total.max(1) as f64;
    CountChartRowView {
        label,
        total: counts.total,
        usable: counts.usable,
        excluded: counts.excluded,
        local: kinds.local_decode,
        public: kinds.public_report,
        imported: kinds.imported_spot,
        usable_class: geometry_class(counts.usable as f64 / denominator * 100.0),
        excluded_class: geometry_class(counts.excluded as f64 / denominator * 100.0),
    }
}

fn snr_rows(report: &SessionReport) -> Vec<SnrRowView> {
    let bounds = report
        .chart_data
        .antenna_snr
        .iter()
        .filter_map(|row| row.snr)
        .fold(None::<(f64, f64)>, |bounds, snr| {
            Some(match bounds {
                None => (snr.min_db, snr.max_db),
                Some((min, max)) => (min.min(snr.min_db), max.max(snr.max_db)),
            })
        });

    report
        .chart_data
        .antenna_snr
        .iter()
        .map(|row| {
            let snr = row.snr.map(|snr| {
                let (left_class, width_class, median_class) = bounds.map_or_else(
                    || (String::new(), String::new(), String::new()),
                    |(min, max)| {
                        let range = (max - min).max(1.0);
                        (
                            geometry_class((snr.min_db - min) / range * 100.0),
                            geometry_class((snr.max_db - snr.min_db) / range * 100.0),
                            geometry_class((snr.median_db - min) / range * 100.0),
                        )
                    },
                );
                SnrView {
                    sample_count: snr.sample_count,
                    minimum: format_number(snr.min_db),
                    median: format_number(snr.median_db),
                    mean: format_number(snr.mean_db),
                    maximum: format_number(snr.max_db),
                    left_class,
                    width_class,
                    median_class,
                }
            });
            SnrRowView {
                label: row.antenna_label.clone(),
                usable_observation_count: row.usable_observation_count,
                snr,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AntennaEvidenceRowView {
    pub(in crate::html) label: String,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) contributing_slots: usize,
    pub(in crate::html) counts: String,
    pub(in crate::html) kinds: String,
    pub(in crate::html) exclusions: String,
    pub(in crate::html) snr: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct AntennaSectionView {
    pub(in crate::html) snr_rows: Vec<SnrRowView>,
    pub(in crate::html) evidence_rows: Vec<AntennaEvidenceRowView>,
}

impl AntennaSectionView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        Self {
            snr_rows: snr_rows(report),
            evidence_rows: report
                .evidence
                .antennas
                .iter()
                .map(|row| AntennaEvidenceRowView {
                    label: row.antenna_label.clone(),
                    coverage: evidence_coverage(row.evidence_quality),
                    contributing_slots: row.contributing_slot_count,
                    counts: counts_text(row.evidence.observation_counts),
                    kinds: kinds_text(row.evidence.usable_observation_kinds),
                    exclusions: exclusions_text(&row.evidence),
                    snr: snr_text(row.evidence.snr),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct BandEvidenceRowView {
    pub(in crate::html) band: &'static str,
    pub(in crate::html) counts: String,
    pub(in crate::html) kinds: String,
    pub(in crate::html) exclusions: String,
    pub(in crate::html) snr: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct BandSectionView {
    pub(in crate::html) chart_rows: Vec<CountChartRowView>,
    pub(in crate::html) evidence_rows: Vec<BandEvidenceRowView>,
}

impl BandSectionView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        Self {
            chart_rows: report
                .chart_data
                .band_evidence_counts
                .iter()
                .map(|row| {
                    count_chart_row(
                        band(row.band).to_string(),
                        row.observation_counts,
                        row.usable_observation_kinds,
                    )
                })
                .collect(),
            evidence_rows: report
                .evidence
                .bands
                .iter()
                .map(|row| BandEvidenceRowView {
                    band: band(row.band),
                    counts: counts_text(row.evidence.observation_counts),
                    kinds: kinds_text(row.evidence.usable_observation_kinds),
                    exclusions: exclusions_text(&row.evidence),
                    snr: snr_text(row.evidence.snr),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SlotChartRowView {
    pub(in crate::html) chart: CountChartRowView,
    pub(in crate::html) sequence: u32,
    pub(in crate::html) slot_id: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) planned: String,
    pub(in crate::html) actual: String,
    pub(in crate::html) status: &'static str,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SlotEvidenceRowView {
    pub(in crate::html) sequence: u32,
    pub(in crate::html) slot_id: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) planned: String,
    pub(in crate::html) actual: String,
    pub(in crate::html) status: &'static str,
    pub(in crate::html) starts_at: String,
    pub(in crate::html) ends_at: String,
    pub(in crate::html) usable_start: String,
    pub(in crate::html) switch: String,
    pub(in crate::html) counts: String,
    pub(in crate::html) exclusions: String,
    pub(in crate::html) snr: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct SlotSectionView {
    pub(in crate::html) chart_rows: Vec<SlotChartRowView>,
    pub(in crate::html) evidence_rows: Vec<SlotEvidenceRowView>,
}

impl SlotSectionView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        Self {
            chart_rows: report
                .chart_data
                .slot_evidence_counts
                .iter()
                .map(|row| SlotChartRowView {
                    chart: count_chart_row(
                        format!("#{} {}", row.sequence_number, row.planned_label),
                        row.observation_counts,
                        crate::UsableObservationKindCounts::default(),
                    ),
                    sequence: row.sequence_number,
                    slot_id: row.slot_id.clone(),
                    band: band(row.band),
                    planned: row.planned_label.clone(),
                    actual: row.actual_label.clone().unwrap_or_else(not_recorded),
                    status: slot_status(row.status),
                })
                .collect(),
            evidence_rows: report
                .evidence
                .slots
                .iter()
                .map(|row| {
                    let switch = match (
                        row.switch_event_id.as_deref(),
                        row.switch_timestamp,
                        row.switch_delay_seconds,
                    ) {
                        (Some(event_id), Some(timestamp_value), Some(delay)) => format!(
                            "{} at {}; {delay} s from start",
                            event_id,
                            timestamp(timestamp_value)
                        ),
                        _ => not_recorded(),
                    };
                    SlotEvidenceRowView {
                        sequence: row.sequence_number,
                        slot_id: row.slot_id.clone(),
                        band: band(row.band),
                        planned: row.planned_label.clone(),
                        actual: row.actual_label.clone().unwrap_or_else(not_recorded),
                        status: slot_status(row.status),
                        starts_at: timestamp(row.starts_at),
                        ends_at: timestamp(row.ends_at),
                        usable_start: timestamp(row.usable_start),
                        switch,
                        counts: counts_text(row.evidence.observation_counts),
                        exclusions: exclusions_text(&row.evidence),
                        snr: snr_text(row.evidence.snr),
                    }
                })
                .collect(),
        }
    }
}

fn counts_text(counts: ObservationCounts) -> String {
    format!(
        "{} total; {} usable; {} excluded",
        counts.total, counts.usable, counts.excluded
    )
}

fn kinds_text(kinds: crate::UsableObservationKindCounts) -> String {
    format!(
        "{} local; {} public; {} imported",
        kinds.local_decode, kinds.public_report, kinds.imported_spot
    )
}

fn exclusions_text(evidence: &ReportEvidenceSummary) -> String {
    if evidence.exclusions.is_empty() {
        return "None".to_string();
    }
    evidence
        .exclusions
        .iter()
        .map(|item| format!("{}: {}", exclusion_reason(item.reason), item.count))
        .collect::<Vec<_>>()
        .join("; ")
}

fn snr_text(snr: Option<SnrStatistics>) -> String {
    snr.map(|snr| {
        format!(
            "{} samples; min {}; median {}; mean {}; max {} dB",
            snr.sample_count,
            format_number(snr.min_db),
            format_number(snr.median_db),
            format_number(snr.mean_db),
            format_number(snr.max_db)
        )
    })
    .unwrap_or_else(|| "Not available".to_string())
}
