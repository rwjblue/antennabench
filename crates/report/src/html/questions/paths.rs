use std::collections::BTreeMap;

use super::super::geometry::geometry_class;
use super::*;
use crate::{
    html::{
        templates::{
            render_template, PathQuestionSectionEndTemplate, ReachAuditStartTemplate,
            ReachSectionStartTemplate, ReachTemplate, SamePathAuditStartTemplate,
            SamePathSectionStartTemplate, SamePathTemplate,
        },
        view::{
            ExactPathView, PathDistributionView, PathDotView, PathStratumView, PathTickView,
            ReachBarView, ReachRowView, ReachSegmentView, ReachView, SamePathView,
        },
    },
    ReportOverviewPathMedianDelta,
};

pub(in super::super) fn render_same_path_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(out, &SamePathSectionStartTemplate)?;
    render_same_path_view(out, report, false)?;
    render_template(out, &SamePathAuditStartTemplate)?;
    render_comparison_diagnostics(out, report)?;
    render_paired_differences(out, report)?;
    render_paired_snr_time(out, report)?;
    render_stratum_summaries(out, report)?;
    render_template(out, &PathQuestionSectionEndTemplate)
}

pub(in super::super) fn render_reach_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    render_template(out, &ReachSectionStartTemplate)?;
    render_template(
        out,
        &ReachTemplate {
            view: reach_view(report),
        },
    )?;
    render_template(out, &ReachAuditStartTemplate)?;
    render_overlap(out, report)?;
    render_template(out, &PathQuestionSectionEndTemplate)
}

pub(in super::super) fn render_same_path_view(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
    compact: bool,
) -> Result<(), ReportError> {
    render_template(
        out,
        &SamePathTemplate {
            view: same_path_view(report, compact),
        },
    )
}

fn same_path_view(report: &SessionReport, compact: bool) -> SamePathView {
    if report.overview.strata.is_empty() {
        return SamePathView {
            compact,
            no_groups: true,
            all_unavailable: None,
            orientation: None,
            strata: Vec::new(),
            unavailable: None,
        };
    }
    let (left_label, right_label) = raw_labels(report);
    let orientation = report.overview.scope.delta_orientation.as_ref();
    let available = report
        .overview
        .strata
        .iter()
        .filter(|row| !row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|row| row.path_median_deltas.is_empty())
        .collect::<Vec<_>>();
    if available.is_empty() {
        let all_unavailable = if compact {
            format!(
                "No usable same-path path-median delta is available across {} ({}). This is not a 0 dB result; availability remains explicit in the result table.",
                comparison_groups_label(unavailable.len()),
                raw_strata_list(&unavailable)
            )
        } else {
            let (missing_left, missing_right) = missing_snr_totals(&unavailable);
            format!(
                "No usable same-path signal reports are available across {} ({}). Missing SNR remains separate ({left_label}: {missing_left}, {right_label}: {missing_right}). This is not a 0 dB result.",
                comparison_groups_label(unavailable.len()),
                raw_strata_list(&unavailable)
            )
        };
        return SamePathView {
            compact,
            no_groups: false,
            all_unavailable: Some(all_unavailable),
            orientation: None,
            strata: Vec::new(),
            unavailable: None,
        };
    }
    let orientation_text = orientation.map(|value| {
        format!(
            "Positive values mean {} was stronger; negative values mean {} was stronger. The vertical reference is zero.",
            value.minuend_label, value.subtrahend_label
        )
    });
    let unavailable_message = (!unavailable.is_empty()).then(|| {
        if compact {
            format!(
                "No usable same-path path-median delta in {} of {} comparison groups: {}. Availability remains explicit in the result table.",
                unavailable.len(),
                report.overview.strata.len(),
                raw_strata_list(&unavailable)
            )
        } else {
            let (missing_left, missing_right) = missing_snr_totals(&unavailable);
            format!(
                "No usable same-path signal reports in {} of {} comparison groups: {}. Missing SNR remains separate ({left_label}: {missing_left}, {right_label}: {missing_right}).",
                unavailable.len(),
                report.overview.strata.len(),
                raw_strata_list(&unavailable)
            )
        }
    });
    SamePathView {
        compact,
        no_groups: false,
        all_unavailable: None,
        orientation: orientation_text,
        strata: available
            .into_iter()
            .map(|row| path_stratum_view(row, orientation))
            .collect(),
        unavailable: unavailable_message,
    }
}

fn path_stratum_view(
    row: &ReportOverviewStratum,
    orientation: Option<&antennabench_analysis::DeltaOrientation>,
) -> PathStratumView {
    let matched_paths = row.path_median_deltas.len();
    let empty_message = if row.path_median_deltas.is_empty() {
        Some(
            if row.missing_snr_left_count > 0 || row.missing_snr_right_count > 0 {
                let (left_label, right_label) = orientation
                    .map(raw_orientation_labels)
                    .unwrap_or_else(|| ("Left".into(), "Right".into()));
                format!(
                "No usable same-path signal report is available; missing SNR is retained separately ({left_label}: {}, {right_label}: {}). This is not a 0 dB result.",
                row.missing_snr_left_count, row.missing_snr_right_count
            )
            } else {
                "No usable same-path signal report is available for this comparison group. This is not a 0 dB result."
                .to_string()
            },
        )
    } else {
        None
    };
    PathStratumView {
        label: raw_stratum(&row.stratum),
        matched_paths,
        path_suffix: plural_suffix(matched_paths),
        matched_pairs: row.paired_row_count,
        pair_suffix: plural_suffix(row.paired_row_count),
        blocks: row.contributing_block_count,
        block_suffix: plural_suffix(row.contributing_block_count),
        empty_message,
        distribution: path_distribution_view(row, orientation),
    }
}

fn path_distribution_view(
    row: &ReportOverviewStratum,
    orientation: Option<&antennabench_analysis::DeltaOrientation>,
) -> Option<PathDistributionView> {
    if row.path_median_deltas.is_empty() {
        return None;
    }
    let median = match row.path_delta {
        ReportOverviewPathDelta::Available {
            median_path_delta_right_minus_left_db,
            ..
        } => median_path_delta_right_minus_left_db,
        ReportOverviewPathDelta::Unavailable => return None,
    };
    let max_abs = row
        .path_median_deltas
        .iter()
        .map(|path| path.median_delta_right_minus_left_db.abs())
        .chain(std::iter::once(median.abs()))
        .fold(1.0_f64, f64::max);
    let maximum_absolute = path_axis_limit(max_abs);
    let (negative_label, positive_label) = orientation
        .map(raw_orientation_labels)
        .unwrap_or_else(|| ("Negative side".into(), "Positive side".into()));
    let mut paths = row.path_median_deltas.iter().collect::<Vec<_>>();
    paths.sort_by(|left, right| {
        left.median_delta_right_minus_left_db
            .total_cmp(&right.median_delta_right_minus_left_db)
            .then_with(|| left.remote_path.cmp(&right.remote_path))
    });
    let values = paths
        .iter()
        .map(|path| path.median_delta_right_minus_left_db)
        .collect::<Vec<_>>();
    let tick_step = path_axis_tick_step(maximum_absolute);
    let tick_count = (maximum_absolute / tick_step).round() as i32;
    let ticks = (-tick_count..=tick_count)
        .map(|tick| {
            let value = tick as f64 * tick_step;
            PathTickView {
                x: format!("{:.2}", path_x(value, maximum_absolute)),
                label: if tick == 0 {
                    "0".to_string()
                } else {
                    format_signed(value)
                },
            }
        })
        .collect();
    let dots = path_dots(&paths, maximum_absolute);
    Some(PathDistributionView {
        negative_count: values.iter().filter(|value| **value < 0.0).count(),
        tied_count: values.iter().filter(|value| **value == 0.0).count(),
        positive_count: values.iter().filter(|value| **value > 0.0).count(),
        median: format_signed(median),
        first_quartile: format_signed(interpolated_quantile(&values, 0.25)),
        third_quartile: format_signed(interpolated_quantile(&values, 0.75)),
        aria_label: format!(
            "Distribution of {} signed path-median SNR differences. Negative values favor {negative_label}; positive values favor {positive_label}.",
            paths.len()
        ),
        negative_label,
        positive_label,
        ticks,
        dots,
        orientation_text: if orientation.is_some() {
            "signed".to_string()
        } else {
            "right − left".to_string()
        },
        exact_paths: row
            .path_median_deltas
            .iter()
            .map(|path| ExactPathView {
                remote_path: path.remote_path.clone(),
                pairs: path.paired_row_count,
                delta: format_signed(path.median_delta_right_minus_left_db),
            })
            .collect(),
    })
}

fn path_dots(paths: &[&ReportOverviewPathMedianDelta], maximum_absolute: f64) -> Vec<PathDotView> {
    const BASELINE: f64 = 156.0;
    let mut stack_sizes = BTreeMap::<i64, usize>::new();
    for path in paths {
        *stack_sizes
            .entry((path.median_delta_right_minus_left_db * 1_000.0).round() as i64)
            .or_default() += 1;
    }
    let largest_stack = stack_sizes.values().copied().max().unwrap_or(1);
    let vertical_step = (104.0 / largest_stack as f64).min(11.0);
    let radius = (vertical_step * 0.38).clamp(2.2, 4.2);
    let mut stack_offsets = BTreeMap::<i64, usize>::new();
    paths
        .iter()
        .map(|path| {
            let value = path.median_delta_right_minus_left_db;
            let level = stack_offsets
                .entry((value * 1_000.0).round() as i64)
                .or_default();
            let y = BASELINE - (*level as f64 + 0.5) * vertical_step;
            *level += 1;
            let (class, fill) = if value < 0.0 {
                ("path-dot-negative", "#315da8")
            } else if value > 0.0 {
                ("path-dot-positive", "#b35c00")
            } else {
                ("path-dot-zero", "#5c667a")
            };
            PathDotView {
                detail: format!(
                    "{}: {} dB median across {} matched pair{}",
                    path.remote_path,
                    format_signed(value),
                    path.paired_row_count,
                    plural_suffix(path.paired_row_count)
                ),
                class,
                x: format!("{:.2}", path_x(value, maximum_absolute)),
                y: format!("{y:.2}"),
                radius: format!("{radius:.2}"),
                fill,
            }
        })
        .collect()
}

fn path_x(value: f64, maximum_absolute: f64) -> f64 {
    44.0 + delta_position(value, maximum_absolute) / 100.0 * 632.0
}

fn reach_view(report: &SessionReport) -> ReachView {
    let (left_label, right_label) = raw_labels(report);
    let (available, unavailable): (Vec<_>, Vec<_>) =
        report.overview.strata.iter().partition(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                > 0
        });
    let rows = available
        .into_iter()
        .map(|row| {
            let reach = &row.reach;
            let universe = reach.left_only_unique_path_count
                + reach.both_unique_path_count
                + reach.right_only_unique_path_count;
            ReachRowView {
                label: raw_stratum(&row.stratum),
                left_only: reach.left_only_unique_path_count,
                both: reach.both_unique_path_count,
                right_only: reach.right_only_unique_path_count,
                left_total: reach.left_only_unique_path_count + reach.both_unique_path_count,
                right_total: reach.right_only_unique_path_count + reach.both_unique_path_count,
                universe,
                universe_suffix: plural_suffix(universe),
                missing_left: row.missing_snr_left_count,
                missing_right: row.missing_snr_right_count,
                duplicates: row.exact_duplicate_count,
                conflicts: row.conflicting_duplicate_group_count,
                bar: reach_bar_view(reach, "reach-bar"),
            }
        })
        .collect();
    let unavailable_message = (!unavailable.is_empty()).then(|| {
        let (missing_left, missing_right) = missing_snr_totals(&unavailable);
        format!(
            "No usable path-reach signal reports in {} of {} comparison groups: {}. Missing SNR remains separate ({left_label}: {missing_left}, {right_label}: {missing_right}).",
            unavailable.len(),
            report.overview.strata.len(),
            raw_strata_list(&unavailable)
        )
    });
    ReachView {
        left_label,
        right_label,
        no_groups: report.overview.strata.is_empty(),
        rows,
        unavailable: unavailable_message,
    }
}

fn reach_bar_view(reach: &ReportOverviewReach, class: &str) -> ReachBarView {
    let counts = [
        (reach.left_only_unique_path_count, "left"),
        (reach.both_unique_path_count, "both"),
        (reach.right_only_unique_path_count, "right"),
    ];
    let total = counts.iter().map(|(count, _)| count).sum::<usize>().max(1) as f64;
    ReachBarView {
        class: class.to_string(),
        segments: counts
            .into_iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, side)| ReachSegmentView {
                side,
                geometry_class: geometry_class(count as f64 / total * 100.0),
            })
            .collect(),
    }
}

fn missing_snr_totals(rows: &[&ReportOverviewStratum]) -> (usize, usize) {
    (
        rows.iter().map(|row| row.missing_snr_left_count).sum(),
        rows.iter().map(|row| row.missing_snr_right_count).sum(),
    )
}

fn raw_labels(report: &SessionReport) -> (String, String) {
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

fn raw_orientation_labels(
    orientation: &antennabench_analysis::DeltaOrientation,
) -> (String, String) {
    (
        orientation.subtrahend_label.clone(),
        orientation.minuend_label.clone(),
    )
}

fn raw_stratum(value: &antennabench_analysis::ComparisonStratum) -> String {
    format!(
        "{} · {} · {} · {} · {}",
        path_direction(value.direction),
        band(value.band),
        value.mode.as_str(),
        observation_kind(value.observation_kind),
        record_source(value.source)
    )
}

fn raw_strata_list(rows: &[&ReportOverviewStratum]) -> String {
    rows.iter()
        .map(|row| raw_stratum(&row.stratum))
        .collect::<Vec<_>>()
        .join("; ")
}

fn path_axis_limit(maximum_absolute: f64) -> f64 {
    let step = path_axis_tick_step(maximum_absolute);
    (maximum_absolute / step).ceil().max(1.0) * step
}

fn path_axis_tick_step(maximum_absolute: f64) -> f64 {
    let raw = (maximum_absolute / 4.0).max(f64::MIN_POSITIVE);
    let magnitude = 10_f64.powf(raw.log10().floor());
    for multiplier in [1.0, 2.0, 5.0, 10.0] {
        let candidate = multiplier * magnitude;
        if candidate >= raw {
            return candidate;
        }
    }
    magnitude * 10.0
}

fn interpolated_quantile(sorted: &[f64], probability: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = probability * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let weight = position - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * weight
}

pub(in super::super) fn delta_position(value: f64, maximum_absolute: f64) -> f64 {
    (50.0 + value / maximum_absolute * 50.0).clamp(0.0, 100.0)
}

pub(in super::super) fn plural_suffix(value: usize) -> &'static str {
    if value == 1 {
        ""
    } else {
        "s"
    }
}
