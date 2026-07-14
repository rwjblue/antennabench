use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use antennabench_core::{
    normalize_bundle, validate_bundle_report, BundleContents, BundleFiles, BundleValidationError,
    BundleValidationProfile,
};
use serde::Serialize;
use thiserror::Error;

mod inspection;
mod lossless_copy;
mod upgrade;
mod v2;

pub use inspection::BundleInspection;
pub use lossless_copy::BundleCopyError;
pub use upgrade::BundleUpgradeError;
pub use v2::BundleAttachment;

#[derive(Debug, Clone)]
pub struct BundleStore {
    root: PathBuf,
}

impl BundleStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes the legacy schema-v1 compatibility representation.
    ///
    /// New authored sessions should use [`Self::write_v2`] so provenance and
    /// lifecycle state are retained in the provider-neutral schema.
    pub fn write(&self, bundle: &BundleContents) -> Result<(), BundleStoreError> {
        ensure_writable_root(&self.root)?;
        let paths = self.bundle_paths(&bundle.manifest.files)?;
        paths.ensure_writable_targets()?;
        create_directory(&self.root)?;

        write_json(&paths.manifest, &bundle.manifest)?;
        write_json(&paths.station, &bundle.station)?;
        write_json(&paths.antennas, &bundle.antennas)?;
        write_json(&paths.schedule, &bundle.schedule)?;
        write_jsonl(&paths.events, &bundle.events)?;
        write_jsonl(&paths.observations, &bundle.observations)?;
        write_jsonl(&paths.wsjtx, &bundle.wsjtx)?;
        write_jsonl(&paths.rig, &bundle.rig)?;
        write_jsonl(&paths.propagation, &bundle.propagation)?;
        write_json(&paths.analysis, &bundle.analysis)?;
        create_directory(&paths.attachments_dir)?;

        Ok(())
    }

    pub fn read(&self) -> Result<BundleContents, BundleStoreError> {
        let (bundle, report) = self.inspect()?.into_parts();
        bundle.ok_or_else(|| BundleValidationError::from_report(report).into())
    }

    pub fn read_current(
        &self,
    ) -> Result<antennabench_core::CurrentBundleContents, BundleStoreError> {
        let (bundle, report) = self.inspect()?.into_current_parts();
        bundle.ok_or_else(|| BundleValidationError::from_report(report).into())
    }

    /// Reads a bundle and validates persisted annotations exactly as stored.
    pub fn read_validated(&self) -> Result<BundleContents, BundleStoreError> {
        let (bundle, report) = self.inspect()?.into_parts();
        if !report.is_empty() {
            return Err(BundleValidationError::from_report(report).into());
        }
        bundle.ok_or_else(|| BundleValidationError::from_report(report).into())
    }

    /// Reads a bundle, regenerates observation slot annotations, then validates it.
    pub fn read_normalized_validated(&self) -> Result<BundleContents, BundleStoreError> {
        let (bundle, report) = self.inspect()?.into_parts();
        if !report.allows(BundleValidationProfile::Analysis) {
            return Err(BundleValidationError::from_report(report).into());
        }
        let Some(bundle) = bundle else {
            return Err(BundleValidationError::from_report(report).into());
        };
        let bundle = normalize_bundle(bundle);
        let report = validate_bundle_report(&bundle);
        if !report.allows(BundleValidationProfile::Analysis) {
            return Err(BundleValidationError::from_report(report).into());
        }
        Ok(bundle)
    }

    fn bundle_paths(&self, files: &BundleFiles) -> Result<ResolvedBundlePaths, BundleStoreError> {
        let bootstrap_files = BundleFiles::default();
        let manifest = self.bundle_path(bootstrap_files.manifest.as_str())?;
        let declared_manifest = self.bundle_path(files.manifest.as_str())?;
        let declared_manifest = (declared_manifest != manifest).then_some(declared_manifest);

        let paths = ResolvedBundlePaths {
            manifest,
            declared_manifest,
            station: self.bundle_path(files.station.as_str())?,
            antennas: self.bundle_path(files.antennas.as_str())?,
            schedule: self.bundle_path(files.schedule.as_str())?,
            events: self.bundle_path(files.events.as_str())?,
            observations: self.bundle_path(files.observations.as_str())?,
            wsjtx: self.bundle_path(files.wsjtx.as_str())?,
            rig: self.bundle_path(files.rig.as_str())?,
            propagation: self.bundle_path(files.propagation.as_str())?,
            analysis: self.bundle_path(files.analysis.as_str())?,
            attachments_dir: self.bundle_path(files.attachments_dir.as_str())?,
        };
        paths.ensure_unique()?;

        Ok(paths)
    }

    fn bundle_path(&self, relative_path: &str) -> Result<PathBuf, BundleStoreError> {
        let path = Path::new(relative_path);
        let mut normalized = PathBuf::new();
        let mut normal_components = 0;

        for component in path.components() {
            match component {
                Component::Normal(part) => {
                    normal_components += 1;
                    normalized.push(part);
                }
                Component::CurDir => {}
                _ => {
                    return Err(BundleStoreError::InvalidBundlePath {
                        path: relative_path.to_string(),
                    });
                }
            }
        }

        if normalized.as_os_str().is_empty() || normal_components != 1 {
            return Err(BundleStoreError::InvalidBundlePath {
                path: relative_path.to_string(),
            });
        }

        Ok(self.root.join(normalized))
    }
}

#[derive(Debug, Clone)]
struct ResolvedBundlePaths {
    manifest: PathBuf,
    declared_manifest: Option<PathBuf>,
    station: PathBuf,
    antennas: PathBuf,
    schedule: PathBuf,
    events: PathBuf,
    observations: PathBuf,
    wsjtx: PathBuf,
    rig: PathBuf,
    propagation: PathBuf,
    analysis: PathBuf,
    attachments_dir: PathBuf,
}

impl ResolvedBundlePaths {
    fn ensure_unique(&self) -> Result<(), BundleStoreError> {
        let mut seen = HashSet::new();

        for path in [
            &self.manifest,
            &self.station,
            &self.antennas,
            &self.schedule,
            &self.events,
            &self.observations,
            &self.wsjtx,
            &self.rig,
            &self.propagation,
            &self.analysis,
            &self.attachments_dir,
        ] {
            if !seen.insert(path.clone()) {
                return Err(BundleStoreError::DuplicateBundlePath {
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }

        if let Some(path) = &self.declared_manifest {
            if !seen.insert(path.clone()) {
                return Err(BundleStoreError::DuplicateBundlePath {
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }

        Ok(())
    }

    fn ensure_writable_targets(&self) -> Result<(), BundleStoreError> {
        for path in [
            &self.manifest,
            &self.station,
            &self.antennas,
            &self.schedule,
            &self.events,
            &self.observations,
            &self.wsjtx,
            &self.rig,
            &self.propagation,
            &self.analysis,
        ] {
            if matches!(fs::symlink_metadata(path), Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir())
            {
                return Err(BundleStoreError::InvalidBundleFilePath { path: path.clone() });
            }
        }

        if matches!(fs::symlink_metadata(&self.attachments_dir), Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir())
        {
            return Err(BundleStoreError::InvalidAttachmentsDirectory {
                path: self.attachments_dir.clone(),
            });
        }

        Ok(())
    }

    fn ensure_readable_targets(&self) -> Result<(), BundleStoreError> {
        for path in [
            &self.manifest,
            &self.station,
            &self.antennas,
            &self.schedule,
            &self.events,
            &self.observations,
            &self.wsjtx,
            &self.rig,
            &self.propagation,
            &self.analysis,
        ] {
            if matches!(fs::symlink_metadata(path), Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file())
            {
                return Err(BundleStoreError::InvalidBundleFilePath { path: path.clone() });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum BundleStoreError {
    #[error("failed to create directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse JSON from {path}")]
    ParseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize JSON for {path}")]
    SerializeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("bundle file path must be relative and stay inside the bundle: {path}")]
    InvalidBundlePath { path: String },

    #[error("bundle file paths must be unique: {path}")]
    DuplicateBundlePath { path: String },

    #[error("bundle file path cannot be a directory: {path}")]
    InvalidBundleFilePath { path: PathBuf },

    #[error("bundle attachments path must exist as a directory: {path}")]
    InvalidAttachmentsDirectory { path: PathBuf },

    #[error("bundle root must be a directory path and cannot be a symlink: {path}")]
    InvalidBundleRoot { path: PathBuf },

    #[error("bundle destination already exists: {path}")]
    DestinationExists { path: PathBuf },

    #[error("bundle path has the wrong suffix for schema version {schema_version}: {path}")]
    InvalidBundleSuffix { path: PathBuf, schema_version: u16 },

    #[error("unsupported bundle schema version {actual}; supported versions are 1 and 2")]
    UnsupportedSchemaVersion { actual: u16 },

    #[error("bundle manifest is ambiguous and cannot safely select file paths: {message}")]
    AmbiguousManifest { message: String },

    #[error("invalid schema-v2 bundle invariant: {message}")]
    InvalidV2Bundle { message: String },

    #[error("attachment reference is invalid: {message}")]
    InvalidAttachmentReference { message: String },

    #[error("attachment digest or size does not match its reference: {path}")]
    AttachmentMismatch { path: PathBuf },

    #[error(transparent)]
    Validation {
        #[from]
        source: BundleValidationError,
    },
}

fn create_directory(path: &Path) -> Result<(), BundleStoreError> {
    fs::create_dir_all(path).map_err(|source| BundleStoreError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_writable_root(path: &Path) -> Result<(), BundleStoreError> {
    ensure_bundle_root(path)
}

fn ensure_bundle_root(path: &Path) -> Result<(), BundleStoreError> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(BundleStoreError::InvalidBundleRoot {
            path: path.to_path_buf(),
        })
    }
}

fn ensure_directory(path: &Path) -> Result<(), BundleStoreError> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Err(BundleStoreError::InvalidAttachmentsDirectory {
            path: path.to_path_buf(),
        });
    };

    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(BundleStoreError::InvalidAttachmentsDirectory {
            path: path.to_path_buf(),
        })
    }
}

fn write_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<(), BundleStoreError> {
    let path = path.as_ref();
    let mut contents =
        serde_json::to_string_pretty(value).map_err(|source| BundleStoreError::SerializeJson {
            path: path.to_path_buf(),
            source,
        })?;
    contents.push('\n');

    fs::write(path, contents).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn write_jsonl<T: Serialize>(
    path: impl AsRef<Path>,
    records: &[T],
) -> Result<(), BundleStoreError> {
    let path = path.as_ref();
    let mut file = fs::File::create(path).map_err(|source| BundleStoreError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    for record in records {
        serde_json::to_writer(&mut file, record).map_err(|source| {
            BundleStoreError::SerializeJson {
                path: path.to_path_buf(),
                source,
            }
        })?;
        file.write_all(b"\n")
            .map_err(|source| BundleStoreError::Write {
                path: path.to_path_buf(),
                source,
            })?;
    }

    Ok(())
}
