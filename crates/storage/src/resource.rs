use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::{
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Serialize,
};
use thiserror::Error;

use super::{BundleStore, BundleStoreError};

pub const LOCAL_RESOURCE_PROFILE_NAME: &str = "local-standard-v1";
pub const LOCAL_RESOURCE_PROFILE_VERSION: u16 = 1;
const COPY_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleResourceProfile {
    pub(super) root_json_bytes: u64,
    pub(super) jsonl_line_bytes: u64,
    pub(super) jsonl_stream_bytes: u64,
    pub(super) jsonl_stream_records: u64,
    pub(super) modeled_total_bytes: u64,
    pub(super) modeled_total_records: u64,
    pub(super) json_depth: u64,
    pub(super) scalar_string_bytes: u64,
    pub(super) root_entries: u64,
    pub(super) attachment_depth: u64,
    pub(super) attachment_entries: u64,
    pub(super) attachment_file_bytes: u64,
    pub(super) attachment_total_bytes: u64,
}

pub const LOCAL_STANDARD_V1: BundleResourceProfile = BundleResourceProfile {
    root_json_bytes: 4 * 1024 * 1024,
    jsonl_line_bytes: 256 * 1024,
    jsonl_stream_bytes: 128 * 1024 * 1024,
    jsonl_stream_records: 250_000,
    modeled_total_bytes: 256 * 1024 * 1024,
    modeled_total_records: 500_000,
    json_depth: 64,
    scalar_string_bytes: 128 * 1024,
    root_entries: 64,
    attachment_depth: 8,
    attachment_entries: 4_096,
    attachment_file_bytes: 512 * 1024 * 1024,
    attachment_total_bytes: 2 * 1024 * 1024 * 1024,
};

impl BundleResourceProfile {
    pub fn name(&self) -> &'static str {
        LOCAL_RESOURCE_PROFILE_NAME
    }

    pub fn version(&self) -> u16 {
        LOCAL_RESOURCE_PROFILE_VERSION
    }

    #[cfg(test)]
    pub(crate) fn tiny(limit: u64) -> Self {
        Self {
            root_json_bytes: limit,
            jsonl_line_bytes: limit,
            jsonl_stream_bytes: limit * 4,
            jsonl_stream_records: limit,
            modeled_total_bytes: limit * 16,
            modeled_total_records: limit * 4,
            json_depth: limit,
            scalar_string_bytes: limit,
            root_entries: limit,
            attachment_depth: limit,
            attachment_entries: limit,
            attachment_file_bytes: limit,
            attachment_total_bytes: limit * 4,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOperation {
    Inspect,
    Read,
    Write,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceStage {
    Inventory,
    Metadata,
    Stream,
    Parse,
    Serialize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceUnit {
    Bytes,
    Records,
    Entries,
    Depth,
    Checkpoints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDiagnostic {
    pub code: &'static str,
    pub profile: &'static str,
    pub profile_version: u16,
    pub operation: ResourceOperation,
    pub stage: ResourceStage,
    pub path: PathBuf,
    pub limit: u64,
    pub observed: Option<u64>,
    pub unit: ResourceUnit,
    pub retryable_without_input_change: bool,
    pub complete_result: bool,
    pub evidence_gap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("resource limit {diagnostic:?}")]
pub struct ResourceError {
    pub diagnostic: ResourceDiagnostic,
}

#[derive(Debug, Default)]
pub(super) struct ModeledBudget {
    bytes: u64,
    records: u64,
}

#[derive(Debug, Default)]
pub(super) struct AttachmentBudget {
    entries: u64,
    bytes: u64,
}

impl BundleStore {
    pub(super) fn check_cancelled(
        &self,
        operation: ResourceOperation,
        stage: ResourceStage,
        path: &Path,
    ) -> Result<(), BundleStoreError> {
        if self.cancellation().is_cancelled() {
            Err(resource_error(
                "resource.operation.cancelled",
                operation,
                stage,
                path,
                0,
                Some(1),
                ResourceUnit::Checkpoints,
            )
            .into())
        } else {
            Ok(())
        }
    }

    pub(super) fn inventory_root(
        &self,
        operation: ResourceOperation,
    ) -> Result<Vec<PathBuf>, BundleStoreError> {
        let entries = fs::read_dir(self.root()).map_err(|source| BundleStoreError::Read {
            path: self.root().to_path_buf(),
            source,
        })?;
        let mut paths = Vec::new();
        for entry in entries {
            self.check_cancelled(operation, ResourceStage::Inventory, self.root())?;
            let entry = entry.map_err(|source| BundleStoreError::Read {
                path: self.root().to_path_buf(),
                source,
            })?;
            paths.push(entry.path());
            check_limit(
                self.profile().root_entries,
                u64::try_from(paths.len()).expect("usize fits u64"),
                "resource.bundle.root_entries",
                operation,
                ResourceStage::Inventory,
                self.root(),
                ResourceUnit::Entries,
            )?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(|source| BundleStoreError::Read {
                    path: entry.path(),
                    source,
                })?;
            if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(BundleStoreError::InvalidBundleFilePath { path: entry.path() });
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub(super) fn read_root_json(
        &self,
        path: &Path,
        budget: &mut ModeledBudget,
        operation: ResourceOperation,
    ) -> Result<String, BundleStoreError> {
        let bytes = read_bounded(
            self,
            path,
            self.profile().root_json_bytes,
            "resource.bundle.root_json_bytes",
            operation,
        )?;
        budget.add_bytes(self, path, bytes.len(), operation)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|source| BundleStoreError::InvalidUtf8 {
                path: path.to_path_buf(),
                source,
            })?
            .to_owned();
        scan_json_limits(self, path, &text, operation)?;
        Ok(text)
    }

    pub(super) fn for_each_jsonl(
        &self,
        path: &Path,
        budget: &mut ModeledBudget,
        operation: ResourceOperation,
        mut consume: impl FnMut(usize, usize, &str) -> Result<(), BundleStoreError>,
    ) -> Result<(), BundleStoreError> {
        preflight_metadata(
            path,
            self.profile().jsonl_stream_bytes,
            "resource.jsonl.stream_bytes",
            operation,
        )?;
        let file = File::open(path).map_err(|source| BundleStoreError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let mut reader = BufReader::with_capacity(COPY_CHUNK_BYTES, file);
        let mut line = Vec::new();
        let mut physical_line = 0usize;
        let mut record_index = 0usize;
        let mut stream_bytes = 0u64;
        loop {
            self.check_cancelled(operation, ResourceStage::Stream, path)?;
            line.clear();
            let bytes = read_bounded_line(
                &mut reader,
                &mut line,
                self.profile().jsonl_line_bytes,
                path,
                operation,
            )?;
            if bytes == 0 {
                break;
            }
            physical_line += 1;
            stream_bytes = stream_bytes.saturating_add(u64::try_from(bytes).expect("usize fits"));
            check_limit(
                self.profile().jsonl_stream_bytes,
                stream_bytes,
                "resource.jsonl.stream_bytes",
                operation,
                ResourceStage::Stream,
                path,
                ResourceUnit::Bytes,
            )?;
            budget.add_bytes(self, path, bytes, operation)?;
            let text =
                std::str::from_utf8(&line).map_err(|source| BundleStoreError::InvalidUtf8 {
                    path: path.to_path_buf(),
                    source,
                })?;
            let record = text.strip_suffix('\n').unwrap_or(text);
            let record = record.strip_suffix('\r').unwrap_or(record);
            if record.trim().is_empty() {
                continue;
            }
            record_index += 1;
            check_limit(
                self.profile().jsonl_stream_records,
                u64::try_from(record_index).expect("usize fits"),
                "resource.jsonl.records",
                operation,
                ResourceStage::Stream,
                path,
                ResourceUnit::Records,
            )?;
            budget.add_record(self, path, operation)?;
            scan_json_limits(self, path, record, operation)?;
            consume(physical_line, record_index - 1, record)?;
        }
        Ok(())
    }
}

impl ModeledBudget {
    fn add_bytes(
        &mut self,
        store: &BundleStore,
        path: &Path,
        bytes: usize,
        operation: ResourceOperation,
    ) -> Result<(), BundleStoreError> {
        self.bytes = self
            .bytes
            .saturating_add(u64::try_from(bytes).expect("usize fits u64"));
        check_limit(
            store.profile().modeled_total_bytes,
            self.bytes,
            "resource.bundle.modeled_bytes",
            operation,
            ResourceStage::Stream,
            path,
            ResourceUnit::Bytes,
        )
        .map_err(Into::into)
    }

    fn add_record(
        &mut self,
        store: &BundleStore,
        path: &Path,
        operation: ResourceOperation,
    ) -> Result<(), BundleStoreError> {
        self.records = self.records.saturating_add(1);
        check_limit(
            store.profile().modeled_total_records,
            self.records,
            "resource.bundle.modeled_records",
            operation,
            ResourceStage::Stream,
            path,
            ResourceUnit::Records,
        )
        .map_err(Into::into)
    }
}

pub(super) fn read_bounded(
    store: &BundleStore,
    path: &Path,
    limit: u64,
    code: &'static str,
    operation: ResourceOperation,
) -> Result<Vec<u8>, BundleStoreError> {
    preflight_metadata(path, limit, code, operation)?;
    let mut file = File::open(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut output = Vec::new();
    let mut buffer = [0u8; COPY_CHUNK_BYTES];
    loop {
        store.check_cancelled(operation, ResourceStage::Stream, path)?;
        let read = file
            .read(&mut buffer)
            .map_err(|source| BundleStoreError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        let observed = u64::try_from(output.len().saturating_add(read)).expect("usize fits");
        check_limit(
            limit,
            observed,
            code,
            operation,
            ResourceStage::Stream,
            path,
            ResourceUnit::Bytes,
        )?;
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}

pub(super) fn serialize_bounded<T: Serialize>(
    store: &BundleStore,
    path: &Path,
    value: &T,
    line: bool,
) -> Result<Vec<u8>, BundleStoreError> {
    let limit = if line {
        store.profile().jsonl_line_bytes
    } else {
        store.profile().root_json_bytes
    };
    let code = if line {
        "resource.jsonl.line_bytes"
    } else {
        "resource.bundle.root_json_bytes"
    };
    let mut writer = LimitedWriter::new(limit, code, path, ResourceOperation::Write);
    if line {
        serde_json::to_writer(&mut writer, value)
    } else {
        serde_json::to_writer_pretty(&mut writer, value)
    }
    .map_err(|source| {
        writer
            .error
            .take()
            .unwrap_or_else(|| BundleStoreError::SerializeJson {
                path: path.to_path_buf(),
                source,
            })
    })?;
    writer.write_all(b"\n").map_err(|source| {
        writer.error.take().unwrap_or(BundleStoreError::Write {
            path: path.to_path_buf(),
            source,
        })
    })?;
    let bytes = writer.into_inner();
    let text = std::str::from_utf8(&bytes).expect("JSON serialization is UTF-8");
    scan_json_limits(store, path, text.trim_end(), ResourceOperation::Write)?;
    Ok(bytes)
}

pub(super) fn serialize_root_bounded<T: Serialize>(
    store: &BundleStore,
    path: &Path,
    value: &T,
    budget: &mut ModeledBudget,
) -> Result<Vec<u8>, BundleStoreError> {
    store.check_cancelled(ResourceOperation::Write, ResourceStage::Serialize, path)?;
    let bytes = serialize_bounded(store, path, value, false)?;
    budget.add_bytes(store, path, bytes.len(), ResourceOperation::Write)?;
    Ok(bytes)
}

pub(super) fn serialize_jsonl_bounded<T: Serialize>(
    store: &BundleStore,
    path: &Path,
    records: &[T],
    budget: &mut ModeledBudget,
) -> Result<Vec<u8>, BundleStoreError> {
    check_limit(
        store.profile().jsonl_stream_records,
        u64::try_from(records.len()).expect("usize fits"),
        "resource.jsonl.records",
        ResourceOperation::Write,
        ResourceStage::Serialize,
        path,
        ResourceUnit::Records,
    )?;
    let mut output = Vec::new();
    for record in records {
        store.check_cancelled(ResourceOperation::Write, ResourceStage::Serialize, path)?;
        let line = serialize_bounded(store, path, record, true)?;
        let observed = output.len().saturating_add(line.len());
        check_limit(
            store.profile().jsonl_stream_bytes,
            u64::try_from(observed).expect("usize fits"),
            "resource.jsonl.stream_bytes",
            ResourceOperation::Write,
            ResourceStage::Serialize,
            path,
            ResourceUnit::Bytes,
        )?;
        budget.add_bytes(store, path, line.len(), ResourceOperation::Write)?;
        budget.add_record(store, path, ResourceOperation::Write)?;
        output.extend_from_slice(&line);
    }
    Ok(output)
}

pub(super) fn copy_bounded_file(
    store: &BundleStore,
    source: &Path,
    destination: &Path,
    limit: u64,
    total: &mut u64,
) -> Result<(), BundleStoreError> {
    preflight_metadata(
        source,
        limit,
        "resource.attachments.file_bytes",
        ResourceOperation::Copy,
    )?;
    let mut input = File::open(source).map_err(|source_error| BundleStoreError::Read {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let mut output = File::create(destination).map_err(|source_error| BundleStoreError::Write {
        path: destination.to_path_buf(),
        source: source_error,
    })?;
    let mut buffer = [0u8; COPY_CHUNK_BYTES];
    let mut file_bytes = 0u64;
    loop {
        store.check_cancelled(ResourceOperation::Copy, ResourceStage::Stream, source)?;
        let read = input
            .read(&mut buffer)
            .map_err(|source_error| BundleStoreError::Read {
                path: source.to_path_buf(),
                source: source_error,
            })?;
        if read == 0 {
            break;
        }
        let read = u64::try_from(read).expect("usize fits");
        file_bytes = file_bytes.saturating_add(read);
        *total = total.saturating_add(read);
        check_limit(
            limit,
            file_bytes,
            "resource.attachments.file_bytes",
            ResourceOperation::Copy,
            ResourceStage::Stream,
            source,
            ResourceUnit::Bytes,
        )?;
        check_limit(
            store.profile().attachment_total_bytes,
            *total,
            "resource.attachments.total_bytes",
            ResourceOperation::Copy,
            ResourceStage::Stream,
            source,
            ResourceUnit::Bytes,
        )?;
        output
            .write_all(&buffer[..usize::try_from(read).expect("chunk fits")])
            .map_err(|source_error| BundleStoreError::Write {
                path: destination.to_path_buf(),
                source: source_error,
            })?;
    }
    Ok(())
}

pub(super) fn inventory_attachment_tree(
    store: &BundleStore,
    root: &Path,
    operation: ResourceOperation,
) -> Result<Vec<(PathBuf, bool)>, BundleStoreError> {
    fn visit(
        store: &BundleStore,
        root: &Path,
        current: &Path,
        depth: u64,
        operation: ResourceOperation,
        budget: &mut AttachmentBudget,
        output: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), BundleStoreError> {
        check_limit(
            store.profile().attachment_depth,
            depth,
            "resource.attachments.depth",
            operation,
            ResourceStage::Inventory,
            current,
            ResourceUnit::Depth,
        )?;
        let iterator = fs::read_dir(current).map_err(|source| BundleStoreError::Read {
            path: current.to_path_buf(),
            source,
        })?;
        let mut entries = Vec::new();
        for entry in iterator {
            store.check_cancelled(operation, ResourceStage::Inventory, current)?;
            let entry = entry.map_err(|source| BundleStoreError::Read {
                path: current.to_path_buf(),
                source,
            })?;
            budget.entries = budget.entries.saturating_add(1);
            check_limit(
                store.profile().attachment_entries,
                budget.entries,
                "resource.attachments.entries",
                operation,
                ResourceStage::Inventory,
                root,
                ResourceUnit::Entries,
            )?;
            entries.push(entry.path());
        }
        entries.sort();
        for path in entries {
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| BundleStoreError::Read {
                    path: path.clone(),
                    source,
                })?;
            if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(BundleStoreError::InvalidBundleFilePath { path });
            }
            let directory = metadata.is_dir();
            output.push((path.clone(), directory));
            if directory {
                visit(store, root, &path, depth + 1, operation, budget, output)?;
            } else {
                check_limit(
                    store.profile().attachment_file_bytes,
                    metadata.len(),
                    "resource.attachments.file_bytes",
                    operation,
                    ResourceStage::Metadata,
                    &path,
                    ResourceUnit::Bytes,
                )?;
                budget.bytes = budget.bytes.saturating_add(metadata.len());
                check_limit(
                    store.profile().attachment_total_bytes,
                    budget.bytes,
                    "resource.attachments.total_bytes",
                    operation,
                    ResourceStage::Metadata,
                    root,
                    ResourceUnit::Bytes,
                )?;
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(
        store,
        root,
        root,
        0,
        operation,
        &mut AttachmentBudget::default(),
        &mut output,
    )?;
    Ok(output)
}

pub(super) fn preflight_attachment_write(
    store: &BundleStore,
    root: &Path,
    path: &Path,
    bytes: u64,
    additional_entries: u64,
) -> Result<(), BundleStoreError> {
    let entries = inventory_attachment_tree(store, root, ResourceOperation::Write)?;
    check_limit(
        store.profile().attachment_file_bytes,
        bytes,
        "resource.attachments.file_bytes",
        ResourceOperation::Write,
        ResourceStage::Metadata,
        path,
        ResourceUnit::Bytes,
    )?;
    let existing_bytes = entries
        .iter()
        .filter(|(_, directory)| !directory)
        .try_fold(0u64, |total, (entry, _)| {
            fs::symlink_metadata(entry)
                .map(|metadata| total.saturating_add(metadata.len()))
                .map_err(|source| BundleStoreError::Read {
                    path: entry.clone(),
                    source,
                })
        })?;
    check_limit(
        store.profile().attachment_total_bytes,
        existing_bytes.saturating_add(if additional_entries > 0 { bytes } else { 0 }),
        "resource.attachments.total_bytes",
        ResourceOperation::Write,
        ResourceStage::Metadata,
        root,
        ResourceUnit::Bytes,
    )?;
    check_limit(
        store.profile().attachment_entries,
        u64::try_from(entries.len())
            .expect("usize fits")
            .saturating_add(additional_entries),
        "resource.attachments.entries",
        ResourceOperation::Write,
        ResourceStage::Inventory,
        root,
        ResourceUnit::Entries,
    )?;
    Ok(())
}

/// Inventories a complete bundle tree for storage-safe lossless copying.
///
/// Root entries use the root-entry limit. Every regular file and every entry
/// below a root directory additionally shares the opaque attachment limits so
/// unknown-but-safe evidence cannot evade the copy budget.
pub(super) fn inventory_complete_tree(
    store: &BundleStore,
) -> Result<Vec<(PathBuf, bool)>, BundleStoreError> {
    fn visit(
        store: &BundleStore,
        root: &Path,
        current: &Path,
        depth: u64,
        budget: &mut AttachmentBudget,
        output: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), BundleStoreError> {
        check_limit(
            store.profile().attachment_depth,
            depth,
            "resource.attachments.depth",
            ResourceOperation::Copy,
            ResourceStage::Inventory,
            current,
            ResourceUnit::Depth,
        )?;
        let iterator = fs::read_dir(current).map_err(|source| BundleStoreError::Read {
            path: current.to_path_buf(),
            source,
        })?;
        let mut entries = Vec::new();
        for entry in iterator {
            store.check_cancelled(ResourceOperation::Copy, ResourceStage::Inventory, current)?;
            let entry = entry.map_err(|source| BundleStoreError::Read {
                path: current.to_path_buf(),
                source,
            })?;
            budget.entries = budget.entries.saturating_add(1);
            check_limit(
                store.profile().attachment_entries,
                budget.entries,
                "resource.attachments.entries",
                ResourceOperation::Copy,
                ResourceStage::Inventory,
                root,
                ResourceUnit::Entries,
            )?;
            entries.push(entry.path());
        }
        entries.sort();
        for path in entries {
            let metadata =
                fs::symlink_metadata(&path).map_err(|source| BundleStoreError::Read {
                    path: path.clone(),
                    source,
                })?;
            if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(BundleStoreError::InvalidBundleFilePath { path });
            }
            let directory = metadata.is_dir();
            output.push((path.clone(), directory));
            if directory {
                visit(store, root, &path, depth + 1, budget, output)?;
            } else {
                check_limit(
                    store.profile().attachment_file_bytes,
                    metadata.len(),
                    "resource.attachments.file_bytes",
                    ResourceOperation::Copy,
                    ResourceStage::Metadata,
                    &path,
                    ResourceUnit::Bytes,
                )?;
                budget.bytes = budget.bytes.saturating_add(metadata.len());
                check_limit(
                    store.profile().attachment_total_bytes,
                    budget.bytes,
                    "resource.attachments.total_bytes",
                    ResourceOperation::Copy,
                    ResourceStage::Metadata,
                    root,
                    ResourceUnit::Bytes,
                )?;
            }
        }
        Ok(())
    }

    let root_entries = store.inventory_root(ResourceOperation::Copy)?;
    let mut output = Vec::new();
    let mut budget = AttachmentBudget::default();
    for path in root_entries {
        let metadata = fs::symlink_metadata(&path).map_err(|source| BundleStoreError::Read {
            path: path.clone(),
            source,
        })?;
        let directory = metadata.is_dir();
        output.push((path.clone(), directory));
        if directory {
            visit(store, store.root(), &path, 1, &mut budget, &mut output)?;
        } else {
            check_limit(
                store.profile().attachment_file_bytes,
                metadata.len(),
                "resource.attachments.file_bytes",
                ResourceOperation::Copy,
                ResourceStage::Metadata,
                &path,
                ResourceUnit::Bytes,
            )?;
            budget.bytes = budget.bytes.saturating_add(metadata.len());
            check_limit(
                store.profile().attachment_total_bytes,
                budget.bytes,
                "resource.attachments.total_bytes",
                ResourceOperation::Copy,
                ResourceStage::Metadata,
                store.root(),
                ResourceUnit::Bytes,
            )?;
        }
    }
    Ok(output)
}

fn preflight_metadata(
    path: &Path,
    limit: u64,
    code: &'static str,
    operation: ResourceOperation,
) -> Result<(), BundleStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| BundleStoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(BundleStoreError::InvalidBundleFilePath {
            path: path.to_path_buf(),
        });
    }
    check_limit(
        limit,
        metadata.len(),
        code,
        operation,
        ResourceStage::Metadata,
        path,
        ResourceUnit::Bytes,
    )
    .map_err(Into::into)
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
    limit: u64,
    path: &Path,
    operation: ResourceOperation,
) -> Result<usize, BundleStoreError> {
    loop {
        let available = reader.fill_buf().map_err(|source| BundleStoreError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        let observed = line.len().saturating_add(consumed);
        check_limit(
            limit,
            u64::try_from(observed).expect("usize fits"),
            "resource.jsonl.line_bytes",
            operation,
            ResourceStage::Stream,
            path,
            ResourceUnit::Bytes,
        )?;
        line.extend_from_slice(&available[..consumed]);
        let complete = available[..consumed].ends_with(b"\n");
        reader.consume(consumed);
        if complete {
            return Ok(line.len());
        }
    }
}

fn scan_json_limits(
    store: &BundleStore,
    path: &Path,
    text: &str,
    operation: ResourceOperation,
) -> Result<(), BundleStoreError> {
    let mut violation = None;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let result = JsonLimitSeed {
        store,
        path,
        operation,
        depth: 0,
        violation: &mut violation,
    }
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end());
    if let Some(error) = violation {
        return Err(error.into());
    }
    // This pass owns only resource-limit enforcement. Syntax and projection
    // errors remain the caller's responsibility so inspection can preserve its
    // typed, location-rich diagnostics.
    let _ = result;
    Ok(())
}

struct JsonLimitSeed<'a> {
    store: &'a BundleStore,
    path: &'a Path,
    operation: ResourceOperation,
    depth: u64,
    violation: &'a mut Option<ResourceError>,
}

impl<'de> DeserializeSeed<'de> for JsonLimitSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonLimitVisitor(self))
    }
}

struct JsonLimitVisitor<'a>(JsonLimitSeed<'a>);

impl<'de> Visitor<'de> for JsonLimitVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON within local resource limits")
    }

    fn visit_str<E>(mut self, value: &str) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        self.check_string(value).map_err(E::custom)
    }

    fn visit_string<E>(self, value: String) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_seq<A>(mut self, mut sequence: A) -> Result<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.check_depth().map_err(serde::de::Error::custom)?;
        while sequence.next_element_seed(self.child())?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(mut self, mut map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        self.check_depth().map_err(serde::de::Error::custom)?;
        while let Some(key) = map.next_key::<String>()? {
            self.check_string(&key).map_err(serde::de::Error::custom)?;
            map.next_value_seed(self.child())?;
        }
        Ok(())
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_some<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.0.deserialize(deserializer)
    }
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.0.deserialize(deserializer)
    }
    fn visit_i8<E>(self, _: i8) -> Result<(), E> {
        Ok(())
    }
    fn visit_i16<E>(self, _: i16) -> Result<(), E> {
        Ok(())
    }
    fn visit_i32<E>(self, _: i32) -> Result<(), E> {
        Ok(())
    }
    fn visit_u8<E>(self, _: u8) -> Result<(), E> {
        Ok(())
    }
    fn visit_u16<E>(self, _: u16) -> Result<(), E> {
        Ok(())
    }
    fn visit_u32<E>(self, _: u32) -> Result<(), E> {
        Ok(())
    }
    fn visit_f32<E>(self, _: f32) -> Result<(), E> {
        Ok(())
    }
    fn visit_char<E>(self, value: char) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&value.to_string())
    }
    fn visit_bytes<E>(self, value: &[u8]) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&String::from_utf8_lossy(value))
    }
    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<(), E>
    where
        E: serde::de::Error,
    {
        self.visit_bytes(&value)
    }
}

impl JsonLimitVisitor<'_> {
    fn check_string(&mut self, value: &str) -> Result<(), &'static str> {
        let observed = u64::try_from(value.len()).expect("usize fits");
        if observed > self.0.store.profile().scalar_string_bytes {
            *self.0.violation = Some(resource_error(
                "resource.json.scalar_bytes",
                self.0.operation,
                ResourceStage::Parse,
                self.0.path,
                self.0.store.profile().scalar_string_bytes,
                Some(observed),
                ResourceUnit::Bytes,
            ));
            Err("JSON scalar string exceeds resource limit")
        } else {
            Ok(())
        }
    }

    fn check_depth(&mut self) -> Result<(), &'static str> {
        let observed = self.0.depth + 1;
        if observed > self.0.store.profile().json_depth {
            *self.0.violation = Some(resource_error(
                "resource.json.depth",
                self.0.operation,
                ResourceStage::Parse,
                self.0.path,
                self.0.store.profile().json_depth,
                Some(observed),
                ResourceUnit::Depth,
            ));
            Err("JSON nesting exceeds resource limit")
        } else {
            Ok(())
        }
    }

    fn child(&mut self) -> JsonLimitSeed<'_> {
        JsonLimitSeed {
            store: self.0.store,
            path: self.0.path,
            operation: self.0.operation,
            depth: self.0.depth + 1,
            violation: self.0.violation,
        }
    }
}

struct LimitedWriter<'a> {
    bytes: Vec<u8>,
    limit: u64,
    code: &'static str,
    path: &'a Path,
    operation: ResourceOperation,
    error: Option<BundleStoreError>,
}

impl<'a> LimitedWriter<'a> {
    fn new(limit: u64, code: &'static str, path: &'a Path, operation: ResourceOperation) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            code,
            path,
            operation,
            error: None,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for LimitedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let observed =
            u64::try_from(self.bytes.len().saturating_add(buffer.len())).expect("usize fits u64");
        if observed > self.limit {
            self.error = Some(
                resource_error(
                    self.code,
                    self.operation,
                    ResourceStage::Serialize,
                    self.path,
                    self.limit,
                    Some(observed),
                    ResourceUnit::Bytes,
                )
                .into(),
            );
            return Err(io::Error::other("resource limit exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn check_limit(
    limit: u64,
    observed: u64,
    code: &'static str,
    operation: ResourceOperation,
    stage: ResourceStage,
    path: &Path,
    unit: ResourceUnit,
) -> Result<(), ResourceError> {
    if observed > limit {
        Err(resource_error(
            code,
            operation,
            stage,
            path,
            limit,
            Some(observed),
            unit,
        ))
    } else {
        Ok(())
    }
}

fn resource_error(
    code: &'static str,
    operation: ResourceOperation,
    stage: ResourceStage,
    path: &Path,
    limit: u64,
    observed: Option<u64>,
    unit: ResourceUnit,
) -> ResourceError {
    ResourceError {
        diagnostic: ResourceDiagnostic {
            code,
            profile: LOCAL_RESOURCE_PROFILE_NAME,
            profile_version: LOCAL_RESOURCE_PROFILE_VERSION,
            operation,
            stage,
            path: path.to_path_buf(),
            limit,
            observed,
            unit,
            retryable_without_input_change: false,
            complete_result: false,
            evidence_gap: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn diagnostic(error: BundleStoreError) -> ResourceDiagnostic {
        match error {
            BundleStoreError::Resource(error) => error.diagnostic,
            other => panic!("expected resource error, got {other:?}"),
        }
    }

    #[test]
    fn root_json_boundary_and_growth_are_bounded() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("value.json");
        fs::write(&path, b"{}\n").unwrap();
        let mut profile = LOCAL_STANDARD_V1;
        profile.root_json_bytes = 3;
        let store = BundleStore::with_profile(temp.path(), profile);
        let mut budget = ModeledBudget::default();
        assert_eq!(
            store
                .read_root_json(&path, &mut budget, ResourceOperation::Read)
                .unwrap(),
            "{}\n"
        );

        fs::write(&path, b"{}\n ").unwrap();
        let error = store
            .read_root_json(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
            )
            .unwrap_err();
        let diagnostic = diagnostic(error);
        assert_eq!(diagnostic.code, "resource.bundle.root_json_bytes");
        assert_eq!(diagnostic.limit, 3);
        assert_eq!(diagnostic.observed, Some(4));
        assert_eq!(diagnostic.stage, ResourceStage::Metadata);
        assert!(!diagnostic.retryable_without_input_change);
        assert!(!diagnostic.complete_result);
        assert!(!diagnostic.evidence_gap);
    }

    #[test]
    fn jsonl_line_record_stream_and_aggregate_limits_are_independent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("records.jsonl");
        fs::write(&path, b"{}\n{}\n").unwrap();
        let mut profile = LOCAL_STANDARD_V1;
        profile.jsonl_line_bytes = 3;
        profile.jsonl_stream_bytes = 6;
        profile.jsonl_stream_records = 2;
        profile.modeled_total_bytes = 6;
        profile.modeled_total_records = 2;
        let store = BundleStore::with_profile(temp.path(), profile);
        let mut count = 0;
        store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| {
                    count += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(count, 2);

        fs::write(&path, b"{}\n{}\n{}\n").unwrap();
        profile.jsonl_stream_bytes = 64;
        profile.modeled_total_bytes = 64;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.jsonl.records");

        fs::write(&path, b"[123]\n").unwrap();
        let error = store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.jsonl.line_bytes");

        fs::write(&path, b"{}\n{}\n\n").unwrap();
        profile.jsonl_line_bytes = 64;
        profile.jsonl_stream_bytes = 6;
        profile.jsonl_stream_records = 64;
        profile.modeled_total_bytes = 64;
        profile.modeled_total_records = 64;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.jsonl.stream_bytes");

        profile.jsonl_stream_bytes = 64;
        profile.modeled_total_bytes = 6;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.bundle.modeled_bytes");

        fs::write(&path, b"{}\n{}\n{}\n").unwrap();
        profile.modeled_total_bytes = 64;
        profile.modeled_total_records = 2;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .for_each_jsonl(
                &path,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
                |_, _, _| Ok(()),
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.bundle.modeled_records");
    }

    #[test]
    fn json_shape_root_and_attachment_boundaries_are_enforced() {
        let temp = TempDir::new().unwrap();
        let root_json = temp.path().join("root.json");
        fs::write(&root_json, br#"{"key":"abc"}"#).unwrap();
        let mut profile = LOCAL_STANDARD_V1;
        profile.scalar_string_bytes = 3;
        let store = BundleStore::with_profile(temp.path(), profile);
        store
            .read_root_json(
                &root_json,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
            )
            .unwrap();
        profile.scalar_string_bytes = 2;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .read_root_json(
                &root_json,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.json.scalar_bytes");

        fs::write(&root_json, b"[[]]").unwrap();
        profile.scalar_string_bytes = 128;
        profile.json_depth = 2;
        let store = BundleStore::with_profile(temp.path(), profile);
        store
            .read_root_json(
                &root_json,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
            )
            .unwrap();
        fs::write(&root_json, b"[[[]]]").unwrap();
        let error = store
            .read_root_json(
                &root_json,
                &mut ModeledBudget::default(),
                ResourceOperation::Read,
            )
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.json.depth");

        let attachments = temp.path().join("attachments");
        fs::create_dir(&attachments).unwrap();
        fs::write(attachments.join("one"), b"123").unwrap();
        profile.attachment_file_bytes = 3;
        profile.attachment_total_bytes = 3;
        profile.attachment_entries = 1;
        let store = BundleStore::with_profile(temp.path(), profile);
        inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap();

        profile.attachment_file_bytes = 2;
        profile.attachment_total_bytes = 64;
        profile.attachment_entries = 64;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error =
            inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.attachments.file_bytes");

        profile.attachment_file_bytes = 64;
        profile.attachment_total_bytes = 3;
        fs::write(attachments.join("two"), b"1").unwrap();
        let store = BundleStore::with_profile(temp.path(), profile);
        let error =
            inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.attachments.total_bytes");

        profile.attachment_total_bytes = 64;
        profile.attachment_entries = 1;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error =
            inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.attachments.entries");

        fs::remove_dir_all(&attachments).unwrap();
        fs::create_dir_all(attachments.join("a/b")).unwrap();
        profile.attachment_entries = 64;
        profile.attachment_depth = 2;
        let store = BundleStore::with_profile(temp.path(), profile);
        inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap();
        fs::create_dir(attachments.join("a/b/c")).unwrap();
        let error =
            inventory_attachment_tree(&store, &attachments, ResourceOperation::Read).unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.attachments.depth");

        profile.root_entries = 1;
        let store = BundleStore::with_profile(temp.path(), profile);
        let error = store
            .inventory_root(ResourceOperation::Inspect)
            .unwrap_err();
        assert_eq!(diagnostic(error).code, "resource.bundle.root_entries");
    }

    #[test]
    fn cancellation_stops_before_more_io() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("value.json");
        fs::write(&path, b"{}").unwrap();
        let token = CancellationToken::default();
        token.cancel();
        let store = BundleStore::with_cancellation(temp.path(), token);
        let error =
            read_bounded(&store, &path, 100, "resource.test", ResourceOperation::Read).unwrap_err();
        let diagnostic = diagnostic(error);
        assert_eq!(diagnostic.code, "resource.operation.cancelled");
        assert_eq!(diagnostic.unit, ResourceUnit::Checkpoints);
    }

    #[test]
    fn tiny_profile_remains_available_for_deterministic_injection() {
        let profile = BundleResourceProfile::tiny(7);
        assert_eq!(profile.root_json_bytes, 7);
        assert_eq!(profile.attachment_total_bytes, 28);
    }

    #[test]
    fn local_standard_v1_values_are_pinned() {
        let profile = LOCAL_STANDARD_V1;
        assert_eq!(profile.root_json_bytes, 4 * 1024 * 1024);
        assert_eq!(profile.jsonl_line_bytes, 256 * 1024);
        assert_eq!(profile.jsonl_stream_bytes, 128 * 1024 * 1024);
        assert_eq!(profile.jsonl_stream_records, 250_000);
        assert_eq!(profile.modeled_total_bytes, 256 * 1024 * 1024);
        assert_eq!(profile.modeled_total_records, 500_000);
        assert_eq!(profile.json_depth, 64);
        assert_eq!(profile.scalar_string_bytes, 128 * 1024);
        assert_eq!(profile.root_entries, 64);
        assert_eq!(profile.attachment_depth, 8);
        assert_eq!(profile.attachment_entries, 4_096);
        assert_eq!(profile.attachment_file_bytes, 512 * 1024 * 1024);
        assert_eq!(profile.attachment_total_bytes, 2 * 1024 * 1024 * 1024);
    }
}
