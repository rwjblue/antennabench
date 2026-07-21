use super::super::geometry::geometry_class;
use super::*;
use crate::html::{
    templates::{render_template, ReporterActivityTemplate},
    view::{
        ActivityCycleRowView, ActivityGroupView, ActivityJointSummaryRowView, ActivityOutcomeView,
        ActivityPairedRowView, ActivityReceiverRowView, DetectionRateRowView, DetectionRateView,
        ReporterActivityView,
    },
};
use antennabench_analysis::{
    ReporterActivityCoverage, ReporterActivityJointOutcome, ReporterActivityUnknownReason,
};

pub(in super::super) fn render_reporter_activity_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ReporterActivityTemplate {
            view: activity_view(report),
        },
    )
}

fn activity_view(report: &SessionReport) -> ReporterActivityView {
    let (left_label, right_label) = labels(report);
    ReporterActivityView {
        no_activity: report.reporter_activity.cycle_rates.is_empty(),
        groups: report
            .reporter_activity
            .joint_summaries
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let rates = (row.coverage.is_known()
                    && row.receiver_block_opportunity_count > 0
                    && row.left_detection_rate.is_some()
                    && row.right_detection_rate.is_some())
                .then(|| detection_rate_view(row, &left_label, &right_label));
                let denominator = row.receiver_block_opportunity_count;
                let outcomes = [
                    (row.left_only_count, "left", format!("{left_label} only")),
                    (row.heard_both_count, "both", "Heard both".to_string()),
                    (row.right_only_count, "right", format!("{right_label} only")),
                    (
                        row.heard_neither_count,
                        "neither",
                        "Heard neither".to_string(),
                    ),
                ]
                .into_iter()
                .map(|(count, class, label)| ActivityOutcomeView {
                    label,
                    count,
                    class,
                    geometry_class: (count > 0 && denominator > 0)
                        .then(|| geometry_class(count as f64 / denominator as f64 * 100.0)),
                })
                .collect();
                ActivityGroupView {
                    index,
                    label: stratum(&row.stratum),
                    coverage: coverage_text(row.coverage),
                    known_blocks: row.known_coverage_block_count,
                    eligible_blocks: row.eligible_block_count,
                    unique_receivers: row.unique_active_receiver_count,
                    rates,
                    opportunities: denominator,
                    outcomes,
                }
            })
            .collect(),
        summaries: report
            .reporter_activity
            .joint_summaries
            .iter()
            .map(|row| ActivityJointSummaryRowView {
                group: stratum(&row.stratum),
                unique_receivers: row.unique_active_receiver_count,
                eligible_blocks: row.eligible_block_count,
                left_then_right: row.left_then_right_block_count,
                right_then_left: row.right_then_left_block_count,
                opportunities: row.receiver_block_opportunity_count,
                both: row.heard_both_count,
                left_only: row.left_only_count,
                right_only: row.right_only_count,
                neither: row.heard_neither_count,
                left_rate: aggregate_rate_text(row.left_detection_rate),
                right_rate: aggregate_rate_text(row.right_detection_rate),
                coverage: coverage_text(row.coverage),
                known_blocks: row.known_coverage_block_count,
            })
            .collect(),
        paired_rows: report
            .reporter_activity
            .paired_rates
            .iter()
            .map(|row| ActivityPairedRowView {
                group: stratum(&row.stratum),
                block: row.block_index + 1,
                order: labeled_comparison_order(row.order, &left_label, &right_label),
                left_slot: row.left_slot_id.clone(),
                right_slot: row.right_slot_id.clone(),
                active: row.active_in_both_count,
                both: row.heard_both_count,
                left_only: row.left_only_count,
                right_only: row.right_only_count,
                neither: row.heard_neither_count,
                left_rate: rate_text(
                    row.left_heard_count,
                    row.active_in_both_count,
                    row.left_hearing_rate,
                    row.coverage,
                ),
                right_rate: rate_text(
                    row.right_heard_count,
                    row.active_in_both_count,
                    row.right_hearing_rate,
                    row.coverage,
                ),
                coverage: coverage_text(row.coverage),
            })
            .collect(),
        cycle_rows: report
            .reporter_activity
            .cycle_rates
            .iter()
            .map(|row| ActivityCycleRowView {
                group: stratum(&row.stratum),
                antenna: row.antenna_label.clone(),
                starts: timestamp(row.cycle_starts_at),
                slot: row.slot_id.clone(),
                rate: rate_text(
                    row.heard_reporter_count,
                    row.active_reporter_count,
                    row.hearing_rate,
                    row.coverage,
                ),
                coverage: coverage_text(row.coverage),
            })
            .collect(),
        receiver_rows: report
            .reporter_activity
            .paired_rates
            .iter()
            .flat_map(|row| {
                let group = stratum(&row.stratum);
                let left_label = left_label.clone();
                let right_label = right_label.clone();
                row.receivers
                    .iter()
                    .map(move |receiver| ActivityReceiverRowView {
                        group: group.clone(),
                        block: row.block_index + 1,
                        receiver: receiver.receiver.clone(),
                        locator: receiver
                            .receiver_grid
                            .clone()
                            .unwrap_or_else(|| "Not recorded".into()),
                        outcome: joint_outcome_text(receiver.outcome, &left_label, &right_label),
                    })
            })
            .collect(),
        left_label,
        right_label,
    }
}

fn detection_rate_view(
    row: &antennabench_analysis::ReporterActivityJointSummary,
    left_label: &str,
    right_label: &str,
) -> DetectionRateView {
    let denominator = row.receiver_block_opportunity_count;
    let left_heard = row.heard_both_count + row.left_only_count;
    let right_heard = row.heard_both_count + row.right_only_count;
    let left_rate = row.left_detection_rate.expect("known aggregate rate");
    let right_rate = row.right_detection_rate.expect("known aggregate rate");
    let (takeaway_lead, takeaway_detail) = if left_rate == right_rate {
        (
            format!(
                "In these {denominator} shared opportunities, both antennas had the same detection rate:"
            ),
            format!(
                "{left_label} and {right_label} were each reported {:.1}% of the time. Heard by both contributes to both antenna rates.",
                left_rate * 100.0
            ),
        )
    } else {
        let (higher_label, higher_heard, higher_rate, lower_label, lower_heard, lower_rate) =
            if right_rate >= left_rate {
                (
                    right_label,
                    right_heard,
                    right_rate,
                    left_label,
                    left_heard,
                    left_rate,
                )
            } else {
                (
                    left_label,
                    left_heard,
                    left_rate,
                    right_label,
                    right_heard,
                    right_rate,
                )
            };
        (
            format!(
                "In these {denominator} shared opportunities, {higher_label} was reported more often:"
            ),
            format!(
                "{higher_heard} of {denominator} ({:.1}%) versus {lower_label} with {lower_heard} of {denominator} ({:.1}%), a {:.1} percentage-point difference. Heard by both contributes to both antenna rates.",
                higher_rate * 100.0,
                lower_rate * 100.0,
                (higher_rate - lower_rate) * 100.0
            ),
        )
    };
    DetectionRateView {
        takeaway_lead,
        takeaway_detail,
        rows: vec![
            DetectionRateRowView {
                label: left_label.to_string(),
                heard: left_heard,
                opportunities: denominator,
                geometry_class: geometry_class(left_rate * 100.0),
                rate: format!("{:.1}%", left_rate * 100.0),
                side: "left",
            },
            DetectionRateRowView {
                label: right_label.to_string(),
                heard: right_heard,
                opportunities: denominator,
                geometry_class: geometry_class(right_rate * 100.0),
                rate: format!("{:.1}%", right_rate * 100.0),
                side: "right",
            },
        ],
    }
}

fn labels(report: &SessionReport) -> (String, String) {
    (
        report
            .comparison
            .left_label
            .clone()
            .unwrap_or_else(|| "Left".into()),
        report
            .comparison
            .right_label
            .clone()
            .unwrap_or_else(|| "Right".into()),
    )
}

fn stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        value.mode.as_str(),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}

fn aggregate_rate_text(rate: Option<f64>) -> String {
    rate.map_or_else(
        || "Not defined".to_string(),
        |rate| format!("{:.1}%", rate * 100.0),
    )
}

fn joint_outcome_text(
    outcome: ReporterActivityJointOutcome,
    left_label: &str,
    right_label: &str,
) -> String {
    match outcome {
        ReporterActivityJointOutcome::HeardBoth => "Heard both".to_string(),
        ReporterActivityJointOutcome::LeftOnly => format!("Heard {left_label} only"),
        ReporterActivityJointOutcome::RightOnly => format!("Heard {right_label} only"),
        ReporterActivityJointOutcome::HeardNeither => "Heard neither".to_string(),
    }
}

pub(in super::super) fn coverage_text(coverage: ReporterActivityCoverage) -> &'static str {
    match coverage {
        ReporterActivityCoverage::Complete => "Complete band-qualified census",
        ReporterActivityCoverage::Partial => {
            "Partial census — malformed rows may reduce the denominator"
        }
        ReporterActivityCoverage::Truncated => {
            "Truncated census — capture limit may reduce the denominator"
        }
        ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::NoCensusCoverage) => {
            "Coverage unknown — no band-qualified census covers this cycle"
        }
        ReporterActivityCoverage::Unknown(
            ReporterActivityUnknownReason::UnsupportedReceiveDirection,
        ) => "Coverage unknown — receiver census does not measure receive-direction paths",
        ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::UnsupportedSignalMode) => {
            "Coverage unknown — the live receiver census measures WSPR activity only"
        }
    }
}

fn rate_text(
    heard: usize,
    active: usize,
    rate: Option<f64>,
    coverage: ReporterActivityCoverage,
) -> String {
    if !coverage.is_known() {
        return "Not available (coverage unknown; not zero)".to_string();
    }
    rate.map_or_else(
        || format!("{heard} / {active}; rate not defined with no active reporters"),
        |rate| format!("{heard} / {active} ({:.1}%)", rate * 100.0),
    )
}
