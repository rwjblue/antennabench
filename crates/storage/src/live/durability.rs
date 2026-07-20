use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
};

#[cfg(windows)]
use std::io::Write;

#[cfg(windows)]
use uuid::Uuid;

use super::LivePersistenceHooks;

#[cfg(unix)]
pub(super) fn replace_checkpoint(
    temp: &Path,
    current: &Path,
    previous: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let previous_temp = previous.with_extension("json.next");
    if previous_temp.exists() {
        fs::remove_file(&previous_temp)?;
    }
    fs::copy(current, &previous_temp)?;
    sync_regular_file_with_hooks(&previous_temp, hooks)?;
    fs::rename(&previous_temp, previous)?;
    fs::rename(temp, current)
}

#[cfg(unix)]
pub(super) fn publish_new_bundle(staging: &Path, destination: &Path) -> io::Result<()> {
    // The caller checks for an existing destination immediately before this
    // same-volume rename. Existing paths are never intentionally replaced.
    fs::rename(staging, destination)
}

#[cfg(windows)]
pub(super) fn replace_checkpoint(
    temp: &Path,
    current: &Path,
    previous: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let previous_temp = previous.with_extension("json.next");
    remove_file_if_present(&previous_temp)?;
    fs::copy(current, &previous_temp)?;
    sync_regular_file_with_hooks(&previous_temp, hooks)?;
    move_file_write_through(&previous_temp, previous)?;
    move_file_write_through(temp, current)
}

#[cfg(windows)]
pub(super) fn publish_new_bundle(staging: &Path, destination: &Path) -> io::Result<()> {
    move_file_write_through_without_replacement(staging, destination)
}

#[cfg(unix)]
pub(super) fn probe_live_persistence(
    path: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    sync_directory_with_hooks(path, hooks)
}

#[cfg(windows)]
pub(super) fn probe_live_persistence(
    path: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let probe_id = Uuid::new_v4().simple();
    let current = path.join(format!(".antennabench-durability-{probe_id}.current"));
    let replacement = path.join(format!(".antennabench-durability-{probe_id}.next"));
    let previous = path.join(format!(".antennabench-durability-{probe_id}.previous"));
    let previous_temp = previous.with_extension("json.next");
    let result = (|| {
        write_synced_probe(&current, b"current", hooks)?;
        write_synced_probe(&replacement, b"replacement", hooks)?;
        replace_checkpoint(&replacement, &current, &previous, hooks)?;
        if fs::read(&current)? != b"replacement" {
            return Err(io::Error::other(
                "write-through replacement did not preserve the replacement bytes",
            ));
        }
        if fs::read(&previous)? != b"current" {
            return Err(io::Error::other(
                "write-through replacement did not preserve the prior bytes",
            ));
        }
        Ok(())
    })();
    let cleanup_current = remove_file_if_present(&current);
    let cleanup_replacement = remove_file_if_present(&replacement);
    let cleanup_previous = remove_file_if_present(&previous);
    let cleanup_previous_temp = remove_file_if_present(&previous_temp);
    result
        .and(cleanup_current)
        .and(cleanup_replacement)
        .and(cleanup_previous)
        .and(cleanup_previous_temp)
}

#[cfg(windows)]
fn write_synced_probe(
    path: &Path,
    bytes: &[u8],
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    hooks.sync_all(&file)
}

#[cfg(windows)]
fn move_file_write_through(source: &Path, destination: &Path) -> io::Result<()> {
    move_file_write_through_with_flags(source, destination, true)
}

#[cfg(windows)]
fn move_file_write_through_without_replacement(
    source: &Path,
    destination: &Path,
) -> io::Result<()> {
    move_file_write_through_with_flags(source, destination, false)
}

#[cfg(windows)]
fn move_file_write_through_with_flags(
    source: &Path,
    destination: &Path,
    replace_existing: bool,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;

    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace_existing {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn sync_regular_file(path: &Path) -> io::Result<()> {
    OpenOptions::new().write(true).open(path)?.sync_all()
}

pub(super) fn sync_regular_file_with_hooks(
    path: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    hooks.sync_all(&file)
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(unix)]
pub(super) fn sync_directory_with_hooks(
    path: &Path,
    hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    let directory = File::open(path)?;
    hooks.sync_all(&directory)
}

#[cfg(windows)]
pub(super) fn sync_directory(_path: &Path) -> io::Result<()> {
    // Windows has no supported directory-fsync equivalent. Regular files are
    // synchronized before they become reachable, while checkpoint promotion
    // and the capability probe use MoveFileExW with MOVEFILE_WRITE_THROUGH as
    // the metadata durability barrier.
    Ok(())
}

#[cfg(windows)]
pub(super) fn sync_directory_with_hooks(
    _path: &Path,
    _hooks: &dyn LivePersistenceHooks,
) -> io::Result<()> {
    Ok(())
}
