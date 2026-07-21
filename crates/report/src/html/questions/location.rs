use super::super::geometry::geometry_class;
use super::*;
use crate::html::{
    templates::{
        render_template, CompactFootprintCloseTemplate, CompactFootprintEndTemplate,
        CompactFootprintTemplate, GeographyBeforeSolarTemplate, GeographyEndTemplate,
        GeographyTemplate,
    },
    view::{
        CompactFootprintGroupView, CompactFootprintView, CompositionRowView, ContextCellView,
        ContextSectionView, FootprintReachView, FullProfileGroupView, GeographyView,
        LocationPathAuditView, ObservedPathAuditRowView, ObservedPathAuditView,
        PathContextGroupView, PathContextView, ProfileBarChartView, ProfileBarRowView,
        ProfileDistributionRowView, ProfileDistributionView, ProfileTotalView, ProfileView,
    },
};

pub(in super::super) fn render_distance_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let view = geography_view(report);
    let single_antenna = view.single_antenna;
    render_template(out, &GeographyTemplate { view })?;
    if !single_antenna {
        render_location_views(out, report)?;
    }
    render_template(out, &GeographyBeforeSolarTemplate { single_antenna })?;
    render_solar_context(out, report)?;
    render_template(out, &GeographyEndTemplate)
}

pub(in super::super) fn render_compact_observed_footprint_section(
    out: &mut CheckedHtmlWriter<'_>,
    report: &SessionReport,
) -> Result<(), ReportError> {
    let view = compact_footprint_view(report);
    let no_groups = view.no_groups;
    render_template(out, &CompactFootprintTemplate { view })?;
    if no_groups {
        return render_template(out, &CompactFootprintCloseTemplate);
    }
    render_compact_repeatability_disclosure(out, report)?;
    render_template(
        out,
        &CompactFootprintEndTemplate {
            audit: observed_path_audit_view(report),
        },
    )
}

fn geography_view(report: &SessionReport) -> GeographyView {
    let profiles = report
        .overview
        .strata
        .iter()
        .filter(|stratum| {
            stratum.observed_profile.left.is_some() || stratum.observed_profile.right.is_some()
        })
        .enumerate()
        .map(|(index, stratum)| {
            let profile = &stratum.observed_profile;
            let dominant_summary = match (&profile.left, &profile.right) {
                (Some(left), Some(right)) => {
                    match (strict_dominant_distance(left), strict_dominant_distance(right)) {
                        (Some(left_bin), Some(right_bin)) if left_bin != right_bin => Some(format!(
                            "{}’s observed paths were concentrated in {}, while {}’s observed paths were concentrated in {} in this run.",
                            left.antenna_label,
                            left_bin.label(),
                            right.antenna_label,
                            right_bin.label()
                        )),
                        _ => None,
                    }
                }
                _ => None,
            };
            FullProfileGroupView {
                index,
                label: comparison_group_label(&stratum.stratum),
                dominant_summary,
                profile: profile_view(profile, false),
            }
        })
        .collect::<Vec<_>>();
    GeographyView {
        single_antenna: is_single_antenna_lens(report),
        goal_focus: goal_focus(report),
        no_profiles: profiles.is_empty(),
        profiles,
        audit_rows: observed_path_audit_rows(report),
        path_context: path_context_view(report),
    }
}

fn compact_footprint_view(report: &SessionReport) -> CompactFootprintView {
    let (available, unavailable): (Vec<_>, Vec<_>) =
        report.overview.strata.iter().partition(|stratum| {
            stratum.observed_profile.left.is_some()
                || stratum.observed_profile.right.is_some()
                || stratum.reach.left_only_unique_path_count > 0
                || stratum.reach.both_unique_path_count > 0
                || stratum.reach.right_only_unique_path_count > 0
        });
    let groups = available
        .into_iter()
        .enumerate()
        .map(|(index, stratum)| CompactFootprintGroupView {
            index,
            label: comparison_group_label(&stratum.stratum),
            reach: footprint_reach_view(report, &stratum.reach),
            profile: profile_view(&stratum.observed_profile, true),
        })
        .collect::<Vec<_>>();
    let unavailable_message = (!unavailable.is_empty()).then(|| {
        format!(
            "No usable observed footprint in {} of {} comparison groups: {}. Missing path or location evidence is not rendered as zero.",
            unavailable.len(),
            report.overview.strata.len(),
            raw_comparison_strata_list(&unavailable)
        )
    });
    CompactFootprintView {
        single_antenna: is_single_antenna_lens(report),
        goal_focus: goal_focus(report),
        no_groups: groups.is_empty(),
        groups,
        unavailable: unavailable_message,
    }
}

fn goal_focus(report: &SessionReport) -> Option<String> {
    let bins = &report.overview.goal_lens.as_ref()?.emphasized_distance_bins;
    (!bins.is_empty()).then(|| {
        bins.iter()
            .map(|bin| bin.label())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn profile_view(profile: &crate::ReportOverviewObservedProfile, compact: bool) -> ProfileView {
    let distance_caption = if compact {
        "Exact unique-path distance counts and observation support"
    } else {
        "Side-by-side observed distance distribution"
    };
    let azimuth_caption = if compact {
        "Exact unique-path direction counts and observation support"
    } else {
        "Side-by-side observed azimuth distribution"
    };
    let distance_distribution = profile_distribution_view(
        distance_caption,
        profile.left.as_ref(),
        profile.right.as_ref(),
        |value| &value.distance_bins,
        |cell| cell.category.label(),
    );
    let azimuth_distribution = profile_distribution_view(
        azimuth_caption,
        profile.left.as_ref(),
        profile.right.as_ref(),
        |value| &value.azimuth_sectors,
        |cell| fixed_azimuth_sector_label(cell.category),
    );
    let distance_bars = profile_bar_chart_view(
        "Observed unique paths by distance",
        profile.left.as_ref(),
        profile.right.as_ref(),
        |value| &value.distance_bins,
        |cell| cell.category.label(),
    );
    let azimuth_bars = profile_bar_chart_view(
        "Observed unique paths by direction",
        profile.left.as_ref(),
        profile.right.as_ref(),
        |value| &value.azimuth_sectors,
        |cell| fixed_azimuth_sector_label(cell.category),
    );
    let left_label = profile.left.as_ref().map_or_else(
        || "Left".to_string(),
        |profile| profile.antenna_label.clone(),
    );
    let right_label = profile.right.as_ref().map_or_else(
        || "Right".to_string(),
        |profile| profile.antenna_label.clone(),
    );
    ProfileView {
        totals: [profile.left.as_ref(), profile.right.as_ref()]
            .into_iter()
            .flatten()
            .map(|profile| ProfileTotalView {
                antenna: profile.antenna_label.clone(),
                unique_paths: profile.unique_path_count,
                located: profile.located_path_count,
                missing: profile.missing_location_path_count,
                inconsistent: profile.inconsistent_location_path_count,
            })
            .collect(),
        left_label,
        right_label,
        distributions: vec![distance_distribution, azimuth_distribution],
        bar_charts: vec![distance_bars, azimuth_bars],
        composition: profile
            .distance_composition
            .iter()
            .map(|cell| CompositionRowView {
                distance: cell.category.label(),
                left_only: cell.left_only_unique_path_count,
                shared: cell.shared_unique_path_count,
                right_only: cell.right_only_unique_path_count,
            })
            .collect(),
        composition_unavailable: profile.composition_location_unavailable_count,
        composition_suffix: plural_suffix(profile.composition_location_unavailable_count),
    }
}

fn profile_distribution_view<T: Copy>(
    caption: &'static str,
    left: Option<&ReportObservedAntennaProfile>,
    right: Option<&ReportObservedAntennaProfile>,
    cells: impl Fn(&ReportObservedAntennaProfile) -> &[ReportObservedProfileCell<T>],
    label: impl Fn(&ReportObservedProfileCell<T>) -> &'static str,
) -> ProfileDistributionView {
    let left_label = left.map_or("Left", |profile| profile.antenna_label.as_str());
    let right_label = right.map_or("Right", |profile| profile.antenna_label.as_str());
    let row_count = left
        .map(|profile| cells(profile).len())
        .or_else(|| right.map(|profile| cells(profile).len()))
        .unwrap_or_default();
    ProfileDistributionView {
        caption,
        left_label: left_label.to_string(),
        right_label: right_label.to_string(),
        rows: (0..row_count)
            .map(|index| {
                let left_cell = left.and_then(|profile| cells(profile).get(index));
                let right_cell = right.and_then(|profile| cells(profile).get(index));
                let category = left_cell
                    .or(right_cell)
                    .expect("profile distribution row exists");
                ProfileDistributionRowView {
                    label: label(category),
                    left: observed_profile_cell_text(left_cell),
                    right: observed_profile_cell_text(right_cell),
                }
            })
            .collect(),
    }
}

fn profile_bar_chart_view<T: Copy>(
    heading: &'static str,
    left: Option<&ReportObservedAntennaProfile>,
    right: Option<&ReportObservedAntennaProfile>,
    cells: impl Fn(&ReportObservedAntennaProfile) -> &[ReportObservedProfileCell<T>],
    label: impl Fn(&ReportObservedProfileCell<T>) -> &'static str,
) -> ProfileBarChartView {
    let left_label = left.map_or("First antenna", |profile| profile.antenna_label.as_str());
    let right_label = right.map_or("Second antenna", |profile| profile.antenna_label.as_str());
    let row_count = left
        .map(|profile| cells(profile).len())
        .or_else(|| right.map(|profile| cells(profile).len()))
        .unwrap_or_default();
    let maximum = (0..row_count)
        .flat_map(|index| {
            [
                left.and_then(|profile| cells(profile).get(index))
                    .map_or(0, |cell| cell.unique_path_count),
                right
                    .and_then(|profile| cells(profile).get(index))
                    .map_or(0, |cell| cell.unique_path_count),
            ]
        })
        .max()
        .unwrap_or(0)
        .max(1) as f64;
    ProfileBarChartView {
        heading,
        rows: (0..row_count)
            .map(|index| {
                let left_cell = left.and_then(|profile| cells(profile).get(index));
                let right_cell = right.and_then(|profile| cells(profile).get(index));
                let category = left_cell
                    .or(right_cell)
                    .expect("observed profile category exists");
                let left_count = left_cell.map_or(0, |cell| cell.unique_path_count);
                let right_count = right_cell.map_or(0, |cell| cell.unique_path_count);
                ProfileBarRowView {
                    label: label(category),
                    left_label: left_label.to_string(),
                    left_count,
                    left_class: geometry_class(left_count as f64 / maximum * 100.0),
                    right_label: right_label.to_string(),
                    right_count,
                    right_class: geometry_class(right_count as f64 / maximum * 100.0),
                }
            })
            .collect(),
    }
}

fn observed_profile_cell_text<T>(cell: Option<&ReportObservedProfileCell<T>>) -> String {
    cell.map_or_else(
        || "0 paths / 0 observations".to_string(),
        |cell| {
            format!(
                "{} path{} / {} observation{}",
                cell.unique_path_count,
                plural_suffix(cell.unique_path_count),
                cell.observation_count,
                plural_suffix(cell.observation_count)
            )
        },
    )
}

fn strict_dominant_distance(profile: &ReportObservedAntennaProfile) -> Option<ReportDistanceBin> {
    let mut ranked = profile.distance_bins.iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .unique_path_count
            .cmp(&left.unique_path_count)
            .then_with(|| left.category.index().cmp(&right.category.index()))
    });
    let first = ranked.first()?;
    (first.unique_path_count > 0
        && ranked
            .get(1)
            .is_none_or(|second| first.unique_path_count > second.unique_path_count))
    .then_some(first.category)
}

fn footprint_reach_view(report: &SessionReport, reach: &ReportOverviewReach) -> FootprintReachView {
    let AntennaLabels {
        left: left_label,
        right: right_label,
    } = antenna_labels(report);
    let presentation = reach_presentation(reach, "reach-bar");
    FootprintReachView {
        left_label,
        right_label,
        left_only: presentation.left_only,
        both: presentation.both,
        right_only: presentation.right_only,
        left_total: presentation.left_total,
        right_total: presentation.right_total,
        bar: presentation.bar,
    }
}

fn observed_path_audit_view(report: &SessionReport) -> ObservedPathAuditView {
    ObservedPathAuditView {
        rows: observed_path_audit_rows(report),
    }
}

fn observed_path_audit_rows(report: &SessionReport) -> Vec<ObservedPathAuditRowView> {
    report
        .comparison
        .observed_path_profiles
        .iter()
        .flat_map(|profile| {
            profile.paths.iter().map(|path| {
                let location = match &path.location {
                    antennabench_analysis::ObservedPathLocation::Available {
                        remote_grid,
                        distance_km,
                        initial_bearing_degrees,
                    } => format!(
                        "{} · {:.0} km · {:.0}°",
                        remote_grid, distance_km, initial_bearing_degrees
                    ),
                    antennabench_analysis::ObservedPathLocation::Missing => "Missing".to_string(),
                    antennabench_analysis::ObservedPathLocation::Inconsistent => {
                        "Inconsistent".to_string()
                    }
                };
                let snr = path.snr.map_or_else(
                    || "No finite SNR".to_string(),
                    |snr| {
                        format!(
                            "{} sample{} · median {} dB · range {} to {} dB",
                            snr.sample_count,
                            plural_suffix(snr.sample_count),
                            format_number(snr.median_db),
                            format_number(snr.min_db),
                            format_number(snr.max_db)
                        )
                    },
                );
                ObservedPathAuditRowView {
                    group: comparison_group_label(&profile.stratum),
                    antenna: profile.antenna_label.clone(),
                    remote_path: path.remote_path.clone(),
                    location,
                    block_support: path.block_support_count,
                    slot_support: path.slot_support_count,
                    blocks: path
                        .block_indices
                        .iter()
                        .map(|index| (index + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    slots: path.slot_ids.join(", "),
                    observations: path.observation_count,
                    observation_ids: path.observation_ids.join(", "),
                    snr,
                }
            })
        })
        .collect()
}

fn path_context_view(report: &SessionReport) -> PathContextView {
    if report.overview.strata.is_empty() {
        return PathContextView {
            no_groups: true,
            all_unavailable: None,
            groups: Vec::new(),
            unavailable: None,
        };
    }
    let available = report
        .overview
        .strata
        .iter()
        .enumerate()
        .filter(|(_, stratum)| located_path_count(&stratum.location_context) > 0)
        .collect::<Vec<_>>();
    let unavailable = report
        .overview
        .strata
        .iter()
        .filter(|stratum| located_path_count(&stratum.location_context) == 0)
        .collect::<Vec<_>>();
    if available.is_empty() {
        let (missing, inconsistent) = unavailable_location_counts(&unavailable);
        return PathContextView {
            no_groups: false,
            all_unavailable: Some(format!(
                "No observed matched paths are available for distance or azimuth context across {} ({}). Location unavailable remains separate ({} missing, {} inconsistent). This is not a near-zero path delta.",
                comparison_groups_label(unavailable.len()),
                raw_comparison_strata_list(&unavailable),
                missing,
                inconsistent
            )),
            groups: Vec::new(),
            unavailable: None,
        };
    }
    let groups = available
        .into_iter()
        .map(|(index, stratum)| path_context_group_view(index, stratum))
        .collect();
    let unavailable_message = (!unavailable.is_empty()).then(|| {
        let (missing, inconsistent) = unavailable_location_counts(&unavailable);
        format!(
            "No located matched paths in {} of {} comparison groups: {}. Location unavailable remains separate ({} missing, {} inconsistent).",
            unavailable.len(),
            report.overview.strata.len(),
            raw_comparison_strata_list(&unavailable),
            missing,
            inconsistent
        )
    });
    PathContextView {
        no_groups: false,
        all_unavailable: None,
        groups,
        unavailable: unavailable_message,
    }
}

fn path_context_group_view(index: usize, stratum: &ReportOverviewStratum) -> PathContextGroupView {
    let context = &stratum.location_context;
    let located = located_path_count(context);
    let distance = location_context_section(
        "Observed distance",
        "Fixed distance bins for observed paired paths",
        &context.distance_bins,
        distance_bin_label,
    );
    let azimuth = location_context_section(
        "Observed azimuth",
        "Fixed 45° azimuth sectors for observed paired paths",
        &context.azimuth_sectors,
        fixed_azimuth_sector_label,
    );
    PathContextGroupView {
        index,
        label: comparison_group_label(&stratum.stratum),
        located,
        located_suffix: plural_suffix(located),
        unavailable: context.missing_location_path_count + context.inconsistent_location_path_count,
        missing: context.missing_location_path_count,
        inconsistent: context.inconsistent_location_path_count,
        sections: vec![distance, azimuth],
        paths: context.paths.iter().map(location_path_audit_view).collect(),
    }
}

fn location_context_section<T: Copy>(
    heading: &'static str,
    caption: &'static str,
    cells: &[ReportOverviewLocationCell<T>],
    label: impl Fn(T) -> &'static str,
) -> ContextSectionView {
    ContextSectionView {
        heading,
        caption,
        cells: cells
            .iter()
            .map(|cell| ContextCellView {
                label: label(cell.category),
                unique_paths: cell.unique_located_path_count,
                paired_rows: cell.paired_row_count,
                delta: location_cell_delta(cell),
                evidence: location_cell_evidence(cell),
                empty: cell.unique_located_path_count == 0,
            })
            .collect(),
    }
}

fn location_path_audit_view(path: &crate::ReportOverviewLocationPath) -> LocationPathAuditView {
    let status = match path.availability {
        ReportPathLocationAvailability::Available => "Available",
        ReportPathLocationAvailability::Missing => "Missing",
        ReportPathLocationAvailability::Inconsistent => "Inconsistent",
    };
    LocationPathAuditView {
        remote_path: path.remote_path.clone(),
        pairs: path.paired_row_count,
        delta: format_signed(path.median_delta_right_minus_left_db),
        status,
        distance: optional_measure_f64(path.distance_km, "km"),
        azimuth: optional_measure_f64(path.azimuth_degrees, "°"),
    }
}

fn unavailable_location_counts(rows: &[&ReportOverviewStratum]) -> (usize, usize) {
    (
        rows.iter()
            .map(|row| row.location_context.missing_location_path_count)
            .sum(),
        rows.iter()
            .map(|row| row.location_context.inconsistent_location_path_count)
            .sum(),
    )
}

fn raw_comparison_strata_list(rows: &[&ReportOverviewStratum]) -> String {
    rows.iter()
        .map(|row| comparison_group_label(&row.stratum))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(in super::super) fn located_path_count(
    context: &crate::ReportOverviewLocationContext,
) -> usize {
    context
        .paths
        .iter()
        .filter(|path| path.availability == ReportPathLocationAvailability::Available)
        .count()
}

pub(in super::super) fn location_cell_delta<T>(cell: &ReportOverviewLocationCell<T>) -> String {
    match cell.median_path_delta_right_minus_left_db {
        Some(delta) if delta.abs() < 0.5 => format!("{} dB (near-zero)", format_signed(delta)),
        Some(delta) => format!("{} dB", format_signed(delta)),
        None => "No observed paired paths".into(),
    }
}

pub(in super::super) fn location_cell_evidence<T>(cell: &ReportOverviewLocationCell<T>) -> String {
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

pub(in super::super) fn distance_bin_label(bin: ReportDistanceBin) -> &'static str {
    bin.label()
}

pub(in super::super) fn fixed_azimuth_sector_label(sector: ReportAzimuthSector) -> &'static str {
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
