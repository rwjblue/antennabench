use std::{fs::File, io};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{LivePlanFile, LiveStreamV2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivePersistencePoint {
    BeforePlanWrite(LivePlanFile),
    AfterPlanWrite(LivePlanFile),
    BeforePlanSync(LivePlanFile),
    AfterPlanSync(LivePlanFile),
    BeforeStreamWrite(LiveStreamV2),
    MidStreamWrite(LiveStreamV2),
    AfterStreamWrite(LiveStreamV2),
    BeforeStreamSync(LiveStreamV2),
    AfterStreamSync(LiveStreamV2),
    BeforeCheckpointWrite,
    AfterCheckpointWrite,
    BeforeCheckpointSync,
    AfterCheckpointSync,
    BeforeCheckpointReplace,
    AfterCheckpointReplace,
    BeforeDirectorySync,
    AfterDirectorySync,
    BeforeCheckpointVerify,
    AfterCheckpointVerify,
    BeforeAcknowledge,
}

pub trait LivePersistenceHooks: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self, kind: &str) -> String;

    /// Performs the physical durability barrier. Production hooks should
    /// inherit this implementation; storage tests may override it to avoid
    /// machine-wide flush contention while retaining every logical boundary.
    fn sync_all(&self, file: &File) -> io::Result<()> {
        file.sync_all()
    }

    fn check(&self, _point: LivePersistencePoint) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct SystemLivePersistenceHooks;

impl LivePersistenceHooks for SystemLivePersistenceHooks {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn new_id(&self, kind: &str) -> String {
        format!("{kind}-{}", Uuid::new_v4())
    }
}
