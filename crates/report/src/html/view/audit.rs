use crate::{ReportCompleteness, ReportNotice, SessionReport};
use antennabench_analysis::{EligibilityExclusionCategory, EligibilityScope};

use super::super::{questions::coverage_text, shared::*};

#[derive(Debug, Clone)]
pub(in crate::html) struct ReporterActivityCycleView {
    pub(in crate::html) cycle: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) coverage: &'static str,
    pub(in crate::html) active_reporters: usize,
    pub(in crate::html) summary_record_ids: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReporterActivityRowView {
    pub(in crate::html) cycle: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) reporter: String,
    pub(in crate::html) grid: String,
    pub(in crate::html) census_record_id: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ReporterActivityAuditView {
    pub(in crate::html) cycles: Vec<ReporterActivityCycleView>,
    pub(in crate::html) reporters: Vec<ReporterActivityRowView>,
    pub(in crate::html) bounded_reporters_omitted: bool,
}

impl ReporterActivityAuditView {
    pub(in crate::html) fn new(report: &SessionReport) -> Option<Self> {
        if report.reporter_activity.is_empty() {
            return None;
        }
        let cycles = report
            .reporter_activity
            .census_cycles
            .iter()
            .map(|cycle| ReporterActivityCycleView {
                cycle: timestamp(cycle.cycle_time),
                band: band(cycle.band),
                coverage: coverage_text(cycle.coverage),
                active_reporters: cycle.active_reporters.len(),
                summary_record_ids: cycle.summary_record_ids.join(", "),
            })
            .collect();
        let reporters = report
            .reporter_activity
            .census_cycles
            .iter()
            .flat_map(|cycle| {
                cycle
                    .active_reporters
                    .iter()
                    .map(move |reporter| ReporterActivityRowView {
                        cycle: timestamp(cycle.cycle_time),
                        band: band(cycle.band),
                        reporter: reporter.reporter.clone(),
                        grid: reporter.reporter_grid.clone().unwrap_or_else(not_available),
                        census_record_id: reporter.census_record_id.clone(),
                    })
            })
            .collect::<Vec<_>>();
        Some(Self {
            bounded_reporters_omitted: reporters.is_empty()
                && report.completeness == ReportCompleteness::BoundedOverview,
            cycles,
            reporters,
        })
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct EligibilityRowView {
    pub(in crate::html) code: String,
    pub(in crate::html) category: &'static str,
    pub(in crate::html) scope: &'static str,
    pub(in crate::html) count: usize,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct EligibilityView {
    pub(in crate::html) rows: Vec<EligibilityRowView>,
}

impl EligibilityView {
    pub(in crate::html) fn new(report: &SessionReport) -> Option<Self> {
        if report.eligibility_exclusions.is_empty() {
            return None;
        }
        Some(Self {
            rows: report
                .eligibility_exclusions
                .iter()
                .map(|row| EligibilityRowView {
                    code: row.code.clone(),
                    category: eligibility_category(row.category),
                    scope: eligibility_scope(row.scope),
                    count: row.count,
                })
                .collect(),
        })
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ExclusionRecordView {
    pub(in crate::html) observation_id: String,
    pub(in crate::html) reason: &'static str,
    pub(in crate::html) time: String,
    pub(in crate::html) band: &'static str,
    pub(in crate::html) kind: &'static str,
    pub(in crate::html) source: &'static str,
    pub(in crate::html) mode: String,
    pub(in crate::html) slot_id: String,
    pub(in crate::html) assigned_label: String,
    pub(in crate::html) confidence: String,
}

#[derive(Debug, Clone)]
pub(in crate::html) struct ExclusionRecordsView {
    pub(in crate::html) rows: Vec<ExclusionRecordView>,
}

impl ExclusionRecordsView {
    pub(in crate::html) fn new(report: &SessionReport) -> Self {
        Self {
            rows: report
                .exclusion_records
                .iter()
                .map(|record| ExclusionRecordView {
                    observation_id: record.observation_id.clone(),
                    reason: exclusion_reason(record.reason),
                    time: timestamp(record.timestamp),
                    band: band(record.band),
                    kind: observation_kind(record.observation_kind),
                    source: record_source(record.source),
                    mode: record
                        .mode
                        .clone()
                        .unwrap_or_else(|| "Not recorded".to_string()),
                    slot_id: record
                        .slot_id
                        .clone()
                        .unwrap_or_else(|| "Not assigned".to_string()),
                    assigned_label: record
                        .assigned_label
                        .clone()
                        .unwrap_or_else(|| "Not assigned".to_string()),
                    confidence: format_number(f64::from(record.assignment_confidence)),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::html) struct NoticesView {
    pub(in crate::html) notices: Vec<String>,
}

impl NoticesView {
    pub(in crate::html) fn new(notices: &[ReportNotice]) -> Option<Self> {
        if notices.is_empty() {
            None
        } else {
            Some(Self {
                notices: notices.iter().map(notice_text).collect(),
            })
        }
    }
}

fn eligibility_category(value: EligibilityExclusionCategory) -> &'static str {
    match value {
        EligibilityExclusionCategory::Missing => "Missing",
        EligibilityExclusionCategory::Malformed => "Malformed",
        EligibilityExclusionCategory::Contradictory => "Contradictory",
        EligibilityExclusionCategory::Unsupported => "Unsupported",
        EligibilityExclusionCategory::Duplicate => "Duplicate",
        EligibilityExclusionCategory::DeliberatelyExcluded => "Deliberately excluded",
    }
}

fn eligibility_scope(value: EligibilityScope) -> &'static str {
    match value {
        EligibilityScope::Field => "Field",
        EligibilityScope::Observation => "Observation",
        EligibilityScope::Slot => "Slot",
        EligibilityScope::ComparisonStratum => "Comparison stratum",
        EligibilityScope::ComparisonBlock => "Comparison block",
    }
}
