use std::collections::{BTreeMap, BTreeSet};

use antennabench_core::{
    v2::{AdapterDisposition, AdapterInput, AdapterRecordV2},
    AlignedSlot, Band, BundleContents, ObservationKind, ObservationRecord, RecordSource,
    WSPR_NOMINAL_START_OFFSET_SECONDS,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

use crate::{
    summary::{ClassifiedObservation, ObservationDisposition},
    AnalysisBudget, AnalysisError, AnalysisResourceStage, ComparisonSide, ComparisonStratum,
    PairedComparisonAnalysis, PathDirection, ReporterActivityAnalysis, ReporterActivityCensusCycle,
    ReporterActivityCoverage, ReporterActivityCycleRate, ReporterActivityPairedRate,
    ReporterActivityReporter, ReporterActivityUnknownReason, SignalMode,
};

const ACTIVITY_RECORD_TYPE: &str = "wspr_live_activity_census";
const ACTIVITY_SUMMARY_RECORD_TYPE: &str = "wspr_live_activity_census_summary";
const WSPR_LIVE_PROVIDER_ID: &str = "wspr-live";

type StratumKey = (u8, u8, String, u8, String);
type CensusKey = (DateTime<Utc>, u8);

pub(crate) fn analyze_reporter_activity(
    bundle: &BundleContents,
    aligned_slots: &[AlignedSlot],
    observations: &[ClassifiedObservation<'_>],
    comparison: &PairedComparisonAnalysis,
    adapter_records: &[AdapterRecordV2],
    cycle_directions: &BTreeMap<String, PathDirection>,
    budget: &AnalysisBudget<'_>,
) -> Result<ReporterActivityAnalysis, AnalysisError> {
    if adapter_records.is_empty() && cycle_directions.is_empty() {
        return Ok(ReporterActivityAnalysis::default());
    }
    budget.collection(
        AnalysisResourceStage::Compare,
        "reporter_activity_adapter_records",
        adapter_records.len(),
    )?;

    let summaries = adapter_records
        .iter()
        .filter_map(parse_summary)
        .collect::<Vec<_>>();
    let census_rows = adapter_records.iter().filter_map(parse_census_row).fold(
        BTreeMap::<CensusKey, BTreeMap<String, ReporterActivityReporter>>::new(),
        |mut rows, (key, reporter)| {
            rows.entry(key)
                .or_default()
                .entry(reporter.reporter.clone())
                .or_insert(reporter);
            rows
        },
    );

    let mut strata = BTreeMap::<StratumKey, ComparisonStratum>::new();
    let mut heard = BTreeMap::<(StratumKey, String), BTreeSet<String>>::new();
    let mut observed_directions = BTreeMap::<String, BTreeSet<u8>>::new();
    for classified in observations {
        if !matches!(classified.disposition, ObservationDisposition::Usable) {
            continue;
        }
        let Some(slot_id) = classified.assignment.slot_id.as_deref() else {
            continue;
        };
        let Some((direction, remote)) = path_identity(bundle, classified.observation) else {
            continue;
        };
        observed_directions
            .entry(slot_id.to_string())
            .or_default()
            .insert(direction_rank(direction));
        let Some(mode) = classified
            .observation
            .mode
            .as_deref()
            .and_then(SignalMode::normalize)
        else {
            continue;
        };
        let stratum = ComparisonStratum {
            direction,
            band: classified.observation.band,
            mode,
            observation_kind: classified.observation.observation_kind,
            source: classified.observation.meta.source,
        };
        let key = stratum_key(&stratum);
        strata.entry(key.clone()).or_insert(stratum);
        heard
            .entry((key, slot_id.to_string()))
            .or_default()
            .insert(remote);
    }

    let mut slots = aligned_slots.iter().collect::<Vec<_>>();
    slots.sort_by(|left, right| {
        left.sequence_number
            .cmp(&right.sequence_number)
            .then_with(|| left.slot_id.cmp(&right.slot_id))
    });
    let mut census_cycles = Vec::<ReporterActivityCensusCycle>::new();
    let mut census_indices = BTreeMap::<CensusKey, usize>::new();
    let mut cycle_rates = Vec::new();

    for (stratum_key, stratum) in &strata {
        for slot in &slots {
            if slot.band != stratum.band {
                continue;
            }
            let direction = cycle_directions
                .get(slot.slot_id.as_str())
                .copied()
                .or_else(|| {
                    observed_directions
                        .get(slot.slot_id.as_str())
                        .filter(|values| values.len() == 1)
                        .and_then(|values| values.first().copied())
                        .and_then(direction_from_rank)
                });
            if direction != Some(stratum.direction) {
                continue;
            }
            let antenna_label = slot
                .actual_label
                .clone()
                .unwrap_or_else(|| slot.planned_label.clone());
            let timeline = comparison
                .timeline_rows
                .iter()
                .find(|row| row.slot_id == slot.slot_id);
            let block_index = timeline.map(|row| row.block_index);
            let side = timeline.and_then(|row| row.side);
            let canonical_time =
                slot.starts_at - Duration::seconds(WSPR_NOMINAL_START_OFFSET_SECONDS);

            let (coverage, census_cycle_index, active_reporters) =
                if stratum.direction == PathDirection::Receive {
                    (
                        ReporterActivityCoverage::Unknown(
                            ReporterActivityUnknownReason::UnsupportedReceiveDirection,
                        ),
                        None,
                        Vec::new(),
                    )
                } else {
                    let (coverage, summary_record_ids) =
                        coverage_for(canonical_time, slot.band, &summaries);
                    if coverage.is_known() {
                        let key = (canonical_time, band_rank(slot.band));
                        let reporters = census_rows
                            .get(&key)
                            .map(|rows| rows.values().cloned().collect::<Vec<_>>())
                            .unwrap_or_default();
                        let index = *census_indices.entry(key).or_insert_with(|| {
                            let index = census_cycles.len();
                            census_cycles.push(ReporterActivityCensusCycle {
                                cycle_time: canonical_time,
                                band: slot.band,
                                coverage,
                                active_reporters: reporters.clone(),
                                summary_record_ids,
                            });
                            index
                        });
                        (coverage, Some(index), reporters)
                    } else {
                        (coverage, None, Vec::new())
                    }
                };

            let observed = heard
                .get(&(stratum_key.clone(), slot.slot_id.clone()))
                .cloned()
                .unwrap_or_default();
            let active = active_reporters
                .iter()
                .map(|reporter| reporter.reporter.as_str())
                .collect::<BTreeSet<_>>();
            let heard_reporters = observed
                .iter()
                .filter(|reporter| active.contains(reporter.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let active_reporter_count = active_reporters.len();
            let heard_reporter_count = heard_reporters.len();
            cycle_rates.push(ReporterActivityCycleRate {
                stratum: stratum.clone(),
                block_index,
                side,
                slot_id: slot.slot_id.clone(),
                antenna_label,
                cycle_starts_at: slot.starts_at,
                census_cycle_index,
                coverage,
                active_reporter_count,
                heard_reporter_count,
                hearing_rate: rate(heard_reporter_count, active_reporter_count),
                heard_reporters,
            });
        }
    }

    let mut paired_rates = Vec::new();
    for block in comparison
        .blocks
        .iter()
        .filter(|block| block.eligibility == crate::ComparisonBlockEligibility::Eligible)
    {
        for stratum in strata.values().filter(|stratum| stratum.band == block.band) {
            let left = cycle_rates.iter().find(|row| {
                row.stratum == *stratum
                    && row.block_index == Some(block.block_index)
                    && row.side == Some(ComparisonSide::Left)
            });
            let right = cycle_rates.iter().find(|row| {
                row.stratum == *stratum
                    && row.block_index == Some(block.block_index)
                    && row.side == Some(ComparisonSide::Right)
            });
            let (Some(left), Some(right)) = (left, right) else {
                continue;
            };
            let coverage = combined_coverage(left.coverage, right.coverage);
            let (common, left_heard_count, right_heard_count) =
                match (left.census_cycle_index, right.census_cycle_index) {
                    (Some(left_index), Some(right_index)) => {
                        let left_active = reporter_ids(&census_cycles[left_index]);
                        let right_active = reporter_ids(&census_cycles[right_index]);
                        let common = left_active
                            .intersection(&right_active)
                            .copied()
                            .collect::<BTreeSet<_>>();
                        let left_heard = left
                            .heard_reporters
                            .iter()
                            .filter(|reporter| common.contains(reporter.as_str()))
                            .count();
                        let right_heard = right
                            .heard_reporters
                            .iter()
                            .filter(|reporter| common.contains(reporter.as_str()))
                            .count();
                        (common.len(), left_heard, right_heard)
                    }
                    _ => (0, 0, 0),
                };
            paired_rates.push(ReporterActivityPairedRate {
                stratum: stratum.clone(),
                block_index: block.block_index,
                coverage,
                left_slot_id: left.slot_id.clone(),
                right_slot_id: right.slot_id.clone(),
                active_in_both_count: common,
                left_heard_count,
                right_heard_count,
                left_hearing_rate: rate(left_heard_count, common),
                right_hearing_rate: rate(right_heard_count, common),
            });
        }
    }

    Ok(ReporterActivityAnalysis {
        census_cycles,
        cycle_rates,
        paired_rates,
    })
}

#[derive(Debug)]
struct CensusSummary {
    record_id: String,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    bands: Vec<Band>,
    coverage: ReporterActivityCoverage,
}

fn parse_summary(record: &AdapterRecordV2) -> Option<CensusSummary> {
    if record.record_type != ACTIVITY_SUMMARY_RECORD_TYPE
        || !matches!(
            record.disposition,
            AdapterDisposition::Accepted | AdapterDisposition::PartiallyNormalized
        )
        || record.meta.provenance.provider_id.as_str() != WSPR_LIVE_PROVIDER_ID
    {
        return None;
    }
    let AdapterInput::Inline { data, .. } = &record.input else {
        return None;
    };
    let value: Value = serde_json::from_str(data).ok()?;
    if value.get("status").and_then(Value::as_str) == Some("failed") {
        return None;
    }
    let window_start = serde_json::from_value(value.get("window_start")?.clone()).ok()?;
    let window_end = serde_json::from_value(value.get("window_end")?.clone()).ok()?;
    if window_end <= window_start {
        return None;
    }
    let bands = serde_json::from_value(value.get("selected_bands")?.clone()).ok()?;
    let truncated = value.get("truncated")?.as_bool()?;
    let malformed = value
        .pointer("/counts/malformed")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    Some(CensusSummary {
        record_id: record.record_id.clone(),
        window_start,
        window_end,
        bands,
        coverage: if truncated {
            ReporterActivityCoverage::Truncated
        } else if malformed > 0 || record.disposition == AdapterDisposition::PartiallyNormalized {
            ReporterActivityCoverage::Partial
        } else {
            ReporterActivityCoverage::Complete
        },
    })
}

fn parse_census_row(record: &AdapterRecordV2) -> Option<(CensusKey, ReporterActivityReporter)> {
    if record.record_type != ACTIVITY_RECORD_TYPE
        || record.disposition != AdapterDisposition::Accepted
        || record.meta.provenance.provider_id.as_str() != WSPR_LIVE_PROVIDER_ID
    {
        return None;
    }
    let AdapterInput::Inline { data, .. } = &record.input else {
        return None;
    };
    let value: Value = serde_json::from_str(data).ok()?;
    let cycle_time = serde_json::from_value(value.get("cycle_time")?.clone()).ok()?;
    let band: Band = serde_json::from_value(value.get("band")?.clone()).ok()?;
    let reporter = value.get("reporter")?.as_str()?.to_string();
    let reporter_grid = value
        .get("reporter_grid")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some((
        (cycle_time, band_rank(band)),
        ReporterActivityReporter {
            reporter,
            reporter_grid,
            census_record_id: record.record_id.clone(),
        },
    ))
}

fn coverage_for(
    cycle_time: DateTime<Utc>,
    band: Band,
    summaries: &[CensusSummary],
) -> (ReporterActivityCoverage, Vec<String>) {
    let mut covering = summaries
        .iter()
        .filter(|summary| {
            summary.window_start <= cycle_time
                && cycle_time < summary.window_end
                && summary.bands.contains(&band)
        })
        .collect::<Vec<_>>();
    let Some(best_rank) = covering
        .iter()
        .map(|summary| coverage_rank(summary.coverage))
        .max()
    else {
        return (
            ReporterActivityCoverage::Unknown(ReporterActivityUnknownReason::NoCensusCoverage),
            Vec::new(),
        );
    };
    covering.retain(|summary| coverage_rank(summary.coverage) == best_rank);
    covering.sort_by(|left, right| left.record_id.cmp(&right.record_id));
    (
        covering[0].coverage,
        covering
            .into_iter()
            .map(|summary| summary.record_id.clone())
            .collect(),
    )
}

fn combined_coverage(
    left: ReporterActivityCoverage,
    right: ReporterActivityCoverage,
) -> ReporterActivityCoverage {
    match (left, right) {
        (ReporterActivityCoverage::Unknown(reason), _)
        | (_, ReporterActivityCoverage::Unknown(reason)) => {
            ReporterActivityCoverage::Unknown(reason)
        }
        (ReporterActivityCoverage::Truncated, _) | (_, ReporterActivityCoverage::Truncated) => {
            ReporterActivityCoverage::Truncated
        }
        (ReporterActivityCoverage::Partial, _) | (_, ReporterActivityCoverage::Partial) => {
            ReporterActivityCoverage::Partial
        }
        _ => ReporterActivityCoverage::Complete,
    }
}

fn coverage_rank(coverage: ReporterActivityCoverage) -> u8 {
    match coverage {
        ReporterActivityCoverage::Unknown(_) => 0,
        ReporterActivityCoverage::Truncated => 1,
        ReporterActivityCoverage::Partial => 2,
        ReporterActivityCoverage::Complete => 3,
    }
}

fn reporter_ids(cycle: &ReporterActivityCensusCycle) -> BTreeSet<&str> {
    cycle
        .active_reporters
        .iter()
        .map(|reporter| reporter.reporter.as_str())
        .collect()
}

fn rate(heard: usize, active: usize) -> Option<f64> {
    (active > 0).then(|| heard as f64 / active as f64)
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

fn stratum_key(stratum: &ComparisonStratum) -> StratumKey {
    (
        direction_rank(stratum.direction),
        band_rank(stratum.band),
        stratum.mode.as_str().to_string(),
        observation_kind_rank(stratum.observation_kind),
        record_source_key(stratum.source).to_string(),
    )
}

fn direction_rank(direction: PathDirection) -> u8 {
    match direction {
        PathDirection::Transmit => 0,
        PathDirection::Receive => 1,
    }
}

fn direction_from_rank(value: u8) -> Option<PathDirection> {
    match value {
        0 => Some(PathDirection::Transmit),
        1 => Some(PathDirection::Receive),
        _ => None,
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

fn observation_kind_rank(kind: ObservationKind) -> u8 {
    match kind {
        ObservationKind::LocalDecode => 0,
        ObservationKind::PublicReport => 1,
        ObservationKind::ImportedSpot => 2,
    }
}

fn record_source_key(source: RecordSource) -> &'static str {
    match source {
        RecordSource::Operator => "operator",
        RecordSource::WsjtxUdp => "wsjtx_udp",
        RecordSource::WsjtxLog => "wsjtx_log",
        RecordSource::Wsprnet => "wsprnet",
        RecordSource::WsprLive => "wspr_live",
        RecordSource::ImportedFile => "imported_file",
        RecordSource::RigAdapter => "rig_adapter",
        RecordSource::NoaaSwpc => "noaa_swpc",
        RecordSource::Derived => "derived",
    }
}
