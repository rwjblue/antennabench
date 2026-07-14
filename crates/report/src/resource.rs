use std::io::{self, Write};

use antennabench_analysis::{
    AnalysisCancellationToken, AnalysisResourceLimits, ANALYSIS_RESOURCE_LIMITS,
};
use thiserror::Error;

pub const REPORT_ROWS: u64 = 25_000;
pub const REPORT_MODEL_BYTES: u64 = 8 * 1024 * 1024;
pub const REPORT_HTML_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportResourceLimits {
    pub analysis: AnalysisResourceLimits,
    pub rows: u64,
    pub model_bytes: u64,
    pub html_bytes: u64,
}

pub const REPORT_RESOURCE_LIMITS: ReportResourceLimits = ReportResourceLimits {
    analysis: ANALYSIS_RESOURCE_LIMITS,
    rows: REPORT_ROWS,
    model_bytes: REPORT_MODEL_BYTES,
    html_bytes: REPORT_HTML_BYTES,
};

impl ReportResourceLimits {
    #[doc(hidden)]
    pub fn testing(rows: u64, model_bytes: u64, html_bytes: u64) -> Self {
        Self {
            analysis: ANALYSIS_RESOURCE_LIMITS,
            rows,
            model_bytes,
            html_bytes,
        }
    }
}

pub type ReportCancellationToken = AnalysisCancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportResourceStage {
    Projection,
    Serialize,
    Render,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportResourceDiagnostic {
    pub code: &'static str,
    pub profile: &'static str,
    pub profile_version: u16,
    pub stage: ReportResourceStage,
    pub role: &'static str,
    pub limit: u64,
    pub observed: Option<u64>,
    pub unit: &'static str,
    pub retryable_without_input_change: bool,
    pub complete_report: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("report resource limit {diagnostic:?}")]
pub struct ReportResourceError {
    pub diagnostic: ReportResourceDiagnostic,
}

pub(crate) fn report_resource_error(
    code: &'static str,
    stage: ReportResourceStage,
    role: &'static str,
    limit: u64,
    observed: Option<u64>,
    unit: &'static str,
) -> ReportResourceError {
    ReportResourceError {
        diagnostic: ReportResourceDiagnostic {
            code,
            profile: "local-standard-v1",
            profile_version: 1,
            stage,
            role,
            limit,
            observed,
            unit,
            retryable_without_input_change: false,
            complete_report: false,
        },
    }
}

pub(crate) fn check_cancelled(
    cancellation: &ReportCancellationToken,
    stage: ReportResourceStage,
    role: &'static str,
) -> Result<(), ReportResourceError> {
    if cancellation.is_cancelled() {
        Err(report_resource_error(
            "resource.operation.cancelled",
            stage,
            role,
            0,
            None,
            "checkpoints",
        ))
    } else {
        Ok(())
    }
}

pub(crate) struct CountingWriter {
    limit: u64,
    written: u64,
    exceeded: Option<u64>,
}

impl CountingWriter {
    pub(crate) fn new(limit: u64) -> Self {
        Self {
            limit,
            written: 0,
            exceeded: None,
        }
    }

    pub(crate) fn observed(&self) -> u64 {
        self.exceeded.unwrap_or(self.written)
    }
}

impl Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let observed = self.written.saturating_add(bytes.len() as u64);
        if observed > self.limit {
            self.exceeded = Some(observed);
            return Err(io::Error::new(io::ErrorKind::FileTooLarge, "model limit"));
        }
        self.written = observed;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
