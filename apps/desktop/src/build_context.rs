use std::process::Command;

use antennabench_core::{
    v2::MutationMember,
    v6::{
        BuildChannelV6, BuildIdentityV6, BuildTimestampSourceV6, BuildTimestampV6,
        RuntimeContextV6, RuntimePlatformV6, SourceStateV6,
    },
};
use antennabench_storage::{
    BundleStore, LivePersistenceError, LivePersistenceHooks, LiveSessionV3, RecoveryReportV2,
};
use chrono::{DateTime, Utc};
use std::sync::Arc;

const APPLICATION_ID: &str = "com.rwjblue.antennabench";

pub(crate) fn open_v3_writer(store: &BundleStore) -> Result<LiveSessionV3, LivePersistenceError> {
    if store.schema_version()? == antennabench_core::SCHEMA_VERSION_V6 {
        store.open_v3_writer_in_context(current_runtime_context(Utc::now(), pending_membership()))
    } else {
        store.open_v3_writer()
    }
}

pub(crate) fn open_v3_writer_with_hooks(
    store: &BundleStore,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<LiveSessionV3, LivePersistenceError> {
    if store.schema_version()? == antennabench_core::SCHEMA_VERSION_V6 {
        store.open_v3_writer_with_hooks_in_context(
            hooks.clone(),
            current_runtime_context(hooks.now(), pending_membership()),
        )
    } else {
        store.open_v3_writer_with_hooks(hooks)
    }
}

pub(crate) fn recover_v3_with_hooks(
    store: &BundleStore,
    hooks: Arc<dyn LivePersistenceHooks>,
) -> Result<RecoveryReportV2, LivePersistenceError> {
    if store.schema_version()? == antennabench_core::SCHEMA_VERSION_V6 {
        store.recover_v3_with_hooks_in_context(
            hooks.clone(),
            current_runtime_context(hooks.now(), pending_membership()),
        )
    } else {
        store.recover_v3_with_hooks(hooks)
    }
}

fn pending_membership() -> MutationMember {
    MutationMember {
        mutation_id: "pending-runtime-context".into(),
        member_index: 0,
        member_count: 1,
    }
}

fn compiled(value: &'static str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn current_runtime_context(
    first_recorded_at: DateTime<Utc>,
    mutation: MutationMember,
) -> RuntimeContextV6 {
    let build_timestamp =
        compiled(env!("ANTENNABENCH_BUILD_SOURCE_DATE_EPOCH")).and_then(|epoch| {
            epoch
                .parse::<i64>()
                .ok()
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .map(|value| BuildTimestampV6 {
                    value,
                    source: BuildTimestampSourceV6::SourceDateEpoch,
                })
        });
    RuntimeContextV6::new(
        first_recorded_at,
        mutation,
        BuildIdentityV6 {
            app_version: compiled(env!("ANTENNABENCH_BUILD_APP_VERSION")),
            source_commit: compiled(env!("ANTENNABENCH_BUILD_SOURCE_COMMIT")),
            source_state: match env!("ANTENNABENCH_BUILD_SOURCE_STATE") {
                "clean" => SourceStateV6::Clean,
                "dirty" => SourceStateV6::Dirty,
                _ => SourceStateV6::Unknown,
            },
            build_channel: match env!("ANTENNABENCH_BUILD_CHANNEL_VALUE") {
                "official_release" => BuildChannelV6::OfficialRelease,
                "development" => BuildChannelV6::Development,
                "local" => BuildChannelV6::Local,
                _ => BuildChannelV6::Unknown,
            },
            release_tag: compiled(env!("ANTENNABENCH_BUILD_RELEASE_TAG")),
            target_triple: compiled(env!("ANTENNABENCH_BUILD_TARGET_TRIPLE")),
            build_architecture: compiled(env!("ANTENNABENCH_BUILD_ARCH")),
            build_timestamp,
        },
        RuntimePlatformV6 {
            os_family: Some(std::env::consts::OS.into()),
            os_version: os_version(),
            runtime_architecture: Some(std::env::consts::ARCH.into()),
            application_id: Some(APPLICATION_ID.into()),
        },
    )
}

fn os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        output.status.success().then(|| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .chars()
                .take(64)
                .collect()
        })
    }
    #[cfg(target_os = "linux")]
    {
        let contents = std::fs::read_to_string("/etc/os-release").ok()?;
        contents.lines().find_map(|line| {
            line.strip_prefix("VERSION_ID=")
                .map(|value| value.trim_matches('"').chars().take(64).collect())
        })
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_context_is_bounded_and_private() {
        let context = current_runtime_context(
            "2026-07-19T12:00:00Z".parse().unwrap(),
            MutationMember {
                mutation_id: "context-test".into(),
                member_index: 0,
                member_count: 1,
            },
        );
        assert!(context.has_valid_identity());
        let json = serde_json::to_string(&context).unwrap();
        assert!(!json.contains("HOME"));
        assert!(!json.contains("hostname"));
        assert!(!json.contains("username"));
        assert!(json.len() < 4096);
    }
}
