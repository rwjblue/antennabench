use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use thiserror::Error;

pub const ADAPTER_RESOURCE_PROFILE_NAME: &str = "local-standard-v1";
pub const ADAPTER_RESOURCE_PROFILE_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WsjtxAdapterLimits {
    pub offline_source_bytes: u64,
    pub offline_nonblank_lines: u64,
    pub offline_line_bytes: u64,
    pub udp_datagram_bytes: u64,
    pub udp_queue_datagrams: u64,
    pub udp_queue_bytes: u64,
    pub udp_rate_per_second: u64,
    pub udp_rate_burst: u64,
    pub udp_clients: u64,
    pub udp_client_id_bytes: u64,
    pub udp_dedup_entries_per_client: u64,
    pub udp_dedup_window_seconds: i64,
    pub udp_idle_eviction_seconds: i64,
}

pub const WSJTX_ADAPTER_LIMITS: WsjtxAdapterLimits = WsjtxAdapterLimits {
    offline_source_bytes: 128 * 1024 * 1024,
    offline_nonblank_lines: 250_000,
    offline_line_bytes: 64 * 1024,
    udp_datagram_bytes: 65_535,
    udp_queue_datagrams: 256,
    udp_queue_bytes: 8 * 1024 * 1024,
    udp_rate_per_second: 64,
    udp_rate_burst: 512,
    udp_clients: 32,
    udp_client_id_bytes: 128,
    udp_dedup_entries_per_client: 4_096,
    udp_dedup_window_seconds: 10 * 60,
    udp_idle_eviction_seconds: 5 * 60,
};

impl WsjtxAdapterLimits {
    #[doc(hidden)]
    pub fn testing(limit: u64) -> Self {
        Self {
            offline_source_bytes: limit * 8,
            offline_nonblank_lines: limit,
            offline_line_bytes: limit,
            udp_datagram_bytes: limit,
            udp_queue_datagrams: limit,
            udp_queue_bytes: limit * 8,
            udp_rate_per_second: limit,
            udp_rate_burst: limit,
            udp_clients: limit,
            udp_client_id_bytes: limit,
            udp_dedup_entries_per_client: limit,
            udp_dedup_window_seconds: 10,
            udp_idle_eviction_seconds: 5,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AdapterCancellationToken(Arc<AtomicBool>);

impl AdapterCancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterResourceStage {
    Admission,
    Queue,
    Stream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterResourceUnit {
    Bytes,
    Lines,
    Datagrams,
    Clients,
    Entries,
    Checkpoints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterResourceDiagnostic {
    pub code: &'static str,
    pub profile: &'static str,
    pub profile_version: u16,
    pub adapter: &'static str,
    pub source: String,
    pub stage: AdapterResourceStage,
    pub limit: u64,
    pub observed: Option<u64>,
    pub unit: AdapterResourceUnit,
    pub retryable_without_input_change: bool,
    pub complete_result: bool,
    pub acquisition_gap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("adapter resource limit {diagnostic:?}")]
pub struct AdapterResourceError {
    pub diagnostic: AdapterResourceDiagnostic,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn diagnostic(
    code: &'static str,
    adapter: &'static str,
    source: impl Into<String>,
    stage: AdapterResourceStage,
    limit: u64,
    observed: Option<u64>,
    unit: AdapterResourceUnit,
    acquisition_gap: bool,
) -> AdapterResourceError {
    AdapterResourceError {
        diagnostic: AdapterResourceDiagnostic {
            code,
            profile: ADAPTER_RESOURCE_PROFILE_NAME,
            profile_version: ADAPTER_RESOURCE_PROFILE_VERSION,
            adapter,
            source: source.into(),
            stage,
            limit,
            observed,
            unit,
            retryable_without_input_change: false,
            complete_result: false,
            acquisition_gap,
        },
    }
}
