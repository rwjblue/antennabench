use antennabench_core::v2::MutationMember;
use antennabench_core::v6::{
    DiagnosticCauseV6, DiagnosticDetailStateV6, DiagnosticDetailStatusV6, DiagnosticFactV6,
    DiagnosticFactValueV6, DiagnosticOperationV6, DiagnosticOutcomeV6, DiagnosticPhaseV6,
    DiagnosticRetryV6, DiagnosticSeverityV6, DiagnosticTargetV6, DiagnosticsStateV6,
    EvidenceEffectV6, OperationalDiagnosticV6, RetryDispositionV6, DIAGNOSTIC_MAX_RECORDS,
    DIAGNOSTIC_RECORD_MAX_BYTES, DIAGNOSTIC_STREAM_MAX_BYTES, OPERATIONAL_DIAGNOSTIC_SCHEMA_V1,
};

use super::{
    checkpoint::{commit_checkpoint, read_state, stream_path},
    mutation::{append_line, preflight_live_budget, rollback_v3_streams, serialize_v3_lines},
    LivePersistenceError, LiveSessionV3, LiveStreamV2, RecoveryDispositionV2,
};
use crate::BundleStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveDiagnosticMutationV6 {
    pub expected_revision: u64,
    pub mutation_id: String,
    pub diagnostic: OperationalDiagnosticV6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCommitDispositionV6 {
    Committed,
    Saturated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticCommitReceiptV6 {
    pub revision: u64,
    pub diagnostic_id: Option<String>,
    pub disposition: DiagnosticCommitDispositionV6,
    pub idempotent: bool,
}

impl LiveSessionV3 {
    pub(super) fn append_recovery_diagnostic(
        &mut self,
        starting_revision: u64,
        disposition: RecoveryDispositionV2,
        artifact_count: usize,
        interruption_recorded: bool,
    ) -> Result<DiagnosticCommitReceiptV6, LivePersistenceError> {
        let revision = self.bundle.session_state.revision;
        let disposition_code = match disposition {
            RecoveryDispositionV2::Clean => "clean",
            RecoveryDispositionV2::RolledForward => "rolled_forward",
            RecoveryDispositionV2::RolledBack => "rolled_back",
            RecoveryDispositionV2::IdempotentTailRemoved => "idempotent_tail_removed",
        };
        let attempt_id = format!("checkpoint-recovery-{starting_revision}-{revision}");
        let diagnostic = OperationalDiagnosticV6 {
            schema: OPERATIONAL_DIAGNOSTIC_SCHEMA_V1.into(),
            diagnostic_id: self.allocate_id("diagnostic"),
            correlation_id: format!("checkpoint-recovery-{starting_revision}"),
            attempt_id,
            mutation: MutationMember {
                mutation_id: "pending-diagnostic".into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: String::new(),
            occurred_at: self.hooks.now(),
            operation: DiagnosticOperationV6::CheckpointRecovery,
            phase: DiagnosticPhaseV6::Recover,
            code: "session.checkpoint_recovered".into(),
            summary: "Checkpoint recovery retained earlier evidence and restored a writable head."
                .into(),
            outcome: DiagnosticOutcomeV6::Recovered,
            severity: DiagnosticSeverityV6::Warning,
            revision_before: Some(starting_revision),
            revision_after: Some(revision),
            diagnostic_revision: revision,
            evidence_effect: EvidenceEffectV6::EarlierEvidenceRetained,
            retry: DiagnosticRetryV6 {
                disposition: RetryDispositionV6::NotRetryable,
                guidance_code: "inspect_recovery_artifacts_before_continuing".into(),
            },
            targets: vec![DiagnosticTargetV6::Source {
                id: "session-checkpoint".into(),
            }],
            causes: vec![DiagnosticCauseV6 {
                code: "session.recovery_trigger".into(),
                phase: DiagnosticPhaseV6::Recover,
                facts: vec![
                    DiagnosticFactV6 {
                        name: "disposition".into(),
                        value: DiagnosticFactValueV6::Enum(disposition_code.into()),
                    },
                    DiagnosticFactV6 {
                        name: "artifact_count".into(),
                        value: DiagnosticFactValueV6::U64(
                            u64::try_from(artifact_count).unwrap_or(u64::MAX),
                        ),
                    },
                    DiagnosticFactV6 {
                        name: "interruption_recorded".into(),
                        value: DiagnosticFactValueV6::Bool(interruption_recorded),
                    },
                ],
            }],
            detail_status: DiagnosticDetailStatusV6 {
                state: DiagnosticDetailStateV6::Complete,
                omitted_fact_count: 0,
            },
        };
        let mutation_id = self.allocate_id("diagnostic-mutation");
        self.append_diagnostic(LiveDiagnosticMutationV6 {
            expected_revision: revision,
            mutation_id,
            diagnostic,
        })
    }

    pub fn append_diagnostic(
        &mut self,
        mut mutation: LiveDiagnosticMutationV6,
    ) -> Result<DiagnosticCommitReceiptV6, LivePersistenceError> {
        if self.bundle.manifest.schema_version != antennabench_core::SCHEMA_VERSION_V6 {
            return Err(LivePersistenceError::InvalidMutation {
                message: "operational diagnostics require schema v6".into(),
            });
        }
        if let Some(existing) = self
            .bundle
            .diagnostics
            .iter()
            .find(|existing| existing.attempt_id == mutation.diagnostic.attempt_id)
        {
            return if same_diagnostic_outcome(existing, &mutation.diagnostic) {
                Ok(DiagnosticCommitReceiptV6 {
                    revision: self.bundle.session_state.revision,
                    diagnostic_id: Some(existing.diagnostic_id.clone()),
                    disposition: DiagnosticCommitDispositionV6::Committed,
                    idempotent: true,
                })
            } else {
                Err(LivePersistenceError::MutationConflict {
                    mutation_id: mutation.diagnostic.attempt_id,
                })
            };
        }
        if self
            .bundle
            .session_state
            .last_committed_mutation_id
            .as_deref()
            == Some(mutation.mutation_id.as_str())
            && self
                .bundle
                .session_state
                .diagnostics_status
                .as_ref()
                .is_some_and(|status| status.state == DiagnosticsStateV6::Saturated)
        {
            return Ok(DiagnosticCommitReceiptV6 {
                revision: self.bundle.session_state.revision,
                diagnostic_id: None,
                disposition: DiagnosticCommitDispositionV6::Saturated,
                idempotent: true,
            });
        }
        if mutation.expected_revision != self.bundle.session_state.revision {
            return Err(LivePersistenceError::StaleRevision {
                expected: mutation.expected_revision,
                actual: self.bundle.session_state.revision,
            });
        }
        let next_revision = self
            .bundle
            .session_state
            .revision
            .checked_add(1)
            .ok_or_else(|| LivePersistenceError::InvalidMutation {
                message: "checkpoint revision overflowed".into(),
            })?;
        let occurred_at = self.hooks.now();
        let actor = self.prepare_runtime_actor(&mutation.mutation_id, 1, occurred_at)?;
        mutation.diagnostic.occurred_at = occurred_at;
        mutation.diagnostic.runtime_context_id =
            actor
                .context_id
                .clone()
                .ok_or_else(|| LivePersistenceError::InvalidMutation {
                    message: "diagnostic mutation has no runtime context".into(),
                })?;
        mutation.diagnostic.mutation.mutation_id = mutation.mutation_id.clone();
        mutation.diagnostic.mutation.member_index = actor.member_offset;
        mutation.diagnostic.mutation.member_count = actor.member_count;
        mutation.diagnostic.diagnostic_revision = next_revision;

        let diagnostic_bytes =
            serialize_v3_lines(std::slice::from_ref(&mutation.diagnostic), "diagnostic")?;
        let diagnostic_head = self
            .bundle
            .session_state
            .streams
            .get(LiveStreamV2::Diagnostics.checkpoint_name())
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: "checkpoint is missing diagnostics".into(),
            })?;
        if diagnostic_bytes.len() > DIAGNOSTIC_RECORD_MAX_BYTES {
            return Err(LivePersistenceError::ResourceLimit {
                code: "resource.diagnostic_record_bytes",
                stream: LiveStreamV2::Diagnostics.checkpoint_name(),
                observed: u64::try_from(diagnostic_bytes.len()).unwrap_or(u64::MAX),
                limit: DIAGNOSTIC_RECORD_MAX_BYTES as u64,
            });
        }
        if diagnostic_retention_exhausted(
            diagnostic_head.record_count,
            diagnostic_head.committed_bytes,
            u64::try_from(diagnostic_bytes.len()).unwrap_or(u64::MAX),
        ) {
            return self.commit_diagnostic_saturation(
                mutation.mutation_id,
                occurred_at,
                next_revision,
            );
        }

        let mut candidate = self.bundle.clone();
        Self::apply_runtime_actor(&mut candidate, &actor);
        candidate.diagnostics.push(mutation.diagnostic.clone());
        candidate.session_state.revision = next_revision;
        candidate.session_state.last_committed_mutation_id = Some(mutation.mutation_id.clone());
        BundleStore::refresh_v3_checkpoint(&mut candidate)?;
        crate::v3::validate_v3_model(&candidate)?;

        let context_bytes = actor
            .new_context
            .as_ref()
            .map(|context| serialize_v3_lines(std::slice::from_ref(context), "runtime context"))
            .transpose()?
            .unwrap_or_default();
        let serialized = vec![
            (LiveStreamV2::RuntimeContexts, context_bytes),
            (LiveStreamV2::Diagnostics, diagnostic_bytes),
        ]
        .into_iter()
        .filter(|(_, bytes)| !bytes.is_empty())
        .collect::<Vec<_>>();
        preflight_live_budget(&self.store, &self.bundle.session_state, &serialized)?;

        let baseline = self.bundle.session_state.clone();
        let operation = (|| {
            for (stream, bytes) in &serialized {
                append_line(
                    stream_path(&self.paths, *stream),
                    *stream,
                    bytes,
                    self.hooks.as_ref(),
                )?;
            }
            commit_checkpoint(
                self.store.root(),
                &self.paths.session_state,
                &candidate.session_state,
                self.hooks.as_ref(),
            )
        })();
        if let Err(error) = operation {
            if read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline.revision)
            {
                self.bundle = self.store.read_v3()?;
            } else {
                let mut rollback = vec![LiveStreamV2::Diagnostics];
                if actor.new_context.is_some() {
                    rollback.insert(0, LiveStreamV2::RuntimeContexts);
                }
                rollback_v3_streams(&self.paths, &baseline, &rollback)?;
            }
            return Err(error);
        }
        self.bundle = candidate;
        self.pending_runtime_context = None;
        self.hooks
            .check(super::LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| {
                super::live_io("acknowledge checkpoint", self.store.root(), source)
            })?;
        Ok(DiagnosticCommitReceiptV6 {
            revision: next_revision,
            diagnostic_id: Some(mutation.diagnostic.diagnostic_id),
            disposition: DiagnosticCommitDispositionV6::Committed,
            idempotent: false,
        })
    }

    fn commit_diagnostic_saturation(
        &mut self,
        mutation_id: String,
        occurred_at: chrono::DateTime<chrono::Utc>,
        next_revision: u64,
    ) -> Result<DiagnosticCommitReceiptV6, LivePersistenceError> {
        let mut candidate = self.bundle.clone();
        let status = candidate
            .session_state
            .diagnostics_status
            .as_mut()
            .ok_or_else(|| LivePersistenceError::CheckpointVerification {
                message: "checkpoint is missing diagnostics status".into(),
            })?;
        status.state = DiagnosticsStateV6::Saturated;
        status.omitted_count = status.omitted_count.saturating_add(1);
        status.first_omitted_at.get_or_insert(occurred_at);
        status.reason_code = Some("resource.diagnostic_retention".into());
        candidate.session_state.revision = next_revision;
        candidate.session_state.last_committed_mutation_id = Some(mutation_id);
        BundleStore::refresh_v3_checkpoint(&mut candidate)?;
        crate::v3::validate_v3_model(&candidate)?;
        let baseline_revision = self.bundle.session_state.revision;
        if let Err(error) = commit_checkpoint(
            self.store.root(),
            &self.paths.session_state,
            &candidate.session_state,
            self.hooks.as_ref(),
        ) {
            if read_state(&self.paths.session_state)
                .is_ok_and(|state| state.revision > baseline_revision)
            {
                self.bundle = self.store.read_v3()?;
            }
            return Err(error);
        }
        self.bundle = candidate;
        self.hooks
            .check(super::LivePersistencePoint::BeforeAcknowledge)
            .map_err(|source| {
                super::live_io("acknowledge checkpoint", self.store.root(), source)
            })?;
        Ok(DiagnosticCommitReceiptV6 {
            revision: next_revision,
            diagnostic_id: None,
            disposition: DiagnosticCommitDispositionV6::Saturated,
            idempotent: false,
        })
    }
}

fn diagnostic_retention_exhausted(
    record_count: u64,
    committed_bytes: u64,
    next_bytes: u64,
) -> bool {
    record_count >= DIAGNOSTIC_MAX_RECORDS as u64
        || committed_bytes.saturating_add(next_bytes) > DIAGNOSTIC_STREAM_MAX_BYTES as u64
}

fn same_diagnostic_outcome(
    existing: &OperationalDiagnosticV6,
    proposed: &OperationalDiagnosticV6,
) -> bool {
    existing.correlation_id == proposed.correlation_id
        && existing.attempt_id == proposed.attempt_id
        && existing.operation == proposed.operation
        && existing.phase == proposed.phase
        && existing.code == proposed.code
        && existing.summary == proposed.summary
        && existing.outcome == proposed.outcome
        && existing.severity == proposed.severity
        && existing.revision_before == proposed.revision_before
        && existing.revision_after == proposed.revision_after
        && existing.evidence_effect == proposed.evidence_effect
        && existing.retry == proposed.retry
        && existing.targets == proposed.targets
        && existing.causes == proposed.causes
        && existing.detail_status == proposed.detail_status
}

#[cfg(test)]
mod tests {
    use super::diagnostic_retention_exhausted;
    use antennabench_core::v6::{DIAGNOSTIC_MAX_RECORDS, DIAGNOSTIC_STREAM_MAX_BYTES};

    #[test]
    fn diagnostic_retention_has_explicit_n_minus_one_n_and_n_plus_one_boundaries() {
        let n = DIAGNOSTIC_MAX_RECORDS as u64;
        assert!(!diagnostic_retention_exhausted(n - 1, 0, 1));
        assert!(diagnostic_retention_exhausted(n, 0, 1));
        assert!(diagnostic_retention_exhausted(n + 1, 0, 1));

        let n = DIAGNOSTIC_STREAM_MAX_BYTES as u64;
        assert!(!diagnostic_retention_exhausted(0, n - 1, 1));
        assert!(!diagnostic_retention_exhausted(0, n, 0));
        assert!(diagnostic_retention_exhausted(0, n, 1));
    }
}
