use antennabench_analysis::{
    summarize_bundle_with_resources, summarize_bundle_with_resources_and_activity, AnalysisSummary,
    AntennaEvidenceSummary, BandEvidenceSummary, EvidenceSummary, ObservationKindCount,
    PairedComparisonAnalysis, PathDirection, SlotEvidenceSummary,
};
use antennabench_core::{
    codes, v2::AdapterRecordV2, v3::WsprCycleDirection, validate_bundle_report, BundleContents,
    BundleFileRole, BundleRecordKind, BundleValidationReport, ObservationKind, OperatorEventType,
    PlannedSlot,
};
use chrono::Duration;
use std::collections::BTreeMap;

use crate::{
    answerability::build_question_answerability, check_cancelled, coverage::build_coverage_maps,
    report_resource_error, AntennaEvidenceSection, AntennaSnrRow, BandEvidenceCountRow,
    BandEvidenceSection, CountingWriter, EvidenceSections, ReportAzimuthSector,
    ReportCancellationToken, ReportChartData, ReportComparisonData, ReportCompleteness,
    ReportDetailFamily, ReportDistanceBin, ReportError, ReportEvidenceSummary, ReportNotice,
    ReportOperatorEvent, ReportOperatorEventKind, ReportOverview, ReportOverviewLifecycle,
    ReportOverviewLifecycleState, ReportOverviewLimitation, ReportOverviewLocationCell,
    ReportOverviewLocationContext, ReportOverviewLocationPath, ReportOverviewPathDelta,
    ReportOverviewPathMedianDelta, ReportOverviewReach, ReportOverviewScope, ReportOverviewStratum,
    ReportPathLocationAvailability, ReportResourceLimits, ReportResourceStage,
    ReportRunTimelineRow, ReportSnapshotContext, ReportStratumAvailability, ScheduleOverview,
    ScheduledSlotContext, ScheduledTimeRange, SessionContext, SessionReport, SlotEvidenceCountRow,
    SlotEvidenceSection, StationContext, UsableObservationKindCounts, REPORT_RESOURCE_LIMITS,
};

pub fn build_report(bundle: &BundleContents) -> Result<SessionReport, ReportError> {
    let validation = validate_bundle_report(bundle);
    build_report_with_validation(bundle, &validation)
}

pub fn build_report_with_validation(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
) -> Result<SessionReport, ReportError> {
    build_report_with_snapshot(bundle, validation, ReportSnapshotContext::default())
}

pub fn build_report_with_snapshot(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot(
        bundle,
        validation,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        snapshot,
    )
}

pub fn build_report_with_snapshot_and_activity(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    adapter_records: &[AdapterRecordV2],
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot_and_activity(
        bundle,
        validation,
        adapter_records,
        REPORT_RESOURCE_LIMITS,
        &ReportCancellationToken::default(),
        snapshot,
    )
}

pub fn build_report_with_resources(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot(
        bundle,
        validation,
        limits,
        cancellation,
        ReportSnapshotContext::default(),
    )
}

fn build_report_with_resources_and_snapshot(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    build_report_with_resources_and_snapshot_and_activity(
        bundle,
        validation,
        &[],
        limits,
        cancellation,
        snapshot,
    )
}

fn build_report_with_resources_and_snapshot_and_activity(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    adapter_records: &[AdapterRecordV2],
    limits: ReportResourceLimits,
    cancellation: &ReportCancellationToken,
    snapshot: ReportSnapshotContext,
) -> Result<SessionReport, ReportError> {
    check_cancelled(
        cancellation,
        ReportResourceStage::Projection,
        "report_projection",
    )?;
    let mut snapshot = snapshot;
    if snapshot.operator_events.is_empty() {
        snapshot.operator_events = project_legacy_operator_events(bundle);
    }
    let cycle_directions = snapshot
        .wspr_cycles
        .iter()
        .filter_map(|cycle| {
            cycle.direction.map(|direction| {
                (
                    cycle.intent_id.clone(),
                    match direction {
                        WsprCycleDirection::Transmit => PathDirection::Transmit,
                        WsprCycleDirection::Receive => PathDirection::Receive,
                    },
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let summary = if adapter_records.is_empty() && cycle_directions.is_empty() {
        summarize_bundle_with_resources(bundle, validation, limits.analysis, cancellation)?
    } else {
        summarize_bundle_with_resources_and_activity(
            bundle,
            validation,
            adapter_records,
            &cycle_directions,
            limits.analysis,
            cancellation,
        )?
    };
    let mut coverage_maps = build_coverage_maps(&bundle.station.grid, &summary.reporter_activity);
    let detail_counts = DetailCounts::new(bundle, &summary, &coverage_maps, &snapshot);
    // The question-first views retain every path median, rather than sampling
    // them in the renderer. Count those rows up front so a bounded overview is
    // complete or explicitly rejected, never silently partial.
    // Each stratum retains its headline plus four distance bins and eight
    // azimuth sectors, in addition to one location-status row per paired path.
    let required_overview_rows = summary.eligibility.exclusions.len()
        + summary.comparison.strata.len() * 13
        + summary.comparison.path_summaries.len()
        + summary.reporter_activity.census_cycles.len()
        + summary.reporter_activity.cycle_rates.len()
        + summary.reporter_activity.paired_rates.len()
        + summary.reporter_activity.joint_summaries.len()
        + coverage_maps
            .iter()
            .flat_map(|group| &group.panels)
            .map(|panel| panel.cells.len() + panel.polar_cells.len())
            .sum::<usize>()
        + summary.slots.len()
        + snapshot.operator_events.len();
    if required_overview_rows as u64 > limits.rows {
        return Err(report_resource_error(
            "resource.report.rows",
            ReportResourceStage::Projection,
            "required_overview_rows",
            limits.rows,
            Some(required_overview_rows as u64),
            "rows",
        )
        .into());
    }
    let full_detail = detail_counts.total_rows() <= limits.rows;
    let context = build_context(bundle, &summary.bands, validation, full_detail);
    let overview = build_overview(
        bundle,
        &context,
        &summary.comparison,
        &summary.reporter_activity,
        &summary.slots,
        &snapshot,
    );
    let AnalysisSummary {
        session_id: _,
        evidence_quality,
        overall,
        antennas,
        bands,
        slots,
        comparison,
        mut reporter_activity,
        mut solar_context,
        mut exclusion_records,
        eligibility,
    } = summary;
    let (antenna_evidence, band_evidence, slot_evidence) = if full_detail {
        (
            antennas.into_iter().map(project_antenna).collect(),
            bands.into_iter().map(project_band).collect(),
            slots.into_iter().map(project_slot).collect(),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let evidence = EvidenceSections {
        evidence_quality,
        overall: project_evidence(overall),
        antennas: antenna_evidence,
        bands: band_evidence,
        slots: slot_evidence,
    };
    let chart_data = if full_detail {
        build_chart_data(&evidence)
    } else {
        ReportChartData::default()
    };
    let mut notices = build_notices(&context, &evidence);
    if !full_detail {
        solar_context.rows.clear();
        exclusion_records.clear();
        for cycle in &mut reporter_activity.census_cycles {
            cycle.active_reporters.clear();
        }
        for rate in &mut reporter_activity.cycle_rates {
            rate.heard_reporters.clear();
        }
        for rate in &mut reporter_activity.paired_rates {
            rate.receivers.clear();
        }
        for panel in coverage_maps.iter_mut().flat_map(|group| &mut group.panels) {
            panel.reporters.clear();
        }
        detail_counts.append_notices(&mut notices);
    }

    let mut report = SessionReport {
        completeness: if full_detail {
            ReportCompleteness::FullDetail
        } else {
            ReportCompleteness::BoundedOverview
        },
        overview,
        context,
        evidence,
        comparison: project_comparison(comparison, full_detail),
        reporter_activity,
        coverage_maps,
        solar_context,
        chart_data,
        notices,
        snapshot,
        eligibility_exclusions: eligibility.exclusions,
        exclusion_records,
    };
    check_cancelled(cancellation, ReportResourceStage::Serialize, "report_model")?;
    if let Err(error) = check_model_size(&report, limits.model_bytes) {
        if report.completeness == ReportCompleteness::FullDetail {
            make_overview(&mut report, &detail_counts);
            check_model_size(&report, limits.model_bytes)?;
        } else {
            return Err(error);
        }
    }
    Ok(report)
}

fn build_overview(
    bundle: &BundleContents,
    context: &SessionContext,
    comparison: &PairedComparisonAnalysis,
    reporter_activity: &antennabench_analysis::ReporterActivityAnalysis,
    slots: &[SlotEvidenceSummary],
    snapshot: &ReportSnapshotContext,
) -> ReportOverview {
    let observed_directions = comparison
        .strata
        .iter()
        .fold(Vec::new(), |mut directions, row| {
            if !directions.contains(&row.stratum.direction) {
                directions.push(row.stratum.direction);
            }
            directions
        });
    let limitations = build_overview_limitations(comparison);
    let answerability = build_question_answerability(comparison, reporter_activity);

    ReportOverview {
        scope: ReportOverviewScope {
            session_id: context.session_id.clone(),
            station: context.station.clone(),
            goal: Some(context.goal),
            experiment_mode: Some(context.experiment_mode),
            bands: context.bands.clone(),
            antenna_labels: bundle
                .antennas
                .antennas
                .iter()
                .map(|antenna| antenna.label.clone())
                .collect(),
            observed_directions,
            delta_orientation: comparison.delta_orientation.clone(),
        },
        lifecycle: ReportOverviewLifecycle {
            checkpoint_revision: snapshot.checkpoint_revision,
            state: snapshot
                .lifecycle
                .map(ReportOverviewLifecycleState::Recorded)
                .unwrap_or_default(),
        },
        answerability,
        comparison_availability: comparison.availability,
        strata: comparison
            .strata
            .iter()
            .map(|summary| project_overview_stratum(summary, comparison))
            .collect(),
        timeline: build_run_timeline(bundle, comparison, slots, snapshot),
        limitations,
    }
}

fn project_overview_stratum(
    summary: &antennabench_analysis::PairedStratumSummary,
    comparison: &PairedComparisonAnalysis,
) -> ReportOverviewStratum {
    let path_delta = match (
        summary.minimum_delta_right_minus_left_db,
        summary.median_path_delta_right_minus_left_db,
        summary.maximum_delta_right_minus_left_db,
    ) {
        (Some(minimum), Some(median_path), Some(maximum)) => ReportOverviewPathDelta::Available {
            minimum_delta_right_minus_left_db: minimum,
            median_path_delta_right_minus_left_db: median_path,
            maximum_delta_right_minus_left_db: maximum,
        },
        _ => ReportOverviewPathDelta::Unavailable,
    };

    let path_median_deltas = comparison
        .path_summaries
        .iter()
        .filter(|path| path.stratum == summary.stratum)
        .map(|path| ReportOverviewPathMedianDelta {
            remote_path: path.remote_path.clone(),
            paired_row_count: path.paired_row_count,
            median_delta_right_minus_left_db: path.median_delta_right_minus_left_db,
        })
        .collect();
    let reach = comparison
        .overlap_rows
        .iter()
        .filter(|row| row.stratum == summary.stratum)
        .fold(ReportOverviewReach::default(), |mut reach, row| {
            match (row.left_finite_count > 0, row.right_finite_count > 0) {
                (true, false) => reach.left_only_unique_path_count += 1,
                (true, true) => reach.both_unique_path_count += 1,
                (false, true) => reach.right_only_unique_path_count += 1,
                (false, false) => {}
            }
            reach
        });
    let location_context = project_location_context(&summary.stratum, comparison);

    ReportOverviewStratum {
        stratum: summary.stratum.clone(),
        availability: if summary.paired_row_count > 0 {
            ReportStratumAvailability::DescriptivePairsAvailable
        } else {
            ReportStratumAvailability::NoFinitePairedPaths
        },
        paired_row_count: summary.paired_row_count,
        unique_path_count: summary.unique_path_count,
        contributing_block_count: summary.contributing_block_count,
        left_then_right_block_count: summary.left_then_right_block_count,
        right_then_left_block_count: summary.right_then_left_block_count,
        unmatched_left_count: summary.unmatched_left_count,
        unmatched_right_count: summary.unmatched_right_count,
        missing_snr_left_count: summary.missing_snr_left_count,
        missing_snr_right_count: summary.missing_snr_right_count,
        excluded_observation_count: summary.excluded_observation_count,
        exact_duplicate_count: summary.exact_duplicate_count,
        conflicting_duplicate_group_count: summary.conflicting_duplicate_group_count,
        path_delta,
        path_median_deltas,
        reach,
        location_context,
    }
}

fn build_run_timeline(
    bundle: &BundleContents,
    comparison: &PairedComparisonAnalysis,
    slots: &[SlotEvidenceSummary],
    snapshot: &ReportSnapshotContext,
) -> Vec<ReportRunTimelineRow> {
    slots
        .iter()
        .filter_map(|slot| {
            let planned = bundle
                .schedule
                .slots
                .iter()
                .find(|planned| planned.slot_id == slot.slot_id)?;
            let comparison_row = comparison
                .timeline_rows
                .iter()
                .find(|row| row.slot_id == slot.slot_id);
            let cycle = snapshot
                .wspr_cycles
                .iter()
                .find(|cycle| cycle.intent_id == slot.slot_id);
            let block_index = comparison_row.map(|row| row.block_index);
            let block_eligibility = block_index.and_then(|index| {
                comparison
                    .blocks
                    .iter()
                    .find(|block| block.block_index == index)
                    .map(|block| block.eligibility)
            });
            Some(ReportRunTimelineRow {
                item_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                block_index,
                block_eligibility,
                band: slot.band,
                direction: cycle.and_then(|cycle| cycle.direction),
                planned_antenna: slot.planned_label.clone(),
                actual_antenna: cycle
                    .and_then(|cycle| cycle.actual_antenna.clone())
                    .or_else(|| slot.actual_label.clone()),
                planned_starts_at: planned.starts_at,
                planned_ends_at: planned.starts_at
                    + Duration::seconds(i64::from(planned.duration_seconds)),
                actual_starts_at: cycle
                    .and_then(|cycle| cycle.starts_at)
                    .or(slot.switch_timestamp),
                actual_ends_at: cycle.and_then(|cycle| cycle.transmission_ends_at),
                readiness_basis: cycle.and_then(|cycle| cycle.readiness_basis),
                attribution: cycle.map(|cycle| cycle.attribution),
                status: slot.status,
                total_observation_count: slot.evidence.observation_counts.total,
                usable_observation_count: slot.evidence.observation_counts.usable,
                excluded_observation_count: slot.evidence.observation_counts.excluded,
                event_history: snapshot
                    .operator_events
                    .iter()
                    .filter(|event| event.affected_slot_id.as_deref() == Some(&slot.slot_id))
                    .cloned()
                    .collect(),
            })
        })
        .collect()
}

fn project_legacy_operator_events(bundle: &BundleContents) -> Vec<ReportOperatorEvent> {
    bundle
        .events
        .iter()
        .map(|event| ReportOperatorEvent {
            event_id: event.event_id.clone(),
            occurred_at: event.meta.timestamp,
            slot_id: event.slot_id.clone(),
            affected_slot_id: event.slot_id.clone(),
            kind: match event.event_type {
                OperatorEventType::SessionStarted => ReportOperatorEventKind::SessionStarted,
                OperatorEventType::Switched => ReportOperatorEventKind::Switched,
                OperatorEventType::MissedSlot => ReportOperatorEventKind::SlotMissed,
                OperatorEventType::BadSlot => ReportOperatorEventKind::SlotBad,
                OperatorEventType::NoteAdded => ReportOperatorEventKind::NoteAdded,
                OperatorEventType::SessionEnded => ReportOperatorEventKind::SessionEnded,
            },
            detail: event.note.clone(),
            correction: None,
        })
        .collect()
}

fn project_location_context(
    stratum: &antennabench_analysis::ComparisonStratum,
    comparison: &PairedComparisonAnalysis,
) -> ReportOverviewLocationContext {
    let mut rows_by_path =
        BTreeMap::<&str, Vec<&antennabench_analysis::PairedObservationRow>>::new();
    for row in comparison
        .paired_rows
        .iter()
        .filter(|row| row.stratum == *stratum)
    {
        rows_by_path.entry(&row.remote_path).or_default().push(row);
    }

    let mut context = ReportOverviewLocationContext {
        distance_bins: ReportDistanceBin::ALL
            .into_iter()
            .map(|category| ReportOverviewLocationCell {
                category,
                unique_located_path_count: 0,
                paired_row_count: 0,
                median_path_delta_right_minus_left_db: None,
            })
            .collect(),
        azimuth_sectors: ReportAzimuthSector::ALL
            .into_iter()
            .map(|category| ReportOverviewLocationCell {
                category,
                unique_located_path_count: 0,
                paired_row_count: 0,
                median_path_delta_right_minus_left_db: None,
            })
            .collect(),
        ..Default::default()
    };
    let mut distance_deltas = vec![Vec::new(); ReportDistanceBin::ALL.len()];
    let mut azimuth_deltas = vec![Vec::new(); ReportAzimuthSector::ALL.len()];

    for path in comparison
        .path_summaries
        .iter()
        .filter(|path| path.stratum == *stratum)
    {
        let rows = rows_by_path
            .get(path.remote_path.as_str())
            .expect("every paired path summary has paired rows");
        let location = consistent_path_location(rows);
        let (availability, distance_km, azimuth_degrees) = match location {
            PathLocation::Available {
                distance_km,
                azimuth_degrees,
            } => {
                let distance_index = ReportDistanceBin::classify(distance_km)
                    .expect("validated distance has a semantic category")
                    .index();
                let azimuth_index = azimuth_sector_index(azimuth_degrees);
                let distance = &mut context.distance_bins[distance_index];
                distance.unique_located_path_count += 1;
                distance.paired_row_count += path.paired_row_count;
                distance_deltas[distance_index].push(path.median_delta_right_minus_left_db);
                let azimuth = &mut context.azimuth_sectors[azimuth_index];
                azimuth.unique_located_path_count += 1;
                azimuth.paired_row_count += path.paired_row_count;
                azimuth_deltas[azimuth_index].push(path.median_delta_right_minus_left_db);
                (
                    ReportPathLocationAvailability::Available,
                    Some(distance_km),
                    Some(azimuth_degrees),
                )
            }
            PathLocation::Missing => {
                context.missing_location_path_count += 1;
                (ReportPathLocationAvailability::Missing, None, None)
            }
            PathLocation::Inconsistent => {
                context.inconsistent_location_path_count += 1;
                (ReportPathLocationAvailability::Inconsistent, None, None)
            }
        };
        context.paths.push(ReportOverviewLocationPath {
            remote_path: path.remote_path.clone(),
            paired_row_count: path.paired_row_count,
            median_delta_right_minus_left_db: path.median_delta_right_minus_left_db,
            availability,
            distance_km,
            azimuth_degrees,
        });
    }

    for (cell, mut deltas) in context.distance_bins.iter_mut().zip(distance_deltas) {
        deltas.sort_by(f64::total_cmp);
        cell.median_path_delta_right_minus_left_db = median(&deltas);
    }
    for (cell, mut deltas) in context.azimuth_sectors.iter_mut().zip(azimuth_deltas) {
        deltas.sort_by(f64::total_cmp);
        cell.median_path_delta_right_minus_left_db = median(&deltas);
    }
    context
}

enum PathLocation {
    Available {
        distance_km: f64,
        azimuth_degrees: f64,
    },
    Missing,
    Inconsistent,
}

fn consistent_path_location(rows: &[&antennabench_analysis::PairedObservationRow]) -> PathLocation {
    let Some(first_row) = rows.first() else {
        return PathLocation::Missing;
    };
    let first = match paired_row_location(first_row) {
        Ok(location) => location,
        Err(availability) => return unavailable_path_location(availability),
    };
    for row in rows.iter().skip(1) {
        match paired_row_location(row) {
            Ok(location) if location == first => {}
            Ok(_) => return PathLocation::Inconsistent,
            Err(availability) => return unavailable_path_location(availability),
        }
    }
    PathLocation::Available {
        distance_km: first.1,
        azimuth_degrees: first.2,
    }
}

fn unavailable_path_location(availability: ReportPathLocationAvailability) -> PathLocation {
    match availability {
        ReportPathLocationAvailability::Available => {
            unreachable!("available location is not unavailable")
        }
        ReportPathLocationAvailability::Missing => PathLocation::Missing,
        ReportPathLocationAvailability::Inconsistent => PathLocation::Inconsistent,
    }
}

fn paired_row_location(
    row: &antennabench_analysis::PairedObservationRow,
) -> Result<(String, f64, f64), ReportPathLocationAvailability> {
    let left_grid = normalized_grid(row.left_remote_grid.as_deref())
        .ok_or(ReportPathLocationAvailability::Missing)?;
    let right_grid = normalized_grid(row.right_remote_grid.as_deref())
        .ok_or(ReportPathLocationAvailability::Missing)?;
    let left_distance =
        valid_distance(row.left_distance_km).ok_or(ReportPathLocationAvailability::Missing)?;
    let right_distance =
        valid_distance(row.right_distance_km).ok_or(ReportPathLocationAvailability::Missing)?;
    let left_azimuth =
        valid_azimuth(row.left_azimuth_degrees).ok_or(ReportPathLocationAvailability::Missing)?;
    let right_azimuth =
        valid_azimuth(row.right_azimuth_degrees).ok_or(ReportPathLocationAvailability::Missing)?;
    (left_grid == right_grid
        && left_distance.to_bits() == right_distance.to_bits()
        && left_azimuth.to_bits() == right_azimuth.to_bits())
    .then_some((left_grid, left_distance, left_azimuth))
    .ok_or(ReportPathLocationAvailability::Inconsistent)
}

fn normalized_grid(grid: Option<&str>) -> Option<String> {
    grid.map(str::trim)
        .filter(|grid| !grid.is_empty())
        .map(str::to_ascii_uppercase)
}

fn valid_distance(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn valid_azimuth(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0 && *value < 360.0)
}

fn azimuth_sector_index(azimuth_degrees: f64) -> usize {
    ((azimuth_degrees + 22.5) / 45.0).floor() as usize % 8
}

fn median(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| {
        let middle = values.len() / 2;
        if values.len().is_multiple_of(2) {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        }
    })
}

fn build_overview_limitations(
    comparison: &PairedComparisonAnalysis,
) -> Vec<ReportOverviewLimitation> {
    use antennabench_analysis::ComparisonAvailability;

    let mut limitations = match comparison.availability {
        ComparisonAvailability::NotApplicable => {
            vec![ReportOverviewLimitation::ComparisonNotApplicable]
        }
        ComparisonAvailability::UnsupportedComparisonShape => {
            vec![ReportOverviewLimitation::UnsupportedComparisonShape]
        }
        ComparisonAvailability::NoEligibleBlocks => {
            vec![ReportOverviewLimitation::NoEligibleBlocks]
        }
        ComparisonAvailability::NoMatchedPaths => vec![ReportOverviewLimitation::NoMatchedPaths],
        ComparisonAvailability::DescriptivePairsAvailable => Vec::new(),
    };
    let diagnostics = comparison.diagnostics;
    if diagnostics.unmatched_left_count > 0 || diagnostics.unmatched_right_count > 0 {
        limitations.push(ReportOverviewLimitation::UnmatchedPaths {
            left_count: diagnostics.unmatched_left_count,
            right_count: diagnostics.unmatched_right_count,
        });
    }
    if diagnostics.missing_snr_left_count > 0 || diagnostics.missing_snr_right_count > 0 {
        limitations.push(ReportOverviewLimitation::MissingSnr {
            left_count: diagnostics.missing_snr_left_count,
            right_count: diagnostics.missing_snr_right_count,
        });
    }
    if diagnostics.exact_duplicate_count > 0 || diagnostics.conflicting_duplicate_group_count > 0 {
        limitations.push(ReportOverviewLimitation::DuplicateEvidence {
            exact_count: diagnostics.exact_duplicate_count,
            conflicting_group_count: diagnostics.conflicting_duplicate_group_count,
        });
    }
    limitations
}

fn project_comparison(
    comparison: PairedComparisonAnalysis,
    full_detail: bool,
) -> ReportComparisonData {
    let (blocks, overlap_rows, timeline_rows, paired_rows, path_summaries, strata) = if full_detail
    {
        (
            comparison.blocks,
            comparison.overlap_rows,
            comparison.timeline_rows,
            comparison.paired_rows,
            comparison.path_summaries,
            comparison.strata,
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    ReportComparisonData {
        availability: comparison.availability,
        left_label: comparison.left_label,
        right_label: comparison.right_label,
        delta_orientation: comparison.delta_orientation,
        diagnostics: comparison.diagnostics,
        blocks,
        overlap_rows,
        timeline_rows,
        paired_rows,
        path_summaries,
        strata,
    }
}

fn build_context(
    bundle: &BundleContents,
    analyzed_bands: &[BandEvidenceSummary],
    validation: &BundleValidationReport,
    full_detail: bool,
) -> SessionContext {
    let scheduled_time_range = bundle
        .schedule
        .slots
        .first()
        .zip(bundle.schedule.slots.last())
        .map(|(first, last)| ScheduledTimeRange {
            starts_at: first.starts_at,
            ends_at: last.starts_at + Duration::seconds(i64::from(last.duration_seconds)),
        });
    let slots = if full_detail {
        bundle
            .schedule
            .slots
            .iter()
            .map(project_scheduled_slot)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let bands = analyzed_bands
        .iter()
        .filter(|summary| {
            bundle
                .schedule
                .slots
                .iter()
                .any(|slot| slot.band == summary.band)
        })
        .map(|summary| summary.band)
        .collect();

    let mut antennas = if full_detail {
        bundle.antennas.antennas.clone()
    } else {
        Vec::new()
    };
    for diagnostic in validation.diagnostics() {
        if diagnostic.location.record_kind != Some(BundleRecordKind::Antenna)
            || !matches!(
                diagnostic.code.as_str(),
                codes::NON_FINITE_NUMBER | codes::INVALID_RANGE
            )
        {
            continue;
        }
        let Some(antenna) = diagnostic
            .location
            .record_index
            .and_then(|index| antennas.get_mut(index))
        else {
            continue;
        };
        match diagnostic.location.field_path.as_deref() {
            Some(path) if path.ends_with("/height_m") => antenna.height_m = None,
            Some(path) if path.ends_with("/radial_length_m") => antenna.radial_length_m = None,
            Some(path) if path.ends_with("/orientation_degrees") => {
                antenna.orientation_degrees = None;
            }
            _ => {}
        }
    }
    let station_power_excluded = validation.diagnostics().iter().any(|diagnostic| {
        diagnostic.location.file == BundleFileRole::Station
            && diagnostic.location.field_path.as_deref() == Some("/power_watts")
            && matches!(
                diagnostic.code.as_str(),
                codes::NON_FINITE_NUMBER | codes::INVALID_RANGE
            )
    });

    SessionContext {
        session_id: bundle.manifest.session_id.clone(),
        station: StationContext {
            callsign: bundle.station.callsign.clone(),
            grid: bundle.station.grid.clone(),
            power_watts: (!station_power_excluded)
                .then_some(bundle.station.power_watts)
                .flatten(),
        },
        experiment_mode: bundle.schedule.mode,
        goal: bundle.schedule.goal,
        scheduled_time_range,
        antennas,
        bands,
        schedule: ScheduleOverview {
            slot_count: bundle.schedule.slots.len(),
            slots,
        },
    }
}

#[derive(Debug)]
struct DetailCounts {
    families: Vec<(ReportDetailFamily, usize)>,
    eligibility_rows: usize,
}

impl DetailCounts {
    fn new(
        bundle: &BundleContents,
        summary: &AnalysisSummary,
        coverage_maps: &[crate::ReportCoverageMapGroup],
        snapshot: &ReportSnapshotContext,
    ) -> Self {
        let comparison = &summary.comparison;
        let chart_rows = summary.antennas.len() + summary.bands.len() + summary.slots.len();
        Self {
            families: vec![
                (
                    ReportDetailFamily::LifecycleHistory,
                    snapshot.lifecycle_events.len(),
                ),
                (
                    ReportDetailFamily::Schedule,
                    bundle.schedule.slots.len() + snapshot.wspr_cycles.len(),
                ),
                (
                    ReportDetailFamily::AntennaContext,
                    bundle.antennas.antennas.len(),
                ),
                (ReportDetailFamily::AntennaEvidence, summary.antennas.len()),
                (ReportDetailFamily::BandEvidence, summary.bands.len()),
                (ReportDetailFamily::SlotEvidence, summary.slots.len()),
                (
                    ReportDetailFamily::ExclusionRecords,
                    summary.exclusion_records.len(),
                ),
                (
                    ReportDetailFamily::OperatorEvents,
                    snapshot.operator_events.len(),
                ),
                (
                    ReportDetailFamily::ComparisonBlocks,
                    comparison.blocks.len(),
                ),
                (
                    ReportDetailFamily::PathOverlap,
                    comparison.overlap_rows.len(),
                ),
                (
                    ReportDetailFamily::ComparisonTimeline,
                    comparison.timeline_rows.len(),
                ),
                (
                    ReportDetailFamily::PairedObservations,
                    comparison.paired_rows.len(),
                ),
                (
                    ReportDetailFamily::SolarContext,
                    summary.solar_context.rows.len(),
                ),
                (
                    ReportDetailFamily::PathSummaries,
                    comparison.path_summaries.len(),
                ),
                (ReportDetailFamily::Strata, comparison.strata.len()),
                (
                    ReportDetailFamily::ReporterActivityAudit,
                    summary
                        .reporter_activity
                        .census_cycles
                        .iter()
                        .map(|cycle| cycle.active_reporters.len())
                        .sum::<usize>()
                        + summary
                            .reporter_activity
                            .cycle_rates
                            .iter()
                            .map(|rate| rate.heard_reporters.len())
                            .sum::<usize>()
                        + summary
                            .reporter_activity
                            .paired_rates
                            .iter()
                            .map(|rate| rate.receivers.len())
                            .sum::<usize>(),
                ),
                (
                    ReportDetailFamily::CoverageMapReporters,
                    coverage_maps
                        .iter()
                        .flat_map(|group| &group.panels)
                        .map(|panel| panel.reporters.len())
                        .sum(),
                ),
                (ReportDetailFamily::Charts, chart_rows),
            ],
            eligibility_rows: summary.eligibility.exclusions.len(),
        }
    }

    fn total_rows(&self) -> u64 {
        (self.eligibility_rows + self.families.iter().map(|(_, count)| *count).sum::<usize>())
            as u64
    }

    fn append_notices(&self, notices: &mut Vec<ReportNotice>) {
        for (family, row_count) in &self.families {
            if *row_count > 0 {
                notices.push(ReportNotice::DetailOmitted {
                    family: *family,
                    row_count: *row_count,
                });
            }
        }
    }
}

fn make_overview(report: &mut SessionReport, counts: &DetailCounts) {
    report.completeness = ReportCompleteness::BoundedOverview;
    report.context.antennas.clear();
    report.context.schedule.slots.clear();
    report.evidence.antennas.clear();
    report.evidence.bands.clear();
    report.evidence.slots.clear();
    report.exclusion_records.clear();
    report.comparison.blocks.clear();
    report.comparison.overlap_rows.clear();
    report.comparison.timeline_rows.clear();
    report.comparison.paired_rows.clear();
    report.solar_context.rows.clear();
    report.comparison.path_summaries.clear();
    report.comparison.strata.clear();
    for cycle in &mut report.reporter_activity.census_cycles {
        cycle.active_reporters.clear();
    }
    for rate in &mut report.reporter_activity.cycle_rates {
        rate.heard_reporters.clear();
    }
    for rate in &mut report.reporter_activity.paired_rates {
        rate.receivers.clear();
    }
    for panel in report
        .coverage_maps
        .iter_mut()
        .flat_map(|group| &mut group.panels)
    {
        panel.reporters.clear();
    }
    report.chart_data = ReportChartData::default();
    report.snapshot.lifecycle_events.clear();
    report.snapshot.operator_events.clear();
    report.snapshot.wspr_cycles.clear();
    report.snapshot.antenna_control_attempts.clear();
    report
        .notices
        .retain(|notice| !matches!(notice, ReportNotice::DetailOmitted { .. }));
    counts.append_notices(&mut report.notices);
}

fn check_model_size(report: &SessionReport, limit: u64) -> Result<(), ReportError> {
    let mut writer = CountingWriter::new(limit);
    match serde_json::to_writer(&mut writer, report) {
        Ok(()) => Ok(()),
        Err(_error) if writer.observed() > limit => Err(report_resource_error(
            "resource.report.model_bytes",
            ReportResourceStage::Serialize,
            "renderer_neutral_model",
            limit,
            Some(writer.observed()),
            "bytes",
        )
        .into()),
        Err(error) => Err(ReportError::Serialization {
            message: error.to_string(),
        }),
    }
}

fn project_scheduled_slot(slot: &PlannedSlot) -> ScheduledSlotContext {
    ScheduledSlotContext {
        slot_id: slot.slot_id.clone(),
        sequence_number: slot.sequence_number,
        starts_at: slot.starts_at,
        ends_at: slot.starts_at + Duration::seconds(i64::from(slot.duration_seconds)),
        guard_seconds: slot.guard_seconds,
        band: slot.band,
        planned_label: slot.antenna_label.clone(),
    }
}

fn project_antenna(summary: AntennaEvidenceSummary) -> AntennaEvidenceSection {
    AntennaEvidenceSection {
        antenna_label: summary.antenna_label,
        contributing_slot_count: summary.contributing_slot_count,
        evidence_quality: summary.evidence_quality,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_band(summary: BandEvidenceSummary) -> BandEvidenceSection {
    BandEvidenceSection {
        band: summary.band,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_slot(summary: SlotEvidenceSummary) -> SlotEvidenceSection {
    SlotEvidenceSection {
        slot_id: summary.slot_id,
        sequence_number: summary.sequence_number,
        band: summary.band,
        planned_label: summary.planned_label,
        actual_label: summary.actual_label,
        status: summary.status,
        starts_at: summary.starts_at,
        ends_at: summary.ends_at,
        usable_start: summary.usable_start,
        switch_event_id: summary.switch_event_id,
        switch_timestamp: summary.switch_timestamp,
        switch_delay_seconds: summary.switch_delay_seconds,
        evidence: project_evidence(summary.evidence),
    }
}

fn project_evidence(summary: EvidenceSummary) -> ReportEvidenceSummary {
    ReportEvidenceSummary {
        observation_counts: summary.observation_counts,
        exclusions: summary.exclusions,
        usable_observation_kinds: project_observation_kinds(summary.usable_observation_kinds),
        snr: summary.snr,
    }
}

fn project_observation_kinds(
    kinds: impl IntoIterator<Item = ObservationKindCount>,
) -> UsableObservationKindCounts {
    let mut counts = UsableObservationKindCounts::default();

    for kind in kinds {
        match kind.kind {
            ObservationKind::LocalDecode => counts.local_decode += kind.count,
            ObservationKind::PublicReport => counts.public_report += kind.count,
            ObservationKind::ImportedSpot => counts.imported_spot += kind.count,
        }
    }

    counts
}

fn build_chart_data(evidence: &EvidenceSections) -> ReportChartData {
    ReportChartData {
        antenna_snr: evidence
            .antennas
            .iter()
            .map(|antenna| AntennaSnrRow {
                antenna_label: antenna.antenna_label.clone(),
                usable_observation_count: antenna.evidence.observation_counts.usable,
                snr: antenna.evidence.snr,
            })
            .collect(),
        band_evidence_counts: evidence
            .bands
            .iter()
            .map(|band| BandEvidenceCountRow {
                band: band.band,
                observation_counts: band.evidence.observation_counts,
                usable_observation_kinds: band.evidence.usable_observation_kinds,
            })
            .collect(),
        slot_evidence_counts: evidence
            .slots
            .iter()
            .map(|slot| SlotEvidenceCountRow {
                slot_id: slot.slot_id.clone(),
                sequence_number: slot.sequence_number,
                band: slot.band,
                planned_label: slot.planned_label.clone(),
                actual_label: slot.actual_label.clone(),
                status: slot.status,
                observation_counts: slot.evidence.observation_counts,
            })
            .collect(),
    }
}

fn build_notices(context: &SessionContext, evidence: &EvidenceSections) -> Vec<ReportNotice> {
    let mut notices = Vec::new();

    if context.schedule.slots.is_empty() {
        notices.push(ReportNotice::NoScheduledSlots);
    }
    if evidence.overall.observation_counts.usable == 0 {
        notices.push(ReportNotice::NoUsableObservations);
    }
    if evidence.overall.snr.is_none() {
        notices.push(ReportNotice::NoUsableSnrSamples);
    }

    notices
}
