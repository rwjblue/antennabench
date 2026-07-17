use std::{
    collections::BTreeSet,
    fmt::{self, Write},
};

use antennabench_analysis::{
    ComparisonAvailability, ComparisonBlockEligibility, ComparisonOrder, ComparisonSide,
    ComparisonStratum, ComparisonTimelineRow, EligibilityExclusionCategory, EligibilityScope,
    EvidenceQuality, ObservationCounts, ObservationExclusionReason, PairedObservationRow,
    PathDirection, SnrStatistics, SolarEndpointContext, SolarLightState, SolarPositionResult,
};
use antennabench_core::{
    AlignedSlotStatus, Band, ExperimentMode, ObservationKind, RecordSource, SessionGoal,
    SessionLifecycleV2, WsprCycleDirection,
};
use chrono::{SecondsFormat, Utc};

use crate::{
    check_cancelled, report_resource_error, AntennaEvidenceSection, BandEvidenceSection,
    ReportAzimuthSector, ReportCancellationToken, ReportDetailFamily, ReportDistanceBin,
    ReportError, ReportEvidenceSummary, ReportImportedEvidence, ReportLifecycleEventKind,
    ReportNotice, ReportOperatorEvent, ReportOperatorEventKind, ReportOverviewLifecycleState,
    ReportOverviewLimitation, ReportOverviewLocationCell, ReportOverviewPathDelta,
    ReportOverviewStratum, ReportPathLocationAvailability, ReportResourceLimits,
    ReportResourceStage, ReportRunTimelineRow, ReportStratumAvailability, SessionReport,
    SlotEvidenceSection, UsableObservationKindCounts, REPORT_RESOURCE_LIMITS,
};

macro_rules! write_html {
    ($output:expr, $($argument:tt)*) => {
        write!($output, $($argument)*).expect("checked HTML writer records failures")
    };
}

const STYLES: &str = r#"
:root{color-scheme:light;--ink:#172033;--muted:#5c667a;--line:#d8deea;--paper:#fff;--soft:#f5f7fb;--usable:#237a57;--excluded:#b84b4b;--accent:#315da8;--accent-soft:#edf3ff}*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:1rem}body{margin:0;background:var(--soft);color:var(--ink);font:15px/1.42 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}main{width:min(1120px,calc(100% - 2rem));margin:1rem auto 3rem}.skip-link{position:absolute;left:1rem;top:-5rem;padding:.65rem .85rem;background:var(--ink);color:#fff;z-index:2}.skip-link:focus{top:1rem}.hero,.panel,.question-nav{background:var(--paper);border:1px solid var(--line);border-radius:.65rem;box-shadow:0 1px 2px #17203312}.hero{display:grid;grid-template-columns:1fr auto;align-items:end;gap:.1rem 1rem;padding:.7rem 1rem}.hero .eyebrow{grid-column:1/-1}.hero h1{margin:0;font-size:clamp(1.55rem,3vw,2rem)}.hero .muted{margin:0}.eyebrow{margin:0;color:var(--accent);font-size:.72rem;font-weight:700;letter-spacing:.09em;text-transform:uppercase}.muted{color:var(--muted)}.question-nav{margin-top:.5rem;padding:.35rem .55rem}.question-nav ul{display:flex;flex-wrap:wrap;gap:.15rem .35rem;margin:0;padding:0;list-style:none}.question-nav a{display:block;padding:.25rem .4rem;border-radius:.35rem;color:var(--accent);font-size:.82rem;font-weight:700;text-decoration-thickness:.08em;text-underline-offset:.16em}.question-nav a:hover{background:var(--accent-soft)}a:focus-visible,summary:focus-visible{outline:3px solid #e09b22;outline-offset:3px}.panel{margin-top:.75rem;padding:1rem;overflow:hidden}.panel h2{margin:.05rem 0 .4rem}.panel h3{margin:1rem 0 .35rem}.overview{border-top:.3rem solid var(--accent);padding:.75rem 1rem}.overview .answer{margin:.4rem 0;padding:.5rem .65rem;background:var(--accent-soft);border-radius:.5rem;font-size:.98rem}.scope-line{margin:.2rem 0 .5rem;color:var(--muted)}.orientation{margin:.5rem 0;padding:.4rem .55rem;border:1px solid var(--line);border-radius:.45rem}.headline-facts{grid-template-columns:repeat(4,minmax(0,1fr));margin:.45rem 0}.facts,.stat-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.45rem}.fact,.stat{padding:.45rem .55rem;background:var(--soft);border-radius:.45rem}.fact dt,.stat dt{color:var(--muted);font-size:.68rem;font-weight:700;text-transform:uppercase}.fact dd,.stat dd{margin:.08rem 0 0;font-weight:650;overflow-wrap:anywhere}.overview-support{display:grid;grid-template-columns:1fr 1fr;gap:.5rem;margin-top:.5rem}.overview-support section{padding:.45rem .6rem;border:1px solid var(--line);border-radius:.45rem}.overview-support h3{margin:0 0 .15rem;font-size:.88rem}.overview-support ul{margin:.1rem 0;padding-left:1rem;font-size:.82rem;line-height:1.3}.overview-support li+li{margin-top:.1rem}.notice{padding:.6rem .8rem;border-left:.3rem solid #b36b00;background:#fff8e8}.critical{border-left-color:#8a3f00}.badge{display:inline-block;padding:.12rem .45rem;border-radius:999px;background:#e5ebf7;font-size:.78rem;font-weight:700}.empty{padding:.7rem;border:1px dashed var(--line);border-radius:.5rem;color:var(--muted)}.table-wrap{overflow-x:auto}table{width:100%;border-collapse:collapse;font-size:.84rem}caption{text-align:left;font-weight:700;padding:.2rem 0 .35rem}th,td{padding:.38rem .48rem;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}thead th{background:var(--soft);white-space:nowrap}.overview-table{font-size:.78rem;line-height:1.28}.overview-table td:first-child{min-width:13rem}.question-section>p:first-of-type{margin-top:0}.audit-disclosure{margin-top:.55rem;border:1px solid var(--line);border-radius:.5rem;background:var(--paper)}.audit-disclosure>summary{padding:.6rem .7rem;cursor:pointer;color:var(--accent);font-weight:750}.audit-disclosure[open]>summary{border-bottom:1px solid var(--line)}.disclosure-body{padding:0 .7rem .7rem}.audit-disclosure .panel{margin:.65rem 0 0;box-shadow:none}.chart,.comparison-chart{display:grid;gap:.5rem;margin:.5rem 0 1rem;padding:.8rem;background:var(--soft);border-radius:.5rem}.chart-row{display:grid;grid-template-columns:minmax(7rem,14rem) 1fr 4.5rem;gap:.6rem;align-items:center}.comparison-row{display:grid;grid-template-columns:minmax(8rem,16rem) 1fr minmax(5rem,auto);gap:.6rem;align-items:center}.chart-label{overflow-wrap:anywhere}.bar-track,.snr-track,.comparison-track,.snr-pair{position:relative;height:1rem;background:#e1e6ef;border-radius:999px;overflow:hidden}.bar{height:100%;float:left}.bar.usable{background:var(--usable)}.bar.excluded{background:var(--excluded)}.snr-range{position:absolute;top:.3rem;height:.4rem;border-radius:999px;background:var(--accent)}.snr-point{position:absolute;top:.1rem;width:.18rem;height:.8rem;background:var(--ink)}.comparison-zero{position:absolute;left:50%;top:0;width:1px;height:100%;background:var(--muted)}.comparison-delta{position:absolute;top:.2rem;height:.6rem;border-radius:999px;background:var(--accent)}.snr-pair{height:1.2rem}.snr-left,.snr-right{position:absolute;top:.15rem;width:.55rem;height:.9rem;border:2px solid var(--paper);border-radius:50%;transform:translateX(-50%)}.snr-left{background:#315da8}.snr-right{background:#b35c00}.timeline{display:flex;flex-wrap:wrap;gap:.35rem;margin:.5rem 0 1rem;padding:.8rem;background:var(--soft);border-radius:.5rem}.timeline-slot{min-width:2.6rem;padding:.45rem;border:1px solid var(--line);border-radius:.35rem;text-align:center}.timeline-slot.invalid{border-style:dashed;color:var(--muted)}.timeline-slot.issue{background:#fff8e8}.legend{display:flex;gap:1rem;color:var(--muted);font-size:.82rem}.swatch{display:inline-block;width:.7rem;height:.7rem;margin-right:.25rem;border-radius:.15rem}.swatch.usable{background:var(--usable)}.swatch.excluded{background:var(--excluded)}.antenna-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:.75rem}.antenna-card{padding:1rem;border:1px solid var(--line);border-radius:.5rem}.antenna-card h3{margin:0 0 .6rem}.antenna-card dl{margin:0}.antenna-card dt{color:var(--muted);font-size:.8rem;font-weight:700}.antenna-card dd{margin:0 0 .45rem}.footnote{font-size:.84rem;color:var(--muted)}
@media print{body{background:#fff;font-size:10.5pt}main{width:100%;margin:0}.hero,.panel,.question-nav{box-shadow:none}.question-nav,.skip-link{display:none}.hero{padding:.65rem .8rem}.overview{margin-top:.55rem;padding:.7rem;break-after:page}.overview .answer,.orientation{padding:.4rem .55rem;margin:.4rem 0}.headline-facts{margin:.45rem 0;gap:.4rem}.fact{padding:.4rem .5rem}.overview-support{margin-top:.45rem;gap:.45rem}.overview-support section{padding:.4rem .55rem}.overview-support li+li{margin-top:0}.overview table{font-size:8.8pt}details:not([open])>:not(summary){display:none!important}.audit-disclosure{break-inside:avoid}.audit-disclosure[open]{break-inside:auto}.panel{box-shadow:none}.question-section{break-before:page}}
@media(max-width:760px){.headline-facts{grid-template-columns:repeat(2,minmax(0,1fr))}.overview-support{grid-template-columns:1fr}}
@media(max-width:620px){main{width:min(100% - 1rem,1120px);margin:.5rem auto 2rem}.hero{display:block}.hero .muted{margin-top:.2rem}.hero,.panel{border-radius:.55rem}.chart-row,.comparison-row{grid-template-columns:1fr}.chart-value{color:var(--muted)}.question-nav ul{display:grid;grid-template-columns:1fr 1fr}.overview-table thead{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.overview-table tr{display:block;margin:.5rem 0;border:1px solid var(--line);border-radius:.45rem}.overview-table td{display:grid;grid-template-columns:minmax(7rem,42%) 1fr;gap:.5rem;border-bottom:1px solid var(--line)}.overview-table td:last-child{border-bottom:0}.overview-table td::before{content:attr(data-label);color:var(--muted);font-size:.72rem;font-weight:700;text-transform:uppercase}.overview-table td:first-child{min-width:0}.headline-facts{grid-template-columns:1fr 1fr}}
.swatch.left,.bar.left{background:#315da8}.swatch.right,.bar.right{background:#b35c00}
.path-strip{display:grid;gap:.3rem;margin:.55rem 0;padding:.7rem;background:var(--soft);border-radius:.5rem}.path-strip-row{display:grid;grid-template-columns:minmax(7rem,15rem) 1fr 4.5rem;gap:.55rem;align-items:center}.path-strip-track{position:relative;height:1.15rem;background:#e1e6ef;border-radius:.2rem}.path-strip-zero{position:absolute;left:50%;top:0;width:2px;height:100%;background:var(--muted)}.path-strip-dot{position:absolute;top:.2rem;width:.72rem;height:.72rem;background:#315da8;border:2px solid var(--paper);border-radius:50%;transform:translateX(-50%)}.path-strip-median{position:absolute;top:.16rem;width:.82rem;height:.82rem;background:#6d4c9a;border:2px solid var(--paper);transform:translateX(-50%) rotate(45deg)}.path-view-note{margin:.35rem 0;color:var(--muted);font-size:.84rem}.reach-strip{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));overflow:hidden;margin:.55rem 0;border:1px solid var(--line);border-radius:.45rem}.reach-strip span{min-height:2.5rem;padding:.4rem .55rem;border-right:1px solid var(--paper);background:#e8effb}.reach-strip span:nth-child(2){background:#e7e3f2}.reach-strip span:last-child{border-right:0;background:#f8ead9}.reach-strip strong{display:block;font-size:1.15rem}.reach-strip small{display:block;color:var(--muted)}
@media(max-width:620px){.path-strip-row{grid-template-columns:1fr}.reach-strip{grid-template-columns:1fr}.reach-strip span{border-right:0;border-bottom:1px solid var(--paper)}.reach-strip span:last-child{border-bottom:0}}
.location-fill{height:100%;background:#315da8;border-radius:999px}.azimuth-track{position:relative;height:1rem;background:linear-gradient(90deg,#e1e6ef 0 24.8%,#cbd5e7 25% 25.2%,#e1e6ef 25.4% 49.8%,#cbd5e7 50% 50.2%,#e1e6ef 50.4% 74.8%,#cbd5e7 75% 75.2%,#e1e6ef 75.4% 100%);border-radius:999px}.azimuth-marker{position:absolute;top:.1rem;width:.45rem;height:.8rem;background:#b35c00;border:2px solid var(--paper);border-radius:50%;transform:translateX(-50%)}
.location-context{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.45rem;margin:.55rem 0}.location-context-cell{min-height:5.2rem;padding:.5rem .6rem;border:1px solid var(--line);border-radius:.45rem;background:var(--soft)}.location-context-cell strong,.location-context-cell small{display:block}.location-context-cell small{color:var(--muted)}.location-context-cell.empty-cell{border-style:dashed}.location-context-table td:first-child{min-width:12rem}
.answerability-table td:first-child{min-width:14rem}.run-timeline{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:.55rem;margin:.55rem 0}.run-timeline details{margin:0;border:2px solid var(--line);border-radius:.5rem;background:var(--paper)}.run-timeline summary{padding:.55rem .65rem;cursor:pointer;font-weight:700}.run-timeline .timeline-detail{padding:.1rem .65rem .65rem;border-top:1px solid var(--line)}.timeline-state{display:inline-block;min-width:1.45rem;margin-right:.25rem;text-align:center}.state-late{border-style:dashed!important}.state-missed{border-left-color:#805500!important}.state-bad{border-left-color:#8a2f2f!important;border-style:double!important}.state-unknown{border-style:dotted!important}.state-interrupted{border-top-color:#633c91!important;border-bottom-color:#633c91!important}.state-corrected{border-width:3px!important}.lifecycle-strip{display:flex;flex-wrap:wrap;gap:.35rem;margin:.45rem 0}.lifecycle-chip{padding:.25rem .45rem;border:1px solid var(--line);border-radius:.35rem;background:var(--soft);font-size:.82rem}.lifecycle-chip strong{margin-right:.2rem}.audit-event-list{margin:.4rem 0;padding-left:1.2rem}.audit-event-list li+li{margin-top:.25rem}
@media(max-width:620px){.run-timeline{grid-template-columns:1fr}.answerability-table thead{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.answerability-table tr{display:block;margin:.5rem 0;border:1px solid var(--line);border-radius:.45rem}.answerability-table td{display:grid;grid-template-columns:minmax(8rem,42%) 1fr;gap:.5rem}.answerability-table td::before{content:attr(data-label);color:var(--muted);font-size:.72rem;font-weight:700;text-transform:uppercase}.answerability-table td:first-child{min-width:0}}
@media print{.answerability-table{display:block}.answerability-table thead{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.answerability-table tbody{display:grid;grid-template-columns:1fr 1fr;gap:.45rem}.answerability-table tr{display:grid;grid-template-columns:1fr 1fr;align-content:start;break-inside:avoid;border:1px solid var(--line);border-radius:.45rem}.answerability-table td{display:grid;grid-template-columns:minmax(6.5rem,44%) 1fr;gap:.35rem;padding:.28rem .4rem}.answerability-table td::before{content:attr(data-label);color:var(--muted);font-size:7.5pt;font-weight:700;text-transform:uppercase}.answerability-table td:first-child{grid-column:1/-1;min-width:0}.answerability-table td:nth-child(2){grid-column:1/-1}.answerability-table td:nth-last-child(-n+2){border-bottom:0}}
"#;

/// Renders a deterministic, standalone HTML document from renderer-neutral
/// report data. The output contains no scripts, external resources, or
/// unescaped report strings.
pub fn render_standalone_html(report: &SessionReport) -> Result<String, ReportError> {
    render_standalone_html_with_resources(
        report,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
    )
}

pub fn render_standalone_html_with_resources(
    report: &SessionReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<String, ReportError> {
    check_cancelled(cancellation, ReportResourceStage::Render, "standalone_html")?;
    let mut out = CheckedHtmlWriter::new(limits.html_bytes, cancellation);
    out.push_str(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"color-scheme\" content=\"light\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\">\
<title>AntennaBench session report</title><style>",
    );
    out.push_str(STYLES);
    out.push_str("</style></head><body><main><a class=\"skip-link\" href=\"#what-run-show\">Skip to report findings</a>");

    write_html!(
        out,
        "<header class=\"hero\"><p class=\"eyebrow\">AntennaBench local report</p>\
<h1>Session evidence report</h1><p class=\"muted\">Session <code>{}</code></p></header>",
        escape_html(&report.overview.scope.session_id)
    );
    render_question_navigation(&mut out);
    render_answer_first_overview(&mut out, report);
    render_same_path_section(&mut out, report);
    render_reach_section(&mut out, report);
    render_distance_section(&mut out, report);
    render_run_quality_section(&mut out, report);
    render_audit_appendix(&mut out, report);

    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This report is descriptive and does not select an antenna winner.</p></main></body></html>");
    out.finish().map_err(ReportError::from)
}

fn render_question_navigation(out: &mut CheckedHtmlWriter<'_>) {
    out.push_str(
        "<nav class=\"question-nav\" aria-label=\"Report questions\"><ul>\
<li><a href=\"#what-run-show\">What did the run show?</a></li>\
<li><a href=\"#same-path-signal\">Same-path signal</a></li>\
<li><a href=\"#reach-unique-paths\">Reach and unique paths</a></li>\
<li><a href=\"#distance-direction\">Distance and direction</a></li>\
<li><a href=\"#run-quality\">Run quality</a></li>\
<li><a href=\"#audit-appendix\">Audit appendix</a></li>\
</ul></nav>",
    );
}

fn render_answer_first_overview(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

    out.push_str("<section id=\"what-run-show\" class=\"panel overview\" aria-labelledby=\"what-run-show-title\"><p class=\"eyebrow\">Answer first</p><h2 id=\"what-run-show-title\">What did the run show?</h2>");
    write_html!(
        out,
        "<p class=\"scope-line\">Station <strong>{}</strong> at <strong>{}</strong>; goal: <strong>{}</strong>.</p>",
        escape_html(&scope.station.callsign),
        escape_html(&scope.station.grid),
        scope.goal.map(session_goal).unwrap_or("Not recorded"),
    );
    out.push_str("<dl class=\"facts headline-facts\">");
    fact(out, "Antennas", &antennas);
    fact(out, "Bands", &bands);
    fact(out, "Direction / mode", &format!("{directions}; {mode}"));
    fact(out, "Session state", lifecycle_label);
    out.push_str("</dl>");

    match &scope.delta_orientation {
        Some(orientation) => write_html!(
            out,
            "<p class=\"orientation\"><strong>Delta orientation:</strong> {} minus {} (right minus left). Every signed value below uses this fixed orientation.</p>",
            escape_html(&orientation.minuend_label),
            escape_html(&orientation.subtrahend_label),
        ),
        None => out.push_str("<p class=\"orientation\"><strong>Delta orientation:</strong> unavailable because this run does not provide a two-label paired orientation.</p>"),
    }
    write_html!(
        out,
        "<p class=\"answer\"><strong>Comparison availability: <span class=\"badge\">{}</span>.</strong> {}</p>",
        comparison_availability_label(overview.comparison_availability),
        comparison_availability_text(overview.comparison_availability),
    );

    out.push_str("<div class=\"table-wrap\"><table class=\"overview-table\"><caption>Descriptive result by comparison stratum</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Path delta</th><th scope=\"col\">Paths / rows</th><th scope=\"col\">Blocks</th><th scope=\"col\">Coverage</th></tr></thead><tbody>");
    if overview.strata.is_empty() {
        out.push_str("<tr><td data-label=\"Stratum\" colspan=\"5\">No comparison strata are available for this run.</td></tr>");
    } else {
        for row in &overview.strata {
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
                "<tr><td data-label=\"Stratum\">{}</td><td data-label=\"Path delta\">{}</td><td data-label=\"Paths / rows\">{} / {}</td><td data-label=\"Blocks\">{}</td><td data-label=\"Coverage\">{}</td></tr>",
                comparison_stratum(&row.stratum),
                delta,
                row.unique_path_count,
                row.paired_row_count,
                row.contributing_block_count,
                coverage,
            );
        }
    }
    out.push_str("</tbody></table></div><div class=\"overview-support\"><section aria-labelledby=\"supported-title\"><h3 id=\"supported-title\">Supported by this run</h3><ul>");
    write_html!(
        out,
        "<li>The recorded comparison state is <strong>{}</strong>.</li>",
        comparison_availability_label(overview.comparison_availability),
    );
    if overview.strata.is_empty() {
        out.push_str("<li>The session scope and availability state remain explicit even without an available comparison stratum.</li>");
    } else {
        write_html!(
            out,
            "<li>{} paired row(s) are retained in {} unpooled stratum row(s).</li>",
            paired_rows,
            overview.strata.len(),
        );
    }
    out.push_str("</ul></section><section aria-labelledby=\"not-established-title\"><h3 id=\"not-established-title\">Not established by this run</h3><ul><li>This descriptive report does not select a winner or establish antenna superiority.</li><li>Adjacent switched slots reduce elapsed time but do not remove propagation or time confounding.</li>");
    for limitation in &overview.limitations {
        write_html!(out, "<li>{}</li>", overview_limitation_text(*limitation));
    }
    out.push_str("</ul></section></div>");
    render_visible_acquisition_limitations(out, report);
    out.push_str("</section>");
}

fn overview_lifecycle_label(state: ReportOverviewLifecycleState) -> &'static str {
    match state {
        ReportOverviewLifecycleState::NotRecorded => "Not recorded",
        ReportOverviewLifecycleState::Recorded(value) => lifecycle(value),
    }
}

fn overview_limitation_text(value: ReportOverviewLimitation) -> String {
    match value {
        ReportOverviewLimitation::ComparisonNotApplicable => {
            "A/B comparison: not established for single-antenna profiling.".into()
        }
        ReportOverviewLimitation::UnsupportedComparisonShape => {
            "A/B comparison: unavailable without the required two-label shape.".into()
        }
        ReportOverviewLimitation::NoEligibleBlocks => {
            "Eligible blocks: none with one usable actual slot for each label.".into()
        }
        ReportOverviewLimitation::NoMatchedPaths => {
            "Matched paths: no same-stratum path had finite SNR under both labels.".into()
        }
        ReportOverviewLimitation::UnmatchedPaths {
            left_count,
            right_count,
        } => format!("Unmatched paths: {left_count} left / {right_count} right."),
        ReportOverviewLimitation::MissingSnr {
            left_count,
            right_count,
        } => format!("Missing SNR: {left_count} left / {right_count} right."),
        ReportOverviewLimitation::DuplicateEvidence {
            exact_count,
            conflicting_group_count,
        } => format!(
            "Duplicates: {exact_count} exact / {conflicting_group_count} conflicting group(s)."
        ),
    }
}

fn render_visible_acquisition_limitations(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let evidence = &report.snapshot.adapter_evidence;
    if !evidence.evidence_complete {
        let message = if evidence.gap_count == 1 {
            "1 recorded acquisition gap; inspect the audit appendix for its durable recorded context"
                .to_string()
        } else if evidence.gap_count > 1 {
            format!(
                "{} recorded acquisition gaps; inspect the audit appendix for their durable recorded context",
                evidence.gap_count
            )
        } else {
            "Recorded acquisition is incomplete; inspect the audit appendix for its durable recorded context"
                .to_string()
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

fn render_same_path_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section id=\"same-path-signal\" class=\"panel question-section\" aria-labelledby=\"same-path-title\"><h2 id=\"same-path-title\">Same-path signal</h2>");
    render_same_path_view(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review same-path signal detail</summary><div class=\"disclosure-body\">");
    render_comparison_diagnostics(out, report);
    render_paired_differences(out, report);
    render_paired_snr_time(out, report);
    render_stratum_summaries(out, report);
    out.push_str("</div></details></section>");
}

fn render_reach_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section id=\"reach-unique-paths\" class=\"panel question-section\" aria-labelledby=\"reach-title\"><h2 id=\"reach-title\">Reach and unique paths</h2>");
    render_reach_view(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review path overlap and missingness</summary><div class=\"disclosure-body\">");
    render_overlap(out, report);
    out.push_str("</div></details></section>");
}

fn render_same_path_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let orientation = report.overview.scope.delta_orientation.as_ref();
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison stratum has same-path evidence available. This is not a zero-delta result.</p>");
        return;
    }
    if let Some(orientation) = orientation {
        write_html!(out, "<p class=\"orientation\"><strong>Orientation:</strong> each value is <strong>{} − {}</strong> SNR in dB. Negative values are toward {}; positive values are toward {}. The vertical reference is zero.</p>", escape_html(&orientation.minuend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.subtrahend_label), escape_html(&orientation.minuend_label));
    }
    out.push_str("<p class=\"path-view-note\">Each blue dot is one unique remote path’s median across its paired rows; the purple diamond is the median across those path medians. A finite 0 dB dot is retained as a true zero, not missing evidence.</p>");
    for row in &report.overview.strata {
        render_same_path_stratum(out, row, orientation);
    }
}

fn render_same_path_stratum(
    out: &mut CheckedHtmlWriter<'_>,
    row: &ReportOverviewStratum,
    orientation: Option<&antennabench_analysis::DeltaOrientation>,
) {
    write_html!(out, "<h3>{}</h3><p class=\"muted\">{} paired path{} · {} paired row{} · {} contributing block{}</p>", comparison_stratum(&row.stratum), row.path_median_deltas.len(), plural_suffix(row.path_median_deltas.len()), row.paired_row_count, plural_suffix(row.paired_row_count), row.contributing_block_count, plural_suffix(row.contributing_block_count));
    if row.path_median_deltas.is_empty() {
        if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
            write_html!(out, "<p class=\"empty\">No finite same-path delta is available; missing SNR is retained separately (left: {}, right: {}). This is not a 0 dB result.</p>", row.missing_snr_left_count, row.missing_snr_right_count);
        } else {
            out.push_str("<p class=\"empty\">No finite same-path paired evidence is available for this stratum. This is not a 0 dB result.</p>");
        }
        return;
    }
    let median = match row.path_delta {
        ReportOverviewPathDelta::Available {
            median_path_delta_right_minus_left_db,
            ..
        } => median_path_delta_right_minus_left_db,
        ReportOverviewPathDelta::Unavailable => return,
    };
    let max_abs = row
        .path_median_deltas
        .iter()
        .map(|path| path.median_delta_right_minus_left_db.abs())
        .chain(std::iter::once(median.abs()))
        .fold(1.0_f64, f64::max);
    out.push_str("<div class=\"path-strip\" aria-hidden=\"true\">");
    for path in &row.path_median_deltas {
        let position = delta_position(path.median_delta_right_minus_left_db, max_abs);
        write_html!(out, "<div class=\"path-strip-row\"><span class=\"chart-label\">{}</span><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-dot\" style=\"left:{position:.3}%\"></span></span><span>{} dB</span></div>", escape_html(&path.remote_path), format_signed(path.median_delta_right_minus_left_db));
    }
    let median_position = delta_position(median, max_abs);
    write_html!(out, "<div class=\"path-strip-row\"><strong>Stratum median</strong><span class=\"path-strip-track\"><span class=\"path-strip-zero\"></span><span class=\"path-strip-median\" style=\"left:{median_position:.3}%\"></span></span><strong>{} dB</strong></div></div>", format_signed(median));
    let orientation_text = orientation
        .map(|orientation| {
            format!(
                "{} − {}",
                orientation.minuend_label, orientation.subtrahend_label
            )
        })
        .unwrap_or_else(|| "right − left".to_string());
    write_html!(out, "<div class=\"table-wrap\"><table><caption>One path-median {} SNR delta per remote path for {}; the stratum median is {} dB.</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Paired rows</th><th scope=\"col\">Median delta</th></tr></thead><tbody>", escape_html(&orientation_text), comparison_stratum(&row.stratum), format_signed(median));
    for path in &row.path_median_deltas {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{} dB</td></tr>",
            escape_html(&path.remote_path),
            path.paired_row_count,
            format_signed(path.median_delta_right_minus_left_db)
        );
    }
    out.push_str("</tbody></table></div>");
}

fn render_reach_view(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<p class=\"muted\">Counts are unique finite remote paths within each stratum. “Both” records finite observations for the path on both antennas and supplies the path universe for same-path analysis; left-only and right-only paths are operationally interesting, but are <strong>not</strong> zero-SNR measurements.</p>");
    if report.overview.strata.is_empty() {
        out.push_str(
            "<p class=\"empty\">No comparison stratum has path-reach evidence available.</p>",
        );
        return;
    }
    for row in &report.overview.strata {
        let reach = &row.reach;
        write_html!(out, "<h3>{}</h3>", comparison_stratum(&row.stratum));
        if reach.left_only_unique_path_count
            + reach.both_unique_path_count
            + reach.right_only_unique_path_count
            == 0
        {
            if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
                write_html!(out, "<p class=\"empty\">No finite path reach counts; missing SNR is retained separately (left: {}, right: {}).</p>", row.missing_snr_left_count, row.missing_snr_right_count);
            } else {
                out.push_str("<p class=\"empty\">No finite path-reach evidence is available for this stratum.</p>");
            }
            continue;
        }
        write_html!(out, "<div class=\"reach-strip\" aria-hidden=\"true\"><span><strong>{}</strong><small>left only</small></span><span><strong>{}</strong><small>both</small></span><span><strong>{}</strong><small>right only</small></span></div><div class=\"table-wrap\"><table><caption>Unique finite remote-path reach counts for {}. Unmatched paths are not zero-SNR measurements.</caption><thead><tr><th scope=\"col\">Left only</th><th scope=\"col\">Both</th><th scope=\"col\">Right only</th><th scope=\"col\">Missing SNR left</th><th scope=\"col\">Missing SNR right</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody><tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr></tbody></table></div>", reach.left_only_unique_path_count, reach.both_unique_path_count, reach.right_only_unique_path_count, comparison_stratum(&row.stratum), reach.left_only_unique_path_count, reach.both_unique_path_count, reach.right_only_unique_path_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
}

fn delta_position(value: f64, maximum_absolute: f64) -> f64 {
    (50.0 + value / maximum_absolute * 50.0).clamp(0.0, 100.0)
}

fn plural_suffix(value: usize) -> &'static str {
    if value == 1 {
        ""
    } else {
        "s"
    }
}

fn render_distance_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section id=\"distance-direction\" class=\"panel question-section\" aria-labelledby=\"distance-direction-title\"><h2 id=\"distance-direction-title\">Distance and azimuth</h2><p class=\"notice\">These views describe only paired paths observed in this session. They are not a radiation pattern, propagation model, or causal conclusion about antenna performance in observed or unobserved directions and distances.</p>");
    render_observed_path_context(out, report);
    out.push_str("<details class=\"audit-disclosure\"><summary>Review exact paired-row distance and azimuth detail</summary><div class=\"disclosure-body\">");
    render_location_views(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review derived solar context</summary><div class=\"disclosure-body\">");
    render_solar_context(out, report);
    out.push_str("</div></details></section>");
}

fn render_observed_path_context(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.overview.strata.is_empty() {
        out.push_str("<p class=\"empty\">No observed paired paths are available for distance or azimuth context. This is not a near-zero path delta.</p>");
        return;
    }
    out.push_str("<p class=\"muted\">Each located paired path contributes once to one fixed distance bin and one fixed 45° compass sector. The supporting paired-row count stays visible; repeated rows from one endpoint do not increase a cell’s path count.</p>");
    for (index, stratum) in report.overview.strata.iter().enumerate() {
        let context = &stratum.location_context;
        write_html!(
            out,
            "<section aria-labelledby=\"path-context-{index}\"><h3 id=\"path-context-{index}\">{}</h3>",
            comparison_stratum(&stratum.stratum)
        );
        write_html!(out, "<p class=\"muted\">{} located paired path{}; {} location unavailable ({} missing, {} inconsistent). Exact left/right values remain in the paired-row audit table.</p>", located_path_count(context), plural_suffix(located_path_count(context)), context.missing_location_path_count + context.inconsistent_location_path_count, context.missing_location_path_count, context.inconsistent_location_path_count);
        render_location_context_cells(
            out,
            "Observed distance",
            "Fixed distance bins for observed paired paths",
            &context.distance_bins,
            distance_bin_label,
        );
        render_location_context_cells(
            out,
            "Observed azimuth",
            "Fixed 45° azimuth sectors for observed paired paths",
            &context.azimuth_sectors,
            fixed_azimuth_sector_label,
        );
        render_location_path_audit(out, &context.paths);
        out.push_str("</section>");
    }
}

fn render_location_context_cells<T: Copy>(
    out: &mut CheckedHtmlWriter<'_>,
    heading: &str,
    caption: &str,
    cells: &[ReportOverviewLocationCell<T>],
    label: impl Fn(T) -> &'static str,
) {
    write_html!(
        out,
        "<h4>{}</h4><div class=\"location-context\" aria-hidden=\"true\">",
        heading
    );
    for cell in cells {
        let class = if cell.unique_located_path_count == 0 {
            " empty-cell"
        } else {
            ""
        };
        write_html!(out, "<div class=\"location-context-cell{}\"><strong>{}</strong><span>{}</span><small>{}</small></div>", class, label(cell.category), location_cell_delta(cell), location_cell_evidence(cell));
    }
    out.push_str("</div><div class=\"table-wrap\"><table class=\"location-context-table\">");
    write_html!(out, "<caption>{}</caption><thead><tr><th scope=\"col\">Bin or sector</th><th scope=\"col\">Unique located paths</th><th scope=\"col\">Supporting paired rows</th><th scope=\"col\">Median path delta</th><th scope=\"col\">Evidence state</th></tr></thead><tbody>", caption);
    for cell in cells {
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            label(cell.category),
            cell.unique_located_path_count,
            cell.paired_row_count,
            location_cell_delta(cell),
            location_cell_evidence(cell)
        );
    }
    out.push_str("</tbody></table></div>");
}

fn located_path_count(context: &crate::ReportOverviewLocationContext) -> usize {
    context
        .paths
        .iter()
        .filter(|path| path.availability == ReportPathLocationAvailability::Available)
        .count()
}

fn location_cell_delta<T>(cell: &ReportOverviewLocationCell<T>) -> String {
    match cell.median_path_delta_right_minus_left_db {
        Some(delta) if delta.abs() < 0.5 => format!("{} dB (near-zero)", format_signed(delta)),
        Some(delta) => format!("{} dB", format_signed(delta)),
        None => "No observed paired paths".into(),
    }
}

fn location_cell_evidence<T>(cell: &ReportOverviewLocationCell<T>) -> String {
    match cell.unique_located_path_count {
        0 => "No observed paired paths".into(),
        1 | 2 => format!(
            "Sparse evidence: {} path(s), {} row(s)",
            cell.unique_located_path_count, cell.paired_row_count
        ),
        _ => format!(
            "{} path(s), {} row(s)",
            cell.unique_located_path_count, cell.paired_row_count
        ),
    }
}

fn render_location_path_audit(
    out: &mut CheckedHtmlWriter<'_>,
    paths: &[crate::ReportOverviewLocationPath],
) {
    out.push_str("<details class=\"audit-disclosure\"><summary>Review paired-path location aggregate audit</summary><div class=\"disclosure-body\"><div class=\"table-wrap\"><table><caption>One location-status record per paired path; raw left/right values remain below in the paired-row audit.</caption><thead><tr><th scope=\"col\">Remote path</th><th scope=\"col\">Paired rows</th><th scope=\"col\">Median path delta</th><th scope=\"col\">Location status</th><th scope=\"col\">Distance</th><th scope=\"col\">Azimuth</th></tr></thead><tbody>");
    for path in paths {
        let status = match path.availability {
            ReportPathLocationAvailability::Available => "Available",
            ReportPathLocationAvailability::Missing => "Missing",
            ReportPathLocationAvailability::Inconsistent => "Inconsistent",
        };
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&path.remote_path),
            path.paired_row_count,
            format_signed(path.median_delta_right_minus_left_db),
            status,
            optional_measure_f64(path.distance_km, "km"),
            optional_measure_f64(path.azimuth_degrees, "°")
        );
    }
    out.push_str("</tbody></table></div></div></details>");
}

fn distance_bin_label(bin: ReportDistanceBin) -> &'static str {
    match bin {
        ReportDistanceBin::Under500Km => "Under 500 km",
        ReportDistanceBin::Km500To1499 => "500–1499 km",
        ReportDistanceBin::Km1500To2999 => "1500–2999 km",
        ReportDistanceBin::Km3000AndAbove => "3000 km and above",
    }
}

fn fixed_azimuth_sector_label(sector: ReportAzimuthSector) -> &'static str {
    match sector {
        ReportAzimuthSector::North => "N (337.5°–22.5°)",
        ReportAzimuthSector::NorthEast => "NE (22.5°–67.5°)",
        ReportAzimuthSector::East => "E (67.5°–112.5°)",
        ReportAzimuthSector::SouthEast => "SE (112.5°–157.5°)",
        ReportAzimuthSector::South => "S (157.5°–202.5°)",
        ReportAzimuthSector::SouthWest => "SW (202.5°–247.5°)",
        ReportAzimuthSector::West => "W (247.5°–292.5°)",
        ReportAzimuthSector::NorthWest => "NW (292.5°–337.5°)",
    }
}

fn render_run_quality_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section id=\"run-quality\" class=\"question-section\" aria-labelledby=\"run-quality-title\"><div class=\"panel\"><h2 id=\"run-quality-title\">Run quality and answerability</h2><p class=\"muted\">Availability is reported from typed evidence states and raw counts. It is not a run-quality score or a strength grade.</p>");
    render_answerability(out, report);
    out.push_str("</div><div class=\"panel\"><h2>Planned versus actual</h2><p class=\"muted\">Symbols, words, and border styles distinguish state without relying on color. Open a row for exact times, readiness, attribution, counts, notes, and corrections.</p>");
    render_lifecycle_strip(out, report);
    render_run_timeline(out, report);
    out.push_str("</div>");
    render_acquisition_summary(out, report);
    render_exclusion_summary(out, report);
    render_notices(out, &report.notices);
    render_eligibility(out, report);
    out.push_str("<div class=\"panel\"><details class=\"audit-disclosure\"><summary>Review overall, antenna, and band evidence accounting</summary><div class=\"disclosure-body\">");
    render_overall(out, report);
    render_antenna_section(out, report);
    render_band_section(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review per-slot evidence accounting</summary><div class=\"disclosure-body\">");
    render_slot_section(out, report);
    out.push_str("</div></details></div></section>");
}

fn render_answerability(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.overview.strata.is_empty() {
        write_html!(
            out,
            "<p class=\"empty\"><strong>{}.</strong> {}</p>",
            comparison_availability_label(report.overview.comparison_availability),
            comparison_availability_text(report.overview.comparison_availability)
        );
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table class=\"answerability-table\"><caption>Answerability by separate comparison stratum</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Availability</th><th scope=\"col\">Unique paired paths</th><th scope=\"col\">Paired rows</th><th scope=\"col\">Blocks</th><th scope=\"col\">A→B / B→A</th><th scope=\"col\">Unmatched L / R</th><th scope=\"col\">Missing SNR L / R</th><th scope=\"col\">Excluded</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>");
    for row in &report.overview.strata {
        let availability = match row.availability {
            ReportStratumAvailability::DescriptivePairsAvailable => {
                "Answerable — descriptive pairs available"
            }
            ReportStratumAvailability::NoFinitePairedPaths => {
                "Unavailable — no finite same-path pair"
            }
        };
        write_html!(out, "<tr><td data-label=\"Stratum\">{}</td><td data-label=\"Availability\">{}</td><td data-label=\"Unique paired paths\">{}</td><td data-label=\"Paired rows\">{}</td><td data-label=\"Blocks\">{}</td><td data-label=\"A→B / B→A\">{} / {}</td><td data-label=\"Unmatched L / R\">{} / {}</td><td data-label=\"Missing SNR L / R\">{} / {}</td><td data-label=\"Excluded\">{}</td><td data-label=\"Duplicates\">{}</td><td data-label=\"Conflicts\">{}</td></tr>", comparison_stratum(&row.stratum), availability, row.unique_path_count, row.paired_row_count, row.contributing_block_count, row.left_then_right_block_count, row.right_then_left_block_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.excluded_observation_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
    out.push_str("</tbody></table></div><p class=\"muted\">Unmatched paths, missing values, exclusions, duplicates, and conflicts remain separate facts. They are not converted into zero-SNR samples.</p>");
}

fn render_lifecycle_strip(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.snapshot.lifecycle_events.is_empty() {
        return;
    }
    out.push_str("<div class=\"lifecycle-strip\" aria-label=\"Lifecycle sequence\">");
    for event in &report.snapshot.lifecycle_events {
        let symbol = match event.kind {
            ReportLifecycleEventKind::Interrupted
            | ReportLifecycleEventKind::InterruptionDetected => "!",
            ReportLifecycleEventKind::Resumed => "↻",
            ReportLifecycleEventKind::Abandoned => "×",
            ReportLifecycleEventKind::Ended => "■",
            ReportLifecycleEventKind::Started => "▶",
        };
        write_html!(
            out,
            "<span class=\"lifecycle-chip\"><strong aria-hidden=\"true\">{}</strong>{} · {}</span>",
            symbol,
            lifecycle_event(event.kind),
            timestamp(event.occurred_at)
        );
    }
    out.push_str("</div>");
}

fn render_run_timeline(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.overview.timeline.is_empty() {
        out.push_str("<p class=\"empty\">No valid planned cycle or slot rows are available for the compact timeline.</p>");
        return;
    }
    out.push_str("<div class=\"run-timeline\">");
    for row in &report.overview.timeline {
        let (class, state, symbol) = timeline_compact_state(row, report);
        write_html!(out, "<details class=\"state-{}\"><summary><span class=\"timeline-state\" aria-hidden=\"true\">{}</span>#{} {} → {}<br><small>{} · {} usable / {} excluded</small></summary><div class=\"timeline-detail\"><dl class=\"facts\">", class, symbol, row.sequence_number, escape_html(&row.planned_antenna), escape_html(row.actual_antenna.as_deref().unwrap_or("Not recorded")), state, row.usable_observation_count, row.excluded_observation_count);
        fact(out, "Item", &row.item_id);
        fact(out, "State", state);
        fact(
            out,
            "Band / direction",
            &format!(
                "{} / {}",
                band(row.band),
                row.direction.map(wspr_direction).unwrap_or("Not recorded")
            ),
        );
        fact(
            out,
            "Block",
            &row.block_index.map_or_else(
                || "Not applicable".into(),
                |index| {
                    format!(
                        "{}; {}",
                        index + 1,
                        row.block_eligibility
                            .map(block_eligibility)
                            .unwrap_or("Not recorded")
                    )
                },
            ),
        );
        fact(
            out,
            "Planned window",
            &format!(
                "{} – {}",
                timestamp(row.planned_starts_at),
                timestamp(row.planned_ends_at)
            ),
        );
        fact(
            out,
            "Actual window",
            &match (row.actual_starts_at, row.actual_ends_at) {
                (Some(start), Some(end)) => format!("{} – {}", timestamp(start), timestamp(end)),
                (Some(start), None) => format!("{} – end not recorded", timestamp(start)),
                _ => "Not recorded".into(),
            },
        );
        fact(
            out,
            "Readiness",
            row.readiness_basis
                .map(wspr_readiness)
                .unwrap_or("Not recorded"),
        );
        fact(
            out,
            "Attribution",
            row.attribution
                .map(wspr_attribution)
                .unwrap_or("Not recorded"),
        );
        fact(
            out,
            "Evidence",
            &format!(
                "{} total; {} usable; {} excluded",
                row.total_observation_count,
                row.usable_observation_count,
                row.excluded_observation_count
            ),
        );
        out.push_str("</dl>");
        render_timeline_events(out, &row.event_history);
        out.push_str("</div></details>");
    }
    out.push_str("</div>");
}

fn timeline_compact_state(
    row: &ReportRunTimelineRow,
    report: &SessionReport,
) -> (&'static str, &'static str, &'static str) {
    if row.event_history.iter().any(|event| {
        event.kind == ReportOperatorEventKind::EventCorrected
            && event
                .correction
                .as_ref()
                .is_some_and(|correction| correction.applied)
    }) {
        return ("corrected", "Corrected", "✓");
    }
    if report.snapshot.lifecycle_events.iter().any(|event| {
        matches!(
            event.kind,
            ReportLifecycleEventKind::Interrupted | ReportLifecycleEventKind::InterruptionDetected
        ) && event.occurred_at >= row.planned_starts_at
            && event.occurred_at < row.planned_ends_at
    }) {
        return ("interrupted", "Interrupted", "!");
    }
    match row.status {
        AlignedSlotStatus::Missed => ("missed", "Missed", "—"),
        AlignedSlotStatus::Bad | AlignedSlotStatus::ConflictingEvidence => {
            ("bad", slot_status(row.status), "×")
        }
        AlignedSlotStatus::UnknownActualState => ("unknown", "Unknown occupancy", "?"),
        AlignedSlotStatus::LateSwitch => ("late", "Late switch", "△"),
        _ if row.attribution == Some(crate::ReportWsprAttribution::UnknownAntennaOccupancy) => {
            ("unknown", "Unknown occupancy", "?")
        }
        _ if row.attribution == Some(crate::ReportWsprAttribution::Skipped) => {
            ("missed", "Skipped", "—")
        }
        _ => ("ordinary", slot_status(row.status), "●"),
    }
}

fn render_timeline_events(out: &mut CheckedHtmlWriter<'_>, events: &[ReportOperatorEvent]) {
    if events.is_empty() {
        out.push_str(
            "<p class=\"muted\">No slot-specific operator note or correction is recorded.</p>",
        );
        return;
    }
    out.push_str("<ul class=\"audit-event-list\">");
    for event in events {
        write_html!(
            out,
            "<li><code>{}</code> · {} · {}",
            escape_html(&event.event_id),
            timestamp(event.occurred_at),
            operator_event_kind(event.kind)
        );
        if let Some(detail) = &event.detail {
            write_html!(out, " · {}", escape_html(detail));
        }
        if let Some(correction) = &event.correction {
            write_html!(
                out,
                " · {} <code>{}</code>: {} ({})",
                correction_action(correction.action),
                escape_html(&correction.target_event_id),
                escape_html(&correction.reason),
                if correction.applied {
                    "applied"
                } else {
                    "not applied"
                }
            );
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
}

fn render_acquisition_summary(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let evidence = &report.snapshot.adapter_evidence;
    out.push_str("<section class=\"panel\" aria-labelledby=\"acquisition-quality-title\"><h2 id=\"acquisition-quality-title\">Acquisition status</h2>");
    if evidence
        .imports
        .iter()
        .any(|import| import.provider_id == "wspr-live")
        && evidence.evidence_complete
    {
        out.push_str("<p><strong>Best-effort public collection completed for the requested windows.</strong> AntennaBench retained the spots returned by the configured WSPR.live queries. The upstream mirror does not provide an independent completeness guarantee.</p>");
    } else if evidence.gap_count > 0 {
        write_html!(out, "<p class=\"notice critical\"><strong>Explicit acquisition gap:</strong> {} recorded gap{}; retained unrelated valid evidence remains usable. Inspect provider windows and durable adapter records in the audit appendix.</p>", evidence.gap_count, plural_suffix(evidence.gap_count));
    } else if !evidence.evidence_complete {
        out.push_str("<p class=\"notice critical\"><strong>Recorded acquisition is incomplete.</strong> No durable gap count is available; inspect provider windows and lifecycle records for the recorded reason.</p>");
    } else if evidence.record_count > 0 {
        out.push_str("<p>No explicit acquisition gap is recorded. Adapter row dispositions remain available in the audit appendix.</p>");
    } else {
        out.push_str("<p class=\"empty\">No adapter or import acquisition record is available; no completeness claim is inferred.</p>");
    }
    out.push_str("</section>");
}

fn render_exclusion_summary(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"exclusion-summary-title\"><h2 id=\"exclusion-summary-title\">Exclusion summary</h2><p class=\"notice\">Affected evidence is excluded only from calculations that require it. Unrelated valid evidence remains usable.</p>");
    if report.evidence.overall.exclusions.is_empty() {
        out.push_str("<p class=\"empty\">No observation exclusions are recorded.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Observation exclusions by existing reason</caption><thead><tr><th scope=\"col\">Reason</th><th scope=\"col\">Count</th></tr></thead><tbody>");
        for exclusion in &report.evidence.overall.exclusions {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td></tr>",
                exclusion_reason(exclusion.reason),
                exclusion.count
            );
        }
        out.push_str("</tbody></table></div>");
    }
    if !report.exclusion_records.is_empty() {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review exact excluded observation records</summary><div class=\"disclosure-body\">");
        render_exclusion_records(out, report);
        out.push_str("</div></details>");
    }
    out.push_str("</section>");
}

fn render_audit_appendix(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section id=\"audit-appendix\" class=\"panel question-section\" aria-labelledby=\"audit-title\"><h2 id=\"audit-title\">Audit appendix</h2><p class=\"muted\">Open only the supporting detail needed for review. Closed disclosures remain closed in default print output.</p>");
    if snapshot_has_detail(report) {
        out.push_str("<details class=\"audit-disclosure\"><summary>Review committed snapshot, lifecycle, acquisition, and controller attempts</summary><div class=\"disclosure-body\">");
        render_snapshot(out, report);
        out.push_str("</div></details>");
    }
    out.push_str("<details class=\"audit-disclosure\"><summary>Review station, antenna, and planned schedule detail</summary><div class=\"disclosure-body\">");
    render_context(out, report);
    out.push_str("</div></details><details class=\"audit-disclosure\"><summary>Review comparison blocks and data-quality timeline</summary><div class=\"disclosure-body\">");
    render_comparison_blocks(out, report);
    render_comparison_timeline(out, report);
    out.push_str("</div></details></section>");
}

fn render_snapshot(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let snapshot = &report.snapshot;
    if !snapshot_has_detail(report) {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"snapshot-title\"><h2 id=\"snapshot-title\">Committed session snapshot</h2><dl class=\"facts\">");
    fact(
        out,
        "Checkpoint revision",
        &snapshot.checkpoint_revision.map_or_else(
            || "Legacy static bundle".into(),
            |revision| revision.to_string(),
        ),
    );
    fact(
        out,
        "Lifecycle",
        snapshot.lifecycle.map_or("Not recorded", lifecycle),
    );
    fact(
        out,
        "Report detail",
        match report.completeness {
            crate::ReportCompleteness::FullDetail => "Full detail",
            crate::ReportCompleteness::BoundedOverview => "Bounded overview",
        },
    );
    fact(
        out,
        "Adapter evidence",
        &format!(
            "{} records; {} accepted; {} malformed; {} unsupported; {} filtered; {} duplicate; {} conflict; {} partial",
            snapshot.adapter_evidence.record_count,
            snapshot.adapter_evidence.accepted_count,
            snapshot.adapter_evidence.malformed_count,
            snapshot.adapter_evidence.unsupported_count,
            snapshot.adapter_evidence.filtered_count,
            snapshot.adapter_evidence.duplicate_count,
            snapshot.adapter_evidence.conflict_count,
            snapshot.adapter_evidence.partially_normalized_count,
        ),
    );
    fact(
        out,
        "Recorded acquisition",
        &if snapshot.adapter_evidence.evidence_complete {
            "No recorded acquisition gaps".into()
        } else if snapshot.adapter_evidence.gap_count == 1 {
            "1 recorded acquisition gap; inspect the durable adapter evidence and lifecycle history for its recorded reason".into()
        } else if snapshot.adapter_evidence.gap_count > 1 {
            format!(
                "{} recorded acquisition gaps; inspect the durable adapter evidence and lifecycle history for their recorded reasons",
                snapshot.adapter_evidence.gap_count,
            )
        } else {
            "Recorded acquisition is incomplete; inspect the durable adapter evidence and lifecycle history for the recorded reason".into()
        },
    );
    let wspr_live_imports = snapshot
        .adapter_evidence
        .imports
        .iter()
        .filter(|import| import.provider_id == "wspr-live")
        .count();
    if wspr_live_imports > 0 {
        fact(
            out,
            "Public collection",
            &if snapshot.adapter_evidence.evidence_complete {
                format!(
                    "Best-effort public collection completed for {} recorded requested window(s)",
                    wspr_live_imports,
                )
            } else {
                format!(
                    "Best-effort public collection retained rows for {} recorded requested window(s); recorded acquisition gaps remain",
                    wspr_live_imports,
                )
            },
        );
        fact(
            out,
            "Public-source boundary",
            "AntennaBench retained the spots returned by the configured WSPR.live queries; the upstream mirror does not provide an independent completeness guarantee.",
        );
    }
    for (index, import) in snapshot.adapter_evidence.imports.iter().enumerate() {
        let bands = import
            .selected_bands
            .iter()
            .map(|value| band(*value))
            .collect::<Vec<_>>()
            .join(", ");
        fact(
            out,
            &format!("Imported evidence {}", index + 1),
            &format!(
                "{} / {}; captured {}; half-open window {} to {}; bands {}; {} rows: {} accepted, {} malformed, {} unsupported, {} filtered, {} duplicate, {} conflict; {} observations created; {}",
                import.provider_id,
                import.source_id,
                timestamp(import.captured_at),
                timestamp(import.window_start),
                timestamp(import.window_end),
                bands,
                import.total_count,
                import.accepted_count,
                import.malformed_count,
                import.unsupported_count,
                import.filtered_count,
                import.duplicate_count,
                import.conflict_count,
                import.observations_created,
                import_source_boundary(import),
            ),
        );
    }
    out.push_str("</dl>");
    if !snapshot.lifecycle_events.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Lifecycle and interruption history</caption><thead><tr><th scope=\"col\">Event</th><th scope=\"col\">Time</th><th scope=\"col\">Detail</th></tr></thead><tbody>");
        for event in &snapshot.lifecycle_events {
            write_html!(
                out,
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                lifecycle_event(event.kind),
                timestamp(event.occurred_at),
                escape_html(event.detail.as_deref().unwrap_or("Not recorded")),
            );
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.operator_events.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Complete operator note and correction history</caption><thead><tr><th scope=\"col\">Event</th><th scope=\"col\">Time</th><th scope=\"col\">Recorded slot</th><th scope=\"col\">Affected slot</th><th scope=\"col\">Kind</th><th scope=\"col\">Detail</th><th scope=\"col\">Correction</th></tr></thead><tbody>");
        for event in &snapshot.operator_events {
            let correction = event.correction.as_ref().map_or_else(
                || "None".to_string(),
                |correction| {
                    format!(
                        "{} {}: {} ({})",
                        correction_action(correction.action),
                        correction.target_event_id,
                        correction.reason,
                        if correction.applied {
                            "applied"
                        } else {
                            "not applied"
                        }
                    )
                },
            );
            write_html!(out, "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(&event.event_id), timestamp(event.occurred_at), escape_html(event.slot_id.as_deref().unwrap_or("Not recorded")), escape_html(event.affected_slot_id.as_deref().unwrap_or("Not recorded")), operator_event_kind(event.kind), escape_html(event.detail.as_deref().unwrap_or("Not recorded")), escape_html(&correction));
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.wspr_cycles.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Intended WSPR order and observed antenna use</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Band</th><th scope=\"col\">Direction</th><th scope=\"col\">Intended antenna</th><th scope=\"col\">Observed antenna</th><th scope=\"col\">Readiness basis</th><th scope=\"col\">Ready</th><th scope=\"col\">Period start</th><th scope=\"col\">Period end</th><th scope=\"col\">Attribution</th></tr></thead><tbody>");
        for cycle in &snapshot.wspr_cycles {
            let direction = match cycle.direction {
                Some(WsprCycleDirection::Receive) => "Receive",
                Some(WsprCycleDirection::Transmit) => "Transmit",
                None => "Not recorded",
            };
            let attribution = match cycle.attribution {
                crate::ReportWsprAttribution::Pending => "Not yet run",
                crate::ReportWsprAttribution::Skipped => "Skipped by operator",
                crate::ReportWsprAttribution::Attributable => "Full antenna occupancy recorded",
                crate::ReportWsprAttribution::UnknownAntennaOccupancy => {
                    "Unknown — antenna changed during transmission"
                }
            };
            let readiness = match cycle.readiness_basis {
                Some(crate::ReportWsprReadinessBasis::OperatorConfirmed) => "Operator confirmed",
                Some(crate::ReportWsprReadinessBasis::CommandVerified) => "Command verified",
                None => "Not recorded",
            };
            write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", cycle.sequence_number, band(cycle.band), direction, escape_html(&cycle.planned_antenna), escape_html(cycle.actual_antenna.as_deref().unwrap_or("Not recorded")), readiness, cycle.ready_at.map_or_else(|| "—".into(), timestamp), cycle.starts_at.map_or_else(|| "—".into(), timestamp), cycle.transmission_ends_at.map_or_else(|| "—".into(), timestamp), attribution);
        }
        out.push_str("</tbody></table></div>");
    }
    if !snapshot.antenna_control_attempts.is_empty() {
        out.push_str("<div class=\"table-wrap\"><table><caption>Antenna-control command attempts</caption><thead><tr><th scope=\"col\">Record</th><th scope=\"col\">Role</th><th scope=\"col\">Intent / target / mode</th><th scope=\"col\">Controller</th><th scope=\"col\">Resolved invocation</th><th scope=\"col\">Outcome</th><th scope=\"col\">Stdout</th><th scope=\"col\">Stderr</th></tr></thead><tbody>");
        for attempt in &snapshot.antenna_control_attempts {
            let arguments = attempt
                .resolved_arguments
                .iter()
                .enumerate()
                .map(|(index, value)| format!("[{index}]={value:?}"))
                .collect::<Vec<_>>()
                .join(" ");
            let outcome = format!(
                "{:?}; {} ms",
                attempt.disposition, attempt.elapsed_milliseconds
            );
            let stdout = format!(
                "{:?}; truncated={}; {}",
                attempt.stdout.encoding, attempt.stdout.truncated, attempt.stdout.data
            );
            let stderr = format!(
                "{:?}; truncated={}; {}",
                attempt.stderr.encoding, attempt.stderr.truncated, attempt.stderr.data
            );
            write_html!(out, "<tr><td><code>{}</code></td><td>{:?}</td><td><code>{}</code><br>{} → {}<br>{}</td><td>{} / {}</td><td><code>{}</code><br>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", escape_html(&attempt.record_id), attempt.role, escape_html(&attempt.intent_id), escape_html(&attempt.antenna), escape_html(&attempt.target), experiment_mode(attempt.mode), escape_html(&attempt.controller_profile_name), escape_html(&attempt.controller_profile_revision), escape_html(&attempt.resolved_program), escape_html(&arguments), escape_html(&outcome), escape_html(&stdout), escape_html(&stderr));
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
}

fn snapshot_has_detail(report: &SessionReport) -> bool {
    let snapshot = &report.snapshot;
    snapshot.checkpoint_revision.is_some()
        || snapshot.lifecycle.is_some()
        || !snapshot.lifecycle_events.is_empty()
        || !snapshot.operator_events.is_empty()
        || !snapshot.wspr_cycles.is_empty()
        || !snapshot.antenna_control_attempts.is_empty()
        || snapshot.adapter_evidence.record_count > 0
        || snapshot.adapter_evidence.gap_count > 0
        || !snapshot.adapter_evidence.evidence_complete
        || !snapshot.adapter_evidence.imports.is_empty()
}

fn lifecycle(value: SessionLifecycleV2) -> &'static str {
    match value {
        SessionLifecycleV2::Draft => "Draft",
        SessionLifecycleV2::Ready => "Ready",
        SessionLifecycleV2::Running => "Running / in progress",
        SessionLifecycleV2::Interrupted => "Interrupted / in progress",
        SessionLifecycleV2::Ended => "Ended / final",
        SessionLifecycleV2::Abandoned => "Abandoned / final",
    }
}

fn lifecycle_event(value: ReportLifecycleEventKind) -> &'static str {
    match value {
        ReportLifecycleEventKind::Started => "Started",
        ReportLifecycleEventKind::Interrupted => "Interrupted",
        ReportLifecycleEventKind::InterruptionDetected => "Interruption detected",
        ReportLifecycleEventKind::Resumed => "Resumed",
        ReportLifecycleEventKind::Ended => "Ended",
        ReportLifecycleEventKind::Abandoned => "Abandoned",
    }
}

struct CheckedHtmlWriter<'a> {
    output: String,
    limit: u64,
    observed: u64,
    failure: Option<crate::ReportResourceError>,
    cancellation: &'a ReportCancellationToken,
    last_cancellation_check: u64,
}

impl<'a> CheckedHtmlWriter<'a> {
    fn new(limit: u64, cancellation: &'a ReportCancellationToken) -> Self {
        Self {
            output: String::with_capacity(32_768.min(limit as usize)),
            limit,
            observed: 0,
            failure: None,
            cancellation,
            last_cancellation_check: 0,
        }
    }

    fn push_str(&mut self, value: &str) {
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

    fn finish(self) -> Result<String, crate::ReportResourceError> {
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

fn render_eligibility(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.eligibility_exclusions.is_empty() {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"eligibility-title\"><h2 id=\"eligibility-title\">Evidence eligibility disclosures</h2><p class=\"notice\">Affected evidence is excluded only from calculations that require it. Unrelated valid evidence remains included.</p><div class=\"table-wrap\"><table><caption>Validation-driven exclusions</caption><thead><tr><th scope=\"col\">Reason code</th><th scope=\"col\">Kind</th><th scope=\"col\">Scope</th><th scope=\"col\">Count</th></tr></thead><tbody>");
    for exclusion in &report.eligibility_exclusions {
        write_html!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&exclusion.code),
            eligibility_category(exclusion.category),
            eligibility_scope(exclusion.scope),
            exclusion.count
        );
    }
    out.push_str("</tbody></table></div></section>");
}

fn render_exclusion_records(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    if report.exclusion_records.is_empty() {
        out.push_str("<p class=\"empty\">Record-level exclusion detail is unavailable or omitted by the bounded overview.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Every excluded observation retained by the report projection</caption><thead><tr><th scope=\"col\">Observation</th><th scope=\"col\">Reason</th><th scope=\"col\">Time</th><th scope=\"col\">Band</th><th scope=\"col\">Kind / source</th><th scope=\"col\">Mode</th><th scope=\"col\">Slot / label</th><th scope=\"col\">Confidence</th></tr></thead><tbody>");
    for record in &report.exclusion_records {
        write_html!(out, "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>", escape_html(&record.observation_id), exclusion_reason(record.reason), timestamp(record.timestamp), band(record.band), observation_kind(record.observation_kind), record_source(record.source), escape_html(record.mode.as_deref().unwrap_or("Not recorded")), escape_html(record.slot_id.as_deref().unwrap_or("Not assigned")), escape_html(record.assigned_label.as_deref().unwrap_or("Not assigned")), format_number(f64::from(record.assignment_confidence)));
    }
    out.push_str("</tbody></table></div>");
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

fn render_notices(out: &mut CheckedHtmlWriter<'_>, notices: &[ReportNotice]) {
    if notices.is_empty() {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"notices-title\"><h2 id=\"notices-title\">Data notices</h2>");
    for notice in notices {
        write_html!(out, "<p class=\"notice\">{}</p>", notice_text(notice));
    }
    out.push_str("</section>");
}

fn render_context(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let context = &report.context;
    out.push_str("<section class=\"panel\" aria-labelledby=\"context-title\"><h2 id=\"context-title\">Session context</h2><dl class=\"facts\">");
    fact(out, "Callsign", &context.station.callsign);
    fact(out, "Grid", &context.station.grid);
    fact(
        out,
        "Power",
        &context
            .station
            .power_watts
            .map(|value| format!("{} W", format_number(f64::from(value))))
            .unwrap_or_else(not_recorded),
    );
    fact(
        out,
        "Experiment mode",
        experiment_mode(context.experiment_mode),
    );
    fact(out, "Goal", session_goal(context.goal));
    fact(
        out,
        "Scheduled range",
        &context
            .scheduled_time_range
            .as_ref()
            .map(|range| {
                format!(
                    "{} – {}",
                    timestamp(range.starts_at),
                    timestamp(range.ends_at)
                )
            })
            .unwrap_or_else(|| "No scheduled time range".to_string()),
    );
    fact(
        out,
        "Scheduled bands",
        &if context.bands.is_empty() {
            "None".to_string()
        } else {
            context
                .bands
                .iter()
                .map(|value| band(*value))
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    fact(
        out,
        "Scheduled slots",
        &context.schedule.slot_count.to_string(),
    );
    out.push_str("</dl><h3>Antennas</h3>");
    if context.antennas.is_empty() {
        out.push_str("<p class=\"empty\">No antennas are present in this report.</p>");
    } else {
        out.push_str("<div class=\"antenna-grid\">");
        for antenna in &context.antennas {
            write_html!(
                out,
                "<article class=\"antenna-card\"><h3>{}</h3><dl>",
                escape_html(&antenna.label)
            );
            detail(out, "Facets", &optional_join(&antenna.facets));
            detail(out, "Height", &optional_measure(antenna.height_m, "m"));
            detail(
                out,
                "Radials",
                &antenna
                    .radial_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(not_recorded),
            );
            detail(
                out,
                "Radial length",
                &optional_measure(antenna.radial_length_m, "m"),
            );
            detail(
                out,
                "Orientation",
                &optional_measure(antenna.orientation_degrees, "°"),
            );
            detail(
                out,
                "Tuner",
                antenna.tuner.as_deref().unwrap_or("Not recorded"),
            );
            detail(
                out,
                "Feedline",
                antenna.feedline.as_deref().unwrap_or("Not recorded"),
            );
            detail(
                out,
                "Notes",
                antenna.notes.as_deref().unwrap_or("Not recorded"),
            );
            out.push_str("</dl></article>");
        }
        out.push_str("</div>");
    }
    render_schedule_table(out, report);
    out.push_str("</section>");
}

fn render_schedule_table(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Schedule overview</h3>");
    if report.context.schedule.slots.is_empty() {
        out.push_str("<p class=\"empty\">No scheduled slots are available.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Planned slots</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Band</th><th scope=\"col\">Antenna</th><th scope=\"col\">Starts</th><th scope=\"col\">Ends</th><th scope=\"col\">Guard</th></tr></thead><tbody>");
    for slot in &report.context.schedule.slots {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} s</td></tr>", slot.sequence_number, escape_html(&slot.slot_id), band(slot.band), escape_html(&slot.planned_label), timestamp(slot.starts_at), timestamp(slot.ends_at), slot.guard_seconds);
    }
    out.push_str("</tbody></table></div>");
}

fn render_overall(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"evidence-title\"><h2 id=\"evidence-title\">Evidence overview</h2>");
    write_html!(
        out,
        "<p>Evidence coverage: <span class=\"badge\">{}</span></p>\
<p class=\"muted\">Coverage reflects usable observations and contributing slots; it is not evidence that one antenna is better.</p>",
        evidence_coverage(report.evidence.evidence_quality)
    );
    evidence_summary(out, &report.evidence.overall);
    out.push_str("</section>");
}

fn render_comparison_diagnostics(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let diagnostics = report.comparison.diagnostics;
    out.push_str("<h3>Coverage and data-quality counts</h3><dl class=\"stat-grid\">");
    comparison_stat(out, "Blocks", diagnostics.block_count);
    comparison_stat(out, "Eligible blocks", diagnostics.eligible_block_count);
    comparison_stat(out, "Invalid blocks", diagnostics.invalid_block_count);
    comparison_stat(
        out,
        "Left then right",
        diagnostics.left_then_right_block_count,
    );
    comparison_stat(
        out,
        "Right then left",
        diagnostics.right_then_left_block_count,
    );
    comparison_stat(out, "Paired rows", diagnostics.paired_row_count);
    comparison_stat(out, "Unique paths", diagnostics.unique_path_count);
    comparison_stat(out, "Unmatched left", diagnostics.unmatched_left_count);
    comparison_stat(out, "Unmatched right", diagnostics.unmatched_right_count);
    comparison_stat(out, "Missing SNR left", diagnostics.missing_snr_left_count);
    comparison_stat(
        out,
        "Missing SNR right",
        diagnostics.missing_snr_right_count,
    );
    comparison_stat(
        out,
        "Missing or invalid mode",
        diagnostics.missing_or_invalid_mode_count,
    );
    comparison_stat(out, "Missing mode", diagnostics.missing_mode_count);
    comparison_stat(out, "Malformed mode", diagnostics.malformed_mode_count);
    comparison_stat(out, "Ambiguous paths", diagnostics.ambiguous_path_count);
    comparison_stat(
        out,
        "Exact duplicates collapsed",
        diagnostics.exact_duplicate_count,
    );
    comparison_stat(
        out,
        "Conflicting duplicate groups",
        diagnostics.conflicting_duplicate_group_count,
    );
    comparison_stat(
        out,
        "Alignment exclusions",
        diagnostics.excluded_observation_count,
    );
    out.push_str("</dl>");
}

fn comparison_stat(out: &mut CheckedHtmlWriter<'_>, label: &str, value: usize) {
    write_html!(
        out,
        "<div class=\"stat\"><dt>{}</dt><dd>{}</dd></div>",
        label,
        value
    );
}

fn render_overlap(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Path overlap and missingness</h3>");
    if report.comparison.overlap_rows.is_empty() {
        out.push_str("<p class=\"empty\">No path-level overlap rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"legend\"><span><i class=\"swatch left\"></i>Left finite</span><span><i class=\"swatch right\"></i>Right finite</span></div><div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in &report.comparison.overlap_rows {
        let total = (row.left_finite_count + row.right_finite_count).max(1) as f64;
        let left_width = row.left_finite_count as f64 / total * 100.0;
        let right_width = row.right_finite_count as f64 / total * 100.0;
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"bar-track\"><span class=\"bar left\" style=\"width:{left_width:.3}%\"></span><span class=\"bar right\" style=\"width:{right_width:.3}%\"></span></span><span>{} / {}</span></div>", escape_html(&row.remote_path), comparison_stratum(&row.stratum), row.left_finite_count, row.right_finite_count);
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Path overlap and missingness data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Left finite</th><th scope=\"col\">Right finite</th><th scope=\"col\">Paired</th><th scope=\"col\">Unmatched left</th><th scope=\"col\">Unmatched right</th><th scope=\"col\">Missing SNR left</th><th scope=\"col\">Missing SNR right</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>");
    for row in &report.comparison.overlap_rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.left_finite_count, row.right_finite_count, row.paired_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
    }
    out.push_str("</tbody></table></div>");
}

fn render_comparison_timeline(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Data-quality timeline</h3>");
    if report.comparison.timeline_rows.is_empty() {
        out.push_str("<p class=\"empty\">No comparison timeline rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"timeline\" aria-hidden=\"true\">");
    for row in &report.comparison.timeline_rows {
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
        write_html!(
            out,
            "<span class=\"timeline-slot{invalid}{issue}\"><strong>{}</strong><br>{}<br>{}</span>",
            row.sequence_number,
            escape_html(row.actual_label.as_deref().unwrap_or("—")),
            slot_status(row.status)
        );
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Data-quality timeline details</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Eligible</th><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Starts</th><th scope=\"col\">Band</th><th scope=\"col\">Actual label</th><th scope=\"col\">Side</th><th scope=\"col\">Status</th><th scope=\"col\">Total</th><th scope=\"col\">Usable</th><th scope=\"col\">Excluded</th><th scope=\"col\">Missing SNR</th><th scope=\"col\">Missing/invalid mode</th><th scope=\"col\">Ambiguous</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th></tr></thead><tbody>");
    for row in &report.comparison.timeline_rows {
        timeline_table_row(out, row);
    }
    out.push_str("</tbody></table></div>");
}

fn render_comparison_blocks(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Comparison block inventory</h3>");
    if report.comparison.blocks.is_empty() {
        out.push_str("<p class=\"empty\">No comparison block rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Exact adjacent same-band block construction</caption><thead><tr><th scope=\"col\">Block</th><th scope=\"col\">Band</th><th scope=\"col\">First slot</th><th scope=\"col\">First actual / status</th><th scope=\"col\">Second slot</th><th scope=\"col\">Second actual / status</th><th scope=\"col\">Order</th><th scope=\"col\">Eligibility</th></tr></thead><tbody>");
    for block in &report.comparison.blocks {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{} · #{} · {}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", block.block_index + 1, band(block.band), escape_html(&block.first_slot_id), block.first_sequence_number, timestamp(block.first_starts_at), escape_html(block.first_label.as_deref().unwrap_or("Not recorded")), slot_status(block.first_status), block.second_slot_id.as_ref().map(|id| format!("{} · #{} · {}", id, block.second_sequence_number.unwrap_or_default(), block.second_starts_at.map(timestamp).unwrap_or_else(|| "Not recorded".into()))).map(|value| escape_html(&value)).unwrap_or_else(|| "Not recorded".into()), escape_html(&format!("{} / {}", block.second_label.as_deref().unwrap_or("Not recorded"), block.second_status.map(slot_status).unwrap_or("Not recorded"))), block.order.map(comparison_order).unwrap_or("Unavailable"), block_eligibility(block.eligibility));
    }
    out.push_str("</tbody></table></div>");
}

fn timeline_table_row(out: &mut CheckedHtmlWriter<'_>, row: &ComparisonTimelineRow) {
    write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", row.block_index + 1, yes_no(row.block_eligible), row.sequence_number, escape_html(&row.slot_id), timestamp(row.starts_at), band(row.band), escape_html(row.actual_label.as_deref().unwrap_or("Not recorded")), row.side.map(comparison_side).unwrap_or("Unavailable"), slot_status(row.status), row.total_observation_count, row.usable_observation_count, row.excluded_observation_count, row.missing_snr_count, row.missing_or_invalid_mode_count, row.ambiguous_path_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count);
}

fn render_paired_differences(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Paired difference distribution</h3>");
    let rows = &report.comparison.paired_rows;
    if rows.is_empty() {
        out.push_str(
            "<p class=\"empty\">No finite same-path paired differences are available.</p>",
        );
        return;
    }
    let max_abs = rows
        .iter()
        .map(|row| row.delta_right_minus_left_db.abs())
        .fold(1.0_f64, f64::max);
    out.push_str("<div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        let width = row.delta_right_minus_left_db.abs() / max_abs * 50.0;
        let left = if row.delta_right_minus_left_db < 0.0 {
            50.0 - width
        } else {
            50.0
        };
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"comparison-track\"><span class=\"comparison-zero\"></span><span class=\"comparison-delta\" style=\"left:{left:.3}%;width:{width:.3}%\"></span></span><span>{} dB</span></div>", escape_html(&row.remote_path), comparison_stratum(&row.stratum), format_signed(row.delta_right_minus_left_db));
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Paired difference data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">Left observation</th><th scope=\"col\">Right observation</th><th scope=\"col\">Left slot</th><th scope=\"col\">Right slot</th><th scope=\"col\">Left SNR</th><th scope=\"col\">Right SNR</th><th scope=\"col\">Right − left</th><th scope=\"col\">Elapsed</th></tr></thead><tbody>");
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{} s</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, comparison_order(row.order), escape_html(&row.left_observation_id), escape_html(&row.right_observation_id), escape_html(&row.left_slot_id), escape_html(&row.right_slot_id), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), row.elapsed_seconds);
    }
    out.push_str("</tbody></table></div>");
}

fn render_paired_snr_time(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Paired SNR over time</h3>");
    let rows = &report.comparison.paired_rows;
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">No paired SNR-over-time rows are available.</p>");
        return;
    }
    let minimum = rows
        .iter()
        .flat_map(|row| [row.left_snr_db, row.right_snr_db])
        .fold(f64::INFINITY, f64::min);
    let maximum = rows
        .iter()
        .flat_map(|row| [row.left_snr_db, row.right_snr_db])
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (maximum - minimum).max(1.0);
    out.push_str("<div class=\"legend\"><span><i class=\"swatch left\"></i>Left</span><span><i class=\"swatch right\"></i>Right</span></div><div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        let left = (row.left_snr_db - minimum) / span * 100.0;
        let right = (row.right_snr_db - minimum) / span * 100.0;
        write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{} · {}</span><span class=\"snr-pair\"><span class=\"snr-left\" style=\"left:{left:.3}%\"></span><span class=\"snr-right\" style=\"left:{right:.3}%\"></span></span><span>{} / {} dB</span></div>", timestamp(row.left_timestamp), escape_html(&row.remote_path), format_number(row.left_snr_db), format_number(row.right_snr_db));
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Paired SNR over time data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">Left time</th><th scope=\"col\">Right time</th><th scope=\"col\">Elapsed</th><th scope=\"col\">Left SNR</th><th scope=\"col\">Right SNR</th></tr></thead><tbody>");
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} s</td><td>{} dB</td><td>{} dB</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, comparison_order(row.order), timestamp(row.left_timestamp), timestamp(row.right_timestamp), row.elapsed_seconds, format_number(row.left_snr_db), format_number(row.right_snr_db));
    }
    out.push_str("</tbody></table></div>");
}

fn render_location_views(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Distance and azimuth path context</h3><p class=\"notice\">Distance and azimuth describe only the remote paths observed in these paired rows. Missing location stays visible, and geographic concentration limits how broadly these paths represent other distances or directions.</p>");
    if report.comparison.paired_rows.is_empty() {
        out.push_str("<p class=\"empty\">No paired rows are available for location views.</p>");
        return;
    }

    let mut strata = Vec::<ComparisonStratum>::new();
    for row in &report.comparison.paired_rows {
        if !strata.contains(&row.stratum) {
            strata.push(row.stratum.clone());
        }
    }
    for (index, stratum) in strata.iter().enumerate() {
        let rows = report
            .comparison
            .paired_rows
            .iter()
            .filter(|row| row.stratum == *stratum)
            .collect::<Vec<_>>();
        write_html!(
            out,
            "<section aria-labelledby=\"location-stratum-{index}\"><h4 id=\"location-stratum-{index}\">{}</h4>",
            comparison_stratum(stratum)
        );
        render_geographic_coverage(out, &rows);
        render_distance_view(out, &rows);
        render_azimuth_view(out, &rows);
        out.push_str("</section>");
    }
}

fn render_solar_context(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Derived solar context</h3><p class=\"notice\">Solar elevation and light state are deterministic geometric context derived from UTC timestamps and explicit Maidenhead locator cell centers. They are not captured propagation observations, do not adjust comparison values, and do not establish a cause for an observed difference.</p>");
    write_html!(
        out,
        "<p class=\"footnote\">Algorithm: {} v{}; coordinates: {}. Daylight begins at 0°, civil twilight at −6°, nautical twilight at −12°, astronomical twilight at −18°; gray line denotes any twilight category.</p>",
        escape_html(&report.solar_context.algorithm.algorithm_id),
        report.solar_context.algorithm.algorithm_version,
        escape_html(&report.solar_context.algorithm.coordinate_method)
    );
    if report.solar_context.rows.is_empty() {
        out.push_str(
            "<p class=\"empty\">No eligible paired rows are available for solar context.</p>",
        );
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Derived station and remote-endpoint solar context</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Side</th><th scope=\"col\">Observation</th><th scope=\"col\">UTC time</th><th scope=\"col\">Endpoint</th><th scope=\"col\">Grid</th><th scope=\"col\">Coordinates</th><th scope=\"col\">Elevation</th><th scope=\"col\">Light state</th><th scope=\"col\">Gray line</th></tr></thead><tbody>");
    for row in &report.solar_context.rows {
        for (side, observation) in [("Left", &row.left), ("Right", &row.right)] {
            for endpoint in [&observation.station, &observation.remote] {
                solar_table_row(out, row, side, observation, endpoint);
            }
        }
    }
    out.push_str("</tbody></table></div>");
}

fn solar_table_row(
    out: &mut CheckedHtmlWriter<'_>,
    row: &antennabench_analysis::SolarContextRow,
    side: &str,
    observation: &antennabench_analysis::SolarObservationContext,
    endpoint: &SolarEndpointContext,
) {
    let role = match endpoint.role {
        antennabench_analysis::SolarEndpointRole::Station => "Station",
        antennabench_analysis::SolarEndpointRole::Remote => "Remote",
    };
    let (coordinates, elevation, state, gray_line) = match &endpoint.result {
        SolarPositionResult::Available {
            coordinates,
            elevation_degrees,
            light_state,
            gray_line,
        } => (
            format!(
                "{:.6}°, {:.6}°",
                coordinates.latitude_degrees, coordinates.longitude_degrees
            ),
            format!("{:.3}°", elevation_degrees),
            solar_light_state(*light_state),
            yes_no(*gray_line),
        ),
        SolarPositionResult::Missing { reason } => (
            "Unavailable".into(),
            "Unavailable".into(),
            match reason {
                antennabench_analysis::SolarContextMissingReason::MissingGrid => "Missing grid",
                antennabench_analysis::SolarContextMissingReason::InvalidGrid => "Invalid grid",
            },
            "Unavailable",
        ),
    };
    write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}: {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, side, escape_html(&observation.observation_id), timestamp(observation.timestamp), role, escape_html(&endpoint.endpoint_id), optional_text(endpoint.grid.as_deref()), coordinates, elevation, state, gray_line);
}

fn solar_light_state(state: SolarLightState) -> &'static str {
    match state {
        SolarLightState::Daylight => "Daylight",
        SolarLightState::CivilTwilight => "Civil twilight",
        SolarLightState::NauticalTwilight => "Nautical twilight",
        SolarLightState::AstronomicalTwilight => "Astronomical twilight",
        SolarLightState::Night => "Night",
    }
}

fn render_geographic_coverage(out: &mut CheckedHtmlWriter<'_>, rows: &[&PairedObservationRow]) {
    let unique_paths = rows
        .iter()
        .map(|row| row.remote_path.as_str())
        .collect::<BTreeSet<_>>();
    let located_paths = rows
        .iter()
        .filter(|row| location_available(row))
        .map(|row| row.remote_path.as_str())
        .collect::<BTreeSet<_>>();
    let unavailable_rows = rows.iter().filter(|row| !location_available(row)).count();
    let mut sectors: [BTreeSet<&str>; 8] = std::array::from_fn(|_| BTreeSet::new());
    for row in rows {
        if location_available(row) {
            if let Some(azimuth) = row_azimuth(row) {
                sectors[azimuth_sector_index(azimuth)].insert(row.remote_path.as_str());
            }
        }
    }
    let (sector_index, sector_count) = sectors
        .iter()
        .enumerate()
        .max_by_key(|(index, paths)| (paths.len(), std::cmp::Reverse(*index)))
        .map(|(index, paths)| (index, paths.len()))
        .unwrap_or((0, 0));
    out.push_str("<dl class=\"stat-grid\">");
    comparison_stat(out, "Paired rows in stratum", rows.len());
    comparison_stat(out, "Unique paths in stratum", unique_paths.len());
    comparison_stat(out, "Unique paths with location", located_paths.len());
    comparison_stat(out, "Location-unavailable rows", unavailable_rows);
    write_html!(out, "<div class=\"stat\"><dt>Most populated 45° display sector</dt><dd>{}: {} of {} located paths</dd></div>", azimuth_sector_label(sector_index), sector_count, located_paths.len());
    out.push_str("</dl>");
}

fn render_distance_view(out: &mut CheckedHtmlWriter<'_>, rows: &[&PairedObservationRow]) {
    out.push_str("<h4>Observed distance</h4>");
    let maximum = rows
        .iter()
        .filter_map(|row| row_distance(row))
        .fold(1.0_f64, f64::max);
    out.push_str("<div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        match row_distance(row).filter(|_| location_available(row)) {
            Some(distance) => {
                let width = distance / maximum * 100.0;
                write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"bar-track\"><span class=\"location-fill\" style=\"width:{width:.3}%\"></span></span><span>{} km</span></div>", escape_html(&row.remote_path), format_number(distance));
            }
            None => write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"empty\">Location unavailable</span><span>—</span></div>", escape_html(&row.remote_path)),
        }
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Observed distance path-context data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">Left SNR</th><th scope=\"col\">Right SNR</th><th scope=\"col\">Right − left</th><th scope=\"col\">Left grid</th><th scope=\"col\">Right grid</th><th scope=\"col\">Left distance</th><th scope=\"col\">Right distance</th><th scope=\"col\">Availability</th><th scope=\"col\">Left time</th><th scope=\"col\">Right time</th></tr></thead><tbody>");
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, comparison_order(row.order), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), optional_text(row.left_remote_grid.as_deref()), optional_text(row.right_remote_grid.as_deref()), optional_measure_f64(row.left_distance_km, "km"), optional_measure_f64(row.right_distance_km, "km"), location_availability(row), timestamp(row.left_timestamp), timestamp(row.right_timestamp));
    }
    out.push_str("</tbody></table></div>");
}

fn render_azimuth_view(out: &mut CheckedHtmlWriter<'_>, rows: &[&PairedObservationRow]) {
    out.push_str("<h4>Observed azimuth</h4><div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        match row_azimuth(row).filter(|_| location_available(row)) {
            Some(azimuth) => {
                let left = azimuth / 360.0 * 100.0;
                write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"azimuth-track\"><span class=\"azimuth-marker\" style=\"left:{left:.3}%\"></span></span><span>{}°</span></div>", escape_html(&row.remote_path), format_number(azimuth));
            }
            None => write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"empty\">Location unavailable</span><span>—</span></div>", escape_html(&row.remote_path)),
        }
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Observed azimuth path-context data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">Left SNR</th><th scope=\"col\">Right SNR</th><th scope=\"col\">Right − left</th><th scope=\"col\">Left grid</th><th scope=\"col\">Right grid</th><th scope=\"col\">Left azimuth</th><th scope=\"col\">Right azimuth</th><th scope=\"col\">Display sector</th><th scope=\"col\">Availability</th><th scope=\"col\">Left time</th><th scope=\"col\">Right time</th></tr></thead><tbody>");
    for row in rows {
        let sector = row_azimuth(row)
            .filter(|_| location_available(row))
            .map(|azimuth| azimuth_sector_label(azimuth_sector_index(azimuth)))
            .unwrap_or("Location unavailable");
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, comparison_order(row.order), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), optional_text(row.left_remote_grid.as_deref()), optional_text(row.right_remote_grid.as_deref()), optional_measure_f64(row.left_azimuth_degrees, "°"), optional_measure_f64(row.right_azimuth_degrees, "°"), sector, location_availability(row), timestamp(row.left_timestamp), timestamp(row.right_timestamp));
    }
    out.push_str("</tbody></table></div>");
}

fn location_available(row: &PairedObservationRow) -> bool {
    row_grid(row).is_some() && row_distance(row).is_some() && row_azimuth(row).is_some()
}

fn location_availability(row: &PairedObservationRow) -> &'static str {
    if location_available(row) {
        "Available"
    } else {
        "Location unavailable"
    }
}

fn row_grid(row: &PairedObservationRow) -> Option<&str> {
    row.left_remote_grid
        .as_deref()
        .filter(|grid| !grid.is_empty())
        .or_else(|| {
            row.right_remote_grid
                .as_deref()
                .filter(|grid| !grid.is_empty())
        })
}

fn row_distance(row: &PairedObservationRow) -> Option<f64> {
    row.left_distance_km
        .filter(|value| value.is_finite() && *value >= 0.0)
        .or_else(|| {
            row.right_distance_km
                .filter(|value| value.is_finite() && *value >= 0.0)
        })
}

fn row_azimuth(row: &PairedObservationRow) -> Option<f64> {
    row.left_azimuth_degrees
        .filter(|value| value.is_finite())
        .or_else(|| row.right_azimuth_degrees.filter(|value| value.is_finite()))
        .map(|value| value.rem_euclid(360.0))
}

fn azimuth_sector_index(azimuth: f64) -> usize {
    ((azimuth / 45.0).floor() as usize).min(7)
}

fn azimuth_sector_label(index: usize) -> &'static str {
    match index {
        0 => "0°–<45°",
        1 => "45°–<90°",
        2 => "90°–<135°",
        3 => "135°–<180°",
        4 => "180°–<225°",
        5 => "225°–<270°",
        6 => "270°–<315°",
        _ => "315°–<360°",
    }
}

fn optional_text(value: Option<&str>) -> String {
    value.map(escape_html).unwrap_or_else(not_available)
}

fn optional_measure_f64(value: Option<f64>, unit: &str) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{} {unit}", format_number(value)))
        .unwrap_or_else(not_available)
}

fn render_stratum_summaries(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Stratum descriptive summaries</h3>");
    if report.comparison.strata.is_empty() {
        out.push_str("<p class=\"empty\">No comparison strata are available.</p>");
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Stratum summary data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Rows</th><th scope=\"col\">Paths</th><th scope=\"col\">Blocks</th><th scope=\"col\">Left → right</th><th scope=\"col\">Right → left</th><th scope=\"col\">Unmatched L/R</th><th scope=\"col\">Missing SNR L/R</th><th scope=\"col\">Duplicates</th><th scope=\"col\">Conflicts</th><th scope=\"col\">Observed range</th><th scope=\"col\">Median across paths</th></tr></thead><tbody>");
    for row in &report.comparison.strata {
        let range = row
            .minimum_delta_right_minus_left_db
            .zip(row.maximum_delta_right_minus_left_db)
            .map(|(minimum, maximum)| {
                format!(
                    "{} to {} dB",
                    format_signed(minimum),
                    format_signed(maximum)
                )
            })
            .unwrap_or_else(not_available);
        let median = row
            .median_path_delta_right_minus_left_db
            .map(|value| format!("{} dB", format_signed(value)))
            .unwrap_or_else(not_available);
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), row.paired_row_count, row.unique_path_count, row.contributing_block_count, row.left_then_right_block_count, row.right_then_left_block_count, row.unmatched_left_count, row.unmatched_right_count, row.missing_snr_left_count, row.missing_snr_right_count, row.exact_duplicate_count, row.conflicting_duplicate_group_count, range, median);
    }
    out.push_str("</tbody></table></div>");
}

fn render_antenna_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"antenna-title\"><h2 id=\"antenna-title\">Antenna evidence</h2>");
    render_snr_chart(out, report);
    if report.evidence.antennas.is_empty() {
        out.push_str("<p class=\"empty\">No per-antenna evidence is available.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Evidence by antenna</caption><thead><tr><th scope=\"col\">Antenna</th><th scope=\"col\">Coverage</th><th scope=\"col\">Contributing slots</th><th scope=\"col\">Counts</th><th scope=\"col\">Usable kinds</th><th scope=\"col\">Exclusions</th><th scope=\"col\">SNR</th></tr></thead><tbody>");
        for antenna in &report.evidence.antennas {
            evidence_row(out, antenna);
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
}

fn render_band_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"band-title\"><h2 id=\"band-title\">Band evidence</h2>");
    render_band_chart(out, report);
    if report.evidence.bands.is_empty() {
        out.push_str("<p class=\"empty\">No per-band evidence is available.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Evidence details by band</caption><thead><tr><th scope=\"col\">Band</th><th scope=\"col\">Counts</th><th scope=\"col\">Usable kinds</th><th scope=\"col\">Exclusions</th><th scope=\"col\">SNR</th></tr></thead><tbody>");
        for row in &report.evidence.bands {
            band_evidence_row(out, row);
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
}

fn render_slot_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"slot-title\"><h2 id=\"slot-title\">Slot evidence</h2>");
    render_slot_chart(out, report);
    if report.evidence.slots.is_empty() {
        out.push_str("<p class=\"empty\">No per-slot evidence is available.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Evidence details by slot</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Band</th><th scope=\"col\">Planned / actual</th><th scope=\"col\">Status</th><th scope=\"col\">Window / usable start</th><th scope=\"col\">Switch audit</th><th scope=\"col\">Counts</th><th scope=\"col\">Exclusions</th><th scope=\"col\">SNR</th></tr></thead><tbody>");
        for row in &report.evidence.slots {
            slot_evidence_row(out, row);
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
}

fn render_snr_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Antenna SNR visualization</h3>");
    let rows = &report.chart_data.antenna_snr;
    let bounds = rows
        .iter()
        .filter_map(|row| row.snr)
        .fold(None::<(f64, f64)>, |bounds, snr| {
            Some(match bounds {
                None => (snr.min_db, snr.max_db),
                Some((min, max)) => (min.min(snr.min_db), max.max(snr.max_db)),
            })
        });
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">No antenna SNR rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"chart\" aria-hidden=\"true\">");
    for row in rows {
        write_html!(
            out,
            "<div class=\"chart-row\"><span class=\"chart-label\">{}</span>",
            escape_html(&row.antenna_label)
        );
        match (row.snr, bounds) {
            (Some(snr), Some((min, max))) => {
                let range = (max - min).max(1.0);
                let left = (snr.min_db - min) / range * 100.0;
                let width = (snr.max_db - snr.min_db) / range * 100.0;
                let median = (snr.median_db - min) / range * 100.0;
                write_html!(out, "<span class=\"snr-track\"><span class=\"snr-range\" style=\"left:{left:.3}%;width:{width:.3}%\"></span><span class=\"snr-point\" style=\"left:{median:.3}%\"></span></span><span class=\"chart-value\">{} dB</span>", format_number(snr.median_db));
            }
            _ => out.push_str(
                "<span class=\"snr-track\"></span><span class=\"chart-value\">Unavailable</span>",
            ),
        }
        out.push_str("</div>");
    }
    out.push_str("</div><div class=\"table-wrap\"><table><caption>Antenna SNR chart data</caption><thead><tr><th scope=\"col\">Antenna</th><th scope=\"col\">Usable observations</th><th scope=\"col\">Samples</th><th scope=\"col\">Minimum</th><th scope=\"col\">Median</th><th scope=\"col\">Mean</th><th scope=\"col\">Maximum</th></tr></thead><tbody>");
    for row in rows {
        let cells = snr_cells(row.snr);
        write_html!(
            out,
            "<tr><td>{}</td><td>{}</td>{}</tr>",
            escape_html(&row.antenna_label),
            row.usable_observation_count,
            cells
        );
    }
    out.push_str("</tbody></table></div>");
}

fn render_band_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Band usable and excluded counts</h3>");
    let rows = &report.chart_data.band_evidence_counts;
    count_chart(
        out,
        rows.iter()
            .map(|row| (band(row.band).to_string(), row.observation_counts)),
    );
    if rows.is_empty() {
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Band evidence chart data</caption><thead><tr><th scope=\"col\">Band</th><th scope=\"col\">Total</th><th scope=\"col\">Usable</th><th scope=\"col\">Excluded</th><th scope=\"col\">Local</th><th scope=\"col\">Public</th><th scope=\"col\">Imported</th></tr></thead><tbody>");
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", band(row.band), row.observation_counts.total, row.observation_counts.usable, row.observation_counts.excluded, row.usable_observation_kinds.local_decode, row.usable_observation_kinds.public_report, row.usable_observation_kinds.imported_spot);
    }
    out.push_str("</tbody></table></div>");
}

fn render_slot_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    out.push_str("<h3>Slot usable and excluded counts</h3>");
    let rows = &report.chart_data.slot_evidence_counts;
    count_chart(
        out,
        rows.iter().map(|row| {
            (
                format!("#{} {}", row.sequence_number, row.planned_label),
                row.observation_counts,
            )
        }),
    );
    if rows.is_empty() {
        return;
    }
    out.push_str("<div class=\"table-wrap\"><table><caption>Slot evidence chart data</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Band</th><th scope=\"col\">Planned</th><th scope=\"col\">Actual</th><th scope=\"col\">Status</th><th scope=\"col\">Total</th><th scope=\"col\">Usable</th><th scope=\"col\">Excluded</th></tr></thead><tbody>");
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", row.sequence_number, escape_html(&row.slot_id), band(row.band), escape_html(&row.planned_label), escape_html(row.actual_label.as_deref().unwrap_or("Not recorded")), slot_status(row.status), row.observation_counts.total, row.observation_counts.usable, row.observation_counts.excluded);
    }
    out.push_str("</tbody></table></div>");
}

fn count_chart(
    out: &mut CheckedHtmlWriter<'_>,
    rows: impl IntoIterator<Item = (String, ObservationCounts)>,
) {
    let rows = rows.into_iter().collect::<Vec<_>>();
    if rows.is_empty() {
        out.push_str("<p class=\"empty\">No chart rows are available.</p>");
        return;
    }
    out.push_str("<div class=\"legend\"><span><i class=\"swatch usable\"></i>Usable</span><span><i class=\"swatch excluded\"></i>Excluded</span></div><div class=\"chart\" aria-hidden=\"true\">");
    for (label, counts) in rows {
        let denominator = counts.total.max(1) as f64;
        let usable = counts.usable as f64 / denominator * 100.0;
        let excluded = counts.excluded as f64 / denominator * 100.0;
        write_html!(out, "<div class=\"chart-row\"><span class=\"chart-label\">{}</span><span class=\"bar-track\"><span class=\"bar usable\" style=\"width:{usable:.3}%\"></span><span class=\"bar excluded\" style=\"width:{excluded:.3}%\"></span></span><span class=\"chart-value\">{} / {}</span></div>", escape_html(&label), counts.usable, counts.excluded);
    }
    out.push_str("</div>");
}

fn evidence_summary(out: &mut CheckedHtmlWriter<'_>, evidence: &ReportEvidenceSummary) {
    let counts = evidence.observation_counts;
    write_html!(out, "<dl class=\"stat-grid\"><div class=\"stat\"><dt>Total observations</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable</dt><dd>{}</dd></div><div class=\"stat\"><dt>Excluded</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable kinds</dt><dd>{}</dd></div><div class=\"stat\"><dt>Exclusions</dt><dd>{}</dd></div><div class=\"stat\"><dt>SNR statistics</dt><dd>{}</dd></div></dl>", counts.total, counts.usable, counts.excluded, kinds_text(evidence.usable_observation_kinds), exclusions_text(evidence), snr_text(evidence.snr));
}

fn evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &AntennaEvidenceSection) {
    write_html!(
        out,
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        escape_html(&row.antenna_label),
        evidence_coverage(row.evidence_quality),
        row.contributing_slot_count,
        counts_text(row.evidence.observation_counts),
        kinds_text(row.evidence.usable_observation_kinds),
        exclusions_text(&row.evidence),
        snr_text(row.evidence.snr)
    );
}

fn band_evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &BandEvidenceSection) {
    write_html!(
        out,
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        band(row.band),
        counts_text(row.evidence.observation_counts),
        kinds_text(row.evidence.usable_observation_kinds),
        exclusions_text(&row.evidence),
        snr_text(row.evidence.snr)
    );
}

fn slot_evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &SlotEvidenceSection) {
    let actual = row.actual_label.as_deref().unwrap_or("Not recorded");
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
        _ => "Not recorded".into(),
    };
    write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{} – {}; usable {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", row.sequence_number, escape_html(&row.slot_id), band(row.band), escape_html(&row.planned_label), escape_html(actual), slot_status(row.status), timestamp(row.starts_at), timestamp(row.ends_at), timestamp(row.usable_start), escape_html(&switch), counts_text(row.evidence.observation_counts), exclusions_text(&row.evidence), snr_text(row.evidence.snr));
}

fn snr_cells(snr: Option<SnrStatistics>) -> String {
    match snr {
        Some(snr) => format!(
            "<td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{} dB</td>",
            snr.sample_count,
            format_number(snr.min_db),
            format_number(snr.median_db),
            format_number(snr.mean_db),
            format_number(snr.max_db)
        ),
        None => "<td colspan=\"5\">Not available</td>".to_string(),
    }
}

fn counts_text(counts: ObservationCounts) -> String {
    format!(
        "{} total; {} usable; {} excluded",
        counts.total, counts.usable, counts.excluded
    )
}

fn kinds_text(kinds: UsableObservationKindCounts) -> String {
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

fn fact(out: &mut CheckedHtmlWriter<'_>, label: &str, value: &str) {
    write_html!(
        out,
        "<div class=\"fact\"><dt>{}</dt><dd>{}</dd></div>",
        label,
        escape_html(value)
    );
}

fn detail(out: &mut CheckedHtmlWriter<'_>, label: &str, value: &str) {
    write_html!(out, "<dt>{}</dt><dd>{}</dd>", label, escape_html(value));
}

fn optional_join(values: &[String]) -> String {
    if values.is_empty() {
        not_recorded()
    } else {
        values.join(", ")
    }
}

fn optional_measure(value: Option<f32>, unit: &str) -> String {
    value
        .map(|value| format!("{} {unit}", format_number(f64::from(value))))
        .unwrap_or_else(not_recorded)
}

fn not_recorded() -> String {
    "Not recorded".to_string()
}
fn not_available() -> String {
    "Not available".to_string()
}

fn timestamp(value: chrono::DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn format_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn escape_html(value: &str) -> String {
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

fn band(value: Band) -> &'static str {
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
fn experiment_mode(value: ExperimentMode) -> &'static str {
    match value {
        ExperimentMode::WholeStationAb => "Whole-station A/B",
        ExperimentMode::TxFocused => "Transmit-focused",
        ExperimentMode::RxFocused => "Receive-focused",
        ExperimentMode::SingleAntennaProfiling => "Single-antenna profiling",
    }
}
fn session_goal(value: SessionGoal) -> &'static str {
    match value {
        SessionGoal::Dx => "DX",
        SessionGoal::Regional => "Regional",
        SessionGoal::NvisLocal => "NVIS / local",
        SessionGoal::GeneralCoverage => "General coverage",
        SessionGoal::WeakSignalReliability => "Weak-signal reliability",
        SessionGoal::SingleAntennaProfiling => "Single-antenna profiling",
    }
}
fn evidence_coverage(value: EvidenceQuality) -> &'static str {
    match value {
        EvidenceQuality::Insufficient => "Insufficient",
        EvidenceQuality::Weak => "Weak",
        EvidenceQuality::Moderate => "Moderate",
    }
}
fn comparison_availability_label(value: ComparisonAvailability) -> &'static str {
    match value {
        ComparisonAvailability::NotApplicable => "Not applicable",
        ComparisonAvailability::UnsupportedComparisonShape => "Unsupported comparison shape",
        ComparisonAvailability::NoEligibleBlocks => "No eligible blocks",
        ComparisonAvailability::NoMatchedPaths => "No matched paths",
        ComparisonAvailability::DescriptivePairsAvailable => "Descriptive pairs available",
    }
}
fn comparison_availability_text(value: ComparisonAvailability) -> &'static str {
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
            "Eligible blocks exist, but no same-stratum remote path had finite SNR under both labels."
        }
        ComparisonAvailability::DescriptivePairsAvailable => {
            "Finite same-path paired rows are available for descriptive display only."
        }
    }
}
fn comparison_side(value: ComparisonSide) -> &'static str {
    match value {
        ComparisonSide::Left => "Left",
        ComparisonSide::Right => "Right",
    }
}
fn comparison_order(value: ComparisonOrder) -> &'static str {
    match value {
        ComparisonOrder::LeftThenRight => "Left then right",
        ComparisonOrder::RightThenLeft => "Right then left",
    }
}
fn comparison_stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        escape_html(value.mode.as_str()),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}
fn path_direction(value: PathDirection) -> &'static str {
    match value {
        PathDirection::Transmit => "TX path",
        PathDirection::Receive => "RX path",
    }
}
fn observation_kind(value: ObservationKind) -> &'static str {
    match value {
        ObservationKind::LocalDecode => "Local decode",
        ObservationKind::PublicReport => "Public report",
        ObservationKind::ImportedSpot => "Imported spot",
    }
}
fn record_source(value: RecordSource) -> &'static str {
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

fn block_eligibility(value: ComparisonBlockEligibility) -> &'static str {
    match value {
        ComparisonBlockEligibility::Eligible => "Eligible",
        ComparisonBlockEligibility::AmbiguousSequenceOrder => "Ambiguous sequence order",
        ComparisonBlockEligibility::IncompleteSameBandRun => "Incomplete same-band run",
        ComparisonBlockEligibility::MissingActualLabel => "Missing actual antenna",
        ComparisonBlockEligibility::RepeatedLabel => "Repeated actual antenna",
        ComparisonBlockEligibility::UnsupportedLabel => "Unsupported actual antenna",
    }
}

fn wspr_direction(value: WsprCycleDirection) -> &'static str {
    match value {
        WsprCycleDirection::Receive => "Receive",
        WsprCycleDirection::Transmit => "Transmit",
    }
}

fn wspr_readiness(value: crate::ReportWsprReadinessBasis) -> &'static str {
    match value {
        crate::ReportWsprReadinessBasis::OperatorConfirmed => "Operator confirmed",
        crate::ReportWsprReadinessBasis::CommandVerified => "Command verified",
    }
}

fn wspr_attribution(value: crate::ReportWsprAttribution) -> &'static str {
    match value {
        crate::ReportWsprAttribution::Pending => "Not yet run",
        crate::ReportWsprAttribution::Skipped => "Skipped by operator",
        crate::ReportWsprAttribution::Attributable => "Full antenna occupancy recorded",
        crate::ReportWsprAttribution::UnknownAntennaOccupancy => "Unknown antenna occupancy",
    }
}

fn operator_event_kind(value: ReportOperatorEventKind) -> &'static str {
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

fn correction_action(value: crate::ReportEventCorrectionAction) -> &'static str {
    match value {
        crate::ReportEventCorrectionAction::Retracted => "Retracted",
        crate::ReportEventCorrectionAction::Replaced => "Replaced",
    }
}

fn import_source_boundary(import: &ReportImportedEvidence) -> &'static str {
    if import.provider_id == "wspr-live" {
        "best-effort WSPR.live request-window collection; upstream mirror has no independent completeness guarantee"
    } else if import.completeness_known {
        "upstream completeness guarantee recorded"
    } else {
        "upstream completeness guarantee not independently recorded"
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}
fn format_signed(value: f64) -> String {
    if value > 0.0 {
        format!("+{}", format_number(value))
    } else {
        format_number(value)
    }
}
fn slot_status(value: AlignedSlotStatus) -> &'static str {
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
fn exclusion_reason(value: ObservationExclusionReason) -> &'static str {
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
fn notice_text(value: &ReportNotice) -> String {
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

fn detail_family(value: ReportDetailFamily) -> &'static str {
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
        ReportDetailFamily::Strata => "comparison stratum",
        ReportDetailFamily::Charts => "chart",
    }
}
