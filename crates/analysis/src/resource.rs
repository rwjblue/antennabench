use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use thiserror::Error;

pub const ANALYSIS_COLLECTION_ENTRIES: u64 = 500_000;
pub const ANALYSIS_LIVE_ENTRIES: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisResourceLimits {
    pub collection_entries: u64,
    pub live_entries: u64,
}

pub const ANALYSIS_RESOURCE_LIMITS: AnalysisResourceLimits = AnalysisResourceLimits {
    collection_entries: ANALYSIS_COLLECTION_ENTRIES,
    live_entries: ANALYSIS_LIVE_ENTRIES,
};

impl AnalysisResourceLimits {
    #[doc(hidden)]
    pub fn testing(collection_entries: u64, live_entries: u64) -> Self {
        Self {
            collection_entries,
            live_entries,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisCancellationToken(Arc<AtomicBool>);

impl AnalysisCancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisResourceStage {
    Plan,
    Align,
    Aggregate,
    Compare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResourceDiagnostic {
    pub code: &'static str,
    pub profile: &'static str,
    pub profile_version: u16,
    pub stage: AnalysisResourceStage,
    pub role: &'static str,
    pub limit: u64,
    pub observed: Option<u64>,
    pub unit: &'static str,
    pub retryable_without_input_change: bool,
    pub complete_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("analysis resource limit {diagnostic:?}")]
pub struct AnalysisResourceError {
    pub diagnostic: AnalysisResourceDiagnostic,
}

pub(crate) struct AnalysisBudget<'a> {
    limits: AnalysisResourceLimits,
    cancellation: &'a AnalysisCancellationToken,
}

impl<'a> AnalysisBudget<'a> {
    pub(crate) fn new(
        limits: AnalysisResourceLimits,
        cancellation: &'a AnalysisCancellationToken,
    ) -> Self {
        Self {
            limits,
            cancellation,
        }
    }

    pub(crate) fn collection(
        &self,
        stage: AnalysisResourceStage,
        role: &'static str,
        entries: usize,
    ) -> Result<(), AnalysisResourceError> {
        self.check(
            "resource.analysis.collection_entries",
            stage,
            role,
            self.limits.collection_entries,
            entries as u64,
        )
    }

    pub(crate) fn live(
        &self,
        stage: AnalysisResourceStage,
        role: &'static str,
        entries: u64,
    ) -> Result<(), AnalysisResourceError> {
        self.check(
            "resource.analysis.live_entries",
            stage,
            role,
            self.limits.live_entries,
            entries,
        )
    }

    pub(crate) fn checkpoint(
        &self,
        stage: AnalysisResourceStage,
        role: &'static str,
        work_index: usize,
    ) -> Result<(), AnalysisResourceError> {
        if work_index.is_multiple_of(1_000) {
            self.cancelled(stage, role)?;
        }
        Ok(())
    }

    pub(crate) fn cancelled(
        &self,
        stage: AnalysisResourceStage,
        role: &'static str,
    ) -> Result<(), AnalysisResourceError> {
        if self.cancellation.is_cancelled() {
            Err(AnalysisResourceError {
                diagnostic: AnalysisResourceDiagnostic {
                    code: "resource.operation.cancelled",
                    profile: "local-standard-v1",
                    profile_version: 1,
                    stage,
                    role,
                    limit: 0,
                    observed: None,
                    unit: "checkpoints",
                    retryable_without_input_change: false,
                    complete_result: false,
                },
            })
        } else {
            Ok(())
        }
    }

    fn check(
        &self,
        code: &'static str,
        stage: AnalysisResourceStage,
        role: &'static str,
        limit: u64,
        observed: u64,
    ) -> Result<(), AnalysisResourceError> {
        self.cancelled(stage, role)?;
        if observed > limit {
            Err(AnalysisResourceError {
                diagnostic: AnalysisResourceDiagnostic {
                    code,
                    profile: "local-standard-v1",
                    profile_version: 1,
                    stage,
                    role,
                    limit,
                    observed: Some(observed),
                    unit: "entries",
                    retryable_without_input_change: false,
                    complete_result: false,
                },
            })
        } else {
            Ok(())
        }
    }
}
