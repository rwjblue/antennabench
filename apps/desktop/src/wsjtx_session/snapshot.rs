//! Full checkpoint snapshots used at receiver startup and mutation boundaries.

use antennabench_core::{
    v2::{BundleV2Contents, SessionLifecycleV2},
    v3::{project_wspr_run_v3, BundleV3Contents, WsprCycleDirection},
    BundleContents, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3, SCHEMA_VERSION_V4, SCHEMA_VERSION_V5,
    SCHEMA_VERSION_V6,
};
use antennabench_storage::{BundleStore, LivePersistenceError};
use chrono::{DateTime, Utc};

pub(super) enum WsjtxSnapshot {
    V2(BundleV2Contents),
    V3(BundleV3Contents),
}

impl WsjtxSnapshot {
    pub(super) fn lifecycle(&self) -> SessionLifecycleV2 {
        match self {
            Self::V2(bundle) => bundle.session_state.lifecycle,
            Self::V3(bundle) => bundle.session_state.lifecycle,
        }
    }

    pub(super) fn session_id(&self) -> &str {
        match self {
            Self::V2(bundle) => &bundle.manifest.session_id,
            Self::V3(bundle) => &bundle.manifest.session_id,
        }
    }

    pub(super) fn revision(&self) -> u64 {
        match self {
            Self::V2(bundle) => bundle.session_state.revision,
            Self::V3(bundle) => bundle.session_state.revision,
        }
    }

    pub(super) fn last_committed_mutation_id(&self) -> Option<&str> {
        match self {
            Self::V2(bundle) => bundle.session_state.last_committed_mutation_id.as_deref(),
            Self::V3(bundle) => bundle.session_state.last_committed_mutation_id.as_deref(),
        }
    }

    pub(super) fn station(&self) -> (&str, &str) {
        match self {
            Self::V2(bundle) => (&bundle.station.callsign, &bundle.station.grid),
            Self::V3(bundle) => (&bundle.station.callsign, &bundle.station.grid),
        }
    }

    pub(super) fn earliest_slot_start(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::V2(bundle) => bundle
                .schedule
                .slots
                .iter()
                .map(|slot| slot.starts_at)
                .min(),
            Self::V3(bundle) => bundle
                .clone()
                .into_current()
                .bundle
                .schedule
                .slots
                .iter()
                .map(|slot| slot.starts_at)
                .min(),
        }
    }

    pub(super) fn current_bundle(&self, observed_at: DateTime<Utc>) -> BundleContents {
        match self {
            Self::V2(bundle) => bundle.clone().into_current().bundle,
            Self::V3(bundle) => {
                let receive_intents = bundle
                    .schedule
                    .wspr_cycle_intents
                    .iter()
                    .filter(|intent| {
                        intent.direction.is_none()
                            || intent.direction == Some(WsprCycleDirection::Receive)
                    })
                    .map(|intent| intent.intent_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
                let attributable = projection
                    .cycles
                    .iter()
                    .filter(|cycle| {
                        cycle.occupancy_fully_covers_transmission
                            && cycle.window.transmission_ends_at <= observed_at
                            && receive_intents.contains(cycle.intent_id.as_str())
                    })
                    .map(|cycle| cycle.intent_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                let mut current = bundle.clone().into_current().bundle;
                current
                    .schedule
                    .slots
                    .retain(|slot| attributable.contains(slot.slot_id.as_str()));
                current
            }
        }
    }
}

pub(super) fn read_wsjtx_snapshot(
    store: &BundleStore,
) -> Result<WsjtxSnapshot, LivePersistenceError> {
    match store.schema_version()? {
        SCHEMA_VERSION_V2 => store.read_v2_checkpointed().map(WsjtxSnapshot::V2),
        SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6 => {
            store.read_v3_checkpointed().map(WsjtxSnapshot::V3)
        }
        actual => {
            Err(antennabench_storage::BundleStoreError::UnsupportedSchemaVersion { actual }.into())
        }
    }
}
