use std::{
    fs::{File, TryLockError},
    thread,
    time::{Duration, Instant},
};

// Spawning a child process clones the parent's descriptor table, so an
// advisory lock another thread is closing can stay pinned by the child's
// clone until it execs. That contention clears in milliseconds and does not
// represent a concurrent writer, so absorb it before reporting WriterBusy.
const TRANSIENT_RETRY_BUDGET: Duration = Duration::from_millis(150);
const TRANSIENT_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub(super) fn try_lock_exclusive(lock: &File) -> Result<(), TryLockError> {
    with_transient_retry(|| lock.try_lock())
}

pub(super) fn try_lock_shared(lock: &File) -> Result<(), TryLockError> {
    with_transient_retry(|| lock.try_lock_shared())
}

fn with_transient_retry(
    attempt: impl Fn() -> Result<(), TryLockError>,
) -> Result<(), TryLockError> {
    let deadline = Instant::now() + TRANSIENT_RETRY_BUDGET;
    loop {
        match attempt() {
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(TRANSIENT_RETRY_INTERVAL);
            }
            result => return result,
        }
    }
}
