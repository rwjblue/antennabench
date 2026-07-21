use super::*;
use crate::ReportAcquisitionWorkflowStatus;

pub(in super::super) fn render_question_navigation(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    include_audit: bool,
) {
    out.push_str("<nav class=\"question-nav\" aria-label=\"Report questions\"><ul><li><a href=\"#what-run-show\">What did the run show?</a></li>");
    let mut observed_footprint_linked = false;
    for family in ordered_question_families(report) {
        if !question_family_is_primary_available(report, family) {
            continue;
        }
        match family {
            crate::ReportQuestionFamily::SharedPathSignal => out.push_str("<li><a href=\"#same-path-signal\">Shared-path signal</a></li>"),
            crate::ReportQuestionFamily::CommonOpportunityDetection => out.push_str("<li><a href=\"#reporter-activity\">Detection among receivers active in both cycles</a></li><li><a href=\"#coverage-map\">Active-receiver coverage map</a></li>"),
            crate::ReportQuestionFamily::ObservedReach => {
                if include_audit {
                    out.push_str("<li><a href=\"#reach-unique-paths\">Observed reach</a></li>");
                } else if !observed_footprint_linked {
                    out.push_str("<li><a href=\"#observed-footprint\">Observed footprint</a></li>");
                    observed_footprint_linked = true;
                }
            }
            crate::ReportQuestionFamily::GeographicProfile => {
                if include_audit {
                    out.push_str("<li><a href=\"#distance-direction\">Observed distance and direction profile</a></li>");
                } else if !observed_footprint_linked {
                    out.push_str("<li><a href=\"#observed-footprint\">Observed footprint</a></li>");
                    observed_footprint_linked = true;
                }
            }
            crate::ReportQuestionFamily::Repeatability => {
                if include_audit {
                    out.push_str("<li><a href=\"#coverage-overlap\">Coverage overlap and repeatability</a></li>");
                } else if !observed_footprint_linked {
                    out.push_str("<li><a href=\"#observed-footprint\">Observed footprint</a></li>");
                    observed_footprint_linked = true;
                }
            }
        }
    }
    out.push_str("<li><a href=\"#run-quality\">Run quality</a></li>");
    if include_audit {
        out.push_str("<li><a href=\"#audit-appendix\">Audit appendix</a></li>");
    }
    out.push_str("</ul></nav>");
}
pub(in super::super) fn render_how_to_read(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    compact: bool,
) {
    let (prefix, suffix) = if compact {
        (
            "<details class=\"panel reading-guide\"><summary>How to read this report</summary><div class=\"reading-guide-body\">",
            "</div></details>",
        )
    } else {
        (
            "<aside class=\"panel reading-guide\" aria-labelledby=\"reading-guide-title\"><h2 id=\"reading-guide-title\">How to read this report</h2>",
            "</aside>",
        )
    };
    out.push_str(prefix);
    if is_single_antenna_lens(report) {
        out.push_str("<ul><li>A missing public report is missing evidence, never a zero-strength signal.</li><li>This profiling report describes the recorded antenna’s observed footprint and repetition; it does not infer unobserved coverage.</li><li>Direction, band, mode, evidence kind, and source remain separate.</li><li>Distance categories describe great-circle distance and do not establish a propagation mode.</li></ul>");
    } else {
        out.push_str("<ul><li>A missing public report is missing evidence, never a zero-strength signal, unless a band-qualified activity census proves that reporter was active for that cycle.</li><li>This report describes evidence; it does not select a winner or prove one antenna is better.</li><li>Each comparison group (direction × band × mode × kind × source) is analyzed separately and never combined.</li><li>A block is a back-to-back pair of cycles, one per antenna.</li><li>Alternating antennas reduces but does not eliminate time and propagation effects.</li></ul>");
    }
    out.push_str(suffix);
}
pub(in super::super) fn render_answer_first_overview(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    render_answer_first_overview_with_reference(out, report, "the audit appendix", false);
}
pub(in super::super) fn render_answer_first_overview_with_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
    compact: bool,
) {
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

    out.push_str("<section id=\"what-run-show\" class=\"panel overview\" tabindex=\"-1\" aria-labelledby=\"what-run-show-title\"><p class=\"eyebrow\">Answer first</p><h2 id=\"what-run-show-title\">What did the run show?</h2>");
    render_answerability_headline(out, report);
    render_plain_language_answer(out, report);
    render_headline_metrics(out, report);
    write_html!(
        out,
        "<div class=\"scope-line\">Station <strong>{}</strong> at <strong>{}</strong>; goal: <strong>{}</strong>",
        escape_html(&scope.station.callsign),
        escape_html(&scope.station.grid),
        scope.goal.map(session_goal).unwrap_or("Not recorded"),
    );
    if let Some(lens) = &overview.goal_lens {
        if compact {
            write_html!(out, " <details class=\"goal-help\"><summary aria-label=\"About the recorded {} goal lens\">i</summary><div class=\"goal-help-popover\"><strong>{} lens</strong><p>{}</p>", session_goal(lens.goal), session_goal(lens.goal), escape_html(&lens.practical_meaning));
            if !lens.emphasized_distance_bins.is_empty() {
                write_html!(out, "<p><strong>Prespecified distance focus:</strong> {}. Every other available distance category remains visible.</p>", lens.emphasized_distance_bins.iter().map(|bin| bin.label()).collect::<Vec<_>>().join("; "));
            }
            out.push_str("<p>The goal changes presentation priority only; evidence and calculations are unchanged.</p></div></details>");
        }
        out.push_str(".</div>");
        if !compact {
            write_html!(out, "<aside class=\"goal-lens\" aria-label=\"Predeclared goal lens\"><p><strong>{} lens:</strong> {}</p>", session_goal(lens.goal), escape_html(&lens.practical_meaning));
            if !lens.emphasized_distance_bins.is_empty() {
                write_html!(out, "<p class=\"muted\"><strong>Prespecified distance focus:</strong> {}. Every other available distance category remains visible.</p>", lens.emphasized_distance_bins.iter().map(|bin| bin.label()).collect::<Vec<_>>().join("; "));
            }
            out.push_str("<p class=\"muted\">The recorded goal changes presentation priority only; evidence, calculations, thresholds, and conclusions are unchanged.</p></aside>");
        }
    } else {
        out.push_str(".</div>");
    }
    out.push_str("<dl class=\"facts headline-facts\">");
    fact(out, "Antennas", &antennas);
    fact(out, "Bands", &bands);
    fact(out, "Direction / mode", &format!("{directions}; {mode}"));
    fact(out, "Session state", lifecycle_label);
    out.push_str("</dl>");

    match &scope.delta_orientation {
        Some(orientation) => write_html!(
            out,
            "<p class=\"orientation\"><strong>Signed values:</strong> Positive values mean {} was stronger; negative values mean {} was stronger.</p>",
            escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label),
        ),
        None if is_single_antenna_lens(report) => out.push_str("<p class=\"orientation\"><strong>Signal-difference orientation:</strong> not applicable to this single-antenna profiling run.</p>"),
        None => out.push_str("<p class=\"orientation\"><strong>Delta orientation:</strong> unavailable because this run does not provide a two-label paired orientation.</p>"),
    }
    if !compact
        && overview
            .strata
            .iter()
            .any(|row| matches!(row.path_delta, ReportOverviewPathDelta::Available { .. }))
    {
        out.push_str("<p class=\"muted delta-scale\"><strong>For scale:</strong> a 3 dB difference is the same change as doubling transmit power. Individual WSPR reports are whole-dB values that vary cycle to cycle.</p>");
    }

    if compact {
        out.push_str("<details class=\"audit-disclosure overview-group-disclosure\"><summary>Review exact per-group headline evidence</summary><div class=\"disclosure-body\">");
    }
    out.push_str("<div class=\"table-wrap\"><table class=\"overview-table\"><caption>Descriptive result by comparison group</caption><thead><tr><th scope=\"col\">Comparison group</th><th scope=\"col\">Path delta</th><th scope=\"col\">Paths / matched pairs</th><th scope=\"col\">Blocks <small>(back-to-back cycle pairs, one cycle per antenna)</small></th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    if overview.strata.is_empty() {
        out.push_str("<tr><td data-label=\"Comparison group\" colspan=\"5\">No comparison groups are available for this run.</td></tr>");
    } else {
        for row in overview
            .strata
            .iter()
            .filter(|row| matches!(row.path_delta, ReportOverviewPathDelta::Available { .. }))
        {
            let (delta, coverage) = match row.path_delta {
                ReportOverviewPathDelta::Unavailable => {
                    ("Not available".to_string(), "Unavailable".to_string())
                }
                ReportOverviewPathDelta::Available {
                    minimum_delta_right_minus_left_db,
                    median_path_delta_right_minus_left_db,
                    maximum_delta_right_minus_left_db,
                } => (
                    format!(
                        "{} to {} dB; median across paths {} dB",
                        format_signed(minimum_delta_right_minus_left_db),
                        format_signed(maximum_delta_right_minus_left_db),
                        format_signed(median_path_delta_right_minus_left_db),
                    ),
                    "Available".to_string(),
                ),
            };
            write_html!(
                out,
                "<tr><td data-label=\"Comparison group\">{}</td><td data-label=\"Path delta\">{}</td><td data-label=\"Paths / matched pairs\">{} / {}</td><td data-label=\"Blocks\">{}</td><td data-label=\"Coverage\">{}</td></tr>",
                comparison_stratum(&row.stratum),
                delta,
                row.unique_path_count,
                row.paired_row_count,
                row.contributing_block_count,
                coverage,
            );
        }
        let unavailable = overview
            .strata
            .iter()
            .filter(|row| row.path_delta == ReportOverviewPathDelta::Unavailable)
            .collect::<Vec<_>>();
        if !unavailable.is_empty() {
            write_html!(out, "<tr class=\"collapsed-empty-strata\"><td data-label=\"Comparison group\">No path delta in {}: {}</td><td data-label=\"Path delta\">Not available</td><td data-label=\"Paths / matched pairs\">Kept separate</td><td data-label=\"Blocks\">Kept separate</td><td data-label=\"Coverage\">Unavailable</td></tr>", comparison_groups_label(unavailable.len()), comparison_strata_list(&unavailable));
        }
    }
    out.push_str("</tbody></table></div>");
    if compact {
        out.push_str("</div></details>");
    }
    out.push_str("<div class=\"overview-support\"><section aria-labelledby=\"supported-title\"><h3 id=\"supported-title\">Supported by this run</h3><ul>");
    write_html!(
        out,
        "<li>The recorded comparison state is <strong>{}</strong>.</li>",
        comparison_availability_label(overview.comparison_availability),
    );
    if overview.strata.is_empty() {
        out.push_str("<li>The session scope and availability state remain explicit even without an available comparison group.</li>");
    } else {
        write_html!(
            out,
            "<li>{} matched pair(s) are retained in {} separate comparison group(s).</li>",
            paired_rows,
            overview.strata.len(),
        );
    }
    out.push_str("</ul></section>");
    if !overview.limitations.is_empty() {
        out.push_str("<section aria-labelledby=\"not-established-title\"><h3 id=\"not-established-title\">Not established by this run</h3><ul>");
        for limitation in &overview.limitations {
            write_html!(
                out,
                "<li>{}</li>",
                overview_limitation_text(*limitation, report)
            );
        }
        out.push_str("</ul></section>");
    }
    out.push_str("</div>");
    render_visible_acquisition_limitations(out, report, audit_reference);
    render_answerability_disclosure(out, report);
    out.push_str("</section>");
}

fn render_answerability_headline(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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
    let answered = if answered.len() == applicable_count {
        format!("All {applicable_count} applicable question families have usable evidence.")
    } else if answered.is_empty() {
        "None of the five report question families has usable evidence yet.".to_string()
    } else {
        format!("Answered by this run: {}.", answered.join("; "))
    };
    write_html!(
        out,
        "<p class=\"answerability-headline\"><strong>{}</strong></p>",
        escape_html(&answered)
    );
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

fn render_answerability_disclosure(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let answerability = &report.overview.answerability;
    out.push_str("<details class=\"audit-disclosure answerability-disclosure\"><summary>Question availability and limits</summary><dl class=\"facts answerability-facts disclosure-body\">");
    answerability_fact(
        out,
        "Shared-path signal",
        if answerability.same_path_signal == SamePathSignalAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        },
        same_path_answerability_text(answerability.same_path_signal),
    );
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
    answerability_fact(
        out,
        "Detection among receivers active in both cycles",
        paired_status,
        paired_detectability_answerability_text(answerability.paired_detectability),
    );
    answerability_fact(
        out,
        "Observed reach",
        if answerability.observed_reach == ObservedReachAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        },
        match answerability.observed_reach {
            ObservedReachAnswerability::Available => "Available from unique observed paths",
            ObservedReachAnswerability::NoUsablePaths => "No usable paths",
        },
    );
    answerability_fact(
        out,
        "Observed distance and direction profile",
        if answerability.geographic_profile == GeographicProfileAnswerability::Available {
            "Available"
        } else {
            "Unavailable"
        },
        match answerability.geographic_profile {
            GeographicProfileAnswerability::Available => "Available from located observed paths",
            GeographicProfileAnswerability::NoLocatedPaths => "No located paths",
        },
    );
    answerability_fact(
        out,
        "Repeatability across blocks",
        if answerability.repeatability == RepeatabilityAnswerability::Available {
            "Available"
        } else {
            "Limited"
        },
        match answerability.repeatability {
            RepeatabilityAnswerability::Available => "Available across repeated eligible blocks",
            RepeatabilityAnswerability::InsufficientRepetition => "Insufficient repetition",
        },
    );
    out.push_str("</dl></details>");
}

fn answerability_fact(
    out: &mut CheckedHtmlWriter<'_>,
    question: &str,
    status: &str,
    availability: &str,
) {
    write_html!(
        out,
        "<div class=\"fact availability-row\"><dt>{}</dt><dd><strong class=\"badge availability-status status-{}\">{}</strong><br><span>{}</span></dd></div>",
        escape_html(question),
        status.to_ascii_lowercase(),
        status,
        escape_html(availability)
    );
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
fn render_plain_language_answer(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    write_html!(
        out,
        "<p class=\"answer plain-language-answer\">{}</p>",
        plain_language_answer(report)
    );
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

fn render_headline_metrics(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let groups = report
        .overview
        .strata
        .iter()
        .filter_map(|row| {
            let facts = prioritized_headline_evidence(report, row);
            (!facts.is_empty()).then_some((row, facts))
        })
        .collect::<Vec<_>>();
    for (index, (row, facts)) in groups.iter().enumerate() {
        if groups.len() > 1 {
            write_html!(
                out,
                "<h3 class=\"headline-group-title\" id=\"headline-group-{index}\">{}</h3>",
                comparison_stratum(&row.stratum)
            );
            write_html!(
                out,
                "<p class=\"headline-group-answer\">{}</p>",
                descriptive_group_sentence(report, row, false)
            );
        }
        out.push_str("<dl class=\"facts answer-metrics\">");
        for fact in facts {
            write_html!(
                out,
                "<div class=\"fact\"><dt>{}</dt><dd>{}<br><small class=\"muted\">{}</small></dd></div>",
                fact.label,
                fact.value,
                fact.detail
            );
        }
        out.push_str("</dl>");
    }
}

fn prioritized_headline_evidence(
    report: &SessionReport,
    row: &ReportOverviewStratum,
) -> Vec<HeadlineEvidence> {
    let (left_label, right_label) = report_antenna_labels(report);
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
    let (left_label, right_label) = report_antenna_labels(report);
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
            comparison_stratum(&row.stratum),
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
    let (left_label, right_label) = report_antenna_labels(report);
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
        format!(" in {}", comparison_stratum(&row.stratum))
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
    let (left_label, right_label) = report_antenna_labels(report);
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
pub(in super::super) fn render_visible_acquisition_limitations(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
) {
    let evidence = &report.snapshot.adapter_evidence;
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
        write_html!(
            out,
            "<p class=\"notice critical\"><strong>Recorded acquisition:</strong> {}.</p>",
            message
        );
    }
    if evidence
        .imports
        .iter()
        .any(|import| import.provider_id == "wspr-live")
    {
        out.push_str("<p class=\"notice\"><strong>Public-source boundary:</strong> AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee.</p>");
    }
}
