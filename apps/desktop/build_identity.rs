#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ResolvedSourceIdentity {
    pub(crate) channel: String,
    pub(crate) commit: Option<String>,
    pub(crate) state: String,
}

pub(crate) fn resolve_source_identity(
    profile: &str,
    explicit_channel: Option<String>,
    explicit_commit: Option<String>,
    explicit_state: Option<String>,
    git_commit: Option<String>,
    git_dirty: Option<bool>,
) -> ResolvedSourceIdentity {
    let channel = explicit_channel.unwrap_or_else(|| {
        if profile == "debug" {
            "development".into()
        } else {
            "local".into()
        }
    });
    let commit = explicit_commit.or(git_commit);
    let state = explicit_state.unwrap_or_else(|| match git_dirty {
        Some(true) => "dirty".into(),
        Some(false) => "clean".into(),
        None => "unknown".into(),
    });
    ResolvedSourceIdentity {
        channel,
        commit,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_builds_preserve_actual_dirty_source_identity() {
        assert_eq!(
            resolve_source_identity(
                "debug",
                None,
                None,
                None,
                Some("0123456789abcdef0123456789abcdef01234567".into()),
                Some(true),
            ),
            ResolvedSourceIdentity {
                channel: "development".into(),
                commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
                state: "dirty".into(),
            }
        );
    }

    #[test]
    fn unavailable_git_is_explicitly_unknown_without_fabricated_commit() {
        assert_eq!(
            resolve_source_identity("release", None, None, None, None, None),
            ResolvedSourceIdentity {
                channel: "local".into(),
                commit: None,
                state: "unknown".into(),
            }
        );
    }

    #[test]
    fn authoritative_release_inputs_override_local_probe_values() {
        assert_eq!(
            resolve_source_identity(
                "release",
                Some("official_release".into()),
                Some("fedcba9876543210fedcba9876543210fedcba98".into()),
                Some("clean".into()),
                None,
                None,
            ),
            ResolvedSourceIdentity {
                channel: "official_release".into(),
                commit: Some("fedcba9876543210fedcba9876543210fedcba98".into()),
                state: "clean".into(),
            }
        );
    }
}
