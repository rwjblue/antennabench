use antennabench_core::{PropagationRecord, RecordMeta, RecordSource};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use thiserror::Error;

use crate::{
    DiscardedItem, HttpMetadata, InvalidItemReason, ParsedSwpcRecord, SourceFreshness, SwpcProduct,
    FUTURE_CLOCK_SKEW_ALLOWANCE,
};

const MIN_F107_SFU: f64 = 0.0;
const MAX_F107_SFU: f64 = 1_000.0;
const MIN_KP: f64 = 0.0;
const MAX_KP: f64 = 9.0;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{product:?} response is not valid JSON")]
    InvalidJson {
        product: SwpcProduct,
        #[source]
        source: serde_json::Error,
    },
    #[error("{product:?} response must be a JSON array")]
    ExpectedArray { product: SwpcProduct },
    #[error("{product:?} response contains no valid observations")]
    NoValidObservation {
        product: SwpcProduct,
        discarded_items: Vec<DiscardedItem>,
    },
}

pub fn parse_f107_response(
    session_id: &str,
    captured_at: DateTime<Utc>,
    body: &[u8],
    http: HttpMetadata,
) -> Result<ParsedSwpcRecord, ParseError> {
    parse_response(
        SwpcProduct::SolarFluxF107,
        session_id,
        captured_at,
        body,
        http,
    )
}

pub fn parse_estimated_kp_response(
    session_id: &str,
    captured_at: DateTime<Utc>,
    body: &[u8],
    http: HttpMetadata,
) -> Result<ParsedSwpcRecord, ParseError> {
    parse_response(
        SwpcProduct::EstimatedPlanetaryKp,
        session_id,
        captured_at,
        body,
        http,
    )
}

pub fn parse_response(
    product: SwpcProduct,
    session_id: &str,
    captured_at: DateTime<Utc>,
    body: &[u8],
    http: HttpMetadata,
) -> Result<ParsedSwpcRecord, ParseError> {
    let payload: Value = serde_json::from_slice(body)
        .map_err(|source| ParseError::InvalidJson { product, source })?;
    let items = payload
        .as_array()
        .ok_or(ParseError::ExpectedArray { product })?;
    let mut candidates = Vec::new();
    let mut discarded_items = Vec::new();
    for (index, item) in items.iter().enumerate() {
        match parse_candidate(product, captured_at, item) {
            Ok(candidate) => candidates.push(candidate),
            Err(reason) => discarded_items.push(DiscardedItem { index, reason }),
        }
    }
    let selected = candidates
        .into_iter()
        .max_by(|left, right| {
            left.observed_at
                .cmp(&right.observed_at)
                .then_with(|| left.selected.to_string().cmp(&right.selected.to_string()))
        })
        .ok_or_else(|| ParseError::NoValidObservation {
            product,
            discarded_items: discarded_items.clone(),
        })?;

    let freshness = SourceFreshness::classify(product, selected.observed_at, captured_at);
    let raw = source_envelope(product, captured_at, &selected.selected, &http);
    let value_bits = (selected.value as f32).to_bits();
    let product_name = match product {
        SwpcProduct::SolarFluxF107 => "f107",
        SwpcProduct::EstimatedPlanetaryKp => "estimated-kp",
    };
    let record = PropagationRecord {
        meta: RecordMeta {
            schema_version: 1,
            session_id: session_id.to_string(),
            timestamp: captured_at,
            source: RecordSource::NoaaSwpc,
        },
        record_id: format!(
            "noaa-swpc-{product_name}-{}-{value_bits:08x}",
            selected.observed_at.format("%Y%m%dT%H%M%SZ")
        ),
        observed_at: selected.observed_at,
        solar_flux_f107: (product == SwpcProduct::SolarFluxF107).then_some(selected.value as f32),
        sunspot_number: None,
        kp_index: (product == SwpcProduct::EstimatedPlanetaryKp).then_some(selected.value as f32),
        a_index: None,
        solar_wind_speed_kms: None,
        bz_nt: None,
        alerts: Vec::new(),
        daylight_state: None,
        raw,
    };

    Ok(ParsedSwpcRecord {
        product,
        record,
        freshness,
        discarded_items,
        http,
    })
}

struct Candidate {
    observed_at: DateTime<Utc>,
    value: f64,
    selected: Value,
}

fn parse_candidate(
    product: SwpcProduct,
    captured_at: DateTime<Utc>,
    item: &Value,
) -> Result<Candidate, InvalidItemReason> {
    let object = item.as_object().ok_or(InvalidItemReason::NotAnObject)?;
    let time_tag = object
        .get("time_tag")
        .and_then(Value::as_str)
        .ok_or(InvalidItemReason::MissingTimeTag)?;
    let observed_at = parse_time_tag(time_tag).ok_or(InvalidItemReason::InvalidTimeTag)?;
    if observed_at - captured_at > FUTURE_CLOCK_SKEW_ALLOWANCE {
        return Err(InvalidItemReason::FutureDatedObservation);
    }
    let field = match product {
        SwpcProduct::SolarFluxF107 => "flux",
        SwpcProduct::EstimatedPlanetaryKp => "estimated_kp",
    };
    let raw_value = object.get(field).ok_or(InvalidItemReason::MissingValue)?;
    let value = raw_value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or(InvalidItemReason::InvalidValue)?;
    let valid_range = match product {
        SwpcProduct::SolarFluxF107 => (MIN_F107_SFU..=MAX_F107_SFU).contains(&value),
        SwpcProduct::EstimatedPlanetaryKp => (MIN_KP..=MAX_KP).contains(&value),
    };
    if !valid_range {
        return Err(InvalidItemReason::OutOfRangeValue);
    }
    Ok(Candidate {
        observed_at,
        value,
        selected: item.clone(),
    })
}

fn parse_time_tag(value: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
        .ok()
        .map(|value| value.and_utc())
}

fn source_envelope(
    product: SwpcProduct,
    captured_at: DateTime<Utc>,
    selected: &Value,
    http: &HttpMetadata,
) -> Value {
    let mut http_json = Map::new();
    http_json.insert("status".to_string(), json!(http.status));
    insert_optional(&mut http_json, "etag", http.etag.as_deref());
    insert_optional(
        &mut http_json,
        "last_modified",
        http.last_modified.as_deref(),
    );
    insert_optional(&mut http_json, "date", http.date.as_deref());
    insert_optional(&mut http_json, "content_type", http.content_type.as_deref());
    let semantics = match product {
        SwpcProduct::SolarFluxF107 => "observed_f10_7_solar_flux_sfu",
        SwpcProduct::EstimatedPlanetaryKp => "provisional_estimated_planetary_kp",
    };
    let attribution = match product {
        SwpcProduct::SolarFluxF107 => Some(
            "Measurements provided by the National Research Council Canada in partnership with Natural Resources Canada",
        ),
        SwpcProduct::EstimatedPlanetaryKp => None,
    };
    let mut envelope = json!({
        "provider": "NOAA/NWS Space Weather Prediction Center",
        "endpoint": product.endpoint(),
        "product": product,
        "value_semantics": semantics,
        "retrieved_at": captured_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "selected": selected,
        "http": http_json,
    });
    if let Some(attribution) = attribution {
        envelope
            .as_object_mut()
            .expect("source envelope is an object")
            .insert("source_attribution".to_string(), json!(attribution));
    }
    envelope
}

fn insert_optional(map: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        map.insert(key.to_string(), json!(value));
    }
}
