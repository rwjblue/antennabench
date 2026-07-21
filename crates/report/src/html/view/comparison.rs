use crate::SessionReport;
use antennabench_analysis::ComparisonSide;

use super::super::{geometry::geometry_class, presentation::*, shared::*};

#[derive(Debug, Clone)]
pub(in crate::html) struct StatView {
    pub(in crate::html) label: String,
    pub(in crate::html) value: usize,
}

pub(in crate::html) fn comparison_diagnostic_stats(report: &SessionReport) -> Vec<StatView> {
    let d = report.comparison.diagnostics;
    let AntennaLabels { left, right } = antenna_labels(report);
    vec![
        ("Blocks".into(), d.block_count),
        ("Eligible blocks".into(), d.eligible_block_count),
        ("Invalid blocks".into(), d.invalid_block_count),
        (
            format!("{left} then {right}"),
            d.left_then_right_block_count,
        ),
        (
            format!("{right} then {left}"),
            d.right_then_left_block_count,
        ),
        ("Matched pairs".into(), d.paired_row_count),
        ("Unique paths".into(), d.unique_path_count),
        (format!("Unmatched — {left}"), d.unmatched_left_count),
        (format!("Unmatched — {right}"), d.unmatched_right_count),
        (format!("Missing SNR — {left}"), d.missing_snr_left_count),
        (format!("Missing SNR — {right}"), d.missing_snr_right_count),
        (
            "Missing or invalid mode".into(),
            d.missing_or_invalid_mode_count,
        ),
        ("Missing mode".into(), d.missing_mode_count),
        ("Malformed mode".into(), d.malformed_mode_count),
        ("Ambiguous paths".into(), d.ambiguous_path_count),
        ("Exact duplicates collapsed".into(), d.exact_duplicate_count),
        (
            "Conflicting duplicate groups".into(),
            d.conflicting_duplicate_group_count,
        ),
        ("Alignment exclusions".into(), d.excluded_observation_count),
    ]
    .into_iter()
    .map(|(label, value)| StatView { label, value })
    .collect()
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverlapRowView {
    pub(in crate::html) stratum: String,
    pub(in crate::html) remote_path: String,
    pub(in crate::html) left: usize,
    pub(in crate::html) right: usize,
    pub(in crate::html) paired: usize,
    pub(in crate::html) unmatched_left: usize,
    pub(in crate::html) unmatched_right: usize,
    pub(in crate::html) missing_left: usize,
    pub(in crate::html) missing_right: usize,
    pub(in crate::html) duplicates: usize,
    pub(in crate::html) conflicts: usize,
    pub(in crate::html) left_class: String,
    pub(in crate::html) right_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct OverlapView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) rows: Vec<OverlapRowView>,
}

impl OverlapView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels {
            left: left_label,
            right: right_label,
        } = antenna_labels(report);
        Self {
            left_label,
            right_label,
            rows: report
                .comparison
                .overlap_rows
                .iter()
                .map(|row| {
                    let total = (row.left_finite_count + row.right_finite_count).max(1) as f64;
                    OverlapRowView {
                        stratum: comparison_group_label(&row.stratum),
                        remote_path: row.remote_path.clone(),
                        left: row.left_finite_count,
                        right: row.right_finite_count,
                        paired: row.paired_count,
                        unmatched_left: row.unmatched_left_count,
                        unmatched_right: row.unmatched_right_count,
                        missing_left: row.missing_snr_left_count,
                        missing_right: row.missing_snr_right_count,
                        duplicates: row.exact_duplicate_count,
                        conflicts: row.conflicting_duplicate_group_count,
                        left_class: geometry_class(row.left_finite_count as f64 / total * 100.0),
                        right_class: geometry_class(row.right_finite_count as f64 / total * 100.0),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct TimelineRowView {
    pub(in crate::html) class: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) eligible: &'static str,
    pub(in crate::html) sequence: u32,
    pub(in crate::html) slot_id: String,
    pub(in crate::html) starts_at: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) actual: String,
    pub(in crate::html) side: String,
    pub(in crate::html) status: &'static str,
    pub(in crate::html) total: usize,
    pub(in crate::html) usable: usize,
    pub(in crate::html) excluded: usize,
    pub(in crate::html) missing_snr: usize,
    pub(in crate::html) missing_mode: usize,
    pub(in crate::html) ambiguous: usize,
    pub(in crate::html) duplicates: usize,
    pub(in crate::html) conflicts: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct TimelineView {
    pub(in crate::html) rows: Vec<TimelineRowView>,
}

impl TimelineView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels { left, right } = antenna_labels(report);
        Self {
            rows: report
                .comparison
                .timeline_rows
                .iter()
                .map(|row| {
                    let invalid = if row.block_eligible { "" } else { " invalid" };
                    let issue = if row.excluded_observation_count > 0
                        || row.missing_snr_count > 0
                        || row.missing_or_invalid_mode_count > 0
                        || row.ambiguous_path_count > 0
                        || row.conflicting_duplicate_group_count > 0
                    {
                        " issue"
                    } else {
                        ""
                    };
                    TimelineRowView {
                        class: format!("{invalid}{issue}"),
                        block: row.block_index + 1,
                        eligible: yes_no(row.block_eligible),
                        sequence: row.sequence_number,
                        slot_id: row.slot_id.clone(),
                        starts_at: timestamp(row.starts_at),
                        band: band(row.band),
                        actual: row.actual_label.clone().unwrap_or_else(not_recorded),
                        side: match row.side {
                            Some(ComparisonSide::Left) => left.clone(),
                            Some(ComparisonSide::Right) => right.clone(),
                            None => "Unavailable".into(),
                        },
                        status: slot_status(row.status),
                        total: row.total_observation_count,
                        usable: row.usable_observation_count,
                        excluded: row.excluded_observation_count,
                        missing_snr: row.missing_snr_count,
                        missing_mode: row.missing_or_invalid_mode_count,
                        ambiguous: row.ambiguous_path_count,
                        duplicates: row.exact_duplicate_count,
                        conflicts: row.conflicting_duplicate_group_count,
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ComparisonBlockView {
    pub(in crate::html) block: usize,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) first_slot: String,
    pub(in crate::html) first_actual_status: String,
    pub(in crate::html) second_slot: String,
    pub(in crate::html) second_actual_status: String,
    pub(in crate::html) order: String,
    pub(in crate::html) eligibility: &'static str,
}

pub(in crate::html) fn comparison_blocks(report: &SessionReport) -> Vec<ComparisonBlockView> {
    let AntennaLabels { left, right } = antenna_labels(report);
    report
        .comparison
        .blocks
        .iter()
        .map(|block| ComparisonBlockView {
            block: block.block_index + 1,
            band: band(block.band),
            first_slot: format!(
                "{} · #{} · {}",
                block.first_slot_id,
                block.first_sequence_number,
                timestamp(block.first_starts_at)
            ),
            first_actual_status: format!(
                "{} / {}",
                block.first_label.as_deref().unwrap_or("Not recorded"),
                slot_status(block.first_status)
            ),
            second_slot: block
                .second_slot_id
                .as_ref()
                .map(|id| {
                    format!(
                        "{} · #{} · {}",
                        id,
                        block.second_sequence_number.unwrap_or_default(),
                        block
                            .second_starts_at
                            .map(timestamp)
                            .unwrap_or_else(not_recorded)
                    )
                })
                .unwrap_or_else(not_recorded),
            second_actual_status: format!(
                "{} / {}",
                block.second_label.as_deref().unwrap_or("Not recorded"),
                block
                    .second_status
                    .map(slot_status)
                    .unwrap_or("Not recorded")
            ),
            order: block
                .order
                .map(|value| labeled_comparison_order(value, &left, &right))
                .unwrap_or_else(|| "Unavailable".into()),
            eligibility: block_eligibility(block.eligibility),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PairedRowView {
    pub(in crate::html) stratum: String,
    pub(in crate::html) remote_path: String,
    pub(in crate::html) block: usize,
    pub(in crate::html) order: String,
    pub(in crate::html) left_observation: String,
    pub(in crate::html) right_observation: String,
    pub(in crate::html) left_slot: String,
    pub(in crate::html) right_slot: String,
    pub(in crate::html) left_snr: String,
    pub(in crate::html) right_snr: String,
    pub(in crate::html) delta: String,
    pub(in crate::html) elapsed: i64,
    pub(in crate::html) left_time: String,
    pub(in crate::html) right_time: String,
    pub(in crate::html) delta_left_class: String,
    pub(in crate::html) delta_width_class: String,
    pub(in crate::html) snr_left_class: String,
    pub(in crate::html) snr_right_class: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct PairedRowsView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) rows: Vec<PairedRowView>,
}

impl PairedRowsView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels {
            left: left_label,
            right: right_label,
        } = antenna_labels(report);
        let rows = &report.comparison.paired_rows;
        let max_abs = rows
            .iter()
            .map(|row| row.delta_right_minus_left_db.abs())
            .fold(1.0_f64, f64::max);
        let minimum = rows
            .iter()
            .flat_map(|row| [row.left_snr_db, row.right_snr_db])
            .fold(f64::INFINITY, f64::min);
        let maximum = rows
            .iter()
            .flat_map(|row| [row.left_snr_db, row.right_snr_db])
            .fold(f64::NEG_INFINITY, f64::max);
        let span = (maximum - minimum).max(1.0);
        Self {
            left_label: left_label.clone(),
            right_label: right_label.clone(),
            rows: rows
                .iter()
                .map(|row| {
                    let width = row.delta_right_minus_left_db.abs() / max_abs * 50.0;
                    let left = if row.delta_right_minus_left_db < 0.0 {
                        50.0 - width
                    } else {
                        50.0
                    };
                    PairedRowView {
                        stratum: comparison_group_label(&row.stratum),
                        remote_path: row.remote_path.clone(),
                        block: row.block_index + 1,
                        order: labeled_comparison_order(row.order, &left_label, &right_label),
                        left_observation: row.left_observation_id.clone(),
                        right_observation: row.right_observation_id.clone(),
                        left_slot: row.left_slot_id.clone(),
                        right_slot: row.right_slot_id.clone(),
                        left_snr: format_number(row.left_snr_db),
                        right_snr: format_number(row.right_snr_db),
                        delta: format_signed(row.delta_right_minus_left_db),
                        elapsed: row.elapsed_seconds,
                        left_time: timestamp(row.left_timestamp),
                        right_time: timestamp(row.right_timestamp),
                        delta_left_class: geometry_class(left),
                        delta_width_class: geometry_class(width),
                        snr_left_class: geometry_class((row.left_snr_db - minimum) / span * 100.0),
                        snr_right_class: geometry_class(
                            (row.right_snr_db - minimum) / span * 100.0,
                        ),
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct StratumSummaryRowView {
    pub(in crate::html) stratum: String,
    pub(in crate::html) pairs: usize,
    pub(in crate::html) paths: usize,
    pub(in crate::html) blocks: usize,
    pub(in crate::html) left_right: usize,
    pub(in crate::html) right_left: usize,
    pub(in crate::html) unmatched_left: usize,
    pub(in crate::html) unmatched_right: usize,
    pub(in crate::html) missing_left: usize,
    pub(in crate::html) missing_right: usize,
    pub(in crate::html) duplicates: usize,
    pub(in crate::html) conflicts: usize,
    pub(in crate::html) range: String,
    pub(in crate::html) median: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct StratumSummariesView {
    pub(in crate::html) left_label: String,
    pub(in crate::html) right_label: String,
    pub(in crate::html) rows: Vec<StratumSummaryRowView>,
}

impl StratumSummariesView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        let AntennaLabels {
            left: left_label,
            right: right_label,
        } = antenna_labels(report);
        Self {
            left_label,
            right_label,
            rows: report
                .comparison
                .strata
                .iter()
                .map(|row| StratumSummaryRowView {
                    stratum: comparison_group_label(&row.stratum),
                    pairs: row.paired_row_count,
                    paths: row.unique_path_count,
                    blocks: row.contributing_block_count,
                    left_right: row.left_then_right_block_count,
                    right_left: row.right_then_left_block_count,
                    unmatched_left: row.unmatched_left_count,
                    unmatched_right: row.unmatched_right_count,
                    missing_left: row.missing_snr_left_count,
                    missing_right: row.missing_snr_right_count,
                    duplicates: row.exact_duplicate_count,
                    conflicts: row.conflicting_duplicate_group_count,
                    range: row
                        .minimum_delta_right_minus_left_db
                        .zip(row.maximum_delta_right_minus_left_db)
                        .map(|(min, max)| {
                            format!("{} to {} dB", format_signed(min), format_signed(max))
                        })
                        .unwrap_or_else(not_available),
                    median: row
                        .median_path_delta_right_minus_left_db
                        .map(|value| format!("{} dB", format_signed(value)))
                        .unwrap_or_else(not_available),
                })
                .collect(),
        }
    }
}
