use std::fmt::Write as _;

use crate::{
    AntennaEvidenceSection, BandEvidenceSection, ReportEvidenceSummary, SessionReport,
    SlotEvidenceSection, UsableObservationKindCounts,
};
use antennabench_analysis::{ObservationCounts, SnrStatistics};

use super::{geometry::geometry_class, shared::*};

pub(super) fn render_antenna_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

pub(super) fn render_band_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

pub(super) fn render_slot_section(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

pub(super) fn render_snr_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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
                write_html!(out, "<span class=\"snr-track\"><span class=\"snr-range-position geometry-left {}\"><span class=\"snr-range geometry-width {}\"></span></span><span class=\"snr-point geometry-left {}\"></span></span><span class=\"chart-value\">{} dB</span>", geometry_class(left), geometry_class(width), geometry_class(median), format_number(snr.median_db));
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

pub(super) fn render_band_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

pub(super) fn render_slot_chart(out: &mut CheckedHtmlWriter<'_>, report: &SessionReport) {
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

pub(super) fn count_chart(
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
        write_html!(out, "<div class=\"chart-row\"><span class=\"chart-label\">{}</span><span class=\"bar-track\"><span class=\"bar usable geometry-width {}\"></span><span class=\"bar excluded geometry-width {}\"></span></span><span class=\"chart-value\">{} / {}</span></div>", escape_html(&label), geometry_class(usable), geometry_class(excluded), counts.usable, counts.excluded);
    }
    out.push_str("</div>");
}

pub(super) fn evidence_summary(out: &mut CheckedHtmlWriter<'_>, evidence: &ReportEvidenceSummary) {
    let counts = evidence.observation_counts;
    write_html!(out, "<dl class=\"stat-grid\"><div class=\"stat\"><dt>Total observations</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable</dt><dd>{}</dd></div><div class=\"stat\"><dt>Excluded</dt><dd>{}</dd></div><div class=\"stat\"><dt>Usable kinds</dt><dd>{}</dd></div><div class=\"stat\"><dt>Exclusions</dt><dd>{}</dd></div><div class=\"stat\"><dt>SNR statistics</dt><dd>{}</dd></div></dl>", counts.total, counts.usable, counts.excluded, kinds_text(evidence.usable_observation_kinds), exclusions_text(evidence), snr_text(evidence.snr));
}

pub(super) fn evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &AntennaEvidenceSection) {
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

pub(super) fn band_evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &BandEvidenceSection) {
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

pub(super) fn slot_evidence_row(out: &mut CheckedHtmlWriter<'_>, row: &SlotEvidenceSection) {
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

pub(super) fn snr_cells(snr: Option<SnrStatistics>) -> String {
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

pub(super) fn counts_text(counts: ObservationCounts) -> String {
    format!(
        "{} total; {} usable; {} excluded",
        counts.total, counts.usable, counts.excluded
    )
}

pub(super) fn kinds_text(kinds: UsableObservationKindCounts) -> String {
    format!(
        "{} local; {} public; {} imported",
        kinds.local_decode, kinds.public_report, kinds.imported_spot
    )
}

pub(super) fn exclusions_text(evidence: &ReportEvidenceSummary) -> String {
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

pub(super) fn snr_text(snr: Option<SnrStatistics>) -> String {
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
