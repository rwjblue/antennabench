use std::{collections::HashSet, path::Path};

use antennabench_core::{
    codes, v2::CurrentBundleContents, validate_bundle_report, AnalysisFile, AntennasFile, Band,
    BundleContents, BundleDiagnostic, BundleDiagnosticCategory, BundleDiagnosticLocation,
    BundleDiagnosticSeverity, BundleFileRole, BundleManifest, BundleRecordKind,
    BundleValidationError, BundleValidationProfile, BundleValidationReport, ObservationRecord,
    OperatorEvent, PropagationRecord, RigRecord, Schedule, Station, WsjtXRecord,
    ALL_TYPED_OPERATIONS, SCHEMA_VERSION_V1, SCHEMA_VERSION_V2, SCHEMA_VERSION_V3,
    SCHEMA_VERSION_V4, SCHEMA_VERSION_V5, SCHEMA_VERSION_V6, WRITE_OPERATIONS,
};
use serde::{
    de::{DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserializer,
};
use serde_json::Value;

use super::{
    ensure_bundle_root,
    resource::{inventory_attachment_tree, ModeledBudget, ResourceOperation},
    BundleStore, BundleStoreError,
};

#[derive(Debug, Clone)]
pub struct BundleInspection {
    pub(super) current: Option<CurrentBundleContents>,
    pub(super) report: BundleValidationReport,
    pub(super) native_planned_bands: Vec<Band>,
}

impl BundleInspection {
    pub(super) fn projected(
        current: Option<CurrentBundleContents>,
        report: BundleValidationReport,
    ) -> Self {
        Self {
            current,
            report,
            native_planned_bands: Vec::new(),
        }
    }

    pub fn bundle(&self) -> Option<&BundleContents> {
        self.current.as_ref().map(|current| &current.bundle)
    }

    pub fn current(&self) -> Option<&CurrentBundleContents> {
        self.current.as_ref()
    }

    pub fn report(&self) -> &BundleValidationReport {
        &self.report
    }

    /// Native plan bands retained when a newer schema's intent schedule cannot
    /// be represented by the compatibility projection without actual events.
    pub fn planned_bands(&self) -> &[Band] {
        &self.native_planned_bands
    }

    pub fn into_parts(self) -> (Option<BundleContents>, BundleValidationReport) {
        (self.current.map(|current| current.bundle), self.report)
    }

    pub fn into_current_parts(self) -> (Option<CurrentBundleContents>, BundleValidationReport) {
        (self.current, self.report)
    }
}

#[derive(Debug, Clone)]
struct DocumentContext {
    file: BundleFileRole,
    record_kind: Option<BundleRecordKind>,
    record_id: Option<String>,
    record_index: Option<usize>,
    physical_line: Option<usize>,
}

impl DocumentContext {
    fn root(file: BundleFileRole) -> Self {
        Self {
            file,
            record_kind: None,
            record_id: None,
            record_index: None,
            physical_line: None,
        }
    }

    fn record(file: BundleFileRole, record_index: usize, physical_line: usize) -> Self {
        Self {
            file,
            record_kind: record_kind_for_file(file),
            record_id: None,
            record_index: Some(record_index),
            physical_line: Some(physical_line),
        }
    }

    fn with_record_id(mut self, value: &Value) -> Self {
        let field = match self.file {
            BundleFileRole::Events => "event_id",
            BundleFileRole::Observations => "observation_id",
            BundleFileRole::AdapterRecords => "record_id",
            BundleFileRole::WsjtX | BundleFileRole::Rig | BundleFileRole::Propagation => {
                "record_id"
            }
            _ => return self,
        };
        self.record_id = value.get(field).and_then(Value::as_str).map(str::to_string);
        self
    }
}

#[derive(Debug)]
struct InspectedDocument {
    value: Option<Value>,
    diagnostics: Vec<BundleDiagnostic>,
}

impl BundleStore {
    pub(super) fn inspect_manifest_layout(&self) -> Result<BundleManifest, BundleStoreError> {
        ensure_bundle_root(self.root())?;
        let mut budget = ModeledBudget::default();
        let manifest_path = self.bundle_path("manifest.json")?;
        let contents =
            self.read_root_json(&manifest_path, &mut budget, ResourceOperation::Inspect)?;
        let context = DocumentContext::root(BundleFileRole::Manifest);
        let document = inspect_document(&contents, context.clone());
        let mut report = BundleValidationReport::new(document.diagnostics);
        let manifest: Option<BundleManifest> = document
            .value
            .and_then(|value| project_value(value, context, &mut report));
        let Some(manifest) = manifest else {
            return Err(BundleValidationError::from_report(report).into());
        };
        if manifest.schema_version != SCHEMA_VERSION_V1 {
            report.extend([unsupported_schema_version_diagnostic(
                manifest.schema_version,
            )]);
        }
        if !report.allows(BundleValidationProfile::CompatibilityRead) {
            return Err(BundleValidationError::from_report(report).into());
        }

        Ok(manifest)
    }

    pub fn inspect(&self) -> Result<BundleInspection, BundleStoreError> {
        ensure_bundle_root(self.root())?;
        let mut budget = ModeledBudget::default();
        let manifest_path = self.bundle_path("manifest.json")?;
        let manifest_contents =
            self.read_root_json(&manifest_path, &mut budget, ResourceOperation::Inspect)?;
        let manifest_document = inspect_document(
            &manifest_contents,
            DocumentContext::root(BundleFileRole::Manifest),
        );
        let mut report = BundleValidationReport::new(manifest_document.diagnostics);
        let Some(manifest_value) = manifest_document.value else {
            return Ok(BundleInspection {
                current: None,
                report,
                native_planned_bands: Vec::new(),
            });
        };
        if !report.allows(BundleValidationProfile::CompatibilityRead) {
            return Ok(BundleInspection {
                current: None,
                report,
                native_planned_bands: Vec::new(),
            });
        }

        let schema_version = manifest_value
            .get("schema_version")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok());
        if schema_version == Some(SCHEMA_VERSION_V2) {
            let report = BundleValidationReport::new(
                report
                    .into_diagnostics()
                    .into_iter()
                    .filter(|diagnostic| diagnostic.code != codes::UNKNOWN_FIELD)
                    .collect(),
            );
            return self.inspect_v2(report);
        }
        if matches!(
            schema_version,
            Some(SCHEMA_VERSION_V3 | SCHEMA_VERSION_V4 | SCHEMA_VERSION_V5 | SCHEMA_VERSION_V6)
        ) {
            let report = BundleValidationReport::new(
                report
                    .into_diagnostics()
                    .into_iter()
                    .filter(|diagnostic| diagnostic.code != codes::UNKNOWN_FIELD)
                    .collect(),
            );
            return self.inspect_v3(report);
        }
        if let Some(actual) = schema_version.filter(|version| *version != SCHEMA_VERSION_V1) {
            return Err(BundleStoreError::UnsupportedSchemaVersion { actual });
        }

        let Some(manifest) = project_value::<BundleManifest>(
            manifest_value,
            DocumentContext::root(BundleFileRole::Manifest),
            &mut report,
        ) else {
            return Ok(BundleInspection {
                current: None,
                report,
                native_planned_bands: Vec::new(),
            });
        };
        if manifest.schema_version != SCHEMA_VERSION_V1 {
            report.extend([unsupported_schema_version_diagnostic(
                manifest.schema_version,
            )]);
            return Ok(BundleInspection {
                current: None,
                report,
                native_planned_bands: Vec::new(),
            });
        }

        let paths = self.bundle_paths(&manifest.files)?;
        paths.ensure_readable_targets()?;
        super::ensure_directory(&paths.attachments_dir)?;
        self.inventory_root(ResourceOperation::Inspect)?;
        inventory_attachment_tree(self, &paths.attachments_dir, ResourceOperation::Read)?;

        let station = self.inspect_root_file::<Station>(
            &paths.station,
            BundleFileRole::Station,
            &mut report,
            &mut budget,
        )?;
        let antennas = self.inspect_root_file::<AntennasFile>(
            &paths.antennas,
            BundleFileRole::Antennas,
            &mut report,
            &mut budget,
        )?;
        let schedule = self.inspect_root_file::<Schedule>(
            &paths.schedule,
            BundleFileRole::Schedule,
            &mut report,
            &mut budget,
        )?;
        let events = self.inspect_jsonl_file::<OperatorEvent>(
            &paths.events,
            BundleFileRole::Events,
            &mut report,
            &mut budget,
        )?;
        let observations = self.inspect_jsonl_file::<ObservationRecord>(
            &paths.observations,
            BundleFileRole::Observations,
            &mut report,
            &mut budget,
        )?;
        let wsjtx = self.inspect_jsonl_file::<WsjtXRecord>(
            &paths.wsjtx,
            BundleFileRole::WsjtX,
            &mut report,
            &mut budget,
        )?;
        let rig = self.inspect_jsonl_file::<RigRecord>(
            &paths.rig,
            BundleFileRole::Rig,
            &mut report,
            &mut budget,
        )?;
        let propagation = self.inspect_jsonl_file::<PropagationRecord>(
            &paths.propagation,
            BundleFileRole::Propagation,
            &mut report,
            &mut budget,
        )?;
        let analysis = self.inspect_root_file::<AnalysisFile>(
            &paths.analysis,
            BundleFileRole::Analysis,
            &mut report,
            &mut budget,
        )?;

        let Some((
            station,
            antennas,
            schedule,
            events,
            observations,
            wsjtx,
            rig,
            propagation,
            analysis,
        )) = station
            .zip(antennas)
            .zip(schedule)
            .zip(events)
            .zip(observations)
            .zip(wsjtx)
            .zip(rig)
            .zip(propagation)
            .zip(analysis)
            .map(
                |(
                    (
                        ((((((station, antennas), schedule), events), observations), wsjtx), rig),
                        propagation,
                    ),
                    analysis,
                )| {
                    (
                        station,
                        antennas,
                        schedule,
                        events,
                        observations,
                        wsjtx,
                        rig,
                        propagation,
                        analysis,
                    )
                },
            )
        else {
            return Ok(BundleInspection {
                current: None,
                report,
                native_planned_bands: Vec::new(),
            });
        };

        let bundle = BundleContents {
            manifest,
            station,
            antennas,
            schedule,
            events,
            observations,
            wsjtx,
            rig,
            propagation,
            analysis,
        };
        report.extend(validate_bundle_report(&bundle).into_diagnostics());
        let bundle = report
            .allows(BundleValidationProfile::CompatibilityRead)
            .then_some(bundle);

        Ok(BundleInspection {
            current: bundle.map(CurrentBundleContents::from_v1),
            report,
            native_planned_bands: Vec::new(),
        })
    }
    fn inspect_root_file<T: DeserializeOwned>(
        &self,
        path: &Path,
        file: BundleFileRole,
        report: &mut BundleValidationReport,
        budget: &mut ModeledBudget,
    ) -> Result<Option<T>, BundleStoreError> {
        let contents = self.read_root_json(path, budget, ResourceOperation::Read)?;
        let context = DocumentContext::root(file);
        let document = inspect_document(&contents, context.clone());
        report.extend(document.diagnostics);
        Ok(document
            .value
            .and_then(|value| project_value(value, context, report)))
    }

    fn inspect_jsonl_file<T: DeserializeOwned>(
        &self,
        path: &Path,
        file: BundleFileRole,
        report: &mut BundleValidationReport,
        budget: &mut ModeledBudget,
    ) -> Result<Option<Vec<T>>, BundleStoreError> {
        let mut records = Vec::new();
        let mut projected = true;
        self.for_each_jsonl(
            path,
            budget,
            ResourceOperation::Read,
            |physical_line, record_index, line| {
                let base_context = DocumentContext::record(file, record_index, physical_line);
                let document = inspect_document(line, base_context.clone());
                report.extend(document.diagnostics);
                match document.value {
                    Some(value) => {
                        let context = base_context.with_record_id(&value);
                        if let Some(record) = project_value(value, context, report) {
                            records.push(record);
                        } else {
                            projected = false;
                        }
                    }
                    None => projected = false,
                }
                Ok(())
            },
        )?;
        Ok(projected.then_some(records))
    }
}

fn unsupported_schema_version_diagnostic(schema_version: u16) -> BundleDiagnostic {
    BundleDiagnostic {
        code: codes::UNSUPPORTED_SCHEMA_VERSION.to_string(),
        category: BundleDiagnosticCategory::Wire,
        severity: BundleDiagnosticSeverity::Error,
        blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
        location: BundleDiagnosticLocation {
            field_path: Some("/schema_version".to_string()),
            ..BundleDiagnosticLocation::file(BundleFileRole::Manifest)
        },
        message: format!(
            "schema version {schema_version} is not supported; supported versions are {SCHEMA_VERSION_V1}, {SCHEMA_VERSION_V2}, {SCHEMA_VERSION_V3}, {SCHEMA_VERSION_V4}, {SCHEMA_VERSION_V5}, and {SCHEMA_VERSION_V6}"
        ),
        related_locations: Vec::new(),
    }
}

fn inspect_document(contents: &str, context: DocumentContext) -> InspectedDocument {
    let scan_result = scan_duplicate_members(contents);

    let mut duplicates = match scan_result {
        Ok(duplicates) => duplicates,
        Err(error) => {
            return InspectedDocument {
                value: None,
                diagnostics: vec![invalid_json_diagnostic(&context, error)],
            };
        }
    };

    let value = match serde_json::from_str::<Value>(contents) {
        Ok(value) => value,
        Err(error) => {
            return InspectedDocument {
                value: None,
                diagnostics: vec![invalid_json_diagnostic(&context, error.to_string())],
            }
        }
    };
    let context = context.with_record_id(&value);
    duplicates.sort();
    duplicates.dedup();
    let mut diagnostics = duplicates
        .into_iter()
        .map(|path| duplicate_member_diagnostic(&context, &value, path))
        .collect::<Vec<_>>();
    diagnostics.extend(unknown_field_diagnostics(&context, &value));

    InspectedDocument {
        value: Some(value),
        diagnostics,
    }
}

pub(super) fn scan_duplicate_members(contents: &str) -> Result<Vec<String>, String> {
    let mut duplicates = Vec::new();
    let mut deserializer = serde_json::Deserializer::from_str(contents);
    JsonScanSeed {
        path: String::new(),
        duplicates: &mut duplicates,
    }
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end())
    .map_err(|error| error.to_string())?;
    duplicates.sort();
    duplicates.dedup();
    Ok(duplicates)
}

fn project_value<T: DeserializeOwned>(
    value: Value,
    context: DocumentContext,
    report: &mut BundleValidationReport,
) -> Option<T> {
    match serde_json::from_value(value) {
        Ok(value) => Some(value),
        Err(error) => {
            report.extend([invalid_json_diagnostic(&context, error.to_string())]);
            None
        }
    }
}

fn invalid_json_diagnostic(context: &DocumentContext, message: String) -> BundleDiagnostic {
    BundleDiagnostic {
        code: codes::INVALID_JSON.to_string(),
        category: BundleDiagnosticCategory::Wire,
        severity: BundleDiagnosticSeverity::Error,
        blocked_operations: ALL_TYPED_OPERATIONS.to_vec(),
        location: location_for_path(context, None, None),
        message: format!("modeled JSON cannot be projected unambiguously: {message}"),
        related_locations: Vec::new(),
    }
}

fn duplicate_member_diagnostic(
    context: &DocumentContext,
    value: &Value,
    path: String,
) -> BundleDiagnostic {
    let raw = is_raw_path(context.file, &path);
    BundleDiagnostic {
        code: if raw {
            codes::DUPLICATE_RAW_MEMBER
        } else {
            codes::DUPLICATE_MEMBER
        }
        .to_string(),
        category: BundleDiagnosticCategory::Wire,
        severity: if raw {
            BundleDiagnosticSeverity::Warning
        } else {
            BundleDiagnosticSeverity::Error
        },
        blocked_operations: if raw {
            WRITE_OPERATIONS.to_vec()
        } else {
            ALL_TYPED_OPERATIONS.to_vec()
        },
        location: location_for_path(context, Some(value), Some(path.as_str())),
        message: if raw {
            "duplicate member in legacy raw source evidence is preserved but not interpreted"
                .to_string()
        } else {
            "duplicate member makes modeled JSON interpretation ambiguous".to_string()
        },
        related_locations: Vec::new(),
    }
}

fn unknown_field_diagnostics(context: &DocumentContext, value: &Value) -> Vec<BundleDiagnostic> {
    let mut paths = Vec::new();
    match context.file {
        BundleFileRole::Manifest => {
            collect_unknown(
                value,
                "",
                &[
                    "schema_version",
                    "session_id",
                    "created_at",
                    "app_version",
                    "files",
                ],
                &mut paths,
            );
            if let Some(files) = value.get("files") {
                collect_unknown(
                    files,
                    "/files",
                    &[
                        "manifest",
                        "station",
                        "antennas",
                        "schedule",
                        "events",
                        "observations",
                        "wsjtx",
                        "rig",
                        "propagation",
                        "analysis",
                        "attachments_dir",
                    ],
                    &mut paths,
                );
            }
        }
        BundleFileRole::SessionState | BundleFileRole::AdapterRecords => {}
        BundleFileRole::Station => collect_unknown(
            value,
            "",
            &[
                "schema_version",
                "session_id",
                "callsign",
                "grid",
                "power_watts",
                "operator_notes",
            ],
            &mut paths,
        ),
        BundleFileRole::Antennas => {
            collect_unknown(
                value,
                "",
                &["schema_version", "session_id", "antennas"],
                &mut paths,
            );
            collect_array_objects(
                value.get("antennas"),
                "/antennas",
                &[
                    "label",
                    "facets",
                    "height_m",
                    "radial_count",
                    "radial_length_m",
                    "orientation_degrees",
                    "tuner",
                    "feedline",
                    "notes",
                ],
                &mut paths,
            );
        }
        BundleFileRole::Schedule => {
            collect_unknown(
                value,
                "",
                &["schema_version", "session_id", "mode", "goal", "slots"],
                &mut paths,
            );
            collect_array_objects(
                value.get("slots"),
                "/slots",
                &[
                    "slot_id",
                    "sequence_number",
                    "starts_at",
                    "duration_seconds",
                    "guard_seconds",
                    "band",
                    "antenna_label",
                ],
                &mut paths,
            );
        }
        BundleFileRole::Events => {
            collect_unknown(
                value,
                "",
                &["meta", "event_id", "slot_id", "event_type", "note"],
                &mut paths,
            );
            collect_meta_unknown(value, &mut paths);
        }
        BundleFileRole::Observations => {
            collect_unknown(
                value,
                "",
                &[
                    "meta",
                    "observation_id",
                    "observation_kind",
                    "band",
                    "frequency_hz",
                    "mode",
                    "reporter_call",
                    "heard_call",
                    "reporter_grid",
                    "heard_grid",
                    "distance_km",
                    "azimuth_degrees",
                    "snr_db",
                    "drift_hz_per_minute",
                    "power_watts",
                    "slot_id",
                    "slot_label",
                    "slot_confidence",
                    "raw",
                ],
                &mut paths,
            );
            collect_meta_unknown(value, &mut paths);
        }
        BundleFileRole::WsjtX => {
            collect_unknown(
                value,
                "",
                &["meta", "record_id", "message_type", "raw"],
                &mut paths,
            );
            collect_meta_unknown(value, &mut paths);
        }
        BundleFileRole::Rig => {
            collect_unknown(
                value,
                "",
                &["meta", "record_id", "status", "frequency_hz", "mode", "raw"],
                &mut paths,
            );
            collect_meta_unknown(value, &mut paths);
        }
        BundleFileRole::Propagation => {
            collect_unknown(
                value,
                "",
                &[
                    "meta",
                    "record_id",
                    "observed_at",
                    "solar_flux_f107",
                    "sunspot_number",
                    "kp_index",
                    "a_index",
                    "solar_wind_speed_kms",
                    "bz_nt",
                    "alerts",
                    "daylight_state",
                    "raw",
                ],
                &mut paths,
            );
            collect_meta_unknown(value, &mut paths);
        }
        BundleFileRole::Analysis => collect_unknown(
            value,
            "",
            &[
                "schema_version",
                "session_id",
                "generated_at",
                "status",
                "notes",
            ],
            &mut paths,
        ),
    }

    paths
        .into_iter()
        .map(|path| BundleDiagnostic {
            code: codes::UNKNOWN_FIELD.to_string(),
            category: BundleDiagnosticCategory::Wire,
            severity: BundleDiagnosticSeverity::Warning,
            blocked_operations: WRITE_OPERATIONS.to_vec(),
            location: location_for_path(context, Some(value), Some(path.as_str())),
            message: "unknown field is excluded from normalized meaning and retained only in original bytes".to_string(),
            related_locations: Vec::new(),
        })
        .collect()
}

fn collect_meta_unknown(value: &Value, paths: &mut Vec<String>) {
    if let Some(meta) = value.get("meta") {
        collect_unknown(
            meta,
            "/meta",
            &["schema_version", "session_id", "timestamp", "source"],
            paths,
        );
    }
}

fn collect_array_objects(
    value: Option<&Value>,
    base_path: &str,
    known: &[&str],
    paths: &mut Vec<String>,
) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, value) in values.iter().enumerate() {
        collect_unknown(value, &format!("{base_path}/{index}"), known, paths);
    }
}

fn collect_unknown(value: &Value, base_path: &str, known: &[&str], paths: &mut Vec<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    let known = known.iter().copied().collect::<HashSet<_>>();
    for key in object.keys() {
        if !known.contains(key.as_str()) {
            paths.push(format!("{base_path}/{}", escape_json_pointer(key)));
        }
    }
}

fn location_for_path(
    context: &DocumentContext,
    value: Option<&Value>,
    path: Option<&str>,
) -> BundleDiagnosticLocation {
    let mut location = BundleDiagnosticLocation {
        file: context.file,
        record_kind: context.record_kind,
        record_id: context.record_id.clone(),
        record_index: context.record_index,
        physical_line: context.physical_line,
        field_path: path.map(str::to_string),
    };
    let Some(path) = path else {
        return location;
    };
    let segments = path.split('/').skip(1).collect::<Vec<_>>();
    if context.file == BundleFileRole::Antennas && segments.first() == Some(&"antennas") {
        if let Some(index) = segments
            .get(1)
            .and_then(|index| index.parse::<usize>().ok())
        {
            location.record_kind = Some(BundleRecordKind::Antenna);
            location.record_index = Some(index);
            location.record_id = value
                .and_then(|value| value.get("antennas"))
                .and_then(|value| value.get(index))
                .and_then(|value| value.get("label"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    } else if context.file == BundleFileRole::Schedule && segments.first() == Some(&"slots") {
        if let Some(index) = segments
            .get(1)
            .and_then(|index| index.parse::<usize>().ok())
        {
            location.record_kind = Some(BundleRecordKind::Slot);
            location.record_index = Some(index);
            location.record_id = value
                .and_then(|value| value.get("slots"))
                .and_then(|value| value.get(index))
                .and_then(|value| value.get("slot_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    location
}

fn is_raw_path(file: BundleFileRole, path: &str) -> bool {
    matches!(
        file,
        BundleFileRole::Observations
            | BundleFileRole::AdapterRecords
            | BundleFileRole::WsjtX
            | BundleFileRole::Rig
            | BundleFileRole::Propagation
    ) && path.starts_with("/raw/")
}

fn record_kind_for_file(file: BundleFileRole) -> Option<BundleRecordKind> {
    match file {
        BundleFileRole::Events => Some(BundleRecordKind::OperatorEvent),
        BundleFileRole::Observations => Some(BundleRecordKind::Observation),
        BundleFileRole::AdapterRecords => Some(BundleRecordKind::AdapterRecord),
        BundleFileRole::WsjtX => Some(BundleRecordKind::WsjtXRecord),
        BundleFileRole::Rig => Some(BundleRecordKind::RigRecord),
        BundleFileRole::Propagation => Some(BundleRecordKind::PropagationRecord),
        _ => None,
    }
}

fn escape_json_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

struct JsonScanSeed<'a> {
    path: String,
    duplicates: &'a mut Vec<String>,
}

impl<'de> DeserializeSeed<'de> for JsonScanSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonScanVisitor {
            path: self.path,
            duplicates: self.duplicates,
        })
    }
}

struct JsonScanVisitor<'a> {
    path: String,
    duplicates: &'a mut Vec<String>,
}

impl<'de> Visitor<'de> for JsonScanVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        JsonScanSeed {
            path: self.path,
            duplicates: self.duplicates,
        }
        .deserialize(deserializer)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut index = 0;
        while sequence
            .next_element_seed(JsonScanSeed {
                path: format!("{}/{}", self.path, index),
                duplicates: self.duplicates,
            })?
            .is_some()
        {
            index += 1;
        }
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            let path = format!("{}/{}", self.path, escape_json_pointer(&key));
            if !seen.insert(key) {
                self.duplicates.push(path.clone());
            }
            map.next_value_seed(JsonScanSeed {
                path,
                duplicates: self.duplicates,
            })?;
        }
        Ok(())
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        JsonScanSeed {
            path: self.path,
            duplicates: self.duplicates,
        }
        .deserialize(deserializer)
    }

    fn visit_bytes<E>(self, _value: &[u8]) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_byte_buf<E>(self, _value: Vec<u8>) -> Result<Self::Value, E> {
        Ok(())
    }
}
