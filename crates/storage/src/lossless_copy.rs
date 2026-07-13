use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{BundleStore, BundleStoreError, ResolvedBundlePaths};

impl BundleStore {
    /// Copies the original durable bundle representation without serializing it.
    ///
    /// The destination must not already exist. Manifest-declared root files and
    /// the complete attachments tree are copied byte-for-byte, then the copy is
    /// reopened through the normalized and validated import path. A newly
    /// created destination is removed if copying or verification fails and it
    /// is still safe to do so.
    pub fn copy_losslessly_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BundleStore, BundleCopyError> {
        let bundle = self
            .read_normalized_validated()
            .map_err(|source| BundleCopyError::Source { source })?;
        let source_paths = self
            .bundle_paths(&bundle.manifest.files)
            .map_err(|source| BundleCopyError::Source { source })?;
        let destination = destination.as_ref();

        ensure_destination_outside_source(self.root(), destination)?;
        ensure_destination_absent(destination)?;
        fs::create_dir(destination).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                BundleCopyError::DestinationExists {
                    path: destination.to_path_buf(),
                }
            } else {
                BundleCopyError::CreateDestination {
                    path: destination.to_path_buf(),
                    source,
                }
            }
        })?;

        let result = copy_and_verify(destination, &source_paths, &bundle.manifest.files);

        match result {
            Ok(store) => Ok(store),
            Err(error) => Err(rollback_destination(destination, error)),
        }
    }
}

fn copy_and_verify(
    destination: &Path,
    source_paths: &ResolvedBundlePaths,
    files: &antennabench_core::BundleFiles,
) -> Result<BundleStore, BundleCopyError> {
    let destination_store = BundleStore::new(destination);
    let destination_paths = destination_store
        .bundle_paths(files)
        .map_err(|source| BundleCopyError::DestinationLayout { source })?;

    for (source, destination) in required_root_file_pairs(source_paths, &destination_paths) {
        copy_regular_file(source, destination)?;
    }

    copy_optional_declared_manifest(source_paths, &destination_paths)?;
    copy_attachments_tree(
        &source_paths.attachments_dir,
        &destination_paths.attachments_dir,
    )?;

    destination_store
        .read_normalized_validated()
        .map_err(|source| BundleCopyError::Verification { source })?;

    Ok(destination_store)
}

fn required_root_file_pairs<'a>(
    source: &'a ResolvedBundlePaths,
    destination: &'a ResolvedBundlePaths,
) -> [(&'a Path, &'a Path); 10] {
    [
        (&source.manifest, &destination.manifest),
        (&source.station, &destination.station),
        (&source.antennas, &destination.antennas),
        (&source.schedule, &destination.schedule),
        (&source.events, &destination.events),
        (&source.observations, &destination.observations),
        (&source.wsjtx, &destination.wsjtx),
        (&source.rig, &destination.rig),
        (&source.propagation, &destination.propagation),
        (&source.analysis, &destination.analysis),
    ]
}

fn copy_optional_declared_manifest(
    source: &ResolvedBundlePaths,
    destination: &ResolvedBundlePaths,
) -> Result<(), BundleCopyError> {
    let (Some(source), Some(destination)) =
        (&source.declared_manifest, &destination.declared_manifest)
    else {
        return Ok(());
    };

    match fs::symlink_metadata(source) {
        Ok(_) => copy_regular_file(source, destination),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source_error) => Err(BundleCopyError::InspectSourceEntry {
            path: source.clone(),
            source: source_error,
        }),
    }
}

fn copy_attachments_tree(source: &Path, destination: &Path) -> Result<(), BundleCopyError> {
    create_destination_directory(destination)?;

    let entries =
        fs::read_dir(source).map_err(|source_error| BundleCopyError::ReadSourceDirectory {
            path: source.to_path_buf(),
            source: source_error,
        })?;

    for entry in entries {
        let entry = entry.map_err(|source_error| BundleCopyError::ReadSourceDirectory {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(|source_error| {
            BundleCopyError::InspectSourceEntry {
                path: source_path.clone(),
                source: source_error,
            }
        })?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            return Err(BundleCopyError::UnsupportedSourceEntry {
                path: source_path,
                entry_type: "symbolic link",
            });
        }
        if file_type.is_dir() {
            copy_attachments_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            copy_regular_file(&source_path, &destination_path)?;
        } else {
            return Err(BundleCopyError::UnsupportedSourceEntry {
                path: source_path,
                entry_type: "unsupported filesystem entry",
            });
        }
    }

    Ok(())
}

fn copy_regular_file(source: &Path, destination: &Path) -> Result<(), BundleCopyError> {
    let metadata = fs::symlink_metadata(source).map_err(|source_error| {
        BundleCopyError::InspectSourceEntry {
            path: source.to_path_buf(),
            source: source_error,
        }
    })?;

    if !metadata.file_type().is_file() {
        return Err(BundleCopyError::UnsupportedSourceEntry {
            path: source.to_path_buf(),
            entry_type: if metadata.file_type().is_symlink() {
                "symbolic link"
            } else {
                "non-regular file"
            },
        });
    }

    fs::copy(source, destination).map_err(|source_error| BundleCopyError::CopyFile {
        source_path: source.to_path_buf(),
        destination_path: destination.to_path_buf(),
        source: source_error,
    })?;

    Ok(())
}

fn create_destination_directory(path: &Path) -> Result<(), BundleCopyError> {
    fs::create_dir(path).map_err(|source| BundleCopyError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_destination_absent(path: &Path) -> Result<(), BundleCopyError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(BundleCopyError::DestinationExists {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(BundleCopyError::InspectDestination {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn ensure_destination_outside_source(
    source_root: &Path,
    destination: &Path,
) -> Result<(), BundleCopyError> {
    let source_root =
        fs::canonicalize(source_root).map_err(|source| BundleCopyError::InspectSourceEntry {
            path: source_root.to_path_buf(),
            source,
        })?;
    let destination_parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let destination_parent = fs::canonicalize(destination_parent).map_err(|source| {
        BundleCopyError::InspectDestination {
            path: destination_parent.to_path_buf(),
            source,
        }
    })?;

    if destination_parent.starts_with(&source_root) {
        return Err(BundleCopyError::DestinationInsideSource {
            path: destination.to_path_buf(),
        });
    }

    Ok(())
}

fn rollback_destination(destination: &Path, error: BundleCopyError) -> BundleCopyError {
    let cleanup_result = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(destination)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => return error,
        Ok(_) => Err(io::Error::other(
            "destination changed type before it could be cleaned up safely",
        )),
        Err(source) => Err(source),
    };

    match cleanup_result {
        Ok(()) => error,
        Err(source) => BundleCopyError::CleanupFailed {
            path: destination.to_path_buf(),
            copy_error: Box::new(error),
            source,
        },
    }
}

#[derive(Debug, Error)]
pub enum BundleCopyError {
    #[error("source bundle is not valid for lossless copying")]
    Source {
        #[source]
        source: BundleStoreError,
    },

    #[error("destination already exists: {path}")]
    DestinationExists { path: PathBuf },

    #[error("destination cannot be inside the source bundle: {path}")]
    DestinationInsideSource { path: PathBuf },

    #[error("failed to inspect destination {path}")]
    InspectDestination {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to create destination {path}")]
    CreateDestination {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("destination bundle paths are invalid")]
    DestinationLayout {
        #[source]
        source: BundleStoreError,
    },

    #[error("failed to inspect source entry {path}")]
    InspectSourceEntry {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read source directory {path}")]
    ReadSourceDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("source entry is not safe to copy ({entry_type}): {path}")]
    UnsupportedSourceEntry {
        path: PathBuf,
        entry_type: &'static str,
    },

    #[error("failed to create directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to copy {source_path} to {destination_path}")]
    CopyFile {
        source_path: PathBuf,
        destination_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("copied bundle failed normalized validation")]
    Verification {
        #[source]
        source: BundleStoreError,
    },

    #[error("failed to clean up incomplete destination {path} after: {copy_error}")]
    CleanupFailed {
        path: PathBuf,
        copy_error: Box<BundleCopyError>,
        #[source]
        source: io::Error,
    },
}
