use std::{fs, fs::OpenOptions, sync::Arc};

use antennabench_core::{
    v2::MutationMember, v3::BundleV3Contents, v6::RuntimeContextV6, SCHEMA_VERSION_V6,
};
use chrono::{DateTime, Utc};

use super::{
    checkpoint::verify_exact_checkpoint, live_io, lock, probe_live_persistence,
    LivePersistenceError, LivePersistenceHooks, LiveSessionV3, RecoveryReportV2,
    SystemLivePersistenceHooks, LOCK_FILE,
};
use crate::BundleStore;

pub(super) struct PreparedRuntimeActor {
    pub(super) context_id: Option<String>,
    pub(super) new_context: Option<RuntimeContextV6>,
    pub(super) member_offset: u32,
    pub(super) member_count: u32,
}

impl BundleStore {
    pub fn open_v3_writer(&self) -> Result<LiveSessionV3, LivePersistenceError> {
        self.open_v3_writer_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn open_v3_writer_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<LiveSessionV3, LivePersistenceError> {
        self.open_v3_writer_internal(hooks, None)
    }

    pub fn open_v3_writer_in_context(
        &self,
        context: RuntimeContextV6,
    ) -> Result<LiveSessionV3, LivePersistenceError> {
        self.open_v3_writer_internal(Arc::new(SystemLivePersistenceHooks), Some(context))
    }

    pub fn open_v3_writer_with_hooks_in_context(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
        context: RuntimeContextV6,
    ) -> Result<LiveSessionV3, LivePersistenceError> {
        self.open_v3_writer_internal(hooks, Some(context))
    }

    pub fn recover_v3(&self) -> Result<RecoveryReportV2, LivePersistenceError> {
        self.recover_v3_with_hooks(Arc::new(SystemLivePersistenceHooks))
    }

    pub fn recover_v3_with_hooks(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
    ) -> Result<RecoveryReportV2, LivePersistenceError> {
        self.recover_v3_internal(hooks, None)
    }

    pub fn recover_v3_with_hooks_in_context(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
        context: RuntimeContextV6,
    ) -> Result<RecoveryReportV2, LivePersistenceError> {
        self.recover_v3_internal(hooks, Some(context))
    }

    pub(super) fn open_v3_writer_internal(
        &self,
        hooks: Arc<dyn LivePersistenceHooks>,
        requested_context: Option<RuntimeContextV6>,
    ) -> Result<LiveSessionV3, LivePersistenceError> {
        if !self.root().is_dir() {
            return Err(LivePersistenceError::Capability {
                message: "bundle root is not a regular local directory".into(),
            });
        }
        let lock_path = self.root().join(LOCK_FILE);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| live_io("open writer lock", &lock_path, source))?;
        match lock::try_lock_exclusive(&lock) {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => return Err(LivePersistenceError::WriterBusy),
            Err(fs::TryLockError::Error(source)) => {
                return Err(LivePersistenceError::Capability {
                    message: format!("OS file locking failed: {source}"),
                });
            }
        }
        probe_live_persistence(self.root()).map_err(|source| LivePersistenceError::Capability {
            message: format!("live persistence durability probe failed: {source}"),
        })?;
        let bundle = self.read_v3()?;
        let paths = self.v2_paths_for_state(&bundle.manifest.files, &bundle.session_state)?;
        verify_exact_checkpoint(self, &bundle.session_state, &paths)?;
        let pending_runtime_context = if let Some(context) = requested_context {
            if bundle.manifest.schema_version != SCHEMA_VERSION_V6 || !context.has_valid_identity()
            {
                return Err(LivePersistenceError::Capability {
                    message: "runtime-context writers require a valid schema-v6 bundle".into(),
                });
            }
            (bundle.session_state.active_runtime_context_id.as_deref()
                != Some(context.context_id.as_str()))
            .then_some(context)
        } else {
            None
        };
        Ok(LiveSessionV3 {
            store: self.clone(),
            _lock: lock,
            hooks,
            bundle,
            paths,
            frozen: false,
            pending_runtime_context,
        })
    }
}

impl LiveSessionV3 {
    pub(super) fn prepare_runtime_actor(
        &self,
        mutation_id: &str,
        primary_member_count: u32,
        recorded_at: DateTime<Utc>,
    ) -> Result<PreparedRuntimeActor, LivePersistenceError> {
        if self.bundle.manifest.schema_version != SCHEMA_VERSION_V6 {
            return Ok(PreparedRuntimeActor {
                context_id: None,
                new_context: None,
                member_offset: 0,
                member_count: primary_member_count,
            });
        }
        let requested = self.pending_runtime_context.as_ref();
        let context_id = requested
            .map(|context| context.context_id.clone())
            .or_else(|| self.bundle.session_state.active_runtime_context_id.clone())
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "schema-v6 mutation has no runtime context".into(),
            })?;
        let is_new = !self
            .bundle
            .runtime_contexts
            .iter()
            .any(|context| context.context_id == context_id);
        let member_offset = u32::from(is_new);
        let member_count = primary_member_count
            .checked_add(member_offset)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "schema-v6 mutation member count overflowed".into(),
            })?;
        let new_context = if is_new {
            let mut context =
                requested
                    .cloned()
                    .ok_or_else(|| LivePersistenceError::InvalidMutation {
                        message: "new schema-v6 actor context was not supplied".into(),
                    })?;
            context.first_recorded_at = recorded_at;
            context.mutation = MutationMember {
                mutation_id: mutation_id.into(),
                member_index: 0,
                member_count,
            };
            Some(context)
        } else {
            None
        };
        Ok(PreparedRuntimeActor {
            context_id: Some(context_id),
            new_context,
            member_offset,
            member_count,
        })
    }

    pub(super) fn apply_runtime_actor(
        candidate: &mut BundleV3Contents,
        actor: &PreparedRuntimeActor,
    ) {
        if let Some(context) = &actor.new_context {
            candidate.runtime_contexts.push(context.clone());
        }
        if let Some(context_id) = &actor.context_id {
            candidate.session_state.active_runtime_context_id = Some(context_id.clone());
        }
    }
}
