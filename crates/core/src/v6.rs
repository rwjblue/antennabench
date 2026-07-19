//! Schema-v6 runtime identity and attribution contract.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::v2::MutationMember;
use crate::{v3::BundleV3Contents, SCHEMA_VERSION_V6};

pub const RUNTIME_CONTEXT_SCHEMA_V1: &str = "runtime_context.v1";
pub const RUNTIME_CONTEXT_RECORD_MAX_BYTES: usize = 4 * 1024;
pub const RUNTIME_CONTEXT_MAX_RECORDS: usize = 256;
pub const RUNTIME_CONTEXT_STREAM_MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStateV6 {
    Clean,
    Dirty,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildChannelV6 {
    OfficialRelease,
    Development,
    Local,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildTimestampSourceV6 {
    SourceDateEpoch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildTimestampV6 {
    pub value: DateTime<Utc>,
    pub source: BuildTimestampSourceV6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildIdentityV6 {
    pub app_version: Option<String>,
    pub source_commit: Option<String>,
    pub source_state: SourceStateV6,
    pub build_channel: BuildChannelV6,
    pub release_tag: Option<String>,
    pub target_triple: Option<String>,
    pub build_architecture: Option<String>,
    pub build_timestamp: Option<BuildTimestampV6>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePlatformV6 {
    pub os_family: Option<String>,
    pub os_version: Option<String>,
    pub runtime_architecture: Option<String>,
    pub application_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContextV6 {
    pub schema: String,
    pub context_id: String,
    pub first_recorded_at: DateTime<Utc>,
    pub mutation: MutationMember,
    pub build: BuildIdentityV6,
    pub platform: RuntimePlatformV6,
}

impl RuntimeContextV6 {
    pub fn new(
        first_recorded_at: DateTime<Utc>,
        mutation: MutationMember,
        build: BuildIdentityV6,
        platform: RuntimePlatformV6,
    ) -> Self {
        let context_id = runtime_context_id_v6(&build, &platform);
        Self {
            schema: RUNTIME_CONTEXT_SCHEMA_V1.into(),
            context_id,
            first_recorded_at,
            mutation,
            build,
            platform,
        }
    }

    pub fn has_valid_identity(&self) -> bool {
        self.schema == RUNTIME_CONTEXT_SCHEMA_V1
            && self.context_id == runtime_context_id_v6(&self.build, &self.platform)
    }
}

pub fn runtime_context_id_v6(build: &BuildIdentityV6, platform: &RuntimePlatformV6) -> String {
    let timestamp_value = build.build_timestamp.as_ref().map(|timestamp| {
        timestamp
            .value
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    });
    let timestamp_source = build
        .build_timestamp
        .as_ref()
        .map(|timestamp| match timestamp.source {
            BuildTimestampSourceV6::SourceDateEpoch => "source_date_epoch",
        });
    let source_state = match build.source_state {
        SourceStateV6::Clean => "clean",
        SourceStateV6::Dirty => "dirty",
        SourceStateV6::Unknown => "unknown",
    };
    let channel = match build.build_channel {
        BuildChannelV6::OfficialRelease => "official_release",
        BuildChannelV6::Development => "development",
        BuildChannelV6::Local => "local",
        BuildChannelV6::Unknown => "unknown",
    };
    let fields = [
        build.app_version.as_deref(),
        build.source_commit.as_deref(),
        Some(source_state),
        Some(channel),
        build.release_tag.as_deref(),
        build.target_triple.as_deref(),
        build.build_architecture.as_deref(),
        timestamp_value.as_deref(),
        timestamp_source,
        platform.os_family.as_deref(),
        platform.os_version.as_deref(),
        platform.runtime_architecture.as_deref(),
        platform.application_id.as_deref(),
    ];
    let mut digest = Sha256::new();
    digest.update(b"antennabench.runtime-context.v1\0");
    for field in fields {
        match field {
            Some(value) => {
                let bytes = value.as_bytes();
                digest.update(u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes());
                digest.update(bytes);
            }
            None => digest.update(u32::MAX.to_be_bytes()),
        }
    }
    let mut id = String::with_capacity(68);
    id.push_str("ctx_");
    for byte in digest.finalize() {
        use std::fmt::Write as _;
        write!(&mut id, "{byte:02x}").expect("writing to a string cannot fail");
    }
    id
}

pub fn legacy_creator_context_v6(
    app_version: Option<String>,
    first_recorded_at: DateTime<Utc>,
    mutation: MutationMember,
) -> RuntimeContextV6 {
    RuntimeContextV6::new(
        first_recorded_at,
        mutation,
        BuildIdentityV6 {
            app_version,
            source_commit: None,
            source_state: SourceStateV6::Unknown,
            build_channel: BuildChannelV6::Unknown,
            release_tag: None,
            target_triple: None,
            build_architecture: None,
            build_timestamp: None,
        },
        RuntimePlatformV6 {
            os_family: None,
            os_version: None,
            runtime_architecture: None,
            application_id: None,
        },
    )
}

/// Upgrades legacy modeled contents without inventing historical machine facts.
///
/// Existing records are attributed to a synthetic creator context containing
/// only the legacy manifest's actual application version. The supplied real
/// upgrader context becomes active for subsequent mutations.
pub fn upgrade_v5_bundle_model_to_v6(
    mut bundle: BundleV3Contents,
    mut upgrader: RuntimeContextV6,
) -> BundleV3Contents {
    let legacy = legacy_creator_context_v6(
        Some(bundle.manifest.app_version.clone()),
        bundle.manifest.created_at,
        MutationMember {
            mutation_id: "upgrade-legacy-creator-context".into(),
            member_index: 0,
            member_count: 1,
        },
    );
    let legacy_id = legacy.context_id.clone();
    upgrader.mutation = MutationMember {
        mutation_id: "upgrade-runtime-context".into(),
        member_index: 0,
        member_count: 1,
    };
    let active_id = upgrader.context_id.clone();
    bundle.manifest.schema_version = SCHEMA_VERSION_V6;
    bundle.manifest.files.runtime_contexts = Some("runtime-contexts.jsonl".into());
    bundle.manifest.files.diagnostics = Some("diagnostics.jsonl".into());
    bundle.manifest.creator_runtime_context_id = Some(legacy_id.clone());
    bundle.session_state.schema_version = SCHEMA_VERSION_V6;
    bundle.session_state.active_runtime_context_id = Some(active_id.clone());
    bundle.station.schema_version = SCHEMA_VERSION_V6;
    bundle.antennas.schema_version = SCHEMA_VERSION_V6;
    bundle.schedule.schema_version = SCHEMA_VERSION_V6;
    bundle.analysis.schema_version = SCHEMA_VERSION_V6;
    for meta in bundle
        .events
        .iter_mut()
        .map(|record| &mut record.meta)
        .chain(bundle.rig.iter_mut().map(|record| &mut record.meta))
    {
        meta.schema_version = SCHEMA_VERSION_V6;
        meta.runtime_context_id = Some(legacy_id.clone());
    }
    for meta in bundle
        .observations
        .iter_mut()
        .map(|record| &mut record.meta)
        .chain(
            bundle
                .adapter_records
                .iter_mut()
                .map(|record| &mut record.meta),
        )
        .chain(bundle.propagation.iter_mut().map(|record| &mut record.meta))
    {
        meta.schema_version = SCHEMA_VERSION_V6;
        meta.runtime_context_id = Some(legacy_id.clone());
    }
    bundle.runtime_contexts = if legacy_id == active_id {
        vec![legacy]
    } else {
        vec![legacy, upgrader]
    };
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_stable_and_excludes_record_metadata() {
        let build = BuildIdentityV6 {
            app_version: Some("0.1.0".into()),
            source_commit: Some("7d9f4fef52e831b1e5689ca7d2a93b0a56122fd4".into()),
            source_state: SourceStateV6::Clean,
            build_channel: BuildChannelV6::OfficialRelease,
            release_tag: Some("v0.1.0".into()),
            target_triple: Some("aarch64-apple-darwin".into()),
            build_architecture: Some("aarch64".into()),
            build_timestamp: Some(BuildTimestampV6 {
                value: "2026-07-19T12:00:00Z".parse().unwrap(),
                source: BuildTimestampSourceV6::SourceDateEpoch,
            }),
        };
        let platform = RuntimePlatformV6 {
            os_family: Some("macos".into()),
            os_version: Some("15.5".into()),
            runtime_architecture: Some("aarch64".into()),
            application_id: Some("com.rwjblue.antennabench".into()),
        };
        assert_eq!(
            runtime_context_id_v6(&build, &platform),
            "ctx_68b55d43ba3113d787296c2b5995a45e4d4644d3e2c3a69ef8fc11c823cfaf13"
        );
    }

    #[test]
    fn platform_fixtures_are_distinct_without_device_identity() {
        let build = BuildIdentityV6 {
            app_version: Some("0.1.0-dev".into()),
            source_commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            source_state: SourceStateV6::Dirty,
            build_channel: BuildChannelV6::Development,
            release_tag: None,
            target_triple: Some("portable-test-target".into()),
            build_architecture: Some("test-arch".into()),
            build_timestamp: None,
        };
        let ids = [
            ("macos", "15.5", "aarch64"),
            ("linux", "6", "x86_64"),
            ("windows", "11", "x86_64"),
        ]
        .map(|(family, version, architecture)| {
            let platform = RuntimePlatformV6 {
                os_family: Some(family.into()),
                os_version: Some(version.into()),
                runtime_architecture: Some(architecture.into()),
                application_id: Some("com.rwjblue.antennabench".into()),
            };
            let json = serde_json::to_string(&platform).unwrap();
            for forbidden in ["hostname", "username", "serial", "home", "location"] {
                assert!(!json.contains(forbidden));
            }
            runtime_context_id_v6(&build, &platform)
        });
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
    }
}
