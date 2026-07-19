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
    next_wspr_cycle_after_ready,
    v2::{EventTimeBasisV2, MutationMember, Provenance, SessionLifecycleV2},
    v3::{
        project_wspr_run_v3, BundleV3Contents, OperatorEventPayloadV3, OperatorEventV3,
        RecordMetaV3, RigRecordV3, WsprCycleIntentV3,
    },
    v5::{
        AntennaControlCommandV5, AntennaControlContextV5, AntennaControlDispositionV5,
        AntennaControlInvocationPolicyV5, AntennaControlInvocationV5,
        AntennaControlOutputEncodingV5, AntennaControlOutputV5, AntennaControlPolicyV5,
        AntennaControlRoleV5, WsprReadinessBasisV5, COMMAND_ARGUMENT_COUNT_MAX,
        COMMAND_ARGUMENT_MAX_BYTES, COMMAND_INVOCATION_MAX_BYTES, COMMAND_OUTPUT_MAX_BYTES,
        COMMAND_PROGRAM_MAX_BYTES, COMMAND_TEMPLATE_MAX_BYTES,
    },
    v6::{DiagnosticOperationV6, DiagnosticPhaseV6, EvidenceEffectV6},
    ExperimentMode, RecordSource, SCHEMA_VERSION_V5,
};
use antennabench_storage::{BundleStore, LiveAntennaControlMutationV5, SystemLivePersistenceHooks};
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

mod automation;
mod commands;
mod policy;
mod process;
mod profiles;

pub(crate) use automation::schedule_automatic_coordinator;
pub(crate) use commands::{
    active_session_antenna_controller, antenna_controller_profiles,
    attach_active_session_antenna_controller, delete_antenna_controller_profile,
    run_active_session_antenna_controller, save_antenna_controller_profile,
};
pub(crate) use policy::AntennaControllerState;
pub(crate) use profiles::{
    activate_setup_controller, controller_profiles_for_app, prepare_setup_controller,
    ControllerProfile, PreparedSetupController, SetupControllerDraft, SetupControllerReview,
};
#[cfg(test)]
pub(crate) use profiles::{ControllerCommandDraft, ControllerProfileDraft, ControllerTargetDraft};

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

#[cfg(test)]
pub(crate) mod tests {
    use super::{policy::*, process::*, profiles::*, *};

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
            direction: antennabench_core::v3::WsprCycleDirection::Transmit,
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

    #[test]
    fn deleting_a_profile_removes_its_local_session_associations() {
        let mut catalog = ControllerCatalog {
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

        assert!(remove_profile(&mut catalog, "profile-1"));
        assert!(catalog.profiles.is_empty());
        assert!(catalog.associations.is_empty());
        assert!(!remove_profile(&mut catalog, "profile-1"));
        assert!(validate_catalog(&catalog).is_ok());
    }
}
