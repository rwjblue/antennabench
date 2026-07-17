use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration as StdDuration, Instant},
};

use antennabench_core::{
    project_wspr_run_v3, AntennaControlCommandV5, AntennaControlContextV5,
    AntennaControlDispositionV5, AntennaControlInvocationPolicyV5, AntennaControlInvocationV5,
    AntennaControlOutputEncodingV5, AntennaControlOutputV5, AntennaControlPolicyV5,
    AntennaControlRoleV5, BundleV3Contents, ExperimentMode, MutationMember, Provenance,
    RecordMetaV3, RecordSource, RigRecordV3, SessionLifecycleV2, WsprCycleIntentV3,
    COMMAND_ARGUMENT_COUNT_MAX, COMMAND_ARGUMENT_MAX_BYTES, COMMAND_INVOCATION_MAX_BYTES,
    COMMAND_OUTPUT_MAX_BYTES, COMMAND_PROGRAM_MAX_BYTES, COMMAND_TEMPLATE_MAX_BYTES,
    SCHEMA_VERSION_V5,
};
use antennabench_storage::{BundleStore, LiveAntennaControlMutationV5, SystemLivePersistenceHooks};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::{
    conductor::{live_error_payload, ConductorSessionState, ControllerActionPort},
    open_session::{
        active_session_source, check_ipc_payload, with_foreground_operation, ActiveSessionState,
        SessionErrorKind, SessionErrorPayload,
    },
};

const CATALOG_VERSION: u16 = 1;
const CONTROLLER_IPC_BYTES: u64 = 512 * 1024;
const PROFILE_NAME_MAX_BYTES: usize = 256;
const PROFILE_IDENTITY_MAX_BYTES: usize = 256;
const TARGET_MAX_BYTES: usize = COMMAND_ARGUMENT_MAX_BYTES;
const PROFILE_TIMEOUT_MIN_SECONDS: u8 = 1;
const PROFILE_TIMEOUT_MAX_SECONDS: u8 = 60;
const DEFAULT_PROFILE_TIMEOUT_SECONDS: u8 = 10;
const POLL_INTERVAL: StdDuration = StdDuration::from_millis(10);

#[derive(Default)]
pub(crate) struct AntennaControllerState {
    runtime: Mutex<ControllerRuntime>,
    generation: Arc<AtomicU64>,
}

#[derive(Default)]
struct ControllerRuntime {
    attached: Option<RuntimeAssociation>,
    last_attempt: Option<ControllerAttemptSummary>,
}

#[derive(Debug, Clone)]
struct RuntimeAssociation {
    source: PathBuf,
    session_id: String,
    profile_id: String,
    profile_revision: String,
    targets: BTreeMap<String, String>,
    armed: bool,
    generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ControllerAttemptSummary {
    intent_id: String,
    successful_switch: bool,
    successful_verification: Option<bool>,
    detail: String,
    diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerCommandTemplate {
    pub(crate) program_template: String,
    pub(crate) argument_templates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerProfile {
    pub(crate) profile_id: String,
    pub(crate) revision: String,
    pub(crate) name: String,
    pub(crate) switch_command: ControllerCommandTemplate,
    pub(crate) verification_command: Option<ControllerCommandTemplate>,
    pub(crate) timeout_seconds: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerCommandDraft {
    #[serde(default)]
    pub(crate) one_line: String,
    #[serde(default)]
    pub(crate) program: String,
    #[serde(default)]
    pub(crate) arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerProfileDraft {
    #[serde(default)]
    pub(crate) profile_id: Option<String>,
    pub(crate) name: String,
    pub(crate) switch_command: ControllerCommandDraft,
    #[serde(default)]
    pub(crate) verification_command: Option<ControllerCommandDraft>,
    #[serde(default = "default_timeout")]
    pub(crate) timeout_seconds: u8,
}

fn default_timeout() -> u8 {
    DEFAULT_PROFILE_TIMEOUT_SECONDS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerTargetDraft {
    pub(crate) antenna_label: String,
    pub(crate) target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupControllerDraft {
    pub(crate) enabled: bool,
    pub(crate) arm_for_session: bool,
    pub(crate) profile: ControllerProfileDraft,
    pub(crate) targets: Vec<ControllerTargetDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedSetupController {
    pub(crate) profile: ControllerProfile,
    pub(crate) targets: BTreeMap<String, String>,
    pub(crate) arm_for_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerInvocationPreview {
    pub(crate) intent_id: String,
    pub(crate) antenna: String,
    pub(crate) target: String,
    pub(crate) mode: ExperimentMode,
    pub(crate) direction: String,
    pub(crate) switch_program: String,
    pub(crate) switch_arguments: Vec<String>,
    pub(crate) verification_program: Option<String>,
    pub(crate) verification_arguments: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetupControllerReview {
    pub(crate) profile_id: String,
    pub(crate) profile_revision: String,
    pub(crate) profile_name: String,
    pub(crate) timeout_seconds: u8,
    pub(crate) arm_for_session: bool,
    pub(crate) disclosure: &'static str,
    pub(crate) invocations: Vec<ControllerInvocationPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ControllerCatalog {
    version: u16,
    profiles: Vec<ControllerProfile>,
    associations: Vec<PersistedAssociation>,
}

impl Default for ControllerCatalog {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            profiles: Vec::new(),
            associations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAssociation {
    source: String,
    session_id: String,
    profile_id: String,
    profile_revision: String,
    targets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerCatalogView {
    input_style: &'static str,
    profiles: Vec<ControllerProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveControllerView {
    policy: &'static str,
    attached: bool,
    armed: bool,
    profile_id: Option<String>,
    profile_revision: Option<String>,
    profile_name: Option<String>,
    targets: BTreeMap<String, String>,
    stale_profile: bool,
    last_attempt: Option<ControllerAttemptSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachControllerRequest {
    profile_id: String,
    profile_revision: String,
    targets: Vec<ControllerTargetDraft>,
    armed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunControllerRequest {
    action_token: String,
    expected_revision: u64,
    intent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunControllerOutcome {
    revision: u64,
    intent_id: String,
    switch_disposition: AntennaControlDispositionV5,
    verification_disposition: Option<AntennaControlDispositionV5>,
    verification_ran: bool,
    manual_ready_required: bool,
    detail: String,
    diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedCommand {
    command: AntennaControlCommandV5,
}

#[derive(Debug)]
struct CapturedBytes {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
struct ProcessResult {
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    disposition: AntennaControlDispositionV5,
    stdout: AntennaControlOutputV5,
    stderr: AntennaControlOutputV5,
}

#[derive(Debug)]
struct AttemptExecutionError {
    role: AntennaControlRoleV5,
    detail: String,
}

impl AntennaControllerState {
    pub(crate) fn revoke(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.attached = None;
            runtime.last_attempt = None;
        }
    }

    fn attach(
        &self,
        source: PathBuf,
        session_id: String,
        profile: &ControllerProfile,
        targets: BTreeMap<String, String>,
        armed: bool,
    ) -> Result<(), SessionErrorPayload> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut runtime = self.runtime.lock().map_err(|_| {
            SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
        })?;
        runtime.attached = Some(RuntimeAssociation {
            source,
            session_id,
            profile_id: profile.profile_id.clone(),
            profile_revision: profile.revision.clone(),
            targets,
            armed,
            generation,
        });
        runtime.last_attempt = None;
        Ok(())
    }
}

fn catalog_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("antenna-controller-profiles.json")
}

fn previous_catalog_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("antenna-controller-profiles.previous.json")
}

fn resolved_app_data_dir(app: &AppHandle) -> Result<PathBuf, SessionErrorPayload> {
    app.path().app_data_dir().map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "The system application-data directory is unavailable.",
            error.to_string(),
        )
    })
}

pub(crate) fn controller_profiles_for_app(
    app: &AppHandle,
) -> Result<Vec<ControllerProfile>, SessionErrorPayload> {
    Ok(read_catalog(&resolved_app_data_dir(app)?)?.profiles)
}

fn read_catalog(app_data_dir: &Path) -> Result<ControllerCatalog, SessionErrorPayload> {
    let path = catalog_path(app_data_dir);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::read(previous_catalog_path(app_data_dir)) {
                Ok(bytes) => bytes,
                Err(previous) if previous.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(ControllerCatalog::default());
                }
                Err(previous) => {
                    return Err(SessionErrorPayload::new(
                        SessionErrorKind::Filesystem,
                        "Local antenna-controller profile recovery data could not be read.",
                        previous.to_string(),
                    ));
                }
            }
        }
        Err(error) => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "Local antenna-controller profiles could not be read.",
                format!("{}: {error}", path.display()),
            ))
        }
    };
    if bytes.len() as u64 > CONTROLLER_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.controller_catalog_bytes",
            "antenna_controller_catalog",
            CONTROLLER_IPC_BYTES,
            Some(bytes.len() as u64),
            "bytes",
        ));
    }
    let catalog: ControllerCatalog = serde_json::from_slice(&bytes).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Local antenna-controller profiles are not valid.",
            format!("{}: {error}", path.display()),
        )
    })?;
    if catalog.version != CATALOG_VERSION {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Local antenna-controller profiles use an unsupported version.",
            format!("catalog version {}", catalog.version),
        ));
    }
    validate_catalog(&catalog).map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Local antenna-controller profiles are not valid.",
            detail,
        )
    })?;
    Ok(catalog)
}

fn validate_command_template(template: &ControllerCommandTemplate) -> Result<(), String> {
    if template.program_template.is_empty()
        || template.program_template.len() > COMMAND_TEMPLATE_MAX_BYTES
    {
        return Err("a command program template is empty or exceeds its fixed limit".into());
    }
    if template.argument_templates.len() > COMMAND_ARGUMENT_COUNT_MAX {
        return Err("a command has too many argument templates".into());
    }
    if template
        .argument_templates
        .iter()
        .any(|argument| argument.len() > COMMAND_ARGUMENT_MAX_BYTES)
    {
        return Err("a command argument template exceeds its fixed limit".into());
    }
    Ok(())
}

fn validate_catalog(catalog: &ControllerCatalog) -> Result<(), String> {
    let mut profile_ids = BTreeSet::new();
    for profile in &catalog.profiles {
        if profile.profile_id.is_empty()
            || profile.profile_id.len() > PROFILE_IDENTITY_MAX_BYTES
            || profile.revision.is_empty()
            || profile.revision.len() > PROFILE_IDENTITY_MAX_BYTES
        {
            return Err(
                "a profile identity or revision is empty or exceeds its fixed limit".into(),
            );
        }
        if !profile_ids.insert(profile.profile_id.as_str()) {
            return Err("controller profile identities must be unique".into());
        }
        if profile.name.trim().is_empty() || profile.name.len() > PROFILE_NAME_MAX_BYTES {
            return Err("a controller profile name is empty or exceeds its fixed limit".into());
        }
        if !(PROFILE_TIMEOUT_MIN_SECONDS..=PROFILE_TIMEOUT_MAX_SECONDS)
            .contains(&profile.timeout_seconds)
        {
            return Err("a controller timeout is outside one through sixty seconds".into());
        }
        validate_command_template(&profile.switch_command)?;
        if let Some(verification) = &profile.verification_command {
            validate_command_template(verification)?;
        }
    }
    let mut association_ids = BTreeSet::new();
    for association in &catalog.associations {
        if association.source.is_empty()
            || association.session_id.is_empty()
            || association.profile_id.is_empty()
            || association.profile_revision.is_empty()
        {
            return Err("a saved controller association has an empty identity".into());
        }
        if !profile_ids.contains(association.profile_id.as_str()) {
            return Err("a saved controller association references an unknown profile".into());
        }
        if !association_ids.insert((association.source.as_str(), association.session_id.as_str())) {
            return Err("saved controller associations must be unique per session path".into());
        }
        if association.targets.is_empty()
            || association.targets.iter().any(|(label, target)| {
                label.trim().is_empty()
                    || label.len() > TARGET_MAX_BYTES
                    || target.trim().is_empty()
                    || target.len() > TARGET_MAX_BYTES
            })
        {
            return Err("a saved controller association has an invalid target mapping".into());
        }
    }
    Ok(())
}

fn write_catalog(
    app_data_dir: &Path,
    catalog: &ControllerCatalog,
) -> Result<(), SessionErrorPayload> {
    validate_catalog(catalog).map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Local antenna-controller profiles are not valid.",
            detail,
        )
    })?;
    fs::create_dir_all(app_data_dir).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "Local antenna-controller storage could not be prepared.",
            format!("{}: {error}", app_data_dir.display()),
        )
    })?;
    let bytes = serde_json::to_vec_pretty(catalog).map_err(|error| {
        SessionErrorPayload::report_pipeline(format!(
            "controller catalog serialization failed: {error}"
        ))
    })?;
    if bytes.len() as u64 > CONTROLLER_IPC_BYTES {
        return Err(SessionErrorPayload::resource(
            SessionErrorKind::Resource,
            "resource.desktop.controller_catalog_bytes",
            "antenna_controller_catalog",
            CONTROLLER_IPC_BYTES,
            Some(bytes.len() as u64),
            "bytes",
        ));
    }
    let path = catalog_path(app_data_dir);
    let temporary = app_data_dir.join("antenna-controller-profiles.json.tmp");
    let mut file = File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "Local antenna-controller profiles could not be staged.",
                format!("{}: {error}", temporary.display()),
            )
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            SessionErrorPayload::new(
                SessionErrorKind::Filesystem,
                "Local antenna-controller profiles could not be staged durably.",
                format!("{}: {error}", temporary.display()),
            )
        })?;
    replace_catalog_file(app_data_dir, &temporary, &path).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Filesystem,
            "Local antenna-controller profiles could not be replaced.",
            format!("{} -> {}: {error}", temporary.display(), path.display()),
        )
    })
}

#[cfg(not(windows))]
fn replace_catalog_file(_root: &Path, temporary: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temporary, path)
}

#[cfg(windows)]
fn replace_catalog_file(root: &Path, temporary: &Path, path: &Path) -> std::io::Result<()> {
    let previous = previous_catalog_path(root);
    if previous.exists() {
        fs::remove_file(&previous)?;
    }
    if path.exists() {
        fs::rename(path, &previous)?;
    }
    match fs::rename(temporary, path) {
        Ok(()) => {
            if previous.exists() {
                fs::remove_file(previous)?;
            }
            Ok(())
        }
        Err(error) => {
            if previous.exists() && !path.exists() {
                let _ = fs::rename(previous, path);
            }
            Err(error)
        }
    }
}

pub(crate) fn tokenize_command(input: &str) -> Result<Vec<String>, String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Quote {
        Single,
        Double,
    }
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut token_started = false;
    for character in input.chars() {
        if escaped {
            token.push(character);
            token_started = true;
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            token_started = true;
            continue;
        }
        match (quote, character) {
            (Some(Quote::Single), '\'') => quote = None,
            (Some(Quote::Double), '"') => quote = None,
            (Some(_), value) => {
                token.push(value);
                token_started = true;
            }
            (None, '\'') => {
                quote = Some(Quote::Single);
                token_started = true;
            }
            (None, '"') => {
                quote = Some(Quote::Double);
                token_started = true;
            }
            (None, value) if value.is_whitespace() => {
                if token_started {
                    tokens.push(std::mem::take(&mut token));
                    token_started = false;
                }
            }
            (None, value) => {
                token.push(value);
                token_started = true;
            }
        }
    }
    if escaped {
        return Err("a trailing backslash must escape another character".into());
    }
    if quote.is_some() {
        return Err("quoted command text is not closed".into());
    }
    if token_started {
        tokens.push(token);
    }
    if tokens.is_empty() {
        return Err("a command must contain a program".into());
    }
    Ok(tokens)
}

fn normalize_command(draft: &ControllerCommandDraft) -> Result<ControllerCommandTemplate, String> {
    let (program_template, argument_templates) = if draft.one_line.trim().is_empty() {
        (draft.program.clone(), draft.arguments.clone())
    } else {
        let mut tokens = tokenize_command(&draft.one_line)?;
        let program = tokens.remove(0);
        (program, tokens)
    };
    if program_template.is_empty() {
        return Err("a command program is required".into());
    }
    if program_template.len() > COMMAND_TEMPLATE_MAX_BYTES {
        return Err("the program template exceeds the fixed template limit".into());
    }
    if argument_templates.len() > COMMAND_ARGUMENT_COUNT_MAX {
        return Err("the command has too many arguments".into());
    }
    if argument_templates
        .iter()
        .any(|argument| argument.len() > COMMAND_ARGUMENT_MAX_BYTES)
    {
        return Err("an argument template exceeds the fixed argument limit".into());
    }
    Ok(ControllerCommandTemplate {
        program_template,
        argument_templates,
    })
}

pub(crate) fn normalize_profile(
    draft: &ControllerProfileDraft,
    existing: Option<&ControllerProfile>,
    new_id: impl Fn(&str) -> String,
) -> Result<ControllerProfile, String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() || name.len() > PROFILE_NAME_MAX_BYTES {
        return Err("the controller profile name is required and bounded to 256 bytes".into());
    }
    if !(PROFILE_TIMEOUT_MIN_SECONDS..=PROFILE_TIMEOUT_MAX_SECONDS).contains(&draft.timeout_seconds)
    {
        return Err("the controller timeout must be between 1 and 60 seconds".into());
    }
    let switch_command = normalize_command(&draft.switch_command)?;
    let verification_command = draft
        .verification_command
        .as_ref()
        .map(normalize_command)
        .transpose()?;
    let profile_id = existing
        .map(|profile| profile.profile_id.clone())
        .or_else(|| draft.profile_id.clone())
        .unwrap_or_else(|| new_id("controller-profile"));
    let unchanged = existing.is_some_and(|profile| {
        profile.name == name
            && profile.switch_command == switch_command
            && profile.verification_command == verification_command
            && profile.timeout_seconds == draft.timeout_seconds
    });
    let revision = if unchanged {
        existing.expect("checked existing profile").revision.clone()
    } else {
        new_id("controller-revision")
    };
    Ok(ControllerProfile {
        profile_id,
        revision,
        name,
        switch_command,
        verification_command,
        timeout_seconds: draft.timeout_seconds,
    })
}

fn context_values(context: &AntennaControlContextV5) -> BTreeMap<&'static str, String> {
    let serialized = |value: serde_json::Value| {
        value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string())
    };
    BTreeMap::from([
        ("antenna", context.antenna.clone()),
        ("target", context.target.clone()),
        (
            "mode",
            serialized(serde_json::to_value(context.mode).expect("experiment mode serializes")),
        ),
        (
            "direction",
            serialized(
                serde_json::to_value(context.direction).expect("cycle direction serializes"),
            ),
        ),
        (
            "band",
            serialized(serde_json::to_value(context.band).expect("band serializes")),
        ),
        (
            "frequency_hz",
            context
                .frequency_hz
                .map_or_else(String::new, |value| value.to_string()),
        ),
        ("sequence", context.sequence.to_string()),
        ("intent_id", context.intent_id.clone()),
        ("session_id", context.session_id.clone()),
        ("callsign", context.callsign.clone()),
    ])
}

pub(crate) fn interpolate_template(
    template: &str,
    values: &BTreeMap<&str, String>,
) -> Result<String, String> {
    let characters = template.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;
    while index < characters.len() {
        match characters[index] {
            '{' if characters.get(index + 1) == Some(&'{') => {
                output.push('{');
                index += 2;
            }
            '}' if characters.get(index + 1) == Some(&'}') => {
                output.push('}');
                index += 2;
            }
            '{' => {
                let end = characters[index + 1..]
                    .iter()
                    .position(|character| *character == '}')
                    .map(|offset| index + 1 + offset)
                    .ok_or_else(|| "a placeholder is missing its closing brace".to_string())?;
                let name = characters[index + 1..end].iter().collect::<String>();
                if name.contains('{') || name.is_empty() {
                    return Err("a placeholder name is malformed".into());
                }
                output.push_str(
                    values
                        .get(name.as_str())
                        .ok_or_else(|| format!("unknown placeholder {{{name}}}"))?,
                );
                index = end + 1;
            }
            '}' => return Err("a literal closing brace must be written as }}".into()),
            character => {
                output.push(character);
                index += 1;
            }
        }
    }
    Ok(output)
}

fn resolve_command(
    template: &ControllerCommandTemplate,
    context: &AntennaControlContextV5,
) -> Result<ResolvedCommand, String> {
    let values = context_values(context);
    let resolved_program = interpolate_template(&template.program_template, &values)?;
    let resolved_arguments = template
        .argument_templates
        .iter()
        .map(|argument| interpolate_template(argument, &values))
        .collect::<Result<Vec<_>, _>>()?;
    if resolved_program.is_empty() || resolved_program.len() > COMMAND_PROGRAM_MAX_BYTES {
        return Err("the expanded program is empty or exceeds its fixed limit".into());
    }
    if resolved_arguments
        .iter()
        .any(|argument| argument.len() > COMMAND_ARGUMENT_MAX_BYTES)
    {
        return Err("an expanded argument exceeds its fixed limit".into());
    }
    let total = template.program_template.len()
        + template
            .argument_templates
            .iter()
            .map(String::len)
            .sum::<usize>()
        + resolved_program.len()
        + resolved_arguments.iter().map(String::len).sum::<usize>();
    if total > COMMAND_INVOCATION_MAX_BYTES {
        return Err("the original and expanded invocation exceed the fixed combined limit".into());
    }
    Ok(ResolvedCommand {
        command: AntennaControlCommandV5 {
            program_template: template.program_template.clone(),
            argument_templates: template.argument_templates.clone(),
            resolved_program,
            resolved_arguments,
        },
    })
}

fn validate_targets(
    bundle: &BundleV3Contents,
    targets: &[ControllerTargetDraft],
) -> Result<BTreeMap<String, String>, String> {
    let scheduled = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .map(|intent| intent.antenna_label.clone())
        .collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    for target in targets {
        let label = target.antenna_label.trim().to_string();
        let value = target.target.trim().to_string();
        if !scheduled.contains(&label) {
            return Err(format!(
                "target mapping references unscheduled antenna {label:?}"
            ));
        }
        if value.is_empty() || value.len() > TARGET_MAX_BYTES {
            return Err(format!(
                "antenna {label:?} requires a bounded nonempty target"
            ));
        }
        if result.insert(label.clone(), value).is_some() {
            return Err(format!(
                "antenna {label:?} has more than one target mapping"
            ));
        }
    }
    if result.keys().cloned().collect::<BTreeSet<_>>() != scheduled {
        return Err("every scheduled antenna requires exactly one target mapping".into());
    }
    Ok(result)
}

fn invocation_context(
    bundle: &BundleV3Contents,
    intent: &WsprCycleIntentV3,
    target: String,
) -> Result<AntennaControlContextV5, String> {
    Ok(AntennaControlContextV5 {
        antenna: intent.antenna_label.clone(),
        target,
        mode: bundle.schedule.mode,
        direction: intent
            .direction
            .ok_or_else(|| "antenna control requires an intention direction".to_string())?,
        band: intent.band,
        frequency_hz: intent.signal.as_ref().map(|signal| signal.frequency_hz),
        sequence: intent.sequence_number,
        intent_id: intent.intent_id.clone(),
        session_id: bundle.manifest.session_id.clone(),
        callsign: bundle.station.callsign.clone(),
    })
}

pub(crate) fn prepare_setup_controller(
    draft: &SetupControllerDraft,
    bundle: &mut BundleV3Contents,
    catalog: &[ControllerProfile],
    new_id: impl Fn(&str) -> String + Copy,
) -> Result<(PreparedSetupController, SetupControllerReview), String> {
    let existing = draft.profile.profile_id.as_deref().and_then(|profile_id| {
        catalog
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    });
    if draft.profile.profile_id.is_some() && existing.is_none() {
        return Err("the selected saved controller profile no longer exists".into());
    }
    let profile = normalize_profile(&draft.profile, existing, new_id)?;
    let targets = validate_targets(bundle, &draft.targets)?;
    bundle.schedule.antenna_control = Some(AntennaControlPolicyV5::CommandControlled {
        invocation: AntennaControlInvocationPolicyV5::OperatorTriggered,
        manual_review_required: true,
    });
    let invocations = bundle
        .schedule
        .wspr_cycle_intents
        .iter()
        .map(|intent| {
            let target = targets
                .get(&intent.antenna_label)
                .cloned()
                .ok_or_else(|| "a normalized target mapping is missing".to_string())?;
            let context = invocation_context(bundle, intent, target)?;
            let switch = resolve_command(&profile.switch_command, &context)?;
            let direction = context_values(&context)["direction"].clone();
            let verification = profile
                .verification_command
                .as_ref()
                .map(|command| resolve_command(command, &context))
                .transpose()?;
            Ok(ControllerInvocationPreview {
                intent_id: intent.intent_id.clone(),
                antenna: context.antenna,
                target: context.target,
                mode: context.mode,
                direction,
                switch_program: switch.command.resolved_program,
                switch_arguments: switch.command.resolved_arguments,
                verification_program: verification
                    .as_ref()
                    .map(|resolved| resolved.command.resolved_program.clone()),
                verification_arguments: verification
                    .map(|resolved| resolved.command.resolved_arguments),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let prepared = PreparedSetupController {
        profile: profile.clone(),
        targets,
        arm_for_session: draft.arm_for_session,
    };
    let review = SetupControllerReview {
        profile_id: profile.profile_id.clone(),
        profile_revision: profile.revision.clone(),
        profile_name: profile.name.clone(),
        timeout_seconds: profile.timeout_seconds,
        arm_for_session: draft.arm_for_session,
        disclosure: "Resolved programs, indexed arguments, stdout, and stderr become portable session evidence and may disclose paths, addresses, usernames, or credentials.",
        invocations,
    };
    Ok((prepared, review))
}

fn encode_output(captured: CapturedBytes) -> AntennaControlOutputV5 {
    match String::from_utf8(captured.bytes) {
        Ok(data) => AntennaControlOutputV5 {
            encoding: AntennaControlOutputEncodingV5::Utf8,
            data,
            truncated: captured.truncated,
        },
        Err(error) => AntennaControlOutputV5 {
            encoding: AntennaControlOutputEncodingV5::Base64,
            data: base64::engine::general_purpose::STANDARD.encode(error.into_bytes()),
            truncated: captured.truncated,
        },
    }
}

fn bounded_message(message: String) -> String {
    if message.len() <= COMMAND_ARGUMENT_MAX_BYTES {
        return message;
    }
    let mut end = COMMAND_ARGUMENT_MAX_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_string()
}

fn capture<R: Read + Send + 'static>(mut reader: R) -> thread::JoinHandle<CapturedBytes> {
    thread::spawn(move || {
        let mut retained = Vec::with_capacity(COMMAND_OUTPUT_MAX_BYTES);
        let mut buffer = [0_u8; 8192];
        let mut truncated = false;
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let available = COMMAND_OUTPUT_MAX_BYTES.saturating_sub(retained.len());
            let keep = available.min(count);
            retained.extend_from_slice(&buffer[..keep]);
            truncated |= keep < count;
        }
        CapturedBytes {
            bytes: retained,
            truncated,
        }
    })
}

#[cfg(unix)]
fn signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal(_status: &ExitStatus) -> Option<i32> {
    None
}

fn disposition(status: ExitStatus) -> AntennaControlDispositionV5 {
    status.code().map_or_else(
        || AntennaControlDispositionV5::Signaled {
            signal: signal(&status),
        },
        |code| AntennaControlDispositionV5::Exit { code },
    )
}

fn execute_process(
    command: &AntennaControlCommandV5,
    timeout: StdDuration,
    cancelled: impl Fn() -> bool,
) -> ProcessResult {
    let started_at = Utc::now();
    let mut process = match Command::new(&command.resolved_program)
        .args(&command.resolved_arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(process) => process,
        Err(error) => {
            let completed_at = Utc::now();
            return ProcessResult {
                started_at,
                completed_at,
                disposition: AntennaControlDispositionV5::SpawnError {
                    message: bounded_message(error.to_string()),
                },
                stdout: encode_output(CapturedBytes {
                    bytes: Vec::new(),
                    truncated: false,
                }),
                stderr: encode_output(CapturedBytes {
                    bytes: Vec::new(),
                    truncated: false,
                }),
            };
        }
    };
    let stdout = capture(process.stdout.take().expect("piped stdout is available"));
    let stderr = capture(process.stderr.take().expect("piped stderr is available"));
    let deadline = Instant::now() + timeout;
    let (status, timed_out, was_cancelled) = loop {
        match process.try_wait() {
            Ok(Some(status)) => break (status, false, false),
            Ok(None) if cancelled() => {
                let _ = process.kill();
                let status = process.wait().expect("killed child can be waited");
                break (status, false, true);
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = process.kill();
                let status = process.wait().expect("timed-out child can be waited");
                break (status, true, false);
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(_) => {
                let _ = process.kill();
                let status = process.wait().expect("failed child can be waited");
                break (status, false, true);
            }
        }
    };
    let completed_at = Utc::now();
    let disposition = if timed_out {
        AntennaControlDispositionV5::Timeout
    } else if was_cancelled {
        AntennaControlDispositionV5::Signaled {
            signal: signal(&status),
        }
    } else {
        disposition(status)
    };
    ProcessResult {
        started_at,
        completed_at,
        disposition,
        stdout: encode_output(stdout.join().unwrap_or(CapturedBytes {
            bytes: Vec::new(),
            truncated: true,
        })),
        stderr: encode_output(stderr.join().unwrap_or(CapturedBytes {
            bytes: Vec::new(),
            truncated: true,
        })),
    }
}

fn invocation(
    profile: &ControllerProfile,
    role: AntennaControlRoleV5,
    template: &ControllerCommandTemplate,
    context: AntennaControlContextV5,
    cancelled: impl Fn() -> bool,
) -> Result<AntennaControlInvocationV5, String> {
    let command = resolve_command(template, &context)?.command;
    let result = execute_process(
        &command,
        StdDuration::from_secs(u64::from(profile.timeout_seconds)),
        cancelled,
    );
    let elapsed = (result.completed_at - result.started_at)
        .num_milliseconds()
        .max(0) as u64;
    Ok(AntennaControlInvocationV5 {
        role,
        controller_profile_name: profile.name.clone(),
        controller_profile_revision: profile.revision.clone(),
        command,
        context,
        started_at: result.started_at,
        completed_at: result.completed_at,
        elapsed_milliseconds: elapsed,
        disposition: result.disposition,
        stdout: result.stdout,
        stderr: result.stderr,
    })
}

fn execute_profile_attempt(
    profile: &ControllerProfile,
    context: AntennaControlContextV5,
    cancelled: &impl Fn() -> bool,
) -> Result<
    (
        AntennaControlInvocationV5,
        Option<AntennaControlInvocationV5>,
    ),
    AttemptExecutionError,
> {
    let switch = invocation(
        profile,
        AntennaControlRoleV5::Switch,
        &profile.switch_command,
        context.clone(),
        cancelled,
    )
    .map_err(|detail| AttemptExecutionError {
        role: AntennaControlRoleV5::Switch,
        detail,
    })?;
    let verification = if switch.disposition.is_exit_zero() {
        profile
            .verification_command
            .as_ref()
            .map(|template| {
                invocation(
                    profile,
                    AntennaControlRoleV5::Verification,
                    template,
                    context,
                    cancelled,
                )
            })
            .transpose()
            .map_err(|detail| AttemptExecutionError {
                role: AntennaControlRoleV5::Verification,
                detail,
            })?
    } else {
        None
    };
    Ok((switch, verification))
}

fn rig_record(
    bundle: &BundleV3Contents,
    record_id: String,
    invocation: AntennaControlInvocationV5,
) -> RigRecordV3 {
    RigRecordV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: bundle.manifest.session_id.clone(),
            recorded_at: invocation.completed_at,
            provenance: Provenance::from_legacy(
                RecordSource::RigAdapter,
                "local-direct-process-v1",
            ),
            mutation: MutationMember {
                mutation_id: "pending".into(),
                member_index: 0,
                member_count: 1,
            },
        },
        record_id,
        adapter_record_ids: Vec::new(),
        status: "antenna_control_attempt".into(),
        frequency_hz: None,
        mode: None,
        power_watts: None,
        antenna_control: Some(invocation),
        raw: serde_json::Value::Null,
    }
}

fn invocation_diagnostic(invocation: &AntennaControlInvocationV5) -> String {
    let mut lines = vec![
        format!(
            "{:?} program: {}",
            invocation.role, invocation.command.resolved_program
        ),
        format!(
            "disposition: {}",
            serde_json::to_string(&invocation.disposition)
                .unwrap_or_else(|_| "{\"kind\":\"unavailable\"}".into())
        ),
    ];
    lines.extend(
        invocation
            .command
            .resolved_arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| format!("argv[{index}]: {argument}")),
    );
    lines.push(format!(
        "stdout ({:?}, truncated={}): {}",
        invocation.stdout.encoding, invocation.stdout.truncated, invocation.stdout.data
    ));
    lines.push(format!(
        "stderr ({:?}, truncated={}): {}",
        invocation.stderr.encoding, invocation.stderr.truncated, invocation.stderr.data
    ));
    lines.join("\n")
}

fn attempt_diagnostic(
    switch: &AntennaControlInvocationV5,
    verification: Option<&AntennaControlInvocationV5>,
) -> String {
    verification.map_or_else(
        || invocation_diagnostic(switch),
        |verification| {
            format!(
                "{}\n\n{}",
                invocation_diagnostic(switch),
                invocation_diagnostic(verification)
            )
        },
    )
}

fn next_intent(bundle: &BundleV3Contents) -> Option<&WsprCycleIntentV3> {
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    bundle.schedule.wspr_cycle_intents.iter().find(|intent| {
        !projection
            .cycles
            .iter()
            .any(|cycle| cycle.intent_id == intent.intent_id)
            && !projection
                .skipped_intent_ids
                .iter()
                .any(|intent_id| intent_id == &intent.intent_id)
    })
}

fn ensure_prior_transmission_complete(
    bundle: &BundleV3Contents,
    now: DateTime<Utc>,
) -> Result<(), SessionErrorPayload> {
    let projection = project_wspr_run_v3(&bundle.schedule, &bundle.events);
    if projection
        .cycles
        .last()
        .is_some_and(|cycle| cycle.window.transmission_ends_at > now)
    {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "Wait for the current WSPR transmission interval to finish before switching.",
            "the antenna controller cannot run during the committed transmission interval",
        ));
    }
    Ok(())
}

fn committed_outcome(
    bundle: &BundleV3Contents,
    mutation_id: &str,
    intent_id: &str,
) -> Option<RunControllerOutcome> {
    let invocations = bundle
        .rig
        .iter()
        .filter(|record| record.meta.mutation.mutation_id == mutation_id)
        .filter_map(|record| record.antenna_control.as_ref())
        .collect::<Vec<_>>();
    let switch = invocations
        .iter()
        .find(|invocation| invocation.role == AntennaControlRoleV5::Switch)?;
    if switch.context.intent_id != intent_id {
        return None;
    }
    let verification = invocations
        .iter()
        .find(|invocation| invocation.role == AntennaControlRoleV5::Verification);
    let switch_success = switch.disposition.is_exit_zero();
    let verification_success = verification.map(|invocation| invocation.disposition.is_exit_zero());
    let detail = if !switch_success {
        "Switch did not exit successfully. No verification ran; manual operation remains available."
    } else if verification_success == Some(false) {
        "Switch exited successfully, but verification did not. Confirm hardware manually or retry explicitly."
    } else if verification_success == Some(true) {
        "Switch and verification exited successfully. Operator readiness is still required."
    } else {
        "Switch exited successfully. No verification command is configured; operator readiness is required."
    };
    let diagnostic = attempt_diagnostic(switch, verification.copied());
    Some(RunControllerOutcome {
        revision: bundle.session_state.revision,
        intent_id: intent_id.into(),
        switch_disposition: switch.disposition.clone(),
        verification_disposition: verification.map(|invocation| invocation.disposition.clone()),
        verification_ran: verification.is_some(),
        manual_ready_required: true,
        detail: detail.into(),
        diagnostic,
    })
}

fn persist_profile_and_association(
    app_data_dir: &Path,
    source: &Path,
    session_id: &str,
    prepared: &PreparedSetupController,
) -> Result<(), SessionErrorPayload> {
    let mut catalog = read_catalog(app_data_dir)?;
    catalog
        .profiles
        .retain(|profile| profile.profile_id != prepared.profile.profile_id);
    catalog.profiles.push(prepared.profile.clone());
    catalog.profiles.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.profile_id.cmp(&right.profile_id))
    });
    catalog.associations.retain(|association| {
        association.session_id != session_id || association.source != source.to_string_lossy()
    });
    catalog.associations.push(PersistedAssociation {
        source: source.to_string_lossy().into_owned(),
        session_id: session_id.into(),
        profile_id: prepared.profile.profile_id.clone(),
        profile_revision: prepared.profile.revision.clone(),
        targets: prepared.targets.clone(),
    });
    write_catalog(app_data_dir, &catalog)
}

pub(crate) fn activate_setup_controller(
    app_data_dir: &Path,
    state: &AntennaControllerState,
    source: PathBuf,
    session_id: String,
    prepared: &PreparedSetupController,
) -> Result<(), SessionErrorPayload> {
    persist_profile_and_association(app_data_dir, &source, &session_id, prepared)?;
    state.attach(
        source,
        session_id,
        &prepared.profile,
        prepared.targets.clone(),
        prepared.arm_for_session,
    )
}

#[tauri::command]
pub(crate) fn antenna_controller_profiles(
    app: AppHandle,
) -> Result<ControllerCatalogView, SessionErrorPayload> {
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let view = ControllerCatalogView {
        input_style: if cfg!(windows) {
            "structured"
        } else {
            "one_line"
        },
        profiles: catalog.profiles,
    };
    check_ipc_payload(&view, CONTROLLER_IPC_BYTES, "antenna_controller_profiles")?;
    Ok(view)
}

#[tauri::command]
pub(crate) fn save_antenna_controller_profile(
    app: AppHandle,
    controller_state: State<'_, AntennaControllerState>,
    draft: ControllerProfileDraft,
) -> Result<ControllerProfile, SessionErrorPayload> {
    let app_data_dir = resolved_app_data_dir(&app)?;
    let mut catalog = read_catalog(&app_data_dir)?;
    let existing = draft.profile_id.as_deref().and_then(|profile_id| {
        catalog
            .profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    });
    if draft.profile_id.is_some() && existing.is_none() {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "The saved antenna-controller profile changed. Refresh before saving.",
            "the requested local profile identity no longer exists",
        ));
    }
    let profile = normalize_profile(&draft, existing, |prefix| {
        format!("{prefix}-{}", Uuid::new_v4())
    })
    .map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The local antenna-controller profile is not valid.",
            detail,
        )
    })?;
    let changed = existing.is_none_or(|existing| existing.revision != profile.revision);
    catalog
        .profiles
        .retain(|candidate| candidate.profile_id != profile.profile_id);
    catalog.profiles.push(profile.clone());
    catalog.profiles.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.profile_id.cmp(&right.profile_id))
    });
    write_catalog(&app_data_dir, &catalog)?;
    if changed {
        let should_revoke = controller_state.runtime.lock().ok().is_some_and(|runtime| {
            runtime
                .attached
                .as_ref()
                .is_some_and(|attached| attached.profile_id == profile.profile_id)
        });
        if should_revoke {
            controller_state.revoke();
        }
    }
    Ok(profile)
}

#[tauri::command]
pub(crate) fn attach_active_session_antenna_controller(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    request: AttachControllerRequest,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let bundle = BundleStore::new(&source)
        .read_v3_checkpointed()
        .map_err(live_error_payload)?;
    if bundle.manifest.schema_version != SCHEMA_VERSION_V5 {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Unsupported,
            "Antenna-controller profiles require a schema-v5 session.",
            format!("schema version {}", bundle.manifest.schema_version),
        ));
    }
    if !matches!(
        bundle.schedule.antenna_control.as_ref(),
        Some(AntennaControlPolicyV5::CommandControlled {
            invocation: AntennaControlInvocationPolicyV5::OperatorTriggered,
            manual_review_required: true,
        })
    ) {
        return Err(SessionErrorPayload::new(
            SessionErrorKind::Conflict,
            "This session was not planned for operator-triggered command assistance.",
            "the portable antenna-control policy remains manual",
        ));
    }
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let profile = catalog
        .profiles
        .iter()
        .find(|profile| {
            profile.profile_id == request.profile_id && profile.revision == request.profile_revision
        })
        .ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The selected controller profile changed. Review and attach it again.",
                "the requested local profile revision is unavailable",
            )
        })?;
    let targets = validate_targets(&bundle, &request.targets).map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "The antenna target mapping is not valid.",
            detail,
        )
    })?;
    let prepared = PreparedSetupController {
        profile: profile.clone(),
        targets: targets.clone(),
        arm_for_session: request.armed,
    };
    persist_profile_and_association(
        &resolved_app_data_dir(&app)?,
        &source,
        &bundle.manifest.session_id,
        &prepared,
    )?;
    controller_state.attach(
        source,
        bundle.manifest.session_id,
        profile,
        targets,
        request.armed,
    )?;
    active_controller_view_for(app, active_state, controller_state)
}

#[tauri::command]
pub(crate) fn active_session_antenna_controller(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    active_controller_view_for(app, active_state, controller_state)
}

fn active_controller_view_for(
    app: AppHandle,
    active_state: State<'_, ActiveSessionState>,
    controller_state: State<'_, AntennaControllerState>,
) -> Result<ActiveControllerView, SessionErrorPayload> {
    let (source, _) = active_session_source(active_state.inner())?;
    let bundle = BundleStore::new(&source)
        .read_v3_checkpointed()
        .map_err(live_error_payload)?;
    let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
    let runtime = controller_state.runtime.lock().map_err(|_| {
        SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
    })?;
    let attached = runtime.attached.as_ref().filter(|attached| {
        attached.source == source && attached.session_id == bundle.manifest.session_id
    });
    let persisted = catalog.associations.iter().find(|association| {
        association.source == source.to_string_lossy()
            && association.session_id == bundle.manifest.session_id
    });
    let profile_id = attached
        .map(|attached| attached.profile_id.as_str())
        .or_else(|| persisted.map(|association| association.profile_id.as_str()));
    let profile_revision = attached
        .map(|attached| attached.profile_revision.as_str())
        .or_else(|| persisted.map(|association| association.profile_revision.as_str()));
    let profile = profile_id.and_then(|profile_id| {
        catalog
            .profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    });
    let stale_profile = profile_revision
        .is_some_and(|revision| profile.is_none_or(|profile| profile.revision != revision));
    let view = ActiveControllerView {
        policy: match bundle.schedule.antenna_control.as_ref() {
            Some(AntennaControlPolicyV5::CommandControlled { .. }) => "command_controlled",
            _ => "manual",
        },
        attached: attached.is_some(),
        armed: attached.is_some_and(|attached| attached.armed) && !stale_profile,
        profile_id: profile_id.map(str::to_string),
        profile_revision: profile_revision.map(str::to_string),
        profile_name: profile.map(|profile| profile.name.clone()),
        targets: attached.map_or_else(
            || persisted.map_or_else(BTreeMap::new, |association| association.targets.clone()),
            |attached| attached.targets.clone(),
        ),
        stale_profile,
        last_attempt: runtime.last_attempt.clone(),
    };
    check_ipc_payload(&view, CONTROLLER_IPC_BYTES, "active_antenna_controller")?;
    Ok(view)
}

#[tauri::command]
pub(crate) fn run_active_session_antenna_controller(
    active_state: State<'_, ActiveSessionState>,
    conductor_state: State<'_, ConductorSessionState>,
    controller_state: State<'_, AntennaControllerState>,
    app: AppHandle,
    request: RunControllerRequest,
) -> Result<RunControllerOutcome, SessionErrorPayload> {
    let outcome = with_foreground_operation(active_state.inner(), || {
        let (source, _) = active_session_source(active_state.inner())?;
        let store = BundleStore::new(&source);
        let bundle = store.read_v3_checkpointed().map_err(live_error_payload)?;
        if bundle.manifest.schema_version != SCHEMA_VERSION_V5
            || bundle.session_state.lifecycle != SessionLifecycleV2::Running
        {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "Antenna control is available only for an active schema-v5 session.",
                "the active session is not a running schema-v5 session",
            ));
        }
        if !matches!(
            bundle.schedule.antenna_control.as_ref(),
            Some(AntennaControlPolicyV5::CommandControlled {
                invocation: AntennaControlInvocationPolicyV5::OperatorTriggered,
                manual_review_required: true,
            })
        ) {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "This session does not permit operator-triggered command assistance.",
                "the portable antenna-control policy is not operator-triggered with manual review",
            ));
        }
        if let Some(outcome) = committed_outcome(&bundle, &request.action_token, &request.intent_id)
        {
            return Ok(outcome);
        }
        if request.expected_revision != bundle.session_state.revision {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::StaleRevision,
                "The session changed. Refresh Active Run before switching.",
                format!(
                    "expected checkpoint revision {}, actual revision {}",
                    request.expected_revision, bundle.session_state.revision
                ),
            ));
        }
        ensure_prior_transmission_complete(&bundle, Utc::now())?;
        let intent = next_intent(&bundle).ok_or_else(|| {
            SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "There is no pending antenna intention to switch.",
                "all cycle intentions are armed or skipped",
            )
        })?;
        if intent.intent_id != request.intent_id {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Conflict,
                "The pending antenna intention changed. Refresh before switching.",
                format!(
                    "requested intention {:?}, current intention {:?}",
                    request.intent_id, intent.intent_id
                ),
            ));
        }
        let (association, generation) = {
            let runtime = controller_state.runtime.lock().map_err(|_| {
                SessionErrorPayload::report_pipeline("antenna-controller state is unavailable")
            })?;
            let association = runtime
                .attached
                .as_ref()
                .filter(|attached| {
                    attached.source == source
                        && attached.session_id == bundle.manifest.session_id
                        && attached.armed
                })
                .cloned()
                .ok_or_else(|| {
                    SessionErrorPayload::new(
                        SessionErrorKind::Conflict,
                        "Attach and arm a local controller before switching.",
                        "no armed local controller is associated with this active session",
                    )
                })?;
            let generation = association.generation;
            (association, generation)
        };
        let catalog = read_catalog(&resolved_app_data_dir(&app)?)?;
        let profile = catalog
            .profiles
            .iter()
            .find(|profile| {
                profile.profile_id == association.profile_id
                    && profile.revision == association.profile_revision
            })
            .cloned()
            .ok_or_else(|| {
                controller_state.revoke();
                SessionErrorPayload::new(
                    SessionErrorKind::Conflict,
                    "The attached controller changed. Review and arm it again.",
                    "the pinned local controller revision is stale",
                )
            })?;
        let target = association
            .targets
            .get(&intent.antenna_label)
            .cloned()
            .ok_or_else(|| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "The pending antenna has no local controller target.",
                    format!("missing target for {:?}", intent.antenna_label),
                )
            })?;
        let context = invocation_context(&bundle, intent, target).map_err(|detail| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The pending antenna intention cannot be expanded safely.",
                detail,
            )
        })?;
        resolve_command(&profile.switch_command, &context).map_err(|detail| {
            SessionErrorPayload::new(
                SessionErrorKind::Validation,
                "The switch command could not be expanded.",
                detail,
            )
        })?;
        if let Some(verification) = &profile.verification_command {
            resolve_command(verification, &context).map_err(|detail| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "The verification command could not be expanded.",
                    detail,
                )
            })?;
        }
        conductor_state.authorize_controller_action(
            &request.action_token,
            &bundle.manifest.session_id,
            request.expected_revision,
            Utc::now(),
        )?;
        let generation_state = controller_state.generation.clone();
        let cancelled = || generation_state.load(Ordering::SeqCst) != generation;
        let (switch, verification) = execute_profile_attempt(&profile, context, &cancelled)
            .map_err(|error| {
                let message = if error.role == AntennaControlRoleV5::Switch {
                    "The switch command could not be expanded."
                } else {
                    "The verification command could not be expanded."
                };
                SessionErrorPayload::new(SessionErrorKind::Validation, message, error.detail)
            })?;
        let switch_success = switch.disposition.is_exit_zero();
        let verification_success = verification
            .as_ref()
            .map(|invocation| invocation.disposition.is_exit_zero());
        let mut rig_records = vec![rig_record(
            &bundle,
            format!("rig-{}", Uuid::new_v4()),
            switch.clone(),
        )];
        if let Some(invocation) = verification.clone() {
            rig_records.push(rig_record(
                &bundle,
                format!("rig-{}", Uuid::new_v4()),
                invocation,
            ));
        }
        let receipt = {
            let mut writer = store
                .open_v3_writer_with_hooks(Arc::new(SystemLivePersistenceHooks))
                .map_err(live_error_payload)?;
            writer
                .append_antenna_control(LiveAntennaControlMutationV5 {
                    expected_revision: request.expected_revision,
                    mutation_id: request.action_token.clone(),
                    rig_records,
                    armed_event: None,
                })
                .map_err(live_error_payload)?
        };
        let detail = if !switch_success {
            "Switch did not exit successfully. No verification ran; manual operation remains available."
        } else if verification_success == Some(false) {
            "Switch exited successfully, but verification did not. Confirm hardware manually or retry explicitly."
        } else if verification_success == Some(true) {
            "Switch and verification exited successfully. Operator readiness is still required."
        } else {
            "Switch exited successfully. No verification command is configured; operator readiness is required."
        }
        .to_string();
        let diagnostic = attempt_diagnostic(&switch, verification.as_ref());
        if let Ok(mut runtime) = controller_state.runtime.lock() {
            runtime.last_attempt = Some(ControllerAttemptSummary {
                intent_id: request.intent_id.clone(),
                successful_switch: switch_success,
                successful_verification: verification_success,
                detail: detail.clone(),
                diagnostic: diagnostic.clone(),
            });
        }
        Ok(RunControllerOutcome {
            revision: receipt.revision,
            intent_id: request.intent_id,
            switch_disposition: switch.disposition,
            verification_disposition: verification.map(|invocation| invocation.disposition),
            verification_ran: verification_success.is_some(),
            manual_ready_required: true,
            detail,
            diagnostic,
        })
    })?;
    check_ipc_payload(
        &outcome,
        CONTROLLER_IPC_BYTES,
        "run_active_session_antenna_controller",
    )?;
    Ok(outcome)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn values() -> BTreeMap<&'static str, String> {
        BTreeMap::from([
            ("antenna", "A B".into()),
            ("target", "relay A \"quoted\" {brace};$(nope)".into()),
            ("mode", "tx_focused".into()),
            ("direction", "transmit".into()),
            ("band", "20m".into()),
            ("frequency_hz", "14095600".into()),
            ("sequence", "7".into()),
            ("intent_id", "intent-7".into()),
            ("session_id", "session-1".into()),
            ("callsign", "N1RWJ".into()),
        ])
    }

    #[test]
    fn tokenizer_groups_only_quotes_and_backslashes_without_shell_semantics() {
        assert_eq!(
            tokenize_command("\"/opt/my tool\" 'two words' plain\\ value | > '*' '$()' '&&' \"\"",)
                .unwrap(),
            vec![
                "/opt/my tool",
                "two words",
                "plain value",
                "|",
                ">",
                "*",
                "$()",
                "&&",
                "",
            ]
        );
        assert!(tokenize_command("unterminated '").is_err());
        assert!(tokenize_command("trailing\\").is_err());
    }

    #[test]
    fn interpolation_isolated_inside_one_token_and_supports_literal_braces() {
        assert_eq!(
            interpolate_template("prefix-{target}-{{literal}}-{mode}", &values()).unwrap(),
            "prefix-relay A \"quoted\" {brace};$(nope)-{literal}-tx_focused"
        );
        assert!(interpolate_template("{unknown}", &values()).is_err());
        assert!(interpolate_template("}", &values()).is_err());
        assert!(interpolate_template("{target", &values()).is_err());

        let mut context = fake_context();
        context.target = values()["target"].clone();
        let resolved = resolve_command(
            &ControllerCommandTemplate {
                program_template: "switch".into(),
                argument_templates: vec!["--target={target}".into()],
            },
            &context,
        )
        .unwrap();
        assert_eq!(resolved.command.resolved_arguments.len(), 1);
        assert_eq!(
            resolved.command.resolved_arguments[0],
            "--target=relay A \"quoted\" {brace};$(nope)"
        );
    }

    #[test]
    fn profile_normalization_has_fixed_limits_and_revision_identity() {
        let draft = ControllerProfileDraft {
            profile_id: None,
            name: "Bench switch".into(),
            switch_command: ControllerCommandDraft {
                one_line: "switch --target {target} --mode {mode}".into(),
                program: String::new(),
                arguments: Vec::new(),
            },
            verification_command: None,
            timeout_seconds: 10,
        };
        let profile = normalize_profile(&draft, None, |prefix| format!("{prefix}-1")).unwrap();
        assert_eq!(profile.switch_command.program_template, "switch");
        assert_eq!(profile.switch_command.argument_templates[3], "{mode}");
        assert_eq!(
            normalize_profile(&draft, Some(&profile), |prefix| format!("{prefix}-2"))
                .unwrap()
                .revision,
            profile.revision
        );
        let mut invalid = draft;
        invalid.timeout_seconds = 61;
        assert!(normalize_profile(&invalid, None, |prefix| format!("{prefix}-3")).is_err());
    }

    #[test]
    fn one_line_and_structured_profile_inputs_have_the_same_canonical_form() {
        let one_line = ControllerProfileDraft {
            profile_id: None,
            name: "Bench switch".into(),
            switch_command: ControllerCommandDraft {
                one_line: "\"/opt/My Controller\" --target \"{target}\" --mode {mode}".into(),
                program: String::new(),
                arguments: Vec::new(),
            },
            verification_command: None,
            timeout_seconds: 10,
        };
        let structured = ControllerProfileDraft {
            switch_command: ControllerCommandDraft {
                one_line: String::new(),
                program: "/opt/My Controller".into(),
                arguments: vec![
                    "--target".into(),
                    "{target}".into(),
                    "--mode".into(),
                    "{mode}".into(),
                ],
            },
            ..one_line.clone()
        };
        assert_eq!(
            normalize_profile(&one_line, None, |prefix| format!("{prefix}-1"))
                .unwrap()
                .switch_command,
            normalize_profile(&structured, None, |prefix| format!("{prefix}-2"))
                .unwrap()
                .switch_command
        );
    }

    #[test]
    fn output_encoding_is_utf8_or_base64_and_explicitly_truncated() {
        assert_eq!(
            encode_output(CapturedBytes {
                bytes: b"ok".to_vec(),
                truncated: false,
            }),
            AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: "ok".into(),
                truncated: false,
            }
        );
        let binary = encode_output(CapturedBytes {
            bytes: vec![0xff, 0x00],
            truncated: true,
        });
        assert_eq!(binary.encoding, AntennaControlOutputEncodingV5::Base64);
        assert_eq!(binary.data, "/wA=");
        assert!(binary.truncated);
    }

    fn fake_command(behavior: &str) -> AntennaControlCommandV5 {
        let executable = std::env::current_exe()
            .expect("the controller-process fixture can reuse the current test executable")
            .display()
            .to_string();
        let module = module_path!();
        let module = module
            .split_once("::")
            .map_or(module, |(_, relative)| relative);
        let fixture = format!("{module}::fake_controller_{}", behavior.replace('-', "_"));
        AntennaControlCommandV5 {
            program_template: executable.clone(),
            argument_templates: vec![
                "--ignored".into(),
                "--exact".into(),
                fixture.clone(),
                "--nocapture".into(),
            ],
            resolved_program: executable,
            resolved_arguments: vec![
                "--ignored".into(),
                "--exact".into(),
                fixture,
                "--nocapture".into(),
            ],
        }
    }

    #[test]
    #[ignore = "child-process fixture"]
    fn fake_controller_exit_zero() {
        println!("switched");
    }

    #[test]
    #[ignore = "child-process fixture"]
    fn fake_controller_exit_nonzero() {
        std::process::exit(7);
    }

    #[test]
    #[ignore = "child-process fixture"]
    fn fake_controller_binary() {
        std::io::stdout().write_all(&[0xff, 0x00]).unwrap();
    }

    #[test]
    #[ignore = "child-process fixture"]
    fn fake_controller_flood() {
        std::io::stdout()
            .write_all(&vec![b'x'; COMMAND_OUTPUT_MAX_BYTES + 1])
            .unwrap();
    }

    #[test]
    #[ignore = "child-process fixture"]
    fn fake_controller_timeout() {
        thread::sleep(StdDuration::from_secs(60));
    }

    fn fake_template(behavior: &str) -> ControllerCommandTemplate {
        let command = fake_command(behavior);
        ControllerCommandTemplate {
            program_template: command.program_template,
            argument_templates: command.argument_templates,
        }
    }

    fn fake_context() -> AntennaControlContextV5 {
        AntennaControlContextV5 {
            antenna: "Dipole".into(),
            target: "relay-a".into(),
            mode: ExperimentMode::TxFocused,
            direction: antennabench_core::WsprCycleDirection::Transmit,
            band: antennabench_core::Band::M20,
            frequency_hz: Some(14_095_600),
            sequence: 1,
            intent_id: "intent-1".into(),
            session_id: "session-1".into(),
            callsign: "N1RWJ".into(),
        }
    }

    pub(crate) fn assert_verification_runs_only_after_a_zero_switch_exit() {
        let profile = |switch_behavior| ControllerProfile {
            profile_id: "profile-1".into(),
            revision: "revision-1".into(),
            name: "Fake controller".into(),
            switch_command: fake_template(switch_behavior),
            verification_command: Some(fake_template("exit-zero")),
            timeout_seconds: 10,
        };
        let (failed_switch, skipped_verification) =
            execute_profile_attempt(&profile("exit-nonzero"), fake_context(), &|| false).unwrap();
        assert_eq!(
            failed_switch.disposition,
            AntennaControlDispositionV5::Exit { code: 7 }
        );
        assert!(skipped_verification.is_none());

        let (successful_switch, verification) =
            execute_profile_attempt(&profile("exit-zero"), fake_context(), &|| false).unwrap();
        assert!(successful_switch.disposition.is_exit_zero());
        assert_eq!(
            verification.unwrap().disposition,
            AntennaControlDispositionV5::Exit { code: 0 }
        );
    }

    pub(crate) fn assert_fake_process_covers_exit_binary_truncation_timeout_and_spawn_failure() {
        let zero = execute_process(
            &fake_command("exit-zero"),
            StdDuration::from_secs(10),
            || false,
        );
        assert_eq!(
            zero.disposition,
            AntennaControlDispositionV5::Exit { code: 0 }
        );
        assert!(zero.stdout.data.contains("switched\n"));

        let nonzero = execute_process(
            &fake_command("exit-nonzero"),
            StdDuration::from_secs(10),
            || false,
        );
        assert_eq!(
            nonzero.disposition,
            AntennaControlDispositionV5::Exit { code: 7 }
        );

        let binary = execute_process(&fake_command("binary"), StdDuration::from_secs(10), || {
            false
        });
        assert_eq!(
            binary.stdout.encoding,
            AntennaControlOutputEncodingV5::Base64
        );

        let flood = execute_process(&fake_command("flood"), StdDuration::from_secs(10), || false);
        assert!(flood.stdout.truncated);
        assert_eq!(flood.stdout.data.len(), COMMAND_OUTPUT_MAX_BYTES);

        let timeout = execute_process(
            &fake_command("timeout"),
            StdDuration::from_millis(25),
            || false,
        );
        assert_eq!(timeout.disposition, AntennaControlDispositionV5::Timeout);

        let missing = execute_process(
            &AntennaControlCommandV5 {
                program_template: "/definitely/missing/antennabench-controller".into(),
                argument_templates: Vec::new(),
                resolved_program: "/definitely/missing/antennabench-controller".into(),
                resolved_arguments: Vec::new(),
            },
            StdDuration::from_secs(1),
            || false,
        );
        assert!(matches!(
            missing.disposition,
            AntennaControlDispositionV5::SpawnError { .. }
        ));
    }

    pub(crate) fn assert_cancellation_terminates_the_child_without_claiming_hardware_restoration() {
        let cancelled =
            execute_process(&fake_command("timeout"), StdDuration::from_secs(10), || {
                true
            });
        assert!(matches!(
            cancelled.disposition,
            AntennaControlDispositionV5::Signaled { .. }
        ));
    }

    #[test]
    fn persisted_association_does_not_restore_volatile_arming() {
        let app_data_dir = std::env::temp_dir().join(format!(
            "antennabench-controller-catalog-test-{}",
            Uuid::new_v4()
        ));
        let catalog = ControllerCatalog {
            version: CATALOG_VERSION,
            profiles: vec![ControllerProfile {
                profile_id: "profile-1".into(),
                revision: "revision-1".into(),
                name: "Fake controller".into(),
                switch_command: fake_template("exit-zero"),
                verification_command: None,
                timeout_seconds: 2,
            }],
            associations: vec![PersistedAssociation {
                source: "/tmp/example.antennabundle".into(),
                session_id: "session-1".into(),
                profile_id: "profile-1".into(),
                profile_revision: "revision-1".into(),
                targets: BTreeMap::from([("Dipole".into(), "relay-a".into())]),
            }],
        };
        write_catalog(&app_data_dir, &catalog).unwrap();
        assert_eq!(read_catalog(&app_data_dir).unwrap(), catalog);
        let mut invalid = catalog.clone();
        invalid.profiles[0].timeout_seconds = 0;
        assert!(write_catalog(&app_data_dir, &invalid).is_err());
        let restarted = AntennaControllerState::default();
        assert!(restarted.runtime.lock().unwrap().attached.is_none());
        fs::remove_dir_all(&app_data_dir).unwrap();
    }
}
