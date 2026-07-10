use crate::{
    align_schedule_slots, apply_slot_assignments, BundleContents, ScheduleSlotAlignment,
    SlotAlignmentPolicy,
};

pub fn normalize_bundle(mut bundle: BundleContents) -> BundleContents {
    annotate_bundle_observations(&mut bundle);
    bundle
}

pub fn annotate_bundle_observations(bundle: &mut BundleContents) -> ScheduleSlotAlignment {
    let alignment = align_schedule_slots(
        &bundle.schedule,
        &bundle.events,
        &bundle.observations,
        SlotAlignmentPolicy::default(),
    );
    bundle.observations =
        apply_slot_assignments(&bundle.observations, &alignment.observation_assignments);
    alignment
}
