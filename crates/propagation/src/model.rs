use std::collections::BTreeMap;

use antennabench_core::PropagationRecord;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

pub const F107_ENDPOINT: &str = "https://services.swpc.noaa.gov/products/summary/10cm-flux.json";
pub const ESTIMATED_KP_ENDPOINT: &str =
    "https://services.swpc.noaa.gov/json/planetary_k_index_1m.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwpcProduct {
    SolarFluxF107,
    EstimatedPlanetaryKp,
}

impl SwpcProduct {
    pub const ALL: [Self; 2] = [Self::SolarFluxF107, Self::EstimatedPlanetaryKp];

    pub fn endpoint(self) -> &'static str {
        match self {
            Self::SolarFluxF107 => F107_ENDPOINT,
            Self::EstimatedPlanetaryKp => ESTIMATED_KP_ENDPOINT,
        }
    }

    pub fn policy(self) -> ProductPolicy {
        match self {
            Self::SolarFluxF107 => ProductPolicy {
                minimum_poll_interval: Duration::hours(6),
                stale_after: Duration::hours(36),
            },
            Self::EstimatedPlanetaryKp => ProductPolicy {
                minimum_poll_interval: Duration::minutes(5),
                stale_after: Duration::minutes(10),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductPolicy {
    pub minimum_poll_interval: Duration,
    pub stale_after: Duration,
}

pub const RETRY_BACKOFF: Duration = Duration::minutes(1);
pub const FUTURE_CLOCK_SKEW_ALLOWANCE: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAcquisitionPhase {
    Start,
    ActivePoll,
    End,
}

pub fn should_acquire(
    product: SwpcProduct,
    phase: SessionAcquisitionPhase,
    last_attempt_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    match phase {
        SessionAcquisitionPhase::Start | SessionAcquisitionPhase::End => true,
        SessionAcquisitionPhase::ActivePoll => {
            last_attempt_at.is_none_or(|last| now - last >= product.policy().minimum_poll_interval)
        }
    }
}

pub fn retry_allowed(last_failure_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    last_failure_at.is_none_or(|last| now - last >= RETRY_BACKOFF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum SourceFreshness {
    Current { age_seconds: i64 },
    Stale { age_seconds: i64 },
}

impl SourceFreshness {
    pub fn classify(
        product: SwpcProduct,
        observed_at: DateTime<Utc>,
        captured_at: DateTime<Utc>,
    ) -> Self {
        let age_seconds = (captured_at - observed_at).num_seconds().max(0);
        if age_seconds > product.policy().stale_after.num_seconds() {
            Self::Stale { age_seconds }
        } else {
            Self::Current { age_seconds }
        }
    }

    pub fn is_stale(self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpMetadata {
    pub status: u16,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub date: Option<String>,
    pub content_type: Option<String>,
}

impl HttpMetadata {
    pub fn conditional_request(&self) -> ConditionalRequest {
        ConditionalRequest {
            etag: self.etag.clone(),
            last_modified: self.last_modified.clone(),
        }
    }

    pub(crate) fn from_headers(status: u16, headers: &BTreeMap<String, String>) -> Self {
        Self {
            status,
            etag: header(headers, "etag"),
            last_modified: header(headers, "last-modified"),
            date: header(headers, "date"),
            content_type: header(headers, "content-type"),
        }
    }
}

fn header(headers: &BTreeMap<String, String>, name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConditionalRequest {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSwpcRecord {
    pub product: SwpcProduct,
    pub record: PropagationRecord,
    pub freshness: SourceFreshness,
    pub discarded_items: Vec<DiscardedItem>,
    pub http: HttpMetadata,
}

impl ParsedSwpcRecord {
    pub fn discarded_item_count(&self) -> usize {
        self.discarded_items.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidItemReason {
    NotAnObject,
    MissingTimeTag,
    InvalidTimeTag,
    FutureDatedObservation,
    MissingValue,
    InvalidValue,
    OutOfRangeValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscardedItem {
    pub index: usize,
    pub reason: InvalidItemReason,
}

impl ParsedSwpcRecord {
    pub fn append_outcome(self, existing: &[PropagationRecord]) -> AppendOutcome {
        let unchanged = existing.iter().any(|record| {
            record.meta.source == self.record.meta.source
                && record.observed_at == self.record.observed_at
                && match self.product {
                    SwpcProduct::SolarFluxF107 => {
                        optional_f32_bits(record.solar_flux_f107)
                            == optional_f32_bits(self.record.solar_flux_f107)
                    }
                    SwpcProduct::EstimatedPlanetaryKp => {
                        optional_f32_bits(record.kp_index)
                            == optional_f32_bits(self.record.kp_index)
                    }
                }
        });
        if unchanged {
            AppendOutcome::Unchanged {
                product: self.product,
                observed_at: self.record.observed_at,
                freshness: self.freshness,
            }
        } else {
            AppendOutcome::Append(Box::new(self))
        }
    }
}

fn optional_f32_bits(value: Option<f32>) -> Option<u32> {
    value.map(f32::to_bits)
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppendOutcome {
    Append(Box<ParsedSwpcRecord>),
    Unchanged {
        product: SwpcProduct,
        observed_at: DateTime<Utc>,
        freshness: SourceFreshness,
    },
}
