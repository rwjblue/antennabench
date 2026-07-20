use super::*;
use crate::ReportAcquisitionWorkflowStatus;

pub(in super::super) fn render_question_navigation(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    include_audit: bool,
) {
    out.push_str("<nav class=\"question-nav\" aria-label=\"Report questions\"><ul><li><a href=\"#what-run-show\">What did the run show?</a></li>");
    for family in ordered_question_families(report) {
        if !question_family_is_primary_available(report, family) {
            continue;
        }
        match family {
            crate::ReportQuestionFamily::SharedPathSignal => out.push_str("<li><a href=\"#same-path-signal\">Shared-path signal</a></li>"),
            crate::ReportQuestionFamily::CommonOpportunityDetection => out.push_str("<li><a href=\"#reporter-activity\">Detection among receivers active in both cycles</a></li><li><a href=\"#coverage-map\">Active-receiver coverage map</a></li>"),
            crate::ReportQuestionFamily::ObservedReach => out.push_str("<li><a href=\"#reach-unique-paths\">Observed reach</a></li>"),
            crate::ReportQuestionFamily::GeographicProfile => out.push_str("<li><a href=\"#distance-direction\">Observed distance and direction profile</a></li>"),
            crate::ReportQuestionFamily::Repeatability => out.push_str("<li><a href=\"#coverage-overlap\">Coverage overlap and repeatability</a></li>"),
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
) {
    if is_single_antenna_lens(report) {
        out.push_str("<aside class=\"panel reading-guide\" aria-labelledby=\"reading-guide-title\"><h2 id=\"reading-guide-title\">How to read this report</h2><ul><li>A missing public report is missing evidence, never a zero-strength signal.</li><li>This profiling report describes the recorded antenna’s observed footprint and repetition; it does not infer unobserved coverage.</li><li>Direction, band, mode, evidence kind, and source remain separate.</li><li>Distance categories describe great-circle distance and do not establish a propagation mode.</li></ul></aside>");
    } else {
        out.push_str("<aside class=\"panel reading-guide\" aria-labelledby=\"reading-guide-title\"><h2 id=\"reading-guide-title\">How to read this report</h2><ul><li>A missing public report is missing evidence, never a zero-strength signal, unless a band-qualified activity census proves that reporter was active for that cycle.</li><li>This report describes evidence; it does not select a winner or prove one antenna is better.</li><li>Each comparison group (direction × band × mode × kind × source) is analyzed separately and never combined.</li><li>A block is a back-to-back pair of cycles, one per antenna.</li><li>Alternating antennas reduces but does not eliminate time and propagation effects.</li></ul></aside>");
    }
}
pub(in super::super) fn render_answer_first_overview(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    render_answer_first_overview_with_reference(out, report, "the audit appendix");
}
pub(in super::super) fn render_answer_first_overview_with_reference(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    audit_reference: &str,
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
    write_html!(
        out,
        "<p class=\"scope-line\">Station <strong>{}</strong> at <strong>{}</strong>; goal: <strong>{}</strong>.</p>",
        escape_html(&scope.station.callsign),
        escape_html(&scope.station.grid),
        scope.goal.map(session_goal).unwrap_or("Not recorded"),
    );
    if let Some(lens) = &overview.goal_lens {
        write_html!(out, "<aside class=\"goal-lens\" aria-label=\"Predeclared goal lens\"><p><strong>{} lens:</strong> {}</p>", session_goal(lens.goal), escape_html(&lens.practical_meaning));
        if !lens.emphasized_distance_bins.is_empty() {
            write_html!(out, "<p class=\"muted\"><strong>Prespecified distance focus:</strong> {}. Every other available distance category remains visible.</p>", lens.emphasized_distance_bins.iter().map(|bin| bin.label()).collect::<Vec<_>>().join("; "));
        }
        out.push_str("<p class=\"muted\">The recorded goal changes presentation priority only; evidence, calculations, thresholds, and conclusions are unchanged.</p></aside>");
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
    if overview
        .strata
        .iter()
        .any(|row| matches!(row.path_delta, ReportOverviewPathDelta::Available { .. }))
    {
        out.push_str("<p class=\"muted delta-scale\"><strong>For scale:</strong> a 3 dB difference is the same change as doubling transmit power. Individual WSPR reports are whole-dB values that vary cycle to cycle.</p>");
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
    out.push_str("</tbody></table></div><div class=\"overview-support\"><section aria-labelledby=\"supported-title\"><h3 id=\"supported-title\">Supported by this run</h3><ul>");
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
    let answered = if answered.is_empty() {
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
    out.push_str("<details class=\"answerability-disclosure\"><summary>Question availability and limits</summary><dl class=\"facts answerability-facts\">");
    answerability_fact(
        out,
        "Shared-path signal",
        same_path_answerability_text(answerability.same_path_signal),
    );
    answerability_fact(
        out,
        "Detection among receivers active in both cycles",
        paired_detectability_answerability_text(answerability.paired_detectability),
    );
    answerability_fact(
        out,
        "Observed reach",
        match answerability.observed_reach {
            ObservedReachAnswerability::Available => "Available from unique observed paths",
            ObservedReachAnswerability::NoUsablePaths => "No usable paths",
        },
    );
    answerability_fact(
        out,
        "Observed distance and direction profile",
        match answerability.geographic_profile {
            GeographicProfileAnswerability::Available => "Available from located observed paths",
            GeographicProfileAnswerability::NoLocatedPaths => "No located paths",
        },
    );
    answerability_fact(
        out,
        "Repeatability across blocks",
        match answerability.repeatability {
            RepeatabilityAnswerability::Available => "Available across repeated eligible blocks",
            RepeatabilityAnswerability::InsufficientRepetition => "Insufficient repetition",
        },
    );
    out.push_str("</dl></details>");
}

fn answerability_fact(out: &mut CheckedHtmlWriter<'_>, question: &str, availability: &str) {
    write_html!(
        out,
        "<div><dt>{}</dt><dd>{}</dd></div>",
        escape_html(question),
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
        antennabench_analysis::ComparisonAvailability::NoMatchedPaths => {
            no_matched_path_sentences(report)
        }
        antennabench_analysis::ComparisonAvailability::DescriptivePairsAvailable => {
            descriptive_group_sentences(overview)
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
fn descriptive_group_sentences(overview: &crate::ReportOverview) -> Vec<String> {
    let orientation = overview.scope.delta_orientation.as_ref();
    let mut sentences = overview
        .strata
        .iter()
        .filter_map(|row| {
            let ReportOverviewPathDelta::Available {
                median_path_delta_right_minus_left_db,
                ..
            } = row.path_delta
            else {
                return None;
            };
            let station_count = row.unique_path_count;
            let stations = if station_count == 1 {
                "station"
            } else {
                "stations"
            };
            let group = comparison_stratum(&row.stratum);
            if median_path_delta_right_minus_left_db == 0.0 {
                return Some(format!(
                    "{station_count} {stations} heard both antennas on {group}; the typical (median) difference was 0 dB, with no signed difference at the median."
                ));
            }
            match orientation {
                Some(orientation) => {
                    let (left_label, right_label) = orientation_antenna_labels(orientation);
                    let stronger_label = if median_path_delta_right_minus_left_db > 0.0 {
                        right_label
                    } else {
                        left_label
                    };
                    Some(format!(
                        "{station_count} {stations} heard both antennas on {group}; the typical (median) difference was {} dB, with {stronger_label} stronger.",
                        format_number(median_path_delta_right_minus_left_db.abs())
                    ))
                }
                None => Some(format!(
                    "{station_count} {stations} heard both antennas on {group}; the typical median signed difference was {} dB.",
                    format_signed(median_path_delta_right_minus_left_db)
                )),
            }
        })
        .collect::<Vec<_>>();
    if sentences.is_empty() {
        sentences.push(
            "Matched comparisons are available; review the per-group table for their descriptive results."
                .to_string(),
        );
    } else {
        sentences.push(
            "See the per-group table for the observed spread in each comparison group.".to_string(),
        );
    }
    sentences
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
