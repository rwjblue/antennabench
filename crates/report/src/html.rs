use std::fmt::Write;

use antennabench_analysis::{
    EvidenceQuality, ObservationCounts, ObservationExclusionReason, SnrStatistics,
};
use antennabench_core::{AlignedSlotStatus, Band, ExperimentMode, SessionGoal};
use chrono::{SecondsFormat, Utc};

use crate::{
    AntennaEvidenceSection, BandEvidenceSection, ReportEvidenceSummary, ReportNotice,
    SessionReport, SlotEvidenceSection, UsableObservationKindCounts,
};

macro_rules! write_html {
    ($output:expr, $($argument:tt)*) => {
        write!($output, $($argument)*).expect("writing to a String cannot fail")
    };
}

const STYLES: &str = r#"
:root{color-scheme:light;--ink:#172033;--muted:#5c667a;--line:#d8deea;--paper:#fff;--soft:#f5f7fb;--usable:#237a57;--excluded:#b84b4b;--accent:#315da8}*{box-sizing:border-box}body{margin:0;background:var(--soft);color:var(--ink);font:16px/1.5 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}main{width:min(1120px,calc(100% - 2rem));margin:2rem auto 4rem}.hero,.panel{background:var(--paper);border:1px solid var(--line);border-radius:.75rem;box-shadow:0 1px 2px #17203312}.hero{padding:1.5rem 1.75rem}.hero h1{margin:0 0 .25rem;font-size:clamp(1.7rem,4vw,2.6rem)}.eyebrow{margin:0;color:var(--accent);font-size:.78rem;font-weight:700;letter-spacing:.09em;text-transform:uppercase}.muted{color:var(--muted)}.panel{margin-top:1rem;padding:1.25rem;overflow:hidden}.panel h2{margin:.1rem 0 1rem}.panel h3{margin:1.4rem 0 .6rem}.facts,.stat-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.75rem}.fact,.stat{padding:.75rem;background:var(--soft);border-radius:.5rem}.fact dt,.stat dt{color:var(--muted);font-size:.78rem;font-weight:700;text-transform:uppercase}.fact dd,.stat dd{margin:.2rem 0 0;font-weight:650}.notice{padding:.75rem 1rem;border-left:.3rem solid #b36b00;background:#fff8e8}.badge{display:inline-block;padding:.16rem .55rem;border-radius:999px;background:#e5ebf7;font-size:.82rem;font-weight:700}.empty{padding:.85rem;border:1px dashed var(--line);border-radius:.5rem;color:var(--muted)}.table-wrap{overflow-x:auto}table{width:100%;border-collapse:collapse;font-size:.9rem}caption{text-align:left;font-weight:700;padding:.25rem 0 .55rem}th,td{padding:.55rem .65rem;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}thead th{background:var(--soft);white-space:nowrap}.chart{display:grid;gap:.5rem;margin:.5rem 0 1rem;padding:.8rem;background:var(--soft);border-radius:.5rem}.chart-row{display:grid;grid-template-columns:minmax(7rem,14rem) 1fr 4.5rem;gap:.6rem;align-items:center}.chart-label{overflow-wrap:anywhere}.bar-track,.snr-track{position:relative;height:1rem;background:#e1e6ef;border-radius:999px;overflow:hidden}.bar{height:100%;float:left}.bar.usable{background:var(--usable)}.bar.excluded{background:var(--excluded)}.snr-range{position:absolute;top:.3rem;height:.4rem;border-radius:999px;background:var(--accent)}.snr-point{position:absolute;top:.1rem;width:.18rem;height:.8rem;background:var(--ink)}.legend{display:flex;gap:1rem;color:var(--muted);font-size:.82rem}.swatch{display:inline-block;width:.7rem;height:.7rem;margin-right:.25rem;border-radius:.15rem}.swatch.usable{background:var(--usable)}.swatch.excluded{background:var(--excluded)}.antenna-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:.75rem}.antenna-card{padding:1rem;border:1px solid var(--line);border-radius:.5rem}.antenna-card h3{margin:0 0 .6rem}.antenna-card dl{margin:0}.antenna-card dt{color:var(--muted);font-size:.8rem;font-weight:700}.antenna-card dd{margin:0 0 .45rem}.footnote{font-size:.84rem;color:var(--muted)}@media print{body{background:#fff}main{width:100%;margin:0}.hero,.panel{box-shadow:none;break-inside:avoid}}@media(max-width:620px){.chart-row{grid-template-columns:1fr}.chart-value{color:var(--muted)}}
"#;

/// Renders a deterministic, standalone HTML document from renderer-neutral
/// report data. The output contains no scripts, external resources, or
/// unescaped report strings.
pub fn render_standalone_html(report: &SessionReport) -> String {
    let mut out = String::with_capacity(32_768);
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
    render_context(&mut out, report);
    render_overall(&mut out, report);
    render_antenna_section(&mut out, report);
    render_band_section(&mut out, report);
    render_slot_section(&mut out, report);

    out.push_str("<p class=\"footnote\">Generated locally from deterministic report data. This report is descriptive and does not select an antenna winner.</p></main></body></html>");
    out
}

fn render_notices(out: &mut String, notices: &[ReportNotice]) {
    if notices.is_empty() {
        return;
    }
    out.push_str("<section class=\"panel\" aria-labelledby=\"notices-title\"><h2 id=\"notices-title\">Data notices</h2>");
    for notice in notices {
        write_html!(out, "<p class=\"notice\">{}</p>", notice_text(*notice));
    }
    out.push_str("</section>");
}

fn render_context(out: &mut String, report: &SessionReport) {
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

fn render_schedule_table(out: &mut String, report: &SessionReport) {
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

fn render_overall(out: &mut String, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"evidence-title\"><h2 id=\"evidence-title\">Evidence overview</h2>");
    write_html!(
        out,
        "<p>Evidence quality: <span class=\"badge\">{}</span></p>",
        evidence_quality(report.evidence.evidence_quality)
    );
    evidence_summary(out, &report.evidence.overall);
    out.push_str("</section>");
}

fn render_antenna_section(out: &mut String, report: &SessionReport) {
    out.push_str("<section class=\"panel\" aria-labelledby=\"antenna-title\"><h2 id=\"antenna-title\">Antenna evidence</h2>");
    render_snr_chart(out, report);
    if report.evidence.antennas.is_empty() {
        out.push_str("<p class=\"empty\">No per-antenna evidence is available.</p>");
    } else {
        out.push_str("<div class=\"table-wrap\"><table><caption>Evidence by antenna</caption><thead><tr><th scope=\"col\">Antenna</th><th scope=\"col\">Quality</th><th scope=\"col\">Contributing slots</th><th scope=\"col\">Counts</th><th scope=\"col\">Usable kinds</th><th scope=\"col\">Exclusions</th><th scope=\"col\">SNR</th></tr></thead><tbody>");
        for antenna in &report.evidence.antennas {
            evidence_row(out, antenna);
        }
        out.push_str("</tbody></table></div>");
    }
    out.push_str("</section>");
}

fn render_band_section(out: &mut String, report: &SessionReport) {
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

fn render_slot_section(out: &mut String, report: &SessionReport) {
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

fn render_snr_chart(out: &mut String, report: &SessionReport) {
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

fn render_band_chart(out: &mut String, report: &SessionReport) {
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

fn render_slot_chart(out: &mut String, report: &SessionReport) {
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

fn count_chart(out: &mut String, rows: impl IntoIterator<Item = (String, ObservationCounts)>) {
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

fn evidence_summary(out: &mut String, evidence: &ReportEvidenceSummary) {
    let counts = evidence.observation_counts;
    write_html!(out, "<dl class=\"stat-grid\"><div class=\"stat\"><dt>Total observations</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable</dt><dd>{}</dd></div><div class=\"stat\"><dt>Excluded</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable kinds</dt><dd>{}</dd></div><div class=\"stat\"><dt>Exclusions</dt><dd>{}</dd></div><div class=\"stat\"><dt>SNR statistics</dt><dd>{}</dd></div></dl>", counts.total, counts.usable, counts.excluded, kinds_text(evidence.usable_observation_kinds), exclusions_text(evidence), snr_text(evidence.snr));
}

fn evidence_row(out: &mut String, row: &AntennaEvidenceSection) {
    write_html!(
        out,
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        escape_html(&row.antenna_label),
        evidence_quality(row.evidence_quality),
        row.contributing_slot_count,
        counts_text(row.evidence.observation_counts),
        kinds_text(row.evidence.usable_observation_kinds),
        exclusions_text(&row.evidence),
        snr_text(row.evidence.snr)
    );
}

fn band_evidence_row(out: &mut String, row: &BandEvidenceSection) {
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

fn slot_evidence_row(out: &mut String, row: &SlotEvidenceSection) {
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

fn fact(out: &mut String, label: &str, value: &str) {
    write_html!(
        out,
        "<div class=\"fact\"><dt>{}</dt><dd>{}</dd></div>",
        label,
        escape_html(value)
    );
}

fn detail(out: &mut String, label: &str, value: &str) {
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
fn evidence_quality(value: EvidenceQuality) -> &'static str {
    match value {
        EvidenceQuality::Insufficient => "Insufficient",
        EvidenceQuality::Weak => "Weak",
        EvidenceQuality::Moderate => "Moderate",
    }
}
fn slot_status(value: AlignedSlotStatus) -> &'static str {
    match value {
        AlignedSlotStatus::PlannedNoSwitchEvent => "Planned; no switch event",
        AlignedSlotStatus::Switched => "Switched",
        AlignedSlotStatus::LateSwitch => "Late switch",
        AlignedSlotStatus::Missed => "Missed",
        AlignedSlotStatus::Bad => "Bad",
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
    }
}
fn notice_text(value: ReportNotice) -> &'static str {
    match value {
        ReportNotice::NoScheduledSlots => {
            "No scheduled slots are available; schedule comparisons cannot be shown."
        }
        ReportNotice::NoUsableObservations => {
            "No observations met the current evidence rules; no usable counts are inferred."
        }
        ReportNotice::NoUsableSnrSamples => {
            "No usable SNR samples are available; SNR statistics are shown as unavailable."
        }
    }
}
