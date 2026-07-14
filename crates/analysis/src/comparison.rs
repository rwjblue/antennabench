use std::collections::{BTreeMap, BTreeSet, HashMap};

use antennabench_core::{
    AlignedSlot, Band, BundleContents, ExperimentMode, ObservationKind, ObservationRecord,
    RecordSource,
};

use crate::{
    summary::{ClassifiedObservation, ObservationDisposition},
    ComparisonAvailability, ComparisonBlock, ComparisonBlockEligibility, ComparisonDiagnostics,
    ComparisonOrder, ComparisonSide, ComparisonStratum, ComparisonTimelineRow, DeltaOrientation,
    PairedComparisonAnalysis, PairedObservationRow, PairedPathSummary, PairedStratumSummary,
    PathDirection, PathOverlapRow, SignalMode,
};

type StratumKey = (u8, u8, String, u8, u8);
type GroupKey = (StratumKey, usize, String);

pub(crate) fn analyze_paired_comparison(
    bundle: &BundleContents,
    aligned_slots: &[AlignedSlot],
    observations: &[ClassifiedObservation<'_>],
) -> PairedComparisonAnalysis {
    let labels = scheduled_labels(bundle);
    if bundle.schedule.mode == ExperimentMode::SingleAntennaProfiling {
        return unavailable(ComparisonAvailability::NotApplicable, labels);
    }
    if labels.len() != 2 {
        return unavailable(ComparisonAvailability::UnsupportedComparisonShape, labels);
    }

    let left_label = labels[0].clone();
    let right_label = labels[1].clone();
    let mut slots = aligned_slots.iter().collect::<Vec<_>>();
    slots.sort_by(|left, right| {
        left.sequence_number
            .cmp(&right.sequence_number)
            .then_with(|| left.slot_id.cmp(&right.slot_id))
    });
    let blocks = build_blocks(&slots, &left_label, &right_label);
    let mut diagnostics = ComparisonDiagnostics {
        block_count: blocks.len(),
        eligible_block_count: blocks
            .iter()
            .filter(|block| block.eligibility == ComparisonBlockEligibility::Eligible)
            .count(),
        ..ComparisonDiagnostics::default()
    };
    diagnostics.invalid_block_count = diagnostics.block_count - diagnostics.eligible_block_count;
    for block in &blocks {
        match block.order {
            Some(ComparisonOrder::LeftThenRight) => diagnostics.left_then_right_block_count += 1,
            Some(ComparisonOrder::RightThenLeft) => diagnostics.right_then_left_block_count += 1,
            None => {}
        }
    }

    let mut timeline_rows = build_timeline_rows(&blocks, &left_label, &right_label);
    let slot_locations = timeline_rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.slot_id.clone(), index))
        .collect::<HashMap<_, _>>();
    let block_by_slot = blocks
        .iter()
        .flat_map(|block| {
            std::iter::once((block.first_slot_id.clone(), block.block_index)).chain(
                block
                    .second_slot_id
                    .clone()
                    .map(|slot_id| (slot_id, block.block_index)),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut groups = BTreeMap::<GroupKey, PairGroup<'_>>::new();

    for classified in observations {
        if matches!(classified.disposition, ObservationDisposition::Excluded(_)) {
            diagnostics.excluded_observation_count += 1;
        }
        let Some(slot_id) = classified.assignment.slot_id.as_deref() else {
            continue;
        };
        if let Some(index) = slot_locations.get(slot_id).copied() {
            let timeline = &mut timeline_rows[index];
            timeline.total_observation_count += 1;
            match classified.disposition {
                ObservationDisposition::Usable => timeline.usable_observation_count += 1,
                ObservationDisposition::Excluded(_) => timeline.excluded_observation_count += 1,
            }
        }
        if !matches!(classified.disposition, ObservationDisposition::Usable) {
            continue;
        }
        let Some(block_index) = block_by_slot.get(slot_id).copied() else {
            continue;
        };
        let block = &blocks[block_index];
        if block.eligibility != ComparisonBlockEligibility::Eligible {
            continue;
        }
        let side = match classified.assignment.slot_label.as_deref() {
            Some(label) if label == left_label => ComparisonSide::Left,
            Some(label) if label == right_label => ComparisonSide::Right,
            _ => continue,
        };
        let path = path_identity(bundle, classified.observation);
        if path.is_none() {
            diagnostics.ambiguous_path_count += 1;
            if let Some(index) = slot_locations.get(slot_id).copied() {
                timeline_rows[index].ambiguous_path_count += 1;
            }
        }
        let mode = classified
            .observation
            .mode
            .as_deref()
            .and_then(SignalMode::normalize);
        if mode.is_none() {
            diagnostics.missing_or_invalid_mode_count += 1;
            if let Some(index) = slot_locations.get(slot_id).copied() {
                timeline_rows[index].missing_or_invalid_mode_count += 1;
            }
        }
        let (Some((direction, remote_path)), Some(mode)) = (path, mode) else {
            continue;
        };
        let stratum = ComparisonStratum {
            direction,
            band: classified.observation.band,
            mode,
            observation_kind: classified.observation.observation_kind,
            source: classified.observation.meta.source,
        };
        let key = (stratum_key(&stratum), block_index, remote_path);
        groups
            .entry(key)
            .or_default()
            .add(side, classified.observation);
    }

    let mut paired_rows = Vec::new();
    let mut overlap_accumulators = BTreeMap::<(StratumKey, String), OverlapAccumulator>::new();
    let mut stratum_accumulators = BTreeMap::<StratumKey, StratumAccumulator>::new();

    for ((key, block_index, remote_path), mut group) in groups {
        let stratum = stratum_from_key(&key);
        let block = &blocks[block_index];
        let left = group.left.resolve();
        let right = group.right.resolve();
        mark_quality_timeline(
            &mut timeline_rows,
            &slot_locations,
            group.left.first(),
            left.duplicate_count,
            left.conflict,
        );
        mark_quality_timeline(
            &mut timeline_rows,
            &slot_locations,
            group.right.first(),
            right.duplicate_count,
            right.conflict,
        );
        let duplicate_count = left.duplicate_count + right.duplicate_count;
        let conflict_count = usize::from(left.conflict) + usize::from(right.conflict);
        diagnostics.exact_duplicate_count += duplicate_count;
        diagnostics.conflicting_duplicate_group_count += conflict_count;
        let accumulator = stratum_accumulators.entry(key.clone()).or_default();
        accumulator.exact_duplicate_count += duplicate_count;
        accumulator.conflicting_duplicate_group_count += conflict_count;
        accumulator.blocks.insert(block_index);
        let overlap = overlap_accumulators
            .entry((key, remote_path.clone()))
            .or_default();
        overlap.exact_duplicate_count += duplicate_count;
        overlap.conflicting_duplicate_group_count += conflict_count;
        if left.missing_snr {
            diagnostics.missing_snr_left_count += 1;
            accumulator.missing_snr_left_count += 1;
            overlap.missing_snr_left_count += 1;
        }
        if right.missing_snr {
            diagnostics.missing_snr_right_count += 1;
            accumulator.missing_snr_right_count += 1;
            overlap.missing_snr_right_count += 1;
        }
        if left.finite.is_some() {
            overlap.left_finite_count += 1;
            if right.finite.is_none() && !right.conflict {
                diagnostics.unmatched_left_count += 1;
                accumulator.unmatched_left_count += 1;
                overlap.unmatched_left_count += 1;
            }
        }
        if right.finite.is_some() {
            overlap.right_finite_count += 1;
            if left.finite.is_none() && !left.conflict {
                diagnostics.unmatched_right_count += 1;
                accumulator.unmatched_right_count += 1;
                overlap.unmatched_right_count += 1;
            }
        }
        if left.missing_snr {
            mark_missing_timeline(&mut timeline_rows, &slot_locations, group.left.first());
        }
        if right.missing_snr {
            mark_missing_timeline(&mut timeline_rows, &slot_locations, group.right.first());
        }

        let (Some(left_observation), Some(right_observation)) = (left.finite, right.finite) else {
            continue;
        };
        let left_snr = f64::from(left_observation.snr_db.expect("resolved finite SNR"));
        let right_snr = f64::from(right_observation.snr_db.expect("resolved finite SNR"));
        let order = block.order.expect("eligible block has an order");
        overlap.paired_count += 1;
        accumulator.paired_blocks.insert(block_index);
        accumulator.orders.insert((block_index, order));
        accumulator.deltas.push(right_snr - left_snr);
        paired_rows.push(PairedObservationRow {
            stratum,
            block_index,
            order,
            remote_path,
            left_observation_id: left_observation.observation_id.clone(),
            right_observation_id: right_observation.observation_id.clone(),
            left_slot_id: left_observation
                .slot_id
                .clone()
                .expect("usable observation has slot"),
            right_slot_id: right_observation
                .slot_id
                .clone()
                .expect("usable observation has slot"),
            left_timestamp: left_observation.meta.timestamp,
            right_timestamp: right_observation.meta.timestamp,
            elapsed_seconds: (right_observation.meta.timestamp - left_observation.meta.timestamp)
                .num_seconds()
                .abs(),
            left_snr_db: left_snr,
            right_snr_db: right_snr,
            delta_right_minus_left_db: right_snr - left_snr,
            left_remote_grid: remote_grid(bundle, left_observation),
            right_remote_grid: remote_grid(bundle, right_observation),
            left_distance_km: left_observation.distance_km,
            right_distance_km: right_observation.distance_km,
            left_azimuth_degrees: left_observation.azimuth_degrees,
            right_azimuth_degrees: right_observation.azimuth_degrees,
        });
    }

    paired_rows.sort_by(paired_row_cmp);
    let overlap_rows = overlap_accumulators
        .into_iter()
        .map(|((key, remote_path), row)| row.finish(stratum_from_key(&key), remote_path))
        .collect::<Vec<_>>();
    let path_summaries = build_path_summaries(&paired_rows);
    let strata = build_strata(stratum_accumulators, &path_summaries);
    diagnostics.paired_row_count = paired_rows.len();
    diagnostics.unique_path_count = paired_rows
        .iter()
        .map(|row| row.remote_path.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let availability = if diagnostics.eligible_block_count == 0 {
        ComparisonAvailability::NoEligibleBlocks
    } else if paired_rows.is_empty() {
        ComparisonAvailability::NoMatchedPaths
    } else {
        ComparisonAvailability::DescriptivePairsAvailable
    };

    PairedComparisonAnalysis {
        availability,
        left_label: Some(left_label.clone()),
        right_label: Some(right_label.clone()),
        delta_orientation: Some(DeltaOrientation {
            minuend_label: right_label,
            subtrahend_label: left_label,
        }),
        diagnostics,
        blocks,
        overlap_rows,
        timeline_rows,
        paired_rows,
        path_summaries,
        strata,
    }
}

fn unavailable(
    availability: ComparisonAvailability,
    labels: Vec<String>,
) -> PairedComparisonAnalysis {
    PairedComparisonAnalysis {
        availability,
        left_label: labels.first().cloned(),
        right_label: labels.get(1).cloned(),
        delta_orientation: labels.get(1).zip(labels.first()).map(|(right, left)| {
            DeltaOrientation {
                minuend_label: right.clone(),
                subtrahend_label: left.clone(),
            }
        }),
        diagnostics: ComparisonDiagnostics::default(),
        blocks: Vec::new(),
        overlap_rows: Vec::new(),
        timeline_rows: Vec::new(),
        paired_rows: Vec::new(),
        path_summaries: Vec::new(),
        strata: Vec::new(),
    }
}

fn scheduled_labels(bundle: &BundleContents) -> Vec<String> {
    let mut labels = Vec::new();
    for slot in &bundle.schedule.slots {
        if !labels.iter().any(|label| label == &slot.antenna_label) {
            labels.push(slot.antenna_label.clone());
        }
    }
    labels
}

fn build_blocks(slots: &[&AlignedSlot], left: &str, right: &str) -> Vec<ComparisonBlock> {
    let mut blocks = Vec::new();
    let has_ambiguous_sequence_order = slots
        .windows(2)
        .any(|pair| pair[0].sequence_number == pair[1].sequence_number);
    let mut run_start = 0;
    while run_start < slots.len() {
        let band = slots[run_start].band;
        let mut run_end = run_start + 1;
        while run_end < slots.len() && slots[run_end].band == band {
            run_end += 1;
        }
        for pair in slots[run_start..run_end].chunks(2) {
            let first = pair[0];
            let second = pair.get(1).copied();
            let (order, eligibility) = if has_ambiguous_sequence_order {
                (None, ComparisonBlockEligibility::AmbiguousSequenceOrder)
            } else {
                block_state(first, second, left, right)
            };
            blocks.push(ComparisonBlock {
                block_index: blocks.len(),
                band,
                first_slot_id: first.slot_id.clone(),
                first_sequence_number: first.sequence_number,
                first_starts_at: first.starts_at,
                first_label: first.actual_label.clone(),
                first_status: first.status,
                second_slot_id: second.map(|slot| slot.slot_id.clone()),
                second_sequence_number: second.map(|slot| slot.sequence_number),
                second_starts_at: second.map(|slot| slot.starts_at),
                second_label: second.and_then(|slot| slot.actual_label.clone()),
                second_status: second.map(|slot| slot.status),
                order,
                eligibility,
            });
        }
        run_start = run_end;
    }
    blocks
}

fn block_state(
    first: &AlignedSlot,
    second: Option<&AlignedSlot>,
    left: &str,
    right: &str,
) -> (Option<ComparisonOrder>, ComparisonBlockEligibility) {
    let Some(second) = second else {
        return (None, ComparisonBlockEligibility::IncompleteSameBandRun);
    };
    let (Some(first_label), Some(second_label)) = (
        first.actual_label.as_deref(),
        second.actual_label.as_deref(),
    ) else {
        return (None, ComparisonBlockEligibility::MissingActualLabel);
    };
    if first_label == second_label {
        return (None, ComparisonBlockEligibility::RepeatedLabel);
    }
    match (first_label, second_label) {
        (first, second) if first == left && second == right => (
            Some(ComparisonOrder::LeftThenRight),
            ComparisonBlockEligibility::Eligible,
        ),
        (first, second) if first == right && second == left => (
            Some(ComparisonOrder::RightThenLeft),
            ComparisonBlockEligibility::Eligible,
        ),
        _ => (None, ComparisonBlockEligibility::UnsupportedLabel),
    }
}

fn build_timeline_rows(
    blocks: &[ComparisonBlock],
    left: &str,
    right: &str,
) -> Vec<ComparisonTimelineRow> {
    let mut rows = Vec::new();
    for block in blocks {
        rows.push(timeline_row(
            block,
            block.first_sequence_number,
            block.first_slot_id.clone(),
            block.first_starts_at,
            block.first_label.clone(),
            block.first_status,
            left,
            right,
        ));
        if let (Some(sequence), Some(slot_id), Some(starts_at), Some(status)) = (
            block.second_sequence_number,
            block.second_slot_id.clone(),
            block.second_starts_at,
            block.second_status,
        ) {
            rows.push(timeline_row(
                block,
                sequence,
                slot_id,
                starts_at,
                block.second_label.clone(),
                status,
                left,
                right,
            ));
        }
    }
    rows.sort_by_key(|row| (row.sequence_number, row.slot_id.clone()));
    rows
}

#[allow(clippy::too_many_arguments)]
fn timeline_row(
    block: &ComparisonBlock,
    sequence_number: u32,
    slot_id: String,
    starts_at: chrono::DateTime<chrono::Utc>,
    actual_label: Option<String>,
    status: antennabench_core::AlignedSlotStatus,
    left: &str,
    right: &str,
) -> ComparisonTimelineRow {
    let side = match actual_label.as_deref() {
        Some(label) if label == left => Some(ComparisonSide::Left),
        Some(label) if label == right => Some(ComparisonSide::Right),
        _ => None,
    };
    ComparisonTimelineRow {
        block_index: block.block_index,
        block_eligible: block.eligibility == ComparisonBlockEligibility::Eligible,
        sequence_number,
        slot_id,
        starts_at,
        band: block.band,
        actual_label,
        side,
        status,
        total_observation_count: 0,
        usable_observation_count: 0,
        excluded_observation_count: 0,
        missing_snr_count: 0,
        missing_or_invalid_mode_count: 0,
        ambiguous_path_count: 0,
        exact_duplicate_count: 0,
        conflicting_duplicate_group_count: 0,
    }
}

fn path_identity(
    bundle: &BundleContents,
    observation: &ObservationRecord,
) -> Option<(PathDirection, String)> {
    let local = bundle.station.callsign.trim();
    let reporter = observation.reporter_call.as_deref().map(str::trim);
    let heard = observation.heard_call.as_deref().map(str::trim);
    let local_is_reporter = reporter.is_some_and(|call| call.eq_ignore_ascii_case(local));
    let local_is_heard = heard.is_some_and(|call| call.eq_ignore_ascii_case(local));
    match (local_is_reporter, local_is_heard) {
        (false, true) => reporter
            .filter(|call| !call.is_empty())
            .map(|call| (PathDirection::Transmit, call.to_ascii_uppercase())),
        (true, false) => heard
            .filter(|call| !call.is_empty())
            .map(|call| (PathDirection::Receive, call.to_ascii_uppercase())),
        _ => None,
    }
}

fn remote_grid(bundle: &BundleContents, observation: &ObservationRecord) -> Option<String> {
    let local = bundle.station.callsign.trim();
    if observation
        .heard_call
        .as_deref()
        .is_some_and(|call| call.trim().eq_ignore_ascii_case(local))
    {
        observation.reporter_grid.clone()
    } else if observation
        .reporter_call
        .as_deref()
        .is_some_and(|call| call.trim().eq_ignore_ascii_case(local))
    {
        observation.heard_grid.clone()
    } else {
        None
    }
}

#[derive(Default)]
struct PairGroup<'a> {
    left: ObservationGroup<'a>,
    right: ObservationGroup<'a>,
}

impl<'a> PairGroup<'a> {
    fn add(&mut self, side: ComparisonSide, observation: &'a ObservationRecord) {
        match side {
            ComparisonSide::Left => self.left.observations.push(observation),
            ComparisonSide::Right => self.right.observations.push(observation),
        }
    }
}

#[derive(Default)]
struct ObservationGroup<'a> {
    observations: Vec<&'a ObservationRecord>,
}

impl<'a> ObservationGroup<'a> {
    fn first(&self) -> Option<&'a ObservationRecord> {
        self.observations.first().copied()
    }

    fn resolve(&mut self) -> ResolvedObservation<'a> {
        if self.observations.is_empty() {
            return ResolvedObservation::default();
        }
        self.observations
            .sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
        let first = self.observations[0];
        if !self
            .observations
            .iter()
            .skip(1)
            .all(|observation| observations_are_exact_duplicates(first, observation))
        {
            return ResolvedObservation {
                conflict: true,
                ..ResolvedObservation::default()
            };
        }
        let duplicate_count = self.observations.len() - 1;
        match first.snr_db {
            Some(snr) if snr.is_finite() => ResolvedObservation {
                finite: Some(first),
                duplicate_count,
                ..ResolvedObservation::default()
            },
            _ => ResolvedObservation {
                missing_snr: true,
                duplicate_count,
                ..ResolvedObservation::default()
            },
        }
    }
}

#[derive(Default)]
struct ResolvedObservation<'a> {
    finite: Option<&'a ObservationRecord>,
    missing_snr: bool,
    duplicate_count: usize,
    conflict: bool,
}

fn observations_are_exact_duplicates(left: &ObservationRecord, right: &ObservationRecord) -> bool {
    left.meta.timestamp == right.meta.timestamp
        && left.snr_db.map(f32::to_bits) == right.snr_db.map(f32::to_bits)
        && left.reporter_grid == right.reporter_grid
        && left.heard_grid == right.heard_grid
        && left.distance_km.map(f64::to_bits) == right.distance_km.map(f64::to_bits)
        && left.azimuth_degrees.map(f64::to_bits) == right.azimuth_degrees.map(f64::to_bits)
}

fn mark_missing_timeline(
    timeline: &mut [ComparisonTimelineRow],
    slot_locations: &HashMap<String, usize>,
    observation: Option<&ObservationRecord>,
) {
    if let Some(index) = observation
        .and_then(|observation| observation.slot_id.as_ref())
        .and_then(|slot_id| slot_locations.get(slot_id))
        .copied()
    {
        timeline[index].missing_snr_count += 1;
    }
}

fn mark_quality_timeline(
    timeline: &mut [ComparisonTimelineRow],
    slot_locations: &HashMap<String, usize>,
    observation: Option<&ObservationRecord>,
    duplicate_count: usize,
    conflict: bool,
) {
    if let Some(index) = observation
        .and_then(|observation| observation.slot_id.as_ref())
        .and_then(|slot_id| slot_locations.get(slot_id))
        .copied()
    {
        timeline[index].exact_duplicate_count += duplicate_count;
        timeline[index].conflicting_duplicate_group_count += usize::from(conflict);
    }
}

#[derive(Default)]
struct OverlapAccumulator {
    left_finite_count: usize,
    right_finite_count: usize,
    paired_count: usize,
    unmatched_left_count: usize,
    unmatched_right_count: usize,
    missing_snr_left_count: usize,
    missing_snr_right_count: usize,
    exact_duplicate_count: usize,
    conflicting_duplicate_group_count: usize,
}

impl OverlapAccumulator {
    fn finish(self, stratum: ComparisonStratum, remote_path: String) -> PathOverlapRow {
        PathOverlapRow {
            stratum,
            remote_path,
            left_finite_count: self.left_finite_count,
            right_finite_count: self.right_finite_count,
            paired_count: self.paired_count,
            unmatched_left_count: self.unmatched_left_count,
            unmatched_right_count: self.unmatched_right_count,
            missing_snr_left_count: self.missing_snr_left_count,
            missing_snr_right_count: self.missing_snr_right_count,
            exact_duplicate_count: self.exact_duplicate_count,
            conflicting_duplicate_group_count: self.conflicting_duplicate_group_count,
        }
    }
}

#[derive(Default)]
struct StratumAccumulator {
    blocks: BTreeSet<usize>,
    paired_blocks: BTreeSet<usize>,
    orders: BTreeSet<(usize, ComparisonOrder)>,
    deltas: Vec<f64>,
    unmatched_left_count: usize,
    unmatched_right_count: usize,
    missing_snr_left_count: usize,
    missing_snr_right_count: usize,
    exact_duplicate_count: usize,
    conflicting_duplicate_group_count: usize,
}

fn build_path_summaries(rows: &[PairedObservationRow]) -> Vec<PairedPathSummary> {
    let mut paths = BTreeMap::<(StratumKey, String), Vec<f64>>::new();
    for row in rows {
        paths
            .entry((stratum_key(&row.stratum), row.remote_path.clone()))
            .or_default()
            .push(row.delta_right_minus_left_db);
    }
    paths
        .into_iter()
        .map(|((key, remote_path), mut deltas)| {
            deltas.sort_by(f64::total_cmp);
            PairedPathSummary {
                stratum: stratum_from_key(&key),
                remote_path,
                paired_row_count: deltas.len(),
                median_delta_right_minus_left_db: median(&deltas),
            }
        })
        .collect()
}

fn build_strata(
    accumulators: BTreeMap<StratumKey, StratumAccumulator>,
    paths: &[PairedPathSummary],
) -> Vec<PairedStratumSummary> {
    accumulators
        .into_iter()
        .map(|(key, mut accumulator)| {
            accumulator.deltas.sort_by(f64::total_cmp);
            let mut path_medians = paths
                .iter()
                .filter(|path| stratum_key(&path.stratum) == key)
                .map(|path| path.median_delta_right_minus_left_db)
                .collect::<Vec<_>>();
            path_medians.sort_by(f64::total_cmp);
            PairedStratumSummary {
                stratum: stratum_from_key(&key),
                paired_row_count: accumulator.deltas.len(),
                unique_path_count: path_medians.len(),
                contributing_block_count: accumulator.paired_blocks.len(),
                left_then_right_block_count: accumulator
                    .orders
                    .iter()
                    .filter(|(_, order)| *order == ComparisonOrder::LeftThenRight)
                    .count(),
                right_then_left_block_count: accumulator
                    .orders
                    .iter()
                    .filter(|(_, order)| *order == ComparisonOrder::RightThenLeft)
                    .count(),
                unmatched_left_count: accumulator.unmatched_left_count,
                unmatched_right_count: accumulator.unmatched_right_count,
                missing_snr_left_count: accumulator.missing_snr_left_count,
                missing_snr_right_count: accumulator.missing_snr_right_count,
                exact_duplicate_count: accumulator.exact_duplicate_count,
                conflicting_duplicate_group_count: accumulator.conflicting_duplicate_group_count,
                minimum_delta_right_minus_left_db: accumulator.deltas.first().copied(),
                median_path_delta_right_minus_left_db: (!path_medians.is_empty())
                    .then(|| median(&path_medians)),
                maximum_delta_right_minus_left_db: accumulator.deltas.last().copied(),
            }
        })
        .collect()
}

fn median(values: &[f64]) -> f64 {
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn paired_row_cmp(left: &PairedObservationRow, right: &PairedObservationRow) -> std::cmp::Ordering {
    stratum_key(&left.stratum)
        .cmp(&stratum_key(&right.stratum))
        .then_with(|| left.remote_path.cmp(&right.remote_path))
        .then_with(|| left.block_index.cmp(&right.block_index))
        .then_with(|| left.left_observation_id.cmp(&right.left_observation_id))
        .then_with(|| left.right_observation_id.cmp(&right.right_observation_id))
}

fn stratum_key(stratum: &ComparisonStratum) -> StratumKey {
    (
        match stratum.direction {
            PathDirection::Transmit => 0,
            PathDirection::Receive => 1,
        },
        band_rank(stratum.band),
        stratum.mode.as_str().to_string(),
        match stratum.observation_kind {
            ObservationKind::LocalDecode => 0,
            ObservationKind::PublicReport => 1,
            ObservationKind::ImportedSpot => 2,
        },
        source_rank(stratum.source),
    )
}

fn stratum_from_key(key: &StratumKey) -> ComparisonStratum {
    ComparisonStratum {
        direction: if key.0 == 0 {
            PathDirection::Transmit
        } else {
            PathDirection::Receive
        },
        band: band_from_rank(key.1),
        mode: SignalMode::normalize(&key.2).expect("stratum keys contain normalized modes"),
        observation_kind: match key.3 {
            0 => ObservationKind::LocalDecode,
            1 => ObservationKind::PublicReport,
            _ => ObservationKind::ImportedSpot,
        },
        source: source_from_rank(key.4),
    }
}

fn band_rank(band: Band) -> u8 {
    match band {
        Band::M160 => 0,
        Band::M80 => 1,
        Band::M60 => 2,
        Band::M40 => 3,
        Band::M30 => 4,
        Band::M20 => 5,
        Band::M17 => 6,
        Band::M15 => 7,
        Band::M12 => 8,
        Band::M10 => 9,
        Band::M6 => 10,
        Band::M2 => 11,
    }
}

fn band_from_rank(rank: u8) -> Band {
    match rank {
        0 => Band::M160,
        1 => Band::M80,
        2 => Band::M60,
        3 => Band::M40,
        4 => Band::M30,
        5 => Band::M20,
        6 => Band::M17,
        7 => Band::M15,
        8 => Band::M12,
        9 => Band::M10,
        10 => Band::M6,
        _ => Band::M2,
    }
}

fn source_rank(source: RecordSource) -> u8 {
    match source {
        RecordSource::Operator => 0,
        RecordSource::WsjtxUdp => 1,
        RecordSource::WsjtxLog => 2,
        RecordSource::Wsprnet => 3,
        RecordSource::WsprLive => 4,
        RecordSource::ImportedFile => 5,
        RecordSource::RigAdapter => 6,
        RecordSource::NoaaSwpc => 7,
        RecordSource::Derived => 8,
    }
}

fn source_from_rank(rank: u8) -> RecordSource {
    match rank {
        0 => RecordSource::Operator,
        1 => RecordSource::WsjtxUdp,
        2 => RecordSource::WsjtxLog,
        3 => RecordSource::Wsprnet,
        4 => RecordSource::WsprLive,
        5 => RecordSource::ImportedFile,
        6 => RecordSource::RigAdapter,
        7 => RecordSource::NoaaSwpc,
        _ => RecordSource::Derived,
    }
}
