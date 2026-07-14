use std::collections::HashSet;

use antennabench_core::{
    align_schedule_slots, validate_bundle, AlignedSlot, Band, BundleContents, ObservationKind,
    ObservationRecord, ObservationSlotAssignment, SlotAlignmentPolicy, SlotAssignmentReason,
};

use crate::{
    comparison::analyze_paired_comparison, AnalysisError, AnalysisSummary, AntennaEvidenceSummary,
    BandEvidenceSummary, EvidenceQuality, EvidenceSummary, ExclusionCount, ObservationCounts,
    ObservationExclusionReason, ObservationKindCount, SlotEvidenceSummary, SnrStatistics,
};

const MINIMUM_USABLE_CONFIDENCE: f32 = 0.70;
const EXCLUSION_REASON_COUNT: usize = 7;
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
];

const OBSERVATION_KINDS: [ObservationKind; OBSERVATION_KIND_COUNT] = [
    ObservationKind::LocalDecode,
    ObservationKind::PublicReport,
    ObservationKind::ImportedSpot,
];

pub fn summarize_bundle(bundle: &BundleContents) -> Result<AnalysisSummary, AnalysisError> {
    validate_bundle(bundle)?;
    reject_non_finite_snr(bundle)?;

    let alignment = align_schedule_slots(
        &bundle.schedule,
        &bundle.events,
        &bundle.observations,
        SlotAlignmentPolicy::default(),
    );
    let observations = bundle
        .observations
        .iter()
        .zip(&alignment.observation_assignments)
        .map(|(observation, assignment)| {
            debug_assert_eq!(observation.observation_id, assignment.observation_id);
            ClassifiedObservation {
                observation,
                assignment,
                disposition: classify_assignment(assignment),
            }
        })
        .collect::<Vec<_>>();

    let overall = aggregate(observations.iter().copied()).finish();
    let antennas = bundle
        .antennas
        .antennas
        .iter()
        .map(|antenna| {
            let aggregate = aggregate(observations.iter().copied().filter(|classified| {
                classified.assignment.slot_label.as_deref() == Some(antenna.label.as_str())
            }));
            let contributing_slot_count = aggregate.contributing_slots.len();
            AntennaEvidenceSummary {
                antenna_label: antenna.label.clone(),
                contributing_slot_count,
                evidence_quality: evidence_quality(aggregate.usable, contributing_slot_count),
                evidence: aggregate.finish(),
            }
        })
        .collect::<Vec<_>>();
    let bands = BANDS
        .iter()
        .copied()
        .filter(|band| {
            bundle.schedule.slots.iter().any(|slot| slot.band == *band)
                || bundle
                    .observations
                    .iter()
                    .any(|observation| observation.band == *band)
        })
        .map(|band| BandEvidenceSummary {
            band,
            evidence: aggregate(
                observations
                    .iter()
                    .copied()
                    .filter(|classified| classified.observation.band == band),
            )
            .finish(),
        })
        .collect();
    let slots = alignment
        .slots
        .iter()
        .map(|slot| summarize_slot(slot, &observations))
        .collect();
    let evidence_quality = session_evidence_quality(bundle, &antennas);
    let comparison = analyze_paired_comparison(bundle, &alignment.slots, &observations);

    Ok(AnalysisSummary {
        session_id: bundle.manifest.session_id.clone(),
        evidence_quality,
        overall,
        antennas,
        bands,
        slots,
        comparison,
    })
}

fn reject_non_finite_snr(bundle: &BundleContents) -> Result<(), AnalysisError> {
    for observation in &bundle.observations {
        if observation.snr_db.is_some_and(|snr| !snr.is_finite()) {
            return Err(AnalysisError::NonFiniteSnr {
                observation_id: observation.observation_id.clone(),
            });
        }
    }

    Ok(())
}

fn summarize_slot(
    slot: &AlignedSlot,
    observations: &[ClassifiedObservation<'_>],
) -> SlotEvidenceSummary {
    SlotEvidenceSummary {
        slot_id: slot.slot_id.clone(),
        sequence_number: slot.sequence_number,
        band: slot.band,
        planned_label: slot.planned_label.clone(),
        actual_label: slot.actual_label.clone(),
        status: slot.status,
        evidence: aggregate(observations.iter().copied().filter(|classified| {
            classified.assignment.slot_id.as_deref() == Some(slot.slot_id.as_str())
        }))
        .finish(),
    }
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
) -> EvidenceAggregate {
    let mut aggregate = EvidenceAggregate::default();
    for observation in observations {
        aggregate.add(observation);
    }
    aggregate
}

fn snr_statistics(samples: &[f64]) -> Option<SnrStatistics> {
    if samples.is_empty() {
        return None;
    }

    let middle = samples.len() / 2;
    let median_db = if samples.len() % 2 == 0 {
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
    }
}

fn observation_kind_index(kind: ObservationKind) -> usize {
    match kind {
        ObservationKind::LocalDecode => 0,
        ObservationKind::PublicReport => 1,
        ObservationKind::ImportedSpot => 2,
    }
}
