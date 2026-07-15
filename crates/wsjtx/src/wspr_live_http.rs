use std::{io::Read, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::{blocking::Client, redirect::Policy};
use thiserror::Error;

use crate::{AdapterCancellationToken, WsprLiveAcquisitionPlan, WSPR_LIVE_IMPORT_LIMITS};

pub const WSPR_LIVE_CONNECT_TIMEOUT_SECONDS: u64 = 5;
pub const WSPR_LIVE_TOTAL_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsprLiveHttpResponse {
    pub received_at: DateTime<Utc>,
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WsprLiveAcquisitionError {
    #[error("could not create the bounded WSPR.live HTTP client: {0}")]
    Client(String),
    #[error("WSPR.live request failed: {0}")]
    Transport(String),
    #[error("WSPR.live returned HTTP status {status}")]
    HttpStatus { status: u16 },
    #[error("WSPR.live response exceeded {limit} bytes after {observed} bytes")]
    BodyBytes { limit: u64, observed: u64 },
    #[error("WSPR.live acquisition was cancelled")]
    Cancelled,
}

pub trait WsprLiveHttpTransport {
    fn get(
        &self,
        url: &str,
        body_limit: u64,
        cancellation: &AdapterCancellationToken,
    ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestWsprLiveTransport {
    client: Client,
}

impl ReqwestWsprLiveTransport {
    pub fn new() -> Result<Self, WsprLiveAcquisitionError> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(WSPR_LIVE_CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(WSPR_LIVE_TOTAL_TIMEOUT_SECONDS))
            .redirect(Policy::none())
            .build()
            .map_err(|error| WsprLiveAcquisitionError::Client(error.to_string()))?;
        Ok(Self { client })
    }
}

impl WsprLiveHttpTransport for ReqwestWsprLiveTransport {
    fn get(
        &self,
        url: &str,
        body_limit: u64,
        cancellation: &AdapterCancellationToken,
    ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
        if cancellation.is_cancelled() {
            return Err(WsprLiveAcquisitionError::Cancelled);
        }
        let mut response = self
            .client
            .get(url)
            .send()
            .map_err(|error| WsprLiveAcquisitionError::Transport(error.to_string()))?;
        let received_at = Utc::now();
        let status = response.status().as_u16();
        if let Some(observed) = response.content_length() {
            if observed > body_limit {
                return Err(WsprLiveAcquisitionError::BodyBytes {
                    limit: body_limit,
                    observed,
                });
            }
        }

        let mut body = Vec::new();
        let mut chunk = [0_u8; 8 * 1024];
        loop {
            if cancellation.is_cancelled() {
                return Err(WsprLiveAcquisitionError::Cancelled);
            }
            let count = response
                .read(&mut chunk)
                .map_err(|error| WsprLiveAcquisitionError::Transport(error.to_string()))?;
            if count == 0 {
                break;
            }
            let observed = body.len() as u64 + count as u64;
            if observed > body_limit {
                return Err(WsprLiveAcquisitionError::BodyBytes {
                    limit: body_limit,
                    observed,
                });
            }
            body.extend_from_slice(&chunk[..count]);
        }

        Ok(WsprLiveHttpResponse {
            received_at,
            status,
            body,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WsprLiveAcquirer<T> {
    transport: T,
}

impl<T> WsprLiveAcquirer<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: WsprLiveHttpTransport> WsprLiveAcquirer<T> {
    pub fn acquire(
        &self,
        plan: &WsprLiveAcquisitionPlan,
        cancellation: &AdapterCancellationToken,
    ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
        if cancellation.is_cancelled() {
            return Err(WsprLiveAcquisitionError::Cancelled);
        }
        let response = self.transport.get(
            &plan.query.query_url(),
            WSPR_LIVE_IMPORT_LIMITS.source_bytes,
            cancellation,
        )?;
        if cancellation.is_cancelled() {
            return Err(WsprLiveAcquisitionError::Cancelled);
        }
        if response.status != 200 {
            return Err(WsprLiveAcquisitionError::HttpStatus {
                status: response.status,
            });
        }
        if response.body.len() as u64 > WSPR_LIVE_IMPORT_LIMITS.source_bytes {
            return Err(WsprLiveAcquisitionError::BodyBytes {
                limit: WSPR_LIVE_IMPORT_LIMITS.source_bytes,
                observed: response.body.len() as u64,
            });
        }
        Ok(response)
    }
}
