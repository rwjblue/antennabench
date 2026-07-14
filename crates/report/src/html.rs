use std::{
    collections::BTreeSet,
    fmt::{self, Write},
};

use antennabench_analysis::{
    ComparisonAvailability, ComparisonOrder, ComparisonSide, ComparisonStratum,
    ComparisonTimelineRow, EligibilityExclusionCategory, EligibilityScope, EvidenceQuality,
    ObservationCounts, ObservationExclusionReason, PairedObservationRow, PathDirection,
    SnrStatistics,
};
use antennabench_core::{
    AlignedSlotStatus, Band, ExperimentMode, ObservationKind, RecordSource, SessionGoal,
};
use chrono::{SecondsFormat, Utc};

use crate::{
    check_cancelled, report_resource_error, AntennaEvidenceSection, BandEvidenceSection,
    ReportCancellationToken, ReportDetailFamily, ReportError, ReportEvidenceSummary, ReportNotice,
    ReportResourceLimits, ReportResourceStage, SessionReport, SlotEvidenceSection,
    UsableObservationKindCounts, REPORT_RESOURCE_LIMITS,
};

macro_rules! write_html {
    ($output:expr, $($argument:tt)*) => {
        write!($output, $($argument)*).expect("checked HTML writer records failures")
    };
}

const STYLES: &str = r#"
:root{color-scheme:light;--ink:#172033;--muted:#5c667a;--line:#d8deea;--paper:#fff;--soft:#f5f7fb;--usable:#237a57;--excluded:#b84b4b;--accent:#315da8}*{box-sizing:border-box}body{margin:0;background:var(--soft);color:var(--ink);font:16px/1.5 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}main{width:min(1120px,calc(100% - 2rem));margin:2rem auto 4rem}.hero,.panel{background:var(--paper);border:1px solid var(--line);border-radius:.75rem;box-shadow:0 1px 2px #17203312}.hero{padding:1.5rem 1.75rem}.hero h1{margin:0 0 .25rem;font-size:clamp(1.7rem,4vw,2.6rem)}.eyebrow{margin:0;color:var(--accent);font-size:.78rem;font-weight:700;letter-spacing:.09em;text-transform:uppercase}.muted{color:var(--muted)}.panel{margin-top:1rem;padding:1.25rem;overflow:hidden}.panel h2{margin:.1rem 0 1rem}.panel h3{margin:1.4rem 0 .6rem}.facts,.stat-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.75rem}.fact,.stat{padding:.75rem;background:var(--soft);border-radius:.5rem}.fact dt,.stat dt{color:var(--muted);font-size:.78rem;font-weight:700;text-transform:uppercase}.fact dd,.stat dd{margin:.2rem 0 0;font-weight:650}.notice{padding:.75rem 1rem;border-left:.3rem solid #b36b00;background:#fff8e8}.badge{display:inline-block;padding:.16rem .55rem;border-radius:999px;background:#e5ebf7;font-size:.82rem;font-weight:700}.empty{padding:.85rem;border:1px dashed var(--line);border-radius:.5rem;color:var(--muted)}.table-wrap{overflow-x:auto}table{width:100%;border-collapse:collapse;font-size:.9rem}caption{text-align:left;font-weight:700;padding:.25rem 0 .55rem}th,td{padding:.55rem .65rem;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}thead th{background:var(--soft);white-space:nowrap}.chart,.comparison-chart{display:grid;gap:.5rem;margin:.5rem 0 1rem;padding:.8rem;background:var(--soft);border-radius:.5rem}.chart-row{display:grid;grid-template-columns:minmax(7rem,14rem) 1fr 4.5rem;gap:.6rem;align-items:center}.comparison-row{display:grid;grid-template-columns:minmax(8rem,16rem) 1fr minmax(5rem,auto);gap:.6rem;align-items:center}.chart-label{overflow-wrap:anywhere}.bar-track,.snr-track,.comparison-track,.snr-pair{position:relative;height:1rem;background:#e1e6ef;border-radius:999px;overflow:hidden}.bar{height:100%;float:left}.bar.usable{background:var(--usable)}.bar.excluded{background:var(--excluded)}.snr-range{position:absolute;top:.3rem;height:.4rem;border-radius:999px;background:var(--accent)}.snr-point{position:absolute;top:.1rem;width:.18rem;height:.8rem;background:var(--ink)}.comparison-zero{position:absolute;left:50%;top:0;width:1px;height:100%;background:var(--muted)}.comparison-delta{position:absolute;top:.2rem;height:.6rem;border-radius:999px;background:var(--accent)}.snr-pair{height:1.2rem}.snr-left,.snr-right{position:absolute;top:.15rem;width:.55rem;height:.9rem;border:2px solid var(--paper);border-radius:50%;transform:translateX(-50%)}.snr-left{background:#315da8}.snr-right{background:#b35c00}.timeline{display:flex;flex-wrap:wrap;gap:.35rem;margin:.5rem 0 1rem;padding:.8rem;background:var(--soft);border-radius:.5rem}.timeline-slot{min-width:2.6rem;padding:.45rem;border:1px solid var(--line);border-radius:.35rem;text-align:center}.timeline-slot.invalid{border-style:dashed;color:var(--muted)}.timeline-slot.issue{background:#fff8e8}.legend{display:flex;gap:1rem;color:var(--muted);font-size:.82rem}.swatch{display:inline-block;width:.7rem;height:.7rem;margin-right:.25rem;border-radius:.15rem}.swatch.usable{background:var(--usable)}.swatch.excluded{background:var(--excluded)}.antenna-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:.75rem}.antenna-card{padding:1rem;border:1px solid var(--line);border-radius:.5rem}.antenna-card h3{margin:0 0 .6rem}.antenna-card dl{margin:0}.antenna-card dt{color:var(--muted);font-size:.8rem;font-weight:700}.antenna-card dd{margin:0 0 .45rem}.footnote{font-size:.84rem;color:var(--muted)}@media print{body{background:#fff}main{width:100%;margin:0}.hero,.panel{box-shadow:none;break-inside:avoid}}@media(max-width:620px){.chart-row,.comparison-row{grid-template-columns:1fr}.chart-value{color:var(--muted)}}
.swatch.left,.bar.left{background:#315da8}.swatch.right,.bar.right{background:#b35c00}
.location-fill{height:100%;background:#315da8;border-radius:999px}.azimuth-track{position:relative;height:1rem;background:linear-gradient(90deg,#e1e6ef 0 24.8%,#cbd5e7 25% 25.2%,#e1e6ef 25.4% 49.8%,#cbd5e7 50% 50.2%,#e1e6ef 50.4% 74.8%,#cbd5e7 75% 75.2%,#e1e6ef 75.4% 100%);border-radius:999px}.azimuth-marker{position:absolute;top:.1rem;width:.45rem;height:.8rem;background:#b35c00;border:2px solid var(--paper);border-radius:50%;transform:translateX(-50%)}
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
    out.push_str("</style></head><body><main>");

    write_html!(
        out,
        "<header class=\"hero\"><p class=\"eyebrow\">AntennaBench local report</p>\
<h1>Session evidence report</h1><p class=\"muted\">Session <code>{}</code></p></header>",
        escape_html(&report.context.session_id)
    );
    render_notices(&mut out, &report.notices);
    render_eligibility(&mut out, report);
    render_context(&mut out, report);
    render_overall(&mut out, report);
    render_comparison(&mut out, report);
    render_antenna_section(&mut out, report);
    render_band_section(&mut out, report);
    render_slot_section(&mut out, report);

    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This report is descriptive and does not select an antenna winner.</p></main></body></html>");
    out.finish().map_err(ReportError::from)
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

fn render_comparison(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
    let comparison = &report.comparison;
    out.push_str("<section class=\"panel\" aria-labelledby=\"comparison-title\"><h2 id=\"comparison-title\">Paired comparison diagnostics</h2>");
    write_html!(
        out,
        "<p>Comparison availability: <span class=\"badge\">{}</span></p><p class=\"muted\">{}</p>",
        comparison_availability_label(comparison.availability),
        comparison_availability_text(comparison.availability)
    );
    if let Some(orientation) = &comparison.delta_orientation {
        write_html!(
            out,
            "<p><strong>Delta orientation:</strong> {} minus {} (right minus left).</p>",
            escape_html(&orientation.minuend_label),
            escape_html(&orientation.subtrahend_label)
        );
    }
    out.push_str("<p class=\"notice\">Adjacent switched slots reduce elapsed time but do not remove propagation or time confounding. Paired values are descriptive and do not establish antenna superiority.</p>");
    render_comparison_diagnostics(out, report);
    render_overlap(out, report);
    render_comparison_timeline(out, report);
    render_paired_differences(out, report);
    render_paired_snr_time(out, report);
    render_location_views(out, report);
    render_stratum_summaries(out, report);
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
        out.push_str("<div class=\"table-wrap\"><table><caption>Evidence details by slot</caption><thead><tr><th scope=\"col\">Sequence</th><th scope=\"col\">Slot</th><th scope=\"col\">Band</th><th scope=\"col\">Planned / actual</th><th scope=\"col\">Status</th><th scope=\"col\">Counts</th><th scope=\"col\">Exclusions</th><th scope=\"col\">SNR</th></tr></thead><tbody>");
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
    write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", row.sequence_number, escape_html(&row.slot_id), band(row.band), escape_html(&row.planned_label), escape_html(actual), slot_status(row.status), counts_text(row.evidence.observation_counts), exclusions_text(&row.evidence), snr_text(row.evidence.snr));
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
        RecordSource::WsjtxUdp => "WSJT-X UDP",
        RecordSource::WsjtxLog => "WSJT-X log",
        RecordSource::Wsprnet => "WSPRnet",
        RecordSource::WsprLive => "WSPR.live",
        RecordSource::ImportedFile => "Imported file",
        RecordSource::RigAdapter => "Rig adapter",
        RecordSource::NoaaSwpc => "NOAA SWPC",
        RecordSource::Derived => "Derived",
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
        ReportDetailFamily::Schedule => "schedule",
        ReportDetailFamily::AntennaContext => "antenna context",
        ReportDetailFamily::AntennaEvidence => "antenna evidence",
        ReportDetailFamily::BandEvidence => "band evidence",
        ReportDetailFamily::SlotEvidence => "slot evidence",
        ReportDetailFamily::ComparisonBlocks => "comparison block",
        ReportDetailFamily::PathOverlap => "path overlap",
        ReportDetailFamily::ComparisonTimeline => "comparison timeline",
        ReportDetailFamily::PairedObservations => "paired observation",
        ReportDetailFamily::PathSummaries => "path summary",
        ReportDetailFamily::Strata => "comparison stratum",
        ReportDetailFamily::Charts => "chart",
    }
}
