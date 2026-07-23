use super::process::{context_values, invocation_context, resolve_command, tokenize_command};
use super::*;

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

pub(super) fn default_timeout() -> u8 {
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
    #[serde(default = "default_invocation_policy")]
    pub(crate) invocation: AntennaControlInvocationPolicyV5,
    #[serde(default = "default_manual_review_required")]
    pub(crate) manual_review_required: bool,
    pub(crate) profile: ControllerProfileDraft,
    pub(crate) targets: Vec<ControllerTargetDraft>,
}

pub(super) fn default_invocation_policy() -> AntennaControlInvocationPolicyV5 {
    AntennaControlInvocationPolicyV5::OperatorTriggered
}

pub(super) fn default_manual_review_required() -> bool {
    true
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
    pub(crate) invocation: AntennaControlInvocationPolicyV5,
    pub(crate) manual_review_required: bool,
    pub(crate) authority_summary: &'static str,
    pub(crate) disclosure: &'static str,
    pub(crate) invocations: Vec<ControllerInvocationPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ControllerCatalog {
    pub(super) version: u16,
    pub(super) profiles: Vec<ControllerProfile>,
    pub(super) associations: Vec<PersistedAssociation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) migration_notice: Option<ControllerCatalogMigrationNotice>,
}

impl Default for ControllerCatalog {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            profiles: Vec::new(),
            associations: Vec::new(),
            migration_notice: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ControllerCatalogMigrationNotice {
    pub(super) consolidated_profiles: usize,
    pub(super) renamed_profiles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PersistedAssociation {
    pub(super) source: String,
    pub(super) session_id: String,
    pub(super) profile_id: String,
    pub(super) profile_revision: String,
    pub(super) targets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerCatalogView {
    pub(super) input_style: &'static str,
    pub(super) profiles: Vec<ControllerProfile>,
    pub(super) migration_notice: Option<ControllerCatalogMigrationNotice>,
}
pub(super) fn catalog_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("antenna-controller-profiles.json")
}

pub(super) fn previous_catalog_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("antenna-controller-profiles.previous.json")
}

pub(super) fn resolved_app_data_dir(app: &AppHandle) -> Result<PathBuf, SessionErrorPayload> {
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

pub(super) fn read_catalog(app_data_dir: &Path) -> Result<ControllerCatalog, SessionErrorPayload> {
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
    let mut catalog: ControllerCatalog = serde_json::from_slice(&bytes).map_err(|error| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Local antenna-controller profiles are not valid.",
            format!("{}: {error}", path.display()),
        )
    })?;
    match catalog.version {
        CATALOG_VERSION => {
            validate_catalog(&catalog).map_err(|detail| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "Local antenna-controller profiles are not valid.",
                    detail,
                )
            })?;
        }
        1 => {
            validate_catalog_contents(&catalog, false).map_err(|detail| {
                SessionErrorPayload::new(
                    SessionErrorKind::Validation,
                    "Local antenna-controller profiles are not valid.",
                    detail,
                )
            })?;
            catalog = migrate_v1_catalog(catalog)?;
            write_catalog(app_data_dir, &catalog)?;
        }
        version => {
            return Err(SessionErrorPayload::new(
                SessionErrorKind::Unsupported,
                "Local antenna-controller profiles use an unsupported version.",
                format!("catalog version {version}"),
            ));
        }
    }
    Ok(catalog)
}

pub(super) fn validate_command_template(
    template: &ControllerCommandTemplate,
) -> Result<(), String> {
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

pub(super) fn validate_catalog(catalog: &ControllerCatalog) -> Result<(), String> {
    if catalog.version != CATALOG_VERSION {
        return Err(format!(
            "controller catalog version {} is not writable",
            catalog.version
        ));
    }
    validate_catalog_contents(catalog, true)
}

fn validate_catalog_contents(
    catalog: &ControllerCatalog,
    require_unique_names: bool,
) -> Result<(), String> {
    let mut profile_ids = BTreeSet::new();
    let mut profile_names = BTreeSet::new();
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
        if require_unique_names && !profile_names.insert(normalized_profile_name(&profile.name)) {
            return Err(
                "controller profile names must be unique ignoring case and surrounding whitespace"
                    .into(),
            );
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

pub(super) fn normalized_profile_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn reusable_configuration_matches(left: &ControllerProfile, right: &ControllerProfile) -> bool {
    left.switch_command == right.switch_command
        && left.verification_command == right.verification_command
        && left.timeout_seconds == right.timeout_seconds
}

fn migrated_profile_name(
    base: &str,
    mut ordinal: usize,
    reserved_names: &BTreeSet<String>,
    used_names: &BTreeSet<String>,
) -> String {
    loop {
        let suffix = format!(" (migrated {ordinal})");
        let available = PROFILE_NAME_MAX_BYTES.saturating_sub(suffix.len());
        let mut end = base.len().min(available);
        while !base.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{}", &base[..end], suffix);
        let normalized = normalized_profile_name(&candidate);
        if !reserved_names.contains(&normalized) && !used_names.contains(&normalized) {
            return candidate;
        }
        ordinal += 1;
    }
}

fn migrate_v1_catalog(
    catalog: ControllerCatalog,
) -> Result<ControllerCatalog, SessionErrorPayload> {
    let reserved_names = catalog
        .profiles
        .iter()
        .map(|profile| normalized_profile_name(&profile.name))
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<String, Vec<ControllerProfile>>::new();
    for profile in catalog.profiles {
        grouped
            .entry(normalized_profile_name(&profile.name))
            .or_default()
            .push(profile);
    }

    let mut profiles = Vec::new();
    let mut identity_map = BTreeMap::<String, (String, String)>::new();
    let mut used_names = BTreeSet::new();
    let mut consolidated_profiles = 0usize;
    let mut renamed_profiles = 0usize;

    for mut same_name in grouped.into_values() {
        same_name.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        let mut configurations = Vec::<Vec<ControllerProfile>>::new();
        for profile in same_name {
            if let Some(matches) = configurations.iter_mut().find(|matches| {
                reusable_configuration_matches(
                    matches
                        .first()
                        .expect("a migration configuration group is nonempty"),
                    &profile,
                )
            }) {
                matches.push(profile);
            } else {
                configurations.push(vec![profile]);
            }
        }

        for (configuration_index, mut matches) in configurations.into_iter().enumerate() {
            matches.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
            let mut retained = matches
                .first()
                .cloned()
                .expect("a migration configuration group is nonempty");
            retained.name = retained.name.trim().to_string();
            if configuration_index > 0 {
                retained.name = migrated_profile_name(
                    &retained.name,
                    configuration_index + 1,
                    &reserved_names,
                    &used_names,
                );
                renamed_profiles += 1;
            }
            used_names.insert(normalized_profile_name(&retained.name));
            consolidated_profiles += matches.len().saturating_sub(1);
            for profile in matches {
                identity_map.insert(
                    profile.profile_id,
                    (retained.profile_id.clone(), retained.revision.clone()),
                );
            }
            profiles.push(retained);
        }
    }

    profiles.sort_by(|left, right| {
        normalized_profile_name(&left.name)
            .cmp(&normalized_profile_name(&right.name))
            .then(left.name.cmp(&right.name))
            .then(left.profile_id.cmp(&right.profile_id))
    });
    let associations = catalog
        .associations
        .into_iter()
        .map(|mut association| {
            let (profile_id, profile_revision) = identity_map
                .get(&association.profile_id)
                .expect("legacy catalog validation checked association identities");
            association.profile_id.clone_from(profile_id);
            association.profile_revision.clone_from(profile_revision);
            association
        })
        .collect();
    let migration_notice = (consolidated_profiles > 0 || renamed_profiles > 0).then_some(
        ControllerCatalogMigrationNotice {
            consolidated_profiles,
            renamed_profiles,
        },
    );
    let migrated = ControllerCatalog {
        version: CATALOG_VERSION,
        profiles,
        associations,
        migration_notice,
    };
    validate_catalog(&migrated).map_err(|detail| {
        SessionErrorPayload::new(
            SessionErrorKind::Validation,
            "Local antenna-controller profiles could not be migrated safely.",
            detail,
        )
    })?;
    Ok(migrated)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControllerProfileDraftConflict {
    MissingSelected,
    NameInUse,
}

pub(super) fn existing_profile_for_draft<'a>(
    profiles: &'a [ControllerProfile],
    draft: &ControllerProfileDraft,
) -> Result<Option<&'a ControllerProfile>, ControllerProfileDraftConflict> {
    let name_key = normalized_profile_name(&draft.name);
    match draft.profile_id.as_deref() {
        Some(profile_id) => {
            let selected = profiles
                .iter()
                .find(|profile| profile.profile_id == profile_id)
                .ok_or(ControllerProfileDraftConflict::MissingSelected)?;
            if profiles.iter().any(|profile| {
                profile.profile_id != profile_id
                    && normalized_profile_name(&profile.name) == name_key
            }) {
                return Err(ControllerProfileDraftConflict::NameInUse);
            }
            Ok(Some(selected))
        }
        None => Ok(profiles
            .iter()
            .find(|profile| normalized_profile_name(&profile.name) == name_key)),
    }
}

pub(super) fn remove_profile(catalog: &mut ControllerCatalog, profile_id: &str) -> bool {
    let previous_len = catalog.profiles.len();
    catalog
        .profiles
        .retain(|profile| profile.profile_id != profile_id);
    if catalog.profiles.len() == previous_len {
        return false;
    }
    catalog
        .associations
        .retain(|association| association.profile_id != profile_id);
    true
}

pub(super) fn write_catalog(
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
pub(super) fn replace_catalog_file(
    _root: &Path,
    temporary: &Path,
    path: &Path,
) -> std::io::Result<()> {
    fs::rename(temporary, path)
}

#[cfg(windows)]
pub(super) fn replace_catalog_file(
    root: &Path,
    temporary: &Path,
    path: &Path,
) -> std::io::Result<()> {
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

pub(super) fn normalize_command(
    draft: &ControllerCommandDraft,
) -> Result<ControllerCommandTemplate, String> {
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

pub(super) fn validate_targets(
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

pub(crate) fn prepare_setup_controller(
    draft: &SetupControllerDraft,
    bundle: &mut BundleV3Contents,
    catalog: &[ControllerProfile],
    new_id: impl Fn(&str) -> String + Copy,
) -> Result<(PreparedSetupController, SetupControllerReview), String> {
    let existing = existing_profile_for_draft(catalog, &draft.profile).map_err(|conflict| {
        match conflict {
            ControllerProfileDraftConflict::MissingSelected => {
                "the selected saved controller profile no longer exists"
            }
            ControllerProfileDraftConflict::NameInUse => {
                "another saved controller profile already uses that name"
            }
        }
        .to_string()
    })?;
    let profile = normalize_profile(&draft.profile, existing, new_id)?;
    if !draft.manual_review_required && profile.verification_command.is_none() {
        return Err(
            "command-authorized readiness requires an independent verification command".into(),
        );
    }
    let targets = validate_targets(bundle, &draft.targets)?;
    bundle.schedule.antenna_control = Some(AntennaControlPolicyV5::CommandControlled {
        invocation: draft.invocation,
        manual_review_required: draft.manual_review_required,
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
        invocation: draft.invocation,
        manual_review_required: draft.manual_review_required,
        authority_summary: if draft.manual_review_required {
            "Commands prepare and verify each intention; the named operator ready action remains required."
        } else {
            "Successful switch and independent verification commands authorize the next eligible WSPR boundary without an operator ready action."
        },
        disclosure: "Resolved programs, indexed arguments, stdout, and stderr become portable session evidence and may disclose paths, addresses, usernames, or credentials.",
        invocations,
    };
    Ok((prepared, review))
}

pub(super) fn persist_profile_and_association(
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
        normalized_profile_name(&left.name)
            .cmp(&normalized_profile_name(&right.name))
            .then(left.name.cmp(&right.name))
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
