use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use antennabench_core::v2::AttachmentReference;
use sha2::{Digest, Sha256};

use super::{
    durability::{sync_directory, sync_regular_file},
    live_io, LivePersistenceError,
};
use crate::{v2::ResolvedBundlePathsV2, BundleStore};

pub(super) fn durable_attachment(
    store: &BundleStore,
    paths: &ResolvedBundlePathsV2,
    bytes: &[u8],
    media_type: &str,
    source_locator: Option<String>,
) -> Result<AttachmentReference, LivePersistenceError> {
    let reference = store.write_attachment(bytes, media_type, None, None, source_locator)?;
    let relative =
        reference
            .relative_path()
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: "recovery attachment digest is invalid".into(),
            })?;
    let path = paths.attachments_dir.join(relative);
    sync_regular_file(&path)
        .map_err(|source| live_io("synchronize recovery attachment", &path, source))?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)
            .map_err(|source| live_io("synchronize recovery digest directory", parent, source))?;
    }
    sync_directory(&paths.attachments_dir).map_err(|source| {
        live_io(
            "synchronize recovery attachments directory",
            &paths.attachments_dir,
            source,
        )
    })?;
    Ok(reference)
}

pub(super) fn copy_checkpointed_attachments(
    store: &BundleStore,
    source_attachments: &Path,
    destination_attachments: &Path,
) -> Result<(), LivePersistenceError> {
    let source_digest_dir = source_attachments.join("sha256");
    if !source_digest_dir.exists() {
        return Ok(());
    }
    let destination_digest_dir = destination_attachments.join("sha256");
    fs::create_dir_all(&destination_digest_dir).map_err(|source| {
        live_io(
            "create export attachment directory",
            &destination_digest_dir,
            source,
        )
    })?;
    let mut entries = fs::read_dir(&source_digest_dir)
        .map_err(|source| {
            live_io(
                "inspect checkpointed attachments",
                &source_digest_dir,
                source,
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| {
            live_io(
                "inspect checkpointed attachments",
                &source_digest_dir,
                source,
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| live_io("inspect checkpointed attachment", &source_path, source))?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "checkpointed attachment is not a regular file: {}",
                    source_path.display()
                ),
            });
        }
        let name = entry.file_name();
        let expected = name.to_string_lossy();
        if digest_file(&source_path, store.profile().attachment_file_bytes)? != expected {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "attachment digest name does not match {}",
                    source_path.display()
                ),
            });
        }
        let destination_path = destination_digest_dir.join(&name);
        if !destination_path.exists() {
            fs::copy(&source_path, &destination_path).map_err(|source| {
                live_io("copy checkpointed attachment", &destination_path, source)
            })?;
        }
        if digest_file(&destination_path, store.profile().attachment_file_bytes)? != expected {
            return Err(LivePersistenceError::CheckpointVerification {
                message: format!(
                    "exported attachment digest does not match {}",
                    destination_path.display()
                ),
            });
        }
        sync_regular_file(&destination_path).map_err(|source| {
            live_io("synchronize exported attachment", &destination_path, source)
        })?;
    }
    sync_directory(&destination_digest_dir).map_err(|source| {
        live_io(
            "synchronize exported digest directory",
            &destination_digest_dir,
            source,
        )
    })?;
    sync_directory(destination_attachments).map_err(|source| {
        live_io(
            "synchronize exported attachments directory",
            destination_attachments,
            source,
        )
    })
}

fn digest_file(path: &Path, limit: u64) -> Result<String, LivePersistenceError> {
    let size = fs::metadata(path)
        .map_err(|source| live_io("inspect attachment digest input", path, source))?
        .len();
    if size > limit {
        return Err(LivePersistenceError::CheckpointVerification {
            message: format!(
                "attachment {} exceeds the {} byte live-copy limit",
                path.display(),
                limit
            ),
        });
    }
    let mut file =
        File::open(path).map_err(|source| live_io("open attachment digest input", path, source))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| live_io("read attachment digest input", path, source))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(crate::v2::encode_lower_hex(digest.finalize()))
}
