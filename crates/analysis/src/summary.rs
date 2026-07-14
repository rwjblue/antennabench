use std::collections::{BTreeMap, HashMap, HashSet};

use antennabench_core::{
    align_schedule_slots, codes, validate_bundle_report, AlignedSlot, Band, BundleContents,
    BundleDiagnostic, BundleDiagnosticCategory, BundleDiagnosticLocation, BundleRecordKind,
    BundleValidationError, BundleValidationProfile, BundleValidationReport, ObservationKind,
    ObservationRecord, ObservationSlotAssignment, SlotAlignmentPolicy, SlotAssignmentReason,
};

use crate::{
    comparison::analyze_paired_comparison, AnalysisBudget, AnalysisCancellationToken,
    AnalysisError, AnalysisResourceLimits, AnalysisResourceStage, AnalysisSummary,
    AntennaEvidenceSummary, BandEvidenceSummary, EligibilityExclusionCategory,
    EligibilityExclusionCount, EligibilityScope, EvidenceEligibility, EvidenceQuality,
    EvidenceSummary, ExclusionCount, ObservationCounts, ObservationExclusionReason,
    ObservationKindCount, SlotEvidenceSummary, SnrStatistics, ANALYSIS_RESOURCE_LIMITS,
};

const MINIMUM_USABLE_CONFIDENCE: f32 = 0.70;
const EXCLUSION_REASON_COUNT: usize = 12;
const OBSERVATION_KIND_COUNT: usize = 3;

const BANDS: [Band; 12] = [
    Band::M160,
    Band::M80,
    Band::M60,
    Band::M40,
    Band::M30,
    Band::M20,
    Band::M17,
    Band::M15,
    Band::M12,
    Band::M10,
    Band::M6,
    Band::M2,
];

const EXCLUSION_REASONS: [ObservationExclusionReason; EXCLUSION_REASON_COUNT] = [
    ObservationExclusionReason::GuardTime,
    ObservationExclusionReason::NearBoundary,
    ObservationExclusionReason::BeforeObservedSwitch,
    ObservationExclusionReason::MissedSlot,
    ObservationExclusionReason::BadSlot,
    ObservationExclusionReason::BandMismatch,
    ObservationExclusionReason::OutsideSchedule,
    ObservationExclusionReason::MissingEvidence,
    ObservationExclusionReason::MalformedEvidence,
    ObservationExclusionReason::ContradictoryEvidence,
    ObservationExclusionReason::UnsupportedEvidence,
    ObservationExclusionReason::DuplicateEvidence,
];

const OBSERVATION_KINDS: [ObservationKind; OBSERVATION_KIND_COUNT] = [
    ObservationKind::LocalDecode,
    ObservationKind::PublicReport,
    ObservationKind::ImportedSpot,
];

pub fn summarize_bundle(bundle: &BundleContents) -> Result<AnalysisSummary, AnalysisError> {
    let validation = validate_bundle_report(bundle);
    summarize_bundle_with_report(bundle, &validation)
}

pub fn summarize_bundle_with_report(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
) -> Result<AnalysisSummary, AnalysisError> {
    summarize_bundle_with_resources(
        bundle,
        validation,
        ANALYSIS_RESOURCE_LIMITS,
        &AnalysisCancellationToken::default(),
    )
}

pub fn summarize_bundle_with_resources(
    bundle: &BundleContents,
    validation: &BundleValidationReport,
    limits: AnalysisResourceLimits,
    cancellation: &AnalysisCancellationToken,
) -> Result<AnalysisSummary, AnalysisError> {
    let budget = AnalysisBudget::new(limits, cancellation);
    if validation.diagnostics().iter().any(is_fatal_for_analysis) {
        return Err(BundleValidationError::from_report(validation.clone()).into());
    }
    let input_entries = bundle.schedule.slots.len()
        + bundle.events.len()
        + bundle.observations.len()
        + bundle.antennas.antennas.len();
    for (role, entries) in [
        ("schedule_slots", bundle.schedule.slots.len()),
        ("operator_events", bundle.events.len()),
        ("observations", bundle.observations.len()),
        ("antennas", bundle.antennas.antennas.len()),
    ] {
        budget.collection(AnalysisResourceStage::Plan, role, entries)?;
    }
    budget.live(
        AnalysisResourceStage::Plan,
        "analysis_projection",
        input_entries as u64,
    )?;
    let plan = EligibilityPlan::from_report(validation);
    let plan_entries = plan.excluded_observations.len()
        + plan.excluded_slots.len()
        + plan.excluded_events.len()
        + plan.eligibility.exclusions.len();
    budget.collection(
        AnalysisResourceStage::Plan,
        "eligibility_plan",
        plan_entries,
    )?;
    budget.live(
        AnalysisResourceStage::Plan,
        "projection_and_eligibility",
        (input_entries + plan_entries) as u64,
    )?;
    let mut analysis_bundle = bundle.clone();
    sanitize_analysis_fields(&mut analysis_bundle, validation);
    analysis_bundle.schedule.slots = bundle
        .schedule
        .slots
        .iter()
        .enumerate()
        .filter(|(index, _)| !plan.excluded_slots.contains(index))
        .map(|(_, slot)| slot.clone())
        .collect();
    let retained_slot_ids = analysis_bundle
        .schedule
        .slots
        .iter()
        .map(|slot| slot.slot_id.as_str())
        .collect::<HashSet<_>>();
    analysis_bundle.events = bundle
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            !plan.excluded_events.contains(index)
                && event
                    .slot_id
                    .as_deref()
                    .is_none_or(|slot_id| retained_slot_ids.contains(slot_id))
        })
        .map(|(_, event)| event.clone())
        .collect();

    let alignment = align_schedule_slots(
        &analysis_bundle.schedule,
        &analysis_bundle.events,
        &analysis_bundle.observations,
        SlotAlignmentPolicy::default(),
    );
    budget.collection(
        AnalysisResourceStage::Align,
        "aligned_slots",
        alignment.slots.len(),
    )?;
    budget.collection(
        AnalysisResourceStage::Align,
        "observation_assignments",
        alignment.observation_assignments.len(),
    )?;
    let alignment_entries = alignment.slots.len() + alignment.observation_assignments.len();
    budget.live(
        AnalysisResourceStage::Align,
        "projection_plan_alignment",
        (input_entries + plan_entries + alignment_entries) as u64,
    )?;
    budget.collection(
        AnalysisResourceStage::Aggregate,
        "classified_observations",
        analysis_bundle.observations.len(),
    )?;
    budget.live(
        AnalysisResourceStage::Aggregate,
        "analysis_classification",
        (input_entries + plan_entries + alignment_entries + analysis_bundle.observations.len())
            as u64,
    )?;
    let mut observations = Vec::with_capacity(analysis_bundle.observations.len());
    for (index, (observation, assignment)) in analysis_bundle
        .observations
        .iter()
        .zip(&alignment.observation_assignments)
        .enumerate()
    {
        budget.checkpoint(
            AnalysisResourceStage::Aggregate,
            "classify_observations",
            index,
        )?;
        debug_assert_eq!(observation.observation_id, assignment.observation_id);
        observations.push(ClassifiedObservation {
            observation,
            assignment,
            disposition: plan
                .excluded_observations
                .get(&index)
                .copied()
                .map(ObservationDisposition::Excluded)
                .unwrap_or_else(|| classify_assignment(assignment)),
        });
    }

    budget.cancelled(AnalysisResourceStage::Aggregate, "overall_evidence")?;
    let overall = aggregate(observations.iter().copied(), &budget, "overall_evidence")?.finish();
    budget.collection(
        AnalysisResourceStage::Aggregate,
        "antenna_summaries",
        bundle.antennas.antennas.len(),
    )?;
    let antennas = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| -> Result<_, AnalysisError> {
            let aggregate = aggregate(
                observations.iter().copied().filter(|classified| {
                    classified.assignment.slot_label.as_deref() == Some(antenna.label.as_str())
                }),
                &budget,
                "antenna_evidence",
            )?;
            let contributing_slot_count = aggregate.contributing_slots.len();
            Ok(AntennaEvidenceSummary {
                antenna_label: antenna.label.clone(),
                contributing_slot_count,
                evidence_quality: evidence_quality(aggregate.usable, contributing_slot_count),
                evidence: aggregate.finish(),
            })
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    let bands = BANDS
        .iter()
        .copied()
        .filter(|band| {
            analysis_bundle
                .schedule
                .slots
                .iter()
                .any(|slot| slot.band == *band)
                || analysis_bundle
                    .observations
                    .iter()
                    .any(|observation| observation.band == *band)
        })
        .map(|band| -> Result<_, AnalysisError> {
            Ok(BandEvidenceSummary {
                band,
                evidence: aggregate(
                    observations
                        .iter()
                        .copied()
                        .filter(|classified| classified.observation.band == band),
                    &budget,
                    "band_evidence",
                )?
                .finish(),
            })
        })
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    budget.collection(
        AnalysisResourceStage::Aggregate,
        "slot_summaries",
        alignment.slots.len(),
    )?;
    let slots = alignment
        .slots
        .iter()
        .map(|slot| summarize_slot(slot, &observations, &budget))
        .collect::<Result<Vec<_>, AnalysisError>>()?;
    let evidence_quality = session_evidence_quality(&analysis_bundle, &antennas);
    let comparison_upper_bound = observations
        .len()
        .saturating_mul(5)
        .saturating_add(alignment.slots.len().saturating_mul(2));
    budget.live(
        AnalysisResourceStage::Compare,
        "comparison_intermediates",
        (input_entries
            + plan_entries
            + alignment_entries
            + observations.len()
            + comparison_upper_bound) as u64,
    )?;
    budget.cancelled(AnalysisResourceStage::Compare, "paired_comparison")?;
    let comparison =
        analyze_paired_comparison(&analysis_bundle, &alignment.slots, &observations, &budget)?;
    for (role, entries) in [
        ("comparison_blocks", comparison.blocks.len()),
        ("comparison_overlap_rows", comparison.overlap_rows.len()),
        ("comparison_timeline_rows", comparison.timeline_rows.len()),
        ("comparison_paired_rows", comparison.paired_rows.len()),
        ("comparison_path_summaries", comparison.path_summaries.len()),
        ("comparison_strata", comparison.strata.len()),
    ] {
        budget.collection(AnalysisResourceStage::Compare, role, entries)?;
    }

    Ok(AnalysisSummary {
        session_id: bundle.manifest.session_id.clone(),
        evidence_quality,
        overall,
        antennas,
        bands,
        slots,
        comparison,
        eligibility: plan.eligibility,
    })
}

struct EligibilityPlan {
    excluded_observations: HashMap<usize, ObservationExclusionReason>,
    excluded_slots: HashSet<usize>,
    excluded_events: HashSet<usize>,
    eligibility: EvidenceEligibility,
}

impl EligibilityPlan {
    fn from_report(report: &BundleValidationReport) -> Self {
        let mut excluded_observations = HashMap::new();
        let mut excluded_slots = HashSet::new();
        let mut excluded_events = HashSet::new();
        let mut counts =
            BTreeMap::<(String, EligibilityExclusionCategory, EligibilityScope), usize>::new();

        for diagnostic in report.diagnostics().iter().filter(|diagnostic| {
            diagnostic.blocks(BundleValidationProfile::Analysis)
                || diagnostic.code == codes::ALIGNMENT_ANNOTATION_MISMATCH
                || is_non_finite_snr(diagnostic)
        }) {
            let category = eligibility_category(diagnostic);
            let scope = eligibility_scope(&diagnostic.location);
            *counts
                .entry((diagnostic.code.clone(), category, scope))
                .or_default() += 1;
            for location in
                std::iter::once(&diagnostic.location).chain(diagnostic.related_locations.iter())
            {
                match (location.record_kind, location.record_index) {
                    (Some(BundleRecordKind::Observation), Some(index))
                        if observation_diagnostic_excludes(diagnostic) =>
                    {
                        let reason = observation_reason(category);
                        excluded_observations
                            .entry(index)
                            .and_modify(|existing| {
                                if observation_reason_rank(reason)
                                    > observation_reason_rank(*existing)
                                {
                                    *existing = reason;
                                }
                            })
                            .or_insert(reason);
                    }
                    (Some(BundleRecordKind::Slot), Some(index)) => {
                        excluded_slots.insert(index);
                    }
                    (Some(BundleRecordKind::OperatorEvent), Some(index)) => {
                        excluded_events.insert(index);
                    }
                    _ => {}
                }
            }
        }

        Self {
            excluded_observations,
            excluded_slots,
            excluded_events,
            eligibility: EvidenceEligibility {
                exclusions: counts
                    .into_iter()
                    .map(
                        |((code, category, scope), count)| EligibilityExclusionCount {
                            code,
                            category,
                            scope,
                            count,
                        },
                    )
                    .collect(),
            },
        }
    }
}

fn observation_diagnostic_excludes(diagnostic: &BundleDiagnostic) -> bool {
    matches!(
        diagnostic.code.as_str(),
        codes::DUPLICATE_ID
            | codes::UNKNOWN_OBSERVATION_SLOT
            | codes::INVALID_SLOT_CONFIDENCE
            | codes::ALIGNMENT_ANNOTATION_MISMATCH
            | codes::SESSION_ID_MISMATCH
            | codes::UNSUPPORTED_SCHEMA_VERSION
    ) || is_non_finite_snr(diagnostic)
}

fn sanitize_analysis_fields(bundle: &mut BundleContents, report: &BundleValidationReport) {
    for diagnostic in report.diagnostics() {
        let Some(index) = diagnostic.location.record_index else {
            continue;
        };
        if diagnostic.location.record_kind != Some(BundleRecordKind::Observation) {
            continue;
        }
        let Some(observation) = bundle.observations.get_mut(index) else {
            continue;
        };
        match diagnostic.location.field_path.as_deref() {
            Some("/distance_km") => observation.distance_km = None,
            Some("/azimuth_degrees") => observation.azimuth_degrees = None,
            Some("/power_watts") => observation.power_watts = None,
            Some("/frequency_hz") => observation.frequency_hz = None,
            Some("/drift_hz_per_minute") => observation.drift_hz_per_minute = None,
            _ => {}
        }
    }
}

fn is_fatal_for_analysis(diagnostic: &BundleDiagnostic) -> bool {
    if !diagnostic.blocks(BundleValidationProfile::Analysis) {
        return false;
    }
    if diagnostic.code == codes::DUPLICATE_ANTENNA_LABEL
        || matches!(
            diagnostic.code.as_str(),
            codes::V2_CHECKPOINT_MISMATCH
                | codes::V2_ADAPTER_LINK
                | codes::V2_ATTACHMENT
                | codes::V2_MUTATION
        )
    {
        return true;
    }
    if diagnostic.category == BundleDiagnosticCategory::Wire {
        return diagnostic.location.record_kind.is_none();
    }
    if diagnostic.category == BundleDiagnosticCategory::Structural {
        return !matches!(
            diagnostic.location.record_kind,
            Some(BundleRecordKind::Observation)
                | Some(BundleRecordKind::Slot)
                | Some(BundleRecordKind::OperatorEvent)
                | Some(BundleRecordKind::WsjtXRecord)
                | Some(BundleRecordKind::RigRecord)
                | Some(BundleRecordKind::PropagationRecord)
        );
    }
    false
}

fn is_non_finite_snr(diagnostic: &BundleDiagnostic) -> bool {
    diagnostic.code == codes::NON_FINITE_NUMBER
        && diagnostic.location.record_kind == Some(BundleRecordKind::Observation)
        && diagnostic.location.field_path.as_deref() == Some("/snr_db")
}

fn eligibility_scope(location: &BundleDiagnosticLocation) -> EligibilityScope {
    match location.record_kind {
        Some(BundleRecordKind::Observation) => EligibilityScope::Observation,
        Some(BundleRecordKind::Slot) => EligibilityScope::Slot,
        _ => EligibilityScope::Field,
    }
}

fn eligibility_category(diagnostic: &BundleDiagnostic) -> EligibilityExclusionCategory {
    match diagnostic.code.as_str() {
        codes::DUPLICATE_MEMBER
        | codes::DUPLICATE_RAW_MEMBER
        | codes::DUPLICATE_ID
        | codes::DUPLICATE_ANTENNA_LABEL
        | codes::DUPLICATE_SEQUENCE_NUMBER => EligibilityExclusionCategory::Duplicate,
        codes::UNKNOWN_ANTENNA_LABEL
        | codes::UNKNOWN_EVENT_SLOT
        | codes::UNKNOWN_OBSERVATION_SLOT
        | codes::EMPTY_IDENTITY
        | codes::EMPTY_SCHEDULE => EligibilityExclusionCategory::Missing,
        codes::UNSUPPORTED_SCHEMA_VERSION | codes::EXPERIMENT_SHAPE_MISMATCH => {
            EligibilityExclusionCategory::Unsupported
        }
        codes::SESSION_ID_MISMATCH
        | codes::SLOT_WINDOW_OUT_OF_ORDER
        | codes::SLOT_WINDOW_OVERLAP
        | codes::SLOT_SEQUENCE_OUT_OF_ORDER
        | codes::ALIGNMENT_ANNOTATION_MISMATCH
        | codes::V2_CHECKPOINT_MISMATCH
        | codes::V2_ADAPTER_LINK
        | codes::V2_ATTACHMENT
        | codes::V2_MUTATION => EligibilityExclusionCategory::Contradictory,
        codes::INVALID_JSON
        | codes::INVALID_SLOT_CONFIDENCE
        | codes::INVALID_IDENTITY
        | codes::INVALID_REQUIRED_TEXT
        | codes::INVALID_ANTENNA_LABEL
        | codes::INVALID_SLOT_DURATION
        | codes::INVALID_SLOT_GUARD
        | codes::NON_FINITE_NUMBER
        | codes::INVALID_RANGE => EligibilityExclusionCategory::Malformed,
        _ => EligibilityExclusionCategory::Unsupported,
    }
}

fn observation_reason(category: EligibilityExclusionCategory) -> ObservationExclusionReason {
    match category {
        EligibilityExclusionCategory::Missing => ObservationExclusionReason::MissingEvidence,
        EligibilityExclusionCategory::Malformed => ObservationExclusionReason::MalformedEvidence,
        EligibilityExclusionCategory::Contradictory => {
            ObservationExclusionReason::ContradictoryEvidence
        }
        EligibilityExclusionCategory::Unsupported => {
            ObservationExclusionReason::UnsupportedEvidence
        }
        EligibilityExclusionCategory::Duplicate => ObservationExclusionReason::DuplicateEvidence,
        EligibilityExclusionCategory::DeliberatelyExcluded => {
            ObservationExclusionReason::ContradictoryEvidence
        }
    }
}

fn observation_reason_rank(reason: ObservationExclusionReason) -> u8 {
    match reason {
        ObservationExclusionReason::DuplicateEvidence => 5,
        ObservationExclusionReason::UnsupportedEvidence => 4,
        ObservationExclusionReason::ContradictoryEvidence => 3,
        ObservationExclusionReason::MalformedEvidence => 2,
        ObservationExclusionReason::MissingEvidence => 1,
        _ => 0,
    }
}

fn summarize_slot(
    slot: &AlignedSlot,
    observations: &[ClassifiedObservation<'_>],
    budget: &AnalysisBudget<'_>,
) -> Result<SlotEvidenceSummary, AnalysisError> {
    Ok(SlotEvidenceSummary {
        slot_id: slot.slot_id.clone(),
        sequence_number: slot.sequence_number,
        band: slot.band,
        planned_label: slot.planned_label.clone(),
        actual_label: slot.actual_label.clone(),
        status: slot.status,
        evidence: aggregate(
            observations.iter().copied().filter(|classified| {
                classified.assignment.slot_id.as_deref() == Some(slot.slot_id.as_str())
            }),
            budget,
            "slot_evidence",
        )?
        .finish(),
    })
}

fn session_evidence_quality(
    bundle: &BundleContents,
    antennas: &[AntennaEvidenceSummary],
) -> EvidenceQuality {
    let scheduled_labels = bundle
        .schedule
        .slots
        .iter()
        .map(|slot| slot.antenna_label.as_str())
        .collect::<HashSet<_>>();

    antennas
        .iter()
        .filter(|antenna| scheduled_labels.contains(antenna.antenna_label.as_str()))
        .map(|antenna| antenna.evidence_quality)
        .min_by_key(|quality| quality_rank(*quality))
        .unwrap_or(EvidenceQuality::Insufficient)
}

fn quality_rank(quality: EvidenceQuality) -> u8 {
    match quality {
        EvidenceQuality::Insufficient => 0,
        EvidenceQuality::Weak => 1,
        EvidenceQuality::Moderate => 2,
    }
}

fn evidence_quality(usable: usize, contributing_slots: usize) -> EvidenceQuality {
    if usable >= 5 && contributing_slots >= 3 {
        EvidenceQuality::Moderate
    } else if usable >= 2 && contributing_slots >= 2 {
        EvidenceQuality::Weak
    } else {
        EvidenceQuality::Insufficient
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ClassifiedObservation<'a> {
    pub(crate) observation: &'a ObservationRecord,
    pub(crate) assignment: &'a ObservationSlotAssignment,
    pub(crate) disposition: ObservationDisposition,
}

#[derive(Clone, Copy)]
pub(crate) enum ObservationDisposition {
    Usable,
    Excluded(ObservationExclusionReason),
}

fn classify_assignment(assignment: &ObservationSlotAssignment) -> ObservationDisposition {
    match assignment.reason {
        SlotAssignmentReason::Interior | SlotAssignmentReason::LateSwitch
            if assignment.slot_id.is_some()
                && assignment.slot_label.is_some()
                && assignment.confidence >= MINIMUM_USABLE_CONFIDENCE =>
        {
            ObservationDisposition::Usable
        }
        SlotAssignmentReason::GuardTime => {
            ObservationDisposition::Excluded(ObservationExclusionReason::GuardTime)
        }
        SlotAssignmentReason::NearBoundary => {
            ObservationDisposition::Excluded(ObservationExclusionReason::NearBoundary)
        }
        SlotAssignmentReason::BeforeObservedSwitch => {
            ObservationDisposition::Excluded(ObservationExclusionReason::BeforeObservedSwitch)
        }
        SlotAssignmentReason::MissedSlot => {
            ObservationDisposition::Excluded(ObservationExclusionReason::MissedSlot)
        }
        SlotAssignmentReason::BadSlot => {
            ObservationDisposition::Excluded(ObservationExclusionReason::BadSlot)
        }
        SlotAssignmentReason::BandMismatch => {
            ObservationDisposition::Excluded(ObservationExclusionReason::BandMismatch)
        }
        SlotAssignmentReason::OutsideSchedule => {
            ObservationDisposition::Excluded(ObservationExclusionReason::OutsideSchedule)
        }
        SlotAssignmentReason::Interior | SlotAssignmentReason::LateSwitch => {
            unreachable!("default schedule alignment always labels usable assignment reasons")
        }
    }
}

#[derive(Default)]
struct EvidenceAggregate {
    total: usize,
    usable: usize,
    exclusions: [usize; EXCLUSION_REASON_COUNT],
    usable_kinds: [usize; OBSERVATION_KIND_COUNT],
    snr_samples: Vec<f64>,
    contributing_slots: HashSet<String>,
}

impl EvidenceAggregate {
    fn add(&mut self, classified: ClassifiedObservation<'_>) {
        self.total += 1;

        match classified.disposition {
            ObservationDisposition::Usable => {
                self.usable += 1;
                self.usable_kinds
                    [observation_kind_index(classified.observation.observation_kind)] += 1;
                if let Some(snr) = classified.observation.snr_db {
                    self.snr_samples.push(f64::from(snr));
                }
                if let Some(slot_id) = &classified.assignment.slot_id {
                    self.contributing_slots.insert(slot_id.clone());
                }
            }
            ObservationDisposition::Excluded(reason) => {
                self.exclusions[exclusion_reason_index(reason)] += 1;
            }
        }
    }

    fn finish(mut self) -> EvidenceSummary {
        self.snr_samples.sort_by(f64::total_cmp);

        EvidenceSummary {
            observation_counts: ObservationCounts {
                total: self.total,
                usable: self.usable,
                excluded: self.total - self.usable,
            },
            exclusions: EXCLUSION_REASONS
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, reason)| {
                    let count = self.exclusions[index];
                    (count > 0).then_some(ExclusionCount { reason, count })
                })
                .collect(),
            usable_observation_kinds: OBSERVATION_KINDS
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(index, kind)| {
                    let count = self.usable_kinds[index];
                    (count > 0).then_some(ObservationKindCount { kind, count })
                })
                .collect(),
            snr: snr_statistics(&self.snr_samples),
        }
    }
}

fn aggregate<'a>(
    observations: impl IntoIterator<Item = ClassifiedObservation<'a>>,
    budget: &AnalysisBudget<'_>,
    role: &'static str,
) -> Result<EvidenceAggregate, AnalysisError> {
    let mut aggregate = EvidenceAggregate::default();
    for (index, observation) in observations.into_iter().enumerate() {
        budget.checkpoint(AnalysisResourceStage::Aggregate, role, index)?;
        aggregate.add(observation);
    }
    Ok(aggregate)
}

fn snr_statistics(samples: &[f64]) -> Option<SnrStatistics> {
    if samples.is_empty() {
        return None;
    }

    let middle = samples.len() / 2;
    let median_db = if samples.len().is_multiple_of(2) {
        (samples[middle - 1] + samples[middle]) / 2.0
    } else {
        samples[middle]
    };

    Some(SnrStatistics {
        sample_count: samples.len(),
        min_db: samples[0],
        median_db,
        mean_db: samples.iter().sum::<f64>() / samples.len() as f64,
        max_db: samples[samples.len() - 1],
    })
}

fn exclusion_reason_index(reason: ObservationExclusionReason) -> usize {
    match reason {
        ObservationExclusionReason::GuardTime => 0,
        ObservationExclusionReason::NearBoundary => 1,
        ObservationExclusionReason::BeforeObservedSwitch => 2,
        ObservationExclusionReason::MissedSlot => 3,
        ObservationExclusionReason::BadSlot => 4,
        ObservationExclusionReason::BandMismatch => 5,
        ObservationExclusionReason::OutsideSchedule => 6,
        ObservationExclusionReason::MissingEvidence => 7,
        ObservationExclusionReason::MalformedEvidence => 8,
        ObservationExclusionReason::ContradictoryEvidence => 9,
        ObservationExclusionReason::UnsupportedEvidence => 10,
        ObservationExclusionReason::DuplicateEvidence => 11,
    }
}

fn observation_kind_index(kind: ObservationKind) -> usize {
    match kind {
        ObservationKind::LocalDecode => 0,
        ObservationKind::PublicReport => 1,
        ObservationKind::ImportedSpot => 2,
    }
}
