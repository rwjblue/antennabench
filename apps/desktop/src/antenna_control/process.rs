use super::profiles::{ControllerCommandTemplate, ControllerProfile};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedCommand {
    pub(super) command: AntennaControlCommandV5,
}

#[derive(Debug)]
pub(super) struct CapturedBytes {
    pub(super) bytes: Vec<u8>,
    pub(super) truncated: bool,
}

#[derive(Debug)]
pub(super) struct ProcessResult {
    pub(super) started_at: DateTime<Utc>,
    pub(super) completed_at: DateTime<Utc>,
    pub(super) disposition: AntennaControlDispositionV5,
    pub(super) stdout: AntennaControlOutputV5,
    pub(super) stderr: AntennaControlOutputV5,
}

#[derive(Debug)]
pub(super) struct AttemptExecutionError {
    pub(super) role: AntennaControlRoleV5,
    pub(super) detail: String,
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

pub(super) fn context_values(context: &AntennaControlContextV5) -> BTreeMap<&'static str, String> {
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

pub(super) fn resolve_command(
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

pub(super) fn invocation_context(
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

pub(super) fn encode_output(captured: CapturedBytes) -> AntennaControlOutputV5 {
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

pub(super) fn bounded_message(message: String) -> String {
    if message.len() <= COMMAND_ARGUMENT_MAX_BYTES {
        return message;
    }
    let mut end = COMMAND_ARGUMENT_MAX_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_string()
}

pub(super) fn capture<R: Read + Send + 'static>(
    mut reader: R,
) -> thread::JoinHandle<CapturedBytes> {
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
pub(super) fn signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
pub(super) fn signal(_status: &ExitStatus) -> Option<i32> {
    None
}

pub(super) fn disposition(status: ExitStatus) -> AntennaControlDispositionV5 {
    status.code().map_or_else(
        || AntennaControlDispositionV5::Signaled {
            signal: signal(&status),
        },
        |code| AntennaControlDispositionV5::Exit { code },
    )
}

pub(super) fn execute_process(
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

pub(super) fn invocation(
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

pub(super) fn execute_profile_attempt(
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

pub(super) fn rig_record(
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
            runtime_context_id: None,
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

pub(super) fn invocation_diagnostic(invocation: &AntennaControlInvocationV5) -> String {
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

pub(super) fn attempt_diagnostic(
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
