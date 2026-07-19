use super::super::geometry::geometry_class;
use super::*;
use std::collections::BTreeSet;

pub(in super::super) fn render_location_views(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
    out.push_str("<h3>Distance and azimuth path context</h3><p class=\"notice\">Distance and azimuth describe only the remote paths observed in these matched pairs. Missing location stays visible, and geographic concentration limits how broadly these paths represent other distances or directions.</p>");
    if report.comparison.paired_rows.is_empty() {
        out.push_str("<p class=\"empty\">No matched pairs are available for location views.</p>");
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
        render_distance_view(out, &rows, report);
        render_azimuth_view(out, &rows, report);
        out.push_str("</section>");
    }
}
pub(in super::super) fn render_solar_context(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) {
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
            "<p class=\"empty\">No eligible matched pairs are available for solar context.</p>",
        );
        return;
    }
    let (left_label, right_label) = report_antenna_labels(report);
    out.push_str("<div class=\"table-wrap\"><table><caption>Derived station and remote-endpoint solar context</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Antenna</th><th scope=\"col\">Observation</th><th scope=\"col\">UTC time</th><th scope=\"col\">Endpoint</th><th scope=\"col\">Grid</th><th scope=\"col\">Coordinates</th><th scope=\"col\">Elevation</th><th scope=\"col\">Light state</th><th scope=\"col\">Gray line</th></tr></thead><tbody>");
    for row in &report.solar_context.rows {
        for (side, observation) in [(&left_label, &row.left), (&right_label, &row.right)] {
            for endpoint in [&observation.station, &observation.remote] {
                solar_table_row(out, row, side, observation, endpoint);
            }
        }
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn solar_table_row(
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
pub(in super::super) fn solar_light_state(state: SolarLightState) -> &'static str {
    match state {
        SolarLightState::Daylight => "Daylight",
        SolarLightState::CivilTwilight => "Civil twilight",
        SolarLightState::NauticalTwilight => "Nautical twilight",
        SolarLightState::AstronomicalTwilight => "Astronomical twilight",
        SolarLightState::Night => "Night",
    }
}
pub(in super::super) fn render_geographic_coverage(
    out: &mut CheckedHtmlWriter<'_>,
    rows: &[&PairedObservationRow],
) {
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
pub(in super::super) fn render_distance_view(
    out: &mut CheckedHtmlWriter<'_>,
    rows: &[&PairedObservationRow],
    report: &SessionReport,
) {
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
                write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"bar-track\"><span class=\"location-fill geometry-width {}\"></span></span><span>{} km</span></div>", escape_html(&row.remote_path), geometry_class(width), format_number(distance));
            }
            None => write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"empty\">Location unavailable</span><span>—</span></div>", escape_html(&row.remote_path)),
        }
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "</div><div class=\"table-wrap\"><table><caption>Observed distance path-context data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">{} SNR</th><th scope=\"col\">{} SNR</th><th scope=\"col\">Signed delta</th><th scope=\"col\">{} grid</th><th scope=\"col\">{} grid</th><th scope=\"col\">{} distance</th><th scope=\"col\">{} distance</th><th scope=\"col\">Availability</th><th scope=\"col\">{} time</th><th scope=\"col\">{} time</th></tr></thead><tbody>", left_label, right_label, left_label, right_label, left_label, right_label, left_label, right_label);
    for row in rows {
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, labeled_comparison_order(row.order, &left_label, &right_label), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), optional_text(row.left_remote_grid.as_deref()), optional_text(row.right_remote_grid.as_deref()), optional_measure_f64(row.left_distance_km, "km"), optional_measure_f64(row.right_distance_km, "km"), location_availability(row), timestamp(row.left_timestamp), timestamp(row.right_timestamp));
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn render_azimuth_view(
    out: &mut CheckedHtmlWriter<'_>,
    rows: &[&PairedObservationRow],
    report: &SessionReport,
) {
    out.push_str("<h4>Observed azimuth</h4><div class=\"comparison-chart\" aria-hidden=\"true\">");
    for row in rows {
        match row_azimuth(row).filter(|_| location_available(row)) {
            Some(azimuth) => {
                let left = azimuth / 360.0 * 100.0;
                write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"azimuth-track\"><span class=\"azimuth-marker geometry-left {}\"></span></span><span>{}°</span></div>", escape_html(&row.remote_path), geometry_class(left), format_number(azimuth));
            }
            None => write_html!(out, "<div class=\"comparison-row\"><span class=\"chart-label\">{}</span><span class=\"empty\">Location unavailable</span><span>—</span></div>", escape_html(&row.remote_path)),
        }
    }
    let (left_label, right_label) = report_antenna_labels(report);
    write_html!(out, "</div><div class=\"table-wrap\"><table><caption>Observed azimuth path-context data</caption><thead><tr><th scope=\"col\">Stratum</th><th scope=\"col\">Remote path</th><th scope=\"col\">Block</th><th scope=\"col\">Order</th><th scope=\"col\">{} SNR</th><th scope=\"col\">{} SNR</th><th scope=\"col\">Signed delta</th><th scope=\"col\">{} grid</th><th scope=\"col\">{} grid</th><th scope=\"col\">{} azimuth</th><th scope=\"col\">{} azimuth</th><th scope=\"col\">Display sector</th><th scope=\"col\">Availability</th><th scope=\"col\">{} time</th><th scope=\"col\">{} time</th></tr></thead><tbody>", left_label, right_label, left_label, right_label, left_label, right_label, left_label, right_label);
    for row in rows {
        let sector = row_azimuth(row)
            .filter(|_| location_available(row))
            .map(|azimuth| azimuth_sector_label(azimuth_sector_index(azimuth)))
            .unwrap_or("Location unavailable");
        write_html!(out, "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} dB</td><td>{} dB</td><td>{} dB</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", comparison_stratum(&row.stratum), escape_html(&row.remote_path), row.block_index + 1, labeled_comparison_order(row.order, &left_label, &right_label), format_number(row.left_snr_db), format_number(row.right_snr_db), format_signed(row.delta_right_minus_left_db), optional_text(row.left_remote_grid.as_deref()), optional_text(row.right_remote_grid.as_deref()), optional_measure_f64(row.left_azimuth_degrees, "°"), optional_measure_f64(row.right_azimuth_degrees, "°"), sector, location_availability(row), timestamp(row.left_timestamp), timestamp(row.right_timestamp));
    }
    out.push_str("</tbody></table></div>");
}
pub(in super::super) fn location_available(row: &PairedObservationRow) -> bool {
    row_grid(row).is_some() && row_distance(row).is_some() && row_azimuth(row).is_some()
}
pub(in super::super) fn location_availability(row: &PairedObservationRow) -> &'static str {
    if location_available(row) {
        "Available"
    } else {
        "Location unavailable"
    }
}
pub(in super::super) fn row_grid(row: &PairedObservationRow) -> Option<&str> {
    row.left_remote_grid
        .as_deref()
        .filter(|grid| !grid.is_empty())
        .or_else(|| {
            row.right_remote_grid
                .as_deref()
                .filter(|grid| !grid.is_empty())
        })
}
pub(in super::super) fn row_distance(row: &PairedObservationRow) -> Option<f64> {
    row.left_distance_km
        .filter(|value| value.is_finite() && *value >= 0.0)
        .or_else(|| {
            row.right_distance_km
                .filter(|value| value.is_finite() && *value >= 0.0)
        })
}
pub(in super::super) fn row_azimuth(row: &PairedObservationRow) -> Option<f64> {
    row.left_azimuth_degrees
        .filter(|value| value.is_finite())
        .or_else(|| row.right_azimuth_degrees.filter(|value| value.is_finite()))
        .map(|value| value.rem_euclid(360.0))
}
pub(in super::super) fn azimuth_sector_index(azimuth: f64) -> usize {
    ((azimuth / 45.0).floor() as usize).min(7)
}
pub(in super::super) fn azimuth_sector_label(index: usize) -> &'static str {
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
pub(in super::super) fn optional_text(value: Option<&str>) -> String {
    value.map(escape_html).unwrap_or_else(not_available)
}
pub(in super::super) fn optional_measure_f64(value: Option<f64>, unit: &str) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{} {unit}", format_number(value)))
        .unwrap_or_else(not_available)
}
