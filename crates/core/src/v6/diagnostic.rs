use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::v2::MutationMember;

pub const OPERATIONAL_DIAGNOSTIC_SCHEMA_V1: &str = "operational_diagnostic.v1";
pub const DIAGNOSTIC_RECORD_MAX_BYTES: usize = 8 * 1024;
pub const DIAGNOSTIC_MAX_RECORDS: usize = 2_048;
pub const DIAGNOSTIC_STREAM_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const DIAGNOSTIC_SUMMARY_MAX_BYTES: usize = 256;
pub const DIAGNOSTIC_MACHINE_VALUE_MAX_BYTES: usize = 64;
pub const DIAGNOSTIC_IDENTIFIER_MAX_BYTES: usize = 128;
pub const DIAGNOSTIC_TARGET_MAX_COUNT: usize = 8;
pub const DIAGNOSTIC_CAUSE_MAX_DEPTH: usize = 4;
pub const DIAGNOSTIC_FACT_MAX_COUNT: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOperationV6 {
    SessionMutation,
    CheckpointRecovery,
    WsprLiveAcquisition,
    AdapterFileImport,
    WsjtxIntake,
    AntennaControllerAttach,
    AntennaControllerSwitch,
    AntennaControllerVerify,
    ReportRender,
    ReportExport,
    BundleExport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticPhaseV6 {
    Admission,
    Preflight,
    Acquire,
    Parse,
    Normalize,
    Serialize,
    Checkpoint,
    Finalize,
    Recover,
    Render,
    WriteDestination,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOutcomeV6 {
    Failed,
    Partial,
    Recovered,
    Cancelled,
    CompletedIdempotently,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverityV6 {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceEffectV6 {
    NoneCommitted,
    EarlierEvidenceRetained,
    PrimaryEvidenceCommitted,
    PriorCommitReused,
    CancelledBeforeEffect,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDispositionV6 {
    Retryable,
    RequiresStateChange,
    RequiresInputChange,
    NotRetryable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRetryV6 {
    pub disposition: RetryDispositionV6,
    pub guidance_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticTargetV6 {
    Adapter {
        id: String,
    },
    Source {
        id: String,
    },
    Mutation {
        id: String,
    },
    Record {
        id: String,
    },
    Slot {
        id: String,
    },
    Intent {
        id: String,
    },
    AcquisitionWindow {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DiagnosticFactValueV6 {
    Bool(bool),
    I64(i64),
    U64(u64),
    Timestamp(DateTime<Utc>),
    Enum(String),
    Identifier(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticFactV6 {
    pub name: String,
    pub value: DiagnosticFactValueV6,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCauseV6 {
    pub code: String,
    pub phase: DiagnosticPhaseV6,
    pub facts: Vec<DiagnosticFactV6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticDetailStateV6 {
    Complete,
    Truncated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDetailStatusV6 {
    pub state: DiagnosticDetailStateV6,
    pub omitted_fact_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationalDiagnosticV6 {
    pub schema: String,
    pub diagnostic_id: String,
    pub correlation_id: String,
    pub attempt_id: String,
    pub mutation: MutationMember,
    pub runtime_context_id: String,
    pub occurred_at: DateTime<Utc>,
    pub operation: DiagnosticOperationV6,
    pub phase: DiagnosticPhaseV6,
    pub code: String,
    pub summary: String,
    pub outcome: DiagnosticOutcomeV6,
    pub severity: DiagnosticSeverityV6,
    pub revision_before: Option<u64>,
    pub revision_after: Option<u64>,
    pub diagnostic_revision: u64,
    pub evidence_effect: EvidenceEffectV6,
    pub retry: DiagnosticRetryV6,
    pub targets: Vec<DiagnosticTargetV6>,
    pub causes: Vec<DiagnosticCauseV6>,
    pub detail_status: DiagnosticDetailStatusV6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsStateV6 {
    Complete,
    Saturated,
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsStatusV6 {
    pub state: DiagnosticsStateV6,
    pub omitted_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_omitted_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
}

impl DiagnosticsStatusV6 {
    pub fn complete() -> Self {
        Self {
            state: DiagnosticsStateV6::Complete,
            omitted_count: 0,
            first_omitted_at: None,
            reason_code: None,
        }
    }
}
