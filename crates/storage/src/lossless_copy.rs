use std::{
    fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use antennabench_core::{BundleManifestV2, SCHEMA_VERSION_V1, SCHEMA_VERSION_V2, V1_BUNDLE_SUFFIX};

use super::{
    ensure_bundle_root,
    inspection::scan_duplicate_members,
    resource::{copy_bounded_file, inventory_complete_tree, ModeledBudget, ResourceOperation},
    v2::ensure_v2_suffix,
    BundleStore, BundleStoreError,
};

impl BundleStore {
    /// Copies the complete durable bundle representation without projecting it
    /// into typed domain values. Safe opaque root entries are preserved too.
    pub fn copy_losslessly_to(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BundleStore, BundleCopyError> {
        ensure_bundle_root(self.root()).map_err(source_error)?;
        let mut budget = ModeledBudget::default();
        let manifest_path = self.root().join("manifest.json");
        let manifest_text = self
            .read_root_json(&manifest_path, &mut budget, ResourceOperation::Copy)
            .map_err(map_source_error)?;
        let duplicate_members = scan_duplicate_members(&manifest_text)
            .map_err(|message| source_error(BundleStoreError::AmbiguousManifest { message }))?;
        if !duplicate_members.is_empty() {
            return Err(source_error(BundleStoreError::AmbiguousManifest {
                message: format!("duplicate JSON members: {}", duplicate_members.join(", ")),
            }));
        }
        let manifest_value: serde_json::Value =
            serde_json::from_str(&manifest_text).map_err(|source| {
                source_error(BundleStoreError::ParseJson {
                    path: manifest_path.clone(),
                    source,
                })
            })?;
        let schema_version = manifest_value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);

        // Resolve only the manifest-declared layout. This proves that paths are
        // safe and required files exist without parsing modeled evidence.
        match schema_version {
            SCHEMA_VERSION_V1 => {
                let manifest = self.inspect_manifest_layout().map_err(map_source_error)?;
                self.bundle_paths(&manifest.files)
                    .and_then(|paths| paths.ensure_readable_targets())
                    .map_err(map_source_error)?;
            }
            SCHEMA_VERSION_V2 => {
                let manifest: BundleManifestV2 =
                    serde_json::from_str(&manifest_text).map_err(|source| {
                        source_error(BundleStoreError::ParseJson {
                            path: manifest_path,
                            source,
                        })
                    })?;
                self.v2_paths(&manifest.files)
                    .and_then(|paths| paths.ensure_readable())
                    .map_err(map_source_error)?;
            }
            actual => {
                return Err(source_error(BundleStoreError::UnsupportedSchemaVersion {
                    actual,
                }));
            }
        }

        let entries = inventory_complete_tree(self).map_err(map_source_error)?;
        let destination = destination.as_ref();
        if schema_version == SCHEMA_VERSION_V2 {
            ensure_v2_suffix(destination)
                .map_err(|source| BundleCopyError::DestinationLayout { source })?;
        } else if !destination.to_string_lossy().ends_with(V1_BUNDLE_SUFFIX) {
            return Err(BundleCopyError::DestinationLayout {
                source: BundleStoreError::InvalidBundleSuffix {
                    path: destination.to_path_buf(),
                    schema_version: SCHEMA_VERSION_V1,
                },
            });
        }
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

        let result = (|| {
            let destination_store = self.derived(destination);
            let mut copied_bytes = 0;
            for (source_path, directory) in entries {
                let relative = source_path
                    .strip_prefix(self.root())
                    .expect("inventoried under root");
                let destination_path = destination.join(relative);
                if directory {
                    fs::create_dir(&destination_path).map_err(|source| {
                        BundleCopyError::CreateDirectory {
                            path: destination_path,
                            source,
                        }
                    })?;
                } else {
                    copy_bounded_file(
                        self,
                        &source_path,
                        &destination_path,
                        self.profile().attachment_file_bytes,
                        &mut copied_bytes,
                    )
                    .map_err(|source| BundleCopyError::Transfer { source })?;
                }
            }
            verify_copied_layout(&destination_store, schema_version)?;
            Ok(destination_store)
        })();

        match result {
            Ok(store) => Ok(store),
            Err(error) => Err(rollback_destination(destination, error)),
        }
    }
}

fn verify_copied_layout(store: &BundleStore, schema_version: u16) -> Result<(), BundleCopyError> {
    if schema_version == SCHEMA_VERSION_V1 {
        store
            .inspect_manifest_layout()
            .map_err(|source| BundleCopyError::Verification { source })?;
    } else {
        let mut budget = ModeledBudget::default();
        let path = store.root().join("manifest.json");
        let text = store
            .read_root_json(&path, &mut budget, ResourceOperation::Copy)
            .map_err(|source| BundleCopyError::Verification { source })?;
        let manifest: BundleManifestV2 =
            serde_json::from_str(&text).map_err(|source| BundleCopyError::Verification {
                source: BundleStoreError::ParseJson { path, source },
            })?;
        store
            .v2_paths(&manifest.files)
            .and_then(|paths| paths.ensure_readable())
            .map_err(|source| BundleCopyError::Verification { source })?;
    }
    Ok(())
}

fn source_error(source: BundleStoreError) -> BundleCopyError {
    BundleCopyError::Source { source }
}

fn map_source_error(source: BundleStoreError) -> BundleCopyError {
    if let BundleStoreError::InvalidBundleFilePath { path } = &source {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            return BundleCopyError::UnsupportedSourceEntry {
                path: path.clone(),
                entry_type: if metadata.file_type().is_symlink() {
                    "symbolic link"
                } else {
                    "unsupported filesystem entry"
                },
            };
        }
    }
    source_error(source)
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
    #[error("lossless copy transfer failed")]
    Transfer {
        #[source]
        source: BundleStoreError,
    },
    #[error("copied bundle failed storage-safe verification")]
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
