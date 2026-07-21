use super::*;
use crate::html::{
    templates::{render_template, AnswerFirstTemplate, NavigationTemplate, ReadingGuideTemplate},
    view::{
        AvailabilityFactView, GoalLensView, HeadlineFactView, HeadlineGroupView,
        NavigationLinkView, NavigationView, NoticeView, OverviewResultRowView, OverviewView,
        ReadingGuideView,
    },
};
use crate::ReportAcquisitionWorkflowStatus;

pub(in super::super) fn render_question_navigation(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    include_audit: bool,
) -> Result<(), ReportError> {
    let mut links = vec![NavigationLinkView {
        href: "#what-run-show",
        label: "What did the run show?",
    }];
    let mut observed_footprint_linked = false;
    for family in ordered_question_families(report) {
        if !question_family_is_primary_available(report, family) {
            continue;
        }
        match family {
            crate::ReportQuestionFamily::SharedPathSignal => links.push(NavigationLinkView {
                href: "#same-path-signal",
                label: "Shared-path signal",
            }),
            crate::ReportQuestionFamily::CommonOpportunityDetection => {
                links.push(NavigationLinkView {
                    href: "#reporter-activity",
                    label: "Detection among receivers active in both cycles",
                });
                links.push(NavigationLinkView {
                    href: "#coverage-map",
                    label: "Active-receiver coverage map",
                });
            }
            crate::ReportQuestionFamily::ObservedReach => {
                if include_audit {
                    links.push(NavigationLinkView {
                        href: "#reach-unique-paths",
                        label: "Observed reach",
                    });
                } else if !observed_footprint_linked {
                    links.push(NavigationLinkView {
                        href: "#observed-footprint",
                        label: "Observed footprint",
                    });
                    observed_footprint_linked = true;
                }
            }
            crate::ReportQuestionFamily::GeographicProfile => {
                if include_audit {
                    links.push(NavigationLinkView {
                        href: "#distance-direction",
                        label: "Observed distance and direction profile",
                    });
                } else if !observed_footprint_linked {
                    links.push(NavigationLinkView {
                        href: "#observed-footprint",
                        label: "Observed footprint",
                    });
                    observed_footprint_linked = true;
                }
            }
            crate::ReportQuestionFamily::Repeatability => {
                if include_audit {
                    links.push(NavigationLinkView {
                        href: "#coverage-overlap",
                        label: "Coverage overlap and repeatability",
                    });
                } else if !observed_footprint_linked {
                    links.push(NavigationLinkView {
                        href: "#observed-footprint",
                        label: "Observed footprint",
                    });
                    observed_footprint_linked = true;
                }
            }
        }
    }
    links.push(NavigationLinkView {
        href: "#run-quality",
        label: "Run quality",
    });
    if include_audit {
        links.push(NavigationLinkView {
            href: "#audit-appendix",
            label: "Audit appendix",
        });
    }
    render_template(
        out,
        &NavigationTemplate {
            view: NavigationView { links },
        },
    )
}
pub(in super::super) fn render_how_to_read(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    compact: bool,
) -> Result<(), ReportError> {
    render_template(
        out,
        &ReadingGuideTemplate {
            view: ReadingGuideView {
                compact,
                single_antenna: is_single_antenna_lens(report),
            },
        },
    )
}
pub(in super::super) fn render_answer_first_overview(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_answer_first_overview_with_reference(out, report, "the audit appendix", false)
}
pub(in super::super) fn render_answer_first_overview_with_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
    compact: bool,
) -> Result<(), ReportError> {
    render_template(
        out,
        &AnswerFirstTemplate {
            view: overview_view(report, audit_reference, compact),
        },
    )
}

fn overview_view(report: &SessionReport, audit_reference: &str, compact: bool) -> OverviewView {
    let overview = &report.overview;
    let scope = &overview.scope;
    let antennas = if scope.antenna_labels.is_empty() {
        "None recorded".to_string()
    } else {
        scope.antenna_labels.join(" / ")
    };
    let bands = if scope.bands.is_empty() {
        "None recorded".to_string()
    } else {
        scope
            .bands
            .iter()
            .map(|value| band(*value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let directions = if scope.observed_directions.is_empty() {
        "No comparison direction observed".to_string()
    } else {
        scope
            .observed_directions
            .iter()
            .map(|value| path_direction(*value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mode = scope
        .experiment_mode
        .map(experiment_mode)
        .unwrap_or("Not recorded");
    let lifecycle_label = overview_lifecycle_label(overview.lifecycle.state);
    let paired_rows = overview
        .strata
        .iter()
        .map(|row| row.paired_row_count)
        .sum::<usize>();
    let headline_groups = headline_groups(report);
    let (orientation_label, orientation) = match &scope.delta_orientation {
        Some(orientation) => (
            "Signed values",
            format!(
                "Positive values mean {} was stronger; negative values mean {} was stronger.",
                orientation.minuend_label, orientation.subtrahend_label
            ),
        ),
        None if is_single_antenna_lens(report) => (
            "Signal-difference orientation",
            "not applicable to this single-antenna profiling run.".to_string(),
        ),
        None => (
            "Delta orientation",
            "unavailable because this run does not provide a two-label paired orientation."
                .to_string(),
        ),
    };
    let rows = overview
        .strata
        .iter()
        .filter_map(|row| match row.path_delta {
            ReportOverviewPathDelta::Unavailable => None,
            ReportOverviewPathDelta::Available {
                minimum_delta_right_minus_left_db,
                median_path_delta_right_minus_left_db,
                maximum_delta_right_minus_left_db,
            } => Some(OverviewResultRowView {
                group: raw_comparison_stratum(&row.stratum),
                delta: format!(
                    "{} to {} dB; median across paths {} dB",
                    format_signed(minimum_delta_right_minus_left_db),
                    format_signed(maximum_delta_right_minus_left_db),
                    format_signed(median_path_delta_right_minus_left_db),
                ),
                paths: row.unique_path_count,
                pairs: row.paired_row_count,
                blocks: row.contributing_block_count,
                coverage: "Available",
            }),
        })
        .collect();
    let unavailable = overview
        .strata
        .iter()
        .filter(|row| row.path_delta == ReportOverviewPathDelta::Unavailable)
        .collect::<Vec<_>>();
    let unavailable_groups = (!unavailable.is_empty()).then(|| {
        format!(
            "No path delta in {}: {}",
            comparison_groups_label(unavailable.len()),
            unavailable
                .iter()
                .map(|row| raw_comparison_stratum(&row.stratum))
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    let support = if overview.strata.is_empty() {
        "The session scope and availability state remain explicit even without an available comparison group."
            .to_string()
    } else {
        format!(
            "{paired_rows} matched pair(s) are retained in {} separate comparison group(s).",
            overview.strata.len()
        )
    };

    OverviewView {
        compact,
        answerability_headline: answerability_headline(report),
        plain_answer: plain_language_answer(report),
        headline_groups,
        callsign: scope.station.callsign.clone(),
        grid: scope.station.grid.clone(),
        goal: scope.goal.map(session_goal).unwrap_or("Not recorded"),
        goal_lens: overview.goal_lens.as_ref().map(|lens| GoalLensView {
            label: session_goal(lens.goal),
            practical_meaning: lens.practical_meaning.clone(),
            distance_focus: (!lens.emphasized_distance_bins.is_empty()).then(|| {
                lens.emphasized_distance_bins
                    .iter()
                    .map(|bin| bin.label())
                    .collect::<Vec<_>>()
                    .join("; ")
            }),
        }),
        antennas,
        bands,
        direction_mode: format!("{directions}; {mode}"),
        lifecycle: lifecycle_label,
        orientation_label,
        orientation,
        show_delta_scale: !compact
            && overview
                .strata
                .iter()
                .any(|row| matches!(row.path_delta, ReportOverviewPathDelta::Available { .. })),
        rows,
        unavailable_groups,
        comparison_state: comparison_availability_label(overview.comparison_availability),
        support,
        limitations: overview
            .limitations
            .iter()
            .map(|limitation| overview_limitation_text(*limitation, report))
            .collect(),
        notices: acquisition_notices(report, audit_reference),
        availability: answerability_facts(report),
    }
}

fn answerability_headline(report: &SessionReport) -> String {
    let default_priority = [
        crate::ReportQuestionFamily::SharedPathSignal,
        crate::ReportQuestionFamily::CommonOpportunityDetection,
        crate::ReportQuestionFamily::ObservedReach,
        crate::ReportQuestionFamily::GeographicProfile,
        crate::ReportQuestionFamily::Repeatability,
    ];
    let priority = report
        .overview
        .goal_lens
        .as_ref()
        .map(|lens| lens.priority.as_slice())
        .unwrap_or(&default_priority);
    let answered = priority
        .iter()
        .copied()
        .filter(|family| question_family_is_primary_available(report, *family))
        .map(question_family_label)
        .collect::<Vec<_>>();
    let applicable_count = if is_single_antenna_lens(report) { 3 } else { 5 };
    if answered.len() == applicable_count {
        format!("All {applicable_count} applicable question families have usable evidence.")
    } else if answered.is_empty() {
        "None of the five report question families has usable evidence yet.".to_string()
    } else {
        format!("Answered by this run: {}.", answered.join("; "))
    }
}

fn question_family_label(family: crate::ReportQuestionFamily) -> &'static str {
    match family {
        crate::ReportQuestionFamily::SharedPathSignal => "Shared-path signal",
        crate::ReportQuestionFamily::CommonOpportunityDetection => {
            "Detection among receivers active in both cycles"
        }
        crate::ReportQuestionFamily::ObservedReach => "Observed reach",
        crate::ReportQuestionFamily::GeographicProfile => "Observed distance and direction profile",
        crate::ReportQuestionFamily::Repeatability => "Repeatability across blocks",
    }
}

fn answerability_facts(report: &SessionReport) -> Vec<AvailabilityFactView> {
    let answerability = &report.overview.answerability;
    let same_path_status =
        if answerability.same_path_signal == SamePathSignalAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        };
    let paired_status =
        if answerability.paired_detectability == PairedDetectabilityAnswerability::Available {
            if report
                .reporter_activity
                .joint_summaries
                .iter()
                .any(|summary| {
                    matches!(
                        summary.coverage,
                        antennabench_analysis::ReporterActivityCoverage::Partial
                            | antennabench_analysis::ReporterActivityCoverage::Truncated
                    )
                })
            {
                "Limited"
            } else {
                "Available"
            }
        } else {
            "Unavailable"
        };
    let observed_reach_status =
        if answerability.observed_reach == ObservedReachAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        };
    let geographic_status =
        if answerability.geographic_profile == GeographicProfileAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        };
    let repeatability_status =
        if answerability.repeatability == RepeatabilityAnswerability::Available {
            "Available"
        } else {
            "Limited"
        };
    [
        (
            "Shared-path signal",
            same_path_status,
            same_path_answerability_text(answerability.same_path_signal),
        ),
        (
            "Detection among receivers active in both cycles",
            paired_status,
            paired_detectability_answerability_text(answerability.paired_detectability),
        ),
        (
            "Observed reach",
            observed_reach_status,
            match answerability.observed_reach {
                ObservedReachAnswerability::Available => "Available from unique observed paths",
                ObservedReachAnswerability::NoUsablePaths => "No usable paths",
            },
        ),
        (
            "Observed distance and direction profile",
            geographic_status,
            match answerability.geographic_profile {
                GeographicProfileAnswerability::Available => {
                    "Available from located observed paths"
                }
                GeographicProfileAnswerability::NoLocatedPaths => "No located paths",
            },
        ),
        (
            "Repeatability across blocks",
            repeatability_status,
            match answerability.repeatability {
                RepeatabilityAnswerability::Available => {
                    "Available across repeated eligible blocks"
                }
                RepeatabilityAnswerability::InsufficientRepetition => "Insufficient repetition",
            },
        ),
    ]
    .into_iter()
    .map(|(question, status, availability)| AvailabilityFactView {
        question,
        status,
        status_class: status.to_ascii_lowercase(),
        availability,
    })
    .collect()
}

fn same_path_answerability_text(value: SamePathSignalAnswerability) -> &'static str {
    match value {
        SamePathSignalAnswerability::Available => "Available from matched finite-SNR paths",
        SamePathSignalAnswerability::NoMatchedPaths => {
            "No same-path SNR comparison: no matched paths"
        }
        SamePathSignalAnswerability::NoFiniteSnr => "No same-path SNR comparison: no finite SNR",
        SamePathSignalAnswerability::NoEligibleBlocks => {
            "No same-path SNR comparison: no eligible blocks"
        }
        SamePathSignalAnswerability::NotApplicable => "Not applicable for this session shape",
        SamePathSignalAnswerability::UnsupportedShape => "Unsupported comparison shape",
    }
}

fn paired_detectability_answerability_text(
    value: PairedDetectabilityAnswerability,
) -> &'static str {
    match value {
        PairedDetectabilityAnswerability::Available => {
            "Available among receivers active in both cycles"
        }
        PairedDetectabilityAnswerability::NoCommonActiveReporters => "No common active reporters",
        PairedDetectabilityAnswerability::ActivityCoverageUnknown => "Activity coverage unknown",
        PairedDetectabilityAnswerability::UnsupportedDirection => {
            "Unsupported direction: the census supports transmit paths only"
        }
        PairedDetectabilityAnswerability::NoEligibleBlocks => "No eligible blocks",
        PairedDetectabilityAnswerability::NotApplicable => "Not applicable for this session shape",
        PairedDetectabilityAnswerability::UnsupportedShape => "Unsupported comparison shape",
    }
}
#[derive(Clone)]
struct HeadlineEvidence {
    family: crate::ReportQuestionFamily,
    label: &'static str,
    value: String,
    detail: String,
    clause: String,
    direction: i8,
}

fn headline_groups(report: &SessionReport) -> Vec<HeadlineGroupView> {
    let groups = report
        .overview
        .strata
        .iter()
        .filter_map(|row| {
            let facts = prioritized_headline_evidence(report, row);
            (!facts.is_empty()).then_some((row, facts))
        })
        .collect::<Vec<_>>();
    let multiple = groups.len() > 1;
    groups
        .into_iter()
        .enumerate()
        .map(|(index, (row, facts))| HeadlineGroupView {
            index,
            title: multiple.then(|| raw_comparison_stratum(&row.stratum)),
            answer: multiple.then(|| descriptive_group_sentence(report, row, false)),
            facts: facts
                .into_iter()
                .map(|fact| HeadlineFactView {
                    label: fact.label,
                    value: fact.value,
                    detail: fact.detail,
                })
                .collect(),
        })
        .collect()
}

fn prioritized_headline_evidence(
    report: &SessionReport,
    row: &ReportOverviewStratum,
) -> Vec<HeadlineEvidence> {
    let (left_label, right_label) = raw_report_antenna_labels(report);
    let mut facts = Vec::new();
    if let ReportOverviewPathDelta::Available {
        median_path_delta_right_minus_left_db: median,
        ..
    } = row.path_delta
    {
        let stronger = if median > 0.0 {
            right_label.as_str()
        } else if median < 0.0 {
            left_label.as_str()
        } else {
            "neither antenna"
        };
        let clause = if median == 0.0 {
            format!(
                "a 0 dB shared-path median across {} path{}",
                row.unique_path_count,
                plural_suffix(row.unique_path_count)
            )
        } else {
            format!(
                "a {} dB median across {} shared path{}",
                format_signed(median),
                row.unique_path_count,
                plural_suffix(row.unique_path_count)
            )
        };
        facts.push(HeadlineEvidence {
            family: crate::ReportQuestionFamily::SharedPathSignal,
            label: "Shared-path signal",
            value: format!("{} dB median", format_signed(median)),
            detail: format!(
                "{} shared path{} · {stronger} stronger at the median",
                row.unique_path_count,
                plural_suffix(row.unique_path_count)
            ),
            clause,
            direction: median.total_cmp(&0.0) as i8,
        });
    }
    if let Some(summary) = report
        .reporter_activity
        .joint_summaries
        .iter()
        .find(|summary| summary.stratum == row.stratum)
        .filter(|summary| {
            summary.coverage.is_known()
                && summary.receiver_block_opportunity_count > 0
                && summary.left_detection_rate.is_some()
                && summary.right_detection_rate.is_some()
        })
    {
        let left_rate = summary.left_detection_rate.expect("filtered rate");
        let right_rate = summary.right_detection_rate.expect("filtered rate");
        let (first_rate, second_rate, first_label, second_label) = if right_rate >= left_rate {
            (
                right_rate,
                left_rate,
                right_label.as_str(),
                left_label.as_str(),
            )
        } else {
            (
                left_rate,
                right_rate,
                left_label.as_str(),
                right_label.as_str(),
            )
        };
        facts.push(HeadlineEvidence {
            family: crate::ReportQuestionFamily::CommonOpportunityDetection,
            label: "Same active receivers",
            value: format!("{:.1}% vs {:.1}%", first_rate * 100.0, second_rate * 100.0),
            detail: format!(
                "{first_label} / {second_label} · {} opportunities",
                summary.receiver_block_opportunity_count
            ),
            clause: format!(
                "{:.1}% versus {:.1}% detection among {} common-active receiver opportunities",
                first_rate * 100.0,
                second_rate * 100.0,
                summary.receiver_block_opportunity_count
            ),
            direction: right_rate.total_cmp(&left_rate) as i8,
        });
    }
    let left_paths = row.reach.left_only_unique_path_count + row.reach.both_unique_path_count;
    let right_paths = row.reach.right_only_unique_path_count + row.reach.both_unique_path_count;
    if left_paths > 0 || right_paths > 0 {
        let (first_count, second_count, first_label, second_label) = if right_paths >= left_paths {
            (
                right_paths,
                left_paths,
                right_label.as_str(),
                left_label.as_str(),
            )
        } else {
            (
                left_paths,
                right_paths,
                left_label.as_str(),
                right_label.as_str(),
            )
        };
        facts.push(HeadlineEvidence {
            family: crate::ReportQuestionFamily::ObservedReach,
            label: "Observed footprint",
            value: format!("{first_count} vs {second_count} paths"),
            detail: format!("{first_label} / {second_label} · uncontrolled observations"),
            clause: format!("{first_count} versus {second_count} unique observed paths"),
            direction: right_paths.cmp(&left_paths) as i8,
        });
    }
    let default_priority = [
        crate::ReportQuestionFamily::SharedPathSignal,
        crate::ReportQuestionFamily::CommonOpportunityDetection,
        crate::ReportQuestionFamily::ObservedReach,
    ];
    let priority = report
        .overview
        .goal_lens
        .as_ref()
        .map(|lens| lens.priority.as_slice())
        .unwrap_or(&default_priority);
    let mut ordered = Vec::new();
    for family in priority {
        if let Some(fact) = facts.iter().find(|fact| fact.family == *family) {
            ordered.push(fact.clone());
        }
        if ordered.len() == 3 {
            break;
        }
    }
    ordered
}

fn plain_language_answer(report: &SessionReport) -> String {
    let overview = &report.overview;
    let mut sentences = match overview.comparison_availability {
        antennabench_analysis::ComparisonAvailability::NotApplicable => vec![
            "This session profiles one antenna. Comparative signal and detection questions do not apply; review its recorded footprint and repetition evidence when available."
                .to_string(),
        ],
        antennabench_analysis::ComparisonAvailability::UnsupportedComparisonShape => vec![
            "An A/B comparison needs exactly two antenna labels; this session recorded a different comparison shape. Use exactly two labels for a future A/B session."
                .to_string(),
        ],
        antennabench_analysis::ComparisonAvailability::NoEligibleBlocks => vec![
            "This run did not complete a usable back-to-back pair of cycles on both antennas, so no matched comparison was possible."
                .to_string(),
            "To make a future run answerable, complete more repetitions.".to_string(),
        ],
        antennabench_analysis::ComparisonAvailability::NoMatchedPaths
            if overview
                .strata
                .iter()
                .any(|row| !prioritized_headline_evidence(report, row).is_empty()) =>
        {
            descriptive_group_sentences(report)
        }
        antennabench_analysis::ComparisonAvailability::NoMatchedPaths => {
            no_matched_path_sentences(report)
        }
        antennabench_analysis::ComparisonAvailability::DescriptivePairsAvailable => {
            descriptive_group_sentences(report)
        }
    };
    if matches!(
        overview.comparison_availability,
        antennabench_analysis::ComparisonAvailability::NoEligibleBlocks
            | antennabench_analysis::ComparisonAvailability::NoMatchedPaths
    ) && no_public_wspr_spots(overview)
    {
        sentences.push(
            "No public WSPR spots were recorded; check WSPR upload settings before the next run."
                .to_string(),
        );
    }
    sentences.join(" ")
}
fn no_matched_path_sentences(report: &SessionReport) -> Vec<String> {
    let overview = &report.overview;
    let usable_count = report.evidence.overall.observation_counts.usable;
    let (left_label, right_label) = raw_report_antenna_labels(report);
    let reach_rows = overview
        .strata
        .iter()
        .filter(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                > 0
        })
        .collect::<Vec<_>>();
    if usable_count == 0 {
        return vec![
            "No usable observations were recorded, so this run has no reach evidence and no same-path signal delta to summarize."
                .to_string(),
        ];
    }
    if reach_rows.is_empty() {
        return vec![
            format!(
                "This run retained {usable_count} usable observation{} but no usable remote-path reach evidence.",
                plural_suffix(usable_count)
            ),
            "No per-antenna reach counts or same-path signal delta can be computed."
                .to_string(),
        ];
    }

    let left_path_count = reach_rows
        .iter()
        .map(|row| row.reach.left_only_unique_path_count + row.reach.both_unique_path_count)
        .sum::<usize>();
    let right_path_count = reach_rows
        .iter()
        .map(|row| row.reach.right_only_unique_path_count + row.reach.both_unique_path_count)
        .sum::<usize>();
    let mut sentences = vec![match (left_path_count > 0, right_path_count > 0) {
        (true, true) => {
            format!("Both {left_label} and {right_label} produced usable path evidence.")
        }
        (true, false) => format!(
            "{left_label} produced usable path evidence; no usable {right_label} path evidence was recorded."
        ),
        (false, true) => format!(
            "{right_label} produced usable path evidence; no usable {left_label} path evidence was recorded."
        ),
        (false, false) => unreachable!("nonempty reach rows contain a path count"),
    }];
    sentences.extend(reach_rows.into_iter().map(|row| {
        format!(
            "On {}: {} {left_label}-only, {} shared, and {} {right_label}-only unique path{}.",
            raw_comparison_stratum(&row.stratum),
            row.reach.left_only_unique_path_count,
            row.reach.both_unique_path_count,
            row.reach.right_only_unique_path_count,
            plural_suffix(
                row.reach.left_only_unique_path_count
                    + row.reach.both_unique_path_count
                    + row.reach.right_only_unique_path_count
            )
        )
    }));
    sentences.push(
        "No same-path signal delta can be computed because no remote path had usable finite-SNR reports for both antennas within the same eligible block and comparison group."
            .to_string(),
    );
    sentences.push(
        "To make a future run more likely to produce matched pairs, run longer or concentrate on one band."
            .to_string(),
    );
    sentences
}
fn descriptive_group_sentences(report: &SessionReport) -> Vec<String> {
    let overview = &report.overview;
    let supported_group_count = overview
        .strata
        .iter()
        .filter(|row| !prioritized_headline_evidence(report, row).is_empty())
        .count();
    if supported_group_count > 1 {
        return vec![format!(
            "Headline evidence is shown separately for {supported_group_count} comparison groups below and is not pooled. Each conclusion describes this session, not a universal antenna ranking."
        )];
    }
    let sentences = overview
        .strata
        .iter()
        .filter(|row| !prioritized_headline_evidence(report, row).is_empty())
        .map(|row| descriptive_group_sentence(report, row, true))
        .collect::<Vec<_>>();
    if sentences.is_empty() {
        vec![
            "Matched comparisons are available; review the per-group table for their descriptive results."
                .to_string(),
        ]
    } else {
        sentences
    }
}

fn descriptive_group_sentence(
    report: &SessionReport,
    row: &ReportOverviewStratum,
    include_group: bool,
) -> String {
    let overview = &report.overview;
    let goal = overview
        .scope
        .goal
        .map(session_goal)
        .unwrap_or("comparison");
    let (left_label, right_label) = raw_report_antenna_labels(report);
    let facts = prioritized_headline_evidence(report, row);
    let nonzero = facts
        .iter()
        .map(|fact| fact.direction)
        .filter(|direction| *direction != 0)
        .collect::<Vec<_>>();
    let assessment = if nonzero.is_empty() {
        "the available headline measures were tied".to_string()
    } else if nonzero.iter().all(|direction| *direction > 0) {
        format!("{right_label} produced stronger recorded results")
    } else if nonzero.iter().all(|direction| *direction < 0) {
        format!("{left_label} produced stronger recorded results")
    } else {
        "the available headline measures were mixed".to_string()
    };
    let group = if include_group {
        format!(" in {}", raw_comparison_stratum(&row.stratum))
    } else {
        String::new()
    };
    format!(
        "For this {goal} run{group}, {assessment}: {}. These results describe this session, not a universal antenna ranking.",
        facts
            .iter()
            .map(|fact| fact.clause.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn no_public_wspr_spots(overview: &crate::ReportOverview) -> bool {
    let public_wspr_groups = overview.strata.iter().filter(|row| {
        row.stratum.observation_kind == antennabench_core::ObservationKind::PublicReport
            && row.stratum.mode.as_str() == "WSPR"
    });
    let mut found_group = false;
    for row in public_wspr_groups {
        found_group = true;
        let recorded_count = row.paired_row_count
            + row.unmatched_left_count
            + row.unmatched_right_count
            + row.missing_snr_left_count
            + row.missing_snr_right_count
            + row.excluded_observation_count;
        if recorded_count > 0 {
            return false;
        }
    }
    found_group
}
pub(in super::super) fn overview_lifecycle_label(
    state: ReportOverviewLifecycleState,
) -> &'static str {
    match state {
        ReportOverviewLifecycleState::NotRecorded => "Not recorded",
        ReportOverviewLifecycleState::Recorded(value) => lifecycle(value),
    }
}
pub(in super::super) fn overview_limitation_text(
    value: ReportOverviewLimitation,
    report: &SessionReport,
) -> String {
    let (left_label, right_label) = raw_report_antenna_labels(report);
    match value {
        ReportOverviewLimitation::ComparisonNotApplicable => {
            "Comparative questions: not applicable to single-antenna profiling.".into()
        }
        ReportOverviewLimitation::UnsupportedComparisonShape => {
            "A/B comparison: unavailable without the required two-label shape.".into()
        }
        ReportOverviewLimitation::NoEligibleBlocks => {
            "Eligible blocks: none with one usable actual slot for each label.".into()
        }
        ReportOverviewLimitation::NoMatchedPaths => {
            "Matched paths: no path had usable signal reports on both antennas within one comparison group.".into()
        }
        ReportOverviewLimitation::UnmatchedPaths {
            left_count,
            right_count,
        } => format!(
            "Unmatched paths: {left_count} {left_label} / {right_count} {right_label}."
        ),
        ReportOverviewLimitation::MissingSnr {
            left_count,
            right_count,
        } => format!("Missing SNR: {left_count} {left_label} / {right_count} {right_label}."),
        ReportOverviewLimitation::DuplicateEvidence {
            exact_count,
            conflicting_group_count,
        } => format!(
            "Duplicates: {exact_count} exact / {conflicting_group_count} conflicting group(s)."
        ),
    }
}
fn acquisition_notices(report: &SessionReport, audit_reference: &str) -> Vec<NoticeView> {
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
        notices.push(NoticeView {
            critical: false,
            label: "Public-source boundary",
            message: "AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee".to_string(),
        });
    }
    notices
}

fn raw_report_antenna_labels(report: &SessionReport) -> (String, String) {
    (
        report
            .comparison
            .left_label
            .clone()
            .unwrap_or_else(|| "Left".to_string()),
        report
            .comparison
            .right_label
            .clone()
            .unwrap_or_else(|| "Right".to_string()),
    )
}

fn raw_comparison_stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        value.mode.as_str(),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}
