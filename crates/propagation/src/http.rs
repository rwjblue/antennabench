use std::{
    collections::BTreeMap,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use reqwest::{blocking::Client, redirect::Policy};
use thiserror::Error;

use crate::{
    parse_response, ConditionalRequest, HttpMetadata, ParseError, ParsedSwpcRecord, SwpcProduct,
};

pub const HTTP_CONNECT_TIMEOUT_SECONDS: u64 = 5;
pub const HTTP_TOTAL_TIMEOUT_SECONDS: u64 = 20;
pub const HTTP_MAX_REDIRECTS: u64 = 3;
pub const HTTP_MAX_HEADERS: u64 = 64;
pub const HTTP_MAX_HEADER_FIELD_BYTES: u64 = 8 * 1024;
pub const HTTP_MAX_HEADER_BYTES: u64 = 32 * 1024;
pub const HTTP_MAX_BODY_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub received_at: DateTime<Utc>,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResourceStage {
    Redirect,
    Headers,
    Body,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResourceUnit {
    Bytes,
    Headers,
    Redirects,
    Checkpoints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResourceDiagnostic {
    pub code: &'static str,
    pub profile: &'static str,
    pub profile_version: u16,
    pub adapter: &'static str,
    pub source: String,
    pub stage: HttpResourceStage,
    pub limit: u64,
    pub observed: Option<u64>,
    pub unit: HttpResourceUnit,
    pub retryable_without_input_change: bool,
    pub complete_result: bool,
    pub acquisition_gap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpQuarantine {
    pub endpoint: String,
    pub received_at: DateTime<Utc>,
    pub byte_count: u64,
    pub failure_code: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("HTTP adapter resource limit {diagnostic:?}")]
pub struct HttpResourceFailure {
    pub diagnostic: Box<HttpResourceDiagnostic>,
    pub quarantine: Option<Box<HttpQuarantine>>,
}

fn resource_failure(
    code: &'static str,
    source: impl Into<String>,
    stage: HttpResourceStage,
    limit: u64,
    observed: Option<u64>,
    unit: HttpResourceUnit,
    quarantine: Option<HttpQuarantine>,
) -> HttpResourceFailure {
    HttpResourceFailure {
        diagnostic: Box::new(HttpResourceDiagnostic {
            code,
            profile: "local-standard-v1",
            profile_version: 1,
            adapter: "noaa_swpc.http",
            source: source.into(),
            stage,
            limit,
            observed,
            unit,
            retryable_without_input_change: false,
            complete_result: false,
            acquisition_gap: true,
        }),
        quarantine: quarantine.map(Box::new),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TransportErrorKind {
    Other,
    Resource(HttpResourceFailure),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct TransportError {
    pub message: String,
    kind: TransportErrorKind,
}

#[derive(Debug, Clone, Default)]
pub struct HttpCancellationToken(Arc<AtomicBool>);

impl HttpCancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl TransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: TransportErrorKind::Other,
        }
    }

    fn resource(failure: HttpResourceFailure) -> Self {
        Self {
            message: failure.to_string(),
            kind: TransportErrorKind::Resource(failure),
        }
    }

    fn into_resource(self) -> Option<HttpResourceFailure> {
        match self.kind {
            TransportErrorKind::Resource(failure) => Some(failure),
            TransportErrorKind::Other => None,
        }
    }
}

pub trait HttpTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;

    fn get_with_cancellation(
        &self,
        request: &HttpRequest,
        cancellation: &HttpCancellationToken,
    ) -> Result<HttpResponse, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::resource(cancelled(&request.url, None)));
        }
        let response = self.get(request)?;
        if cancellation.is_cancelled() {
            return Err(TransportError::resource(cancelled(
                &request.url,
                Some(response.body.len() as u64),
            )));
        }
        Ok(response)
    }
}

#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, TransportError> {
        let redirects = Policy::custom(|attempt| {
            let original_host = attempt.previous().first().and_then(|url| url.host_str());
            let next = attempt.url();
            if attempt.previous().len() > HTTP_MAX_REDIRECTS as usize {
                return attempt.error("NOAA redirect limit exceeded");
            }
            if next.scheme() != "https" || next.host_str() != original_host {
                return attempt.error("NOAA redirect must remain HTTPS on the original host");
            }
            attempt.follow()
        });
        Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(HTTP_TOTAL_TIMEOUT_SECONDS))
            .redirect(redirects)
            .user_agent(concat!("AntennaBench/", env!("CARGO_PKG_VERSION")))
            .build()
            .map(|client| Self { client })
            .map_err(|error| TransportError::new(error.to_string()))
    }
}

impl HttpTransport for ReqwestTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.get_bounded(request, &HttpCancellationToken::default())
    }

    fn get_with_cancellation(
        &self,
        request: &HttpRequest,
        cancellation: &HttpCancellationToken,
    ) -> Result<HttpResponse, TransportError> {
        self.get_bounded(request, cancellation)
    }
}

impl ReqwestTransport {
    fn get_bounded(
        &self,
        request: &HttpRequest,
        cancellation: &HttpCancellationToken,
    ) -> Result<HttpResponse, TransportError> {
        if cancellation.is_cancelled() {
            return Err(TransportError::resource(cancelled(&request.url, None)));
        }
        ensure_https_endpoint(&request.url).map_err(TransportError::resource)?;
        let mut builder = self.client.get(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let mut response = builder
            .send()
            .map_err(|error| TransportError::new(error.to_string()))?;
        let received_at = Utc::now();
        let status = response.status().as_u16();
        let headers = normalize_response_headers(&request.url, response.headers())?;
        validate_headers(&request.url, &headers).map_err(TransportError::resource)?;
        validate_content_length(&request.url, &headers).map_err(TransportError::resource)?;
        if (200..300).contains(&status) {
            validate_media(&request.url, &headers).map_err(TransportError::resource)?;
        }

        let mut body = Vec::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            if cancellation.is_cancelled() {
                return Err(TransportError::resource(cancelled(
                    &request.url,
                    Some(body.len() as u64),
                )));
            }
            let read = response
                .read(&mut chunk)
                .map_err(|error| TransportError::new(error.to_string()))?;
            if read == 0 {
                break;
            }
            let observed = body.len() as u64 + read as u64;
            if observed > HTTP_MAX_BODY_BYTES {
                return Err(TransportError::resource(resource_failure(
                    "resource.adapter.http.body_bytes",
                    &request.url,
                    HttpResourceStage::Body,
                    HTTP_MAX_BODY_BYTES,
                    Some(observed),
                    HttpResourceUnit::Bytes,
                    Some(HttpQuarantine {
                        endpoint: request.url.clone(),
                        received_at,
                        byte_count: body.len() as u64,
                        failure_code: "resource.adapter.http.body_bytes",
                    }),
                )));
            }
            body.extend_from_slice(&chunk[..read]);
        }
        Ok(HttpResponse {
            received_at,
            status,
            headers,
            body,
        })
    }
}

#[derive(Debug, Error)]
pub enum AcquisitionError {
    #[error("{product:?} transport failed")]
    Transport {
        product: SwpcProduct,
        #[source]
        source: TransportError,
    },
    #[error("{product:?} resource policy rejected the response")]
    Resource {
        product: SwpcProduct,
        #[source]
        failure: HttpResourceFailure,
    },
    #[error("{product:?} endpoint returned HTTP {status}")]
    HttpStatus { product: SwpcProduct, status: u16 },
    #[error(transparent)]
    Parse(#[from] ParseError),
}

#[derive(Debug)]
pub enum OneShotAcquisition {
    Selected(Box<ParsedSwpcRecord>),
    NotModified {
        product: SwpcProduct,
        captured_at: DateTime<Utc>,
        http: HttpMetadata,
    },
}

#[derive(Debug)]
pub enum ProductAcquisition {
    Completed(OneShotAcquisition),
    Failed {
        product: SwpcProduct,
        error: AcquisitionError,
    },
}

#[derive(Debug, Clone)]
pub struct NoaaSwpcAdapter<T> {
    transport: T,
}

impl NoaaSwpcAdapter<ReqwestTransport> {
    pub fn live() -> Result<Self, TransportError> {
        ReqwestTransport::new().map(Self::new)
    }
}

impl<T> NoaaSwpcAdapter<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: HttpTransport> NoaaSwpcAdapter<T> {
    pub fn acquire_product(
        &self,
        product: SwpcProduct,
        session_id: &str,
        conditional: Option<&ConditionalRequest>,
    ) -> Result<OneShotAcquisition, AcquisitionError> {
        self.acquire_product_with_cancellation(
            product,
            session_id,
            conditional,
            &HttpCancellationToken::default(),
        )
    }

    pub fn acquire_product_with_cancellation(
        &self,
        product: SwpcProduct,
        session_id: &str,
        conditional: Option<&ConditionalRequest>,
        cancellation: &HttpCancellationToken,
    ) -> Result<OneShotAcquisition, AcquisitionError> {
        let mut headers = BTreeMap::from([("accept".to_string(), "application/json".to_string())]);
        if let Some(conditional) = conditional {
            if let Some(etag) = &conditional.etag {
                headers.insert("if-none-match".to_string(), etag.clone());
            }
            if let Some(last_modified) = &conditional.last_modified {
                headers.insert("if-modified-since".to_string(), last_modified.clone());
            }
        }
        let request = HttpRequest {
            url: product.endpoint().to_string(),
            headers,
        };
        let response = match self.transport.get_with_cancellation(&request, cancellation) {
            Ok(response) => response,
            Err(source) => {
                if let Some(failure) = source.clone().into_resource() {
                    return Err(AcquisitionError::Resource { product, failure });
                }
                return Err(AcquisitionError::Transport { product, source });
            }
        };
        validate_response(&request.url, &response)
            .map_err(|failure| AcquisitionError::Resource { product, failure })?;
        let http = HttpMetadata::from_headers(response.status, &response.headers);
        if response.status == 304 {
            return Ok(OneShotAcquisition::NotModified {
                product,
                captured_at: response.received_at,
                http,
            });
        }
        if !(200..300).contains(&response.status) {
            return Err(AcquisitionError::HttpStatus {
                product,
                status: response.status,
            });
        }
        parse_response(
            product,
            session_id,
            response.received_at,
            &response.body,
            http,
        )
        .map(Box::new)
        .map(OneShotAcquisition::Selected)
        .map_err(AcquisitionError::from)
    }

    pub fn acquire_snapshot(
        &self,
        session_id: &str,
        conditionals: &BTreeMap<SwpcProduct, ConditionalRequest>,
    ) -> Vec<ProductAcquisition> {
        SwpcProduct::ALL
            .into_iter()
            .map(|product| {
                match self.acquire_product(product, session_id, conditionals.get(&product)) {
                    Ok(outcome) => ProductAcquisition::Completed(outcome),
                    Err(error) => ProductAcquisition::Failed { product, error },
                }
            })
            .collect()
    }
}

pub fn validate_redirect_chain(
    original: &str,
    redirects: &[String],
) -> Result<(), HttpResourceFailure> {
    let original = reqwest::Url::parse(original).map_err(|_| {
        resource_failure(
            "resource.adapter.http.endpoint",
            original,
            HttpResourceStage::Redirect,
            0,
            None,
            HttpResourceUnit::Redirects,
            None,
        )
    })?;
    if redirects.len() as u64 > HTTP_MAX_REDIRECTS {
        return Err(resource_failure(
            "resource.adapter.http.redirects",
            original.as_str(),
            HttpResourceStage::Redirect,
            HTTP_MAX_REDIRECTS,
            Some(redirects.len() as u64),
            HttpResourceUnit::Redirects,
            None,
        ));
    }
    for redirect in redirects {
        let next = reqwest::Url::parse(redirect).map_err(|_| {
            resource_failure(
                "resource.adapter.http.redirect_target",
                redirect,
                HttpResourceStage::Redirect,
                0,
                None,
                HttpResourceUnit::Redirects,
                None,
            )
        })?;
        if next.scheme() != "https" || next.host_str() != original.host_str() {
            return Err(resource_failure(
                "resource.adapter.http.redirect_target",
                redirect,
                HttpResourceStage::Redirect,
                HTTP_MAX_REDIRECTS,
                None,
                HttpResourceUnit::Redirects,
                None,
            ));
        }
    }
    Ok(())
}

fn ensure_https_endpoint(url: &str) -> Result<(), HttpResourceFailure> {
    let parsed = reqwest::Url::parse(url).ok();
    if parsed.as_ref().is_some_and(|url| url.scheme() == "https") {
        Ok(())
    } else {
        Err(resource_failure(
            "resource.adapter.http.endpoint",
            url,
            HttpResourceStage::Redirect,
            0,
            None,
            HttpResourceUnit::Redirects,
            None,
        ))
    }
}

fn cancelled(url: &str, observed: Option<u64>) -> HttpResourceFailure {
    resource_failure(
        "resource.operation.cancelled",
        url,
        HttpResourceStage::Body,
        0,
        observed,
        HttpResourceUnit::Checkpoints,
        None,
    )
}

fn normalize_response_headers(
    url: &str,
    headers: &reqwest::header::HeaderMap,
) -> Result<BTreeMap<String, String>, TransportError> {
    if headers.len() as u64 > HTTP_MAX_HEADERS {
        return Err(TransportError::resource(resource_failure(
            "resource.adapter.http.headers",
            url,
            HttpResourceStage::Headers,
            HTTP_MAX_HEADERS,
            Some(headers.len() as u64),
            HttpResourceUnit::Headers,
            None,
        )));
    }
    let mut normalized = BTreeMap::<String, String>::new();
    let mut total = 0_u64;
    for (name, value) in headers {
        let value = value.to_str().map_err(|_| {
            TransportError::resource(resource_failure(
                "resource.adapter.http.header_field_bytes",
                url,
                HttpResourceStage::Headers,
                HTTP_MAX_HEADER_FIELD_BYTES,
                None,
                HttpResourceUnit::Bytes,
                None,
            ))
        })?;
        let field = name.as_str().len() as u64 + value.len() as u64;
        if field > HTTP_MAX_HEADER_FIELD_BYTES {
            return Err(TransportError::resource(resource_failure(
                "resource.adapter.http.header_field_bytes",
                url,
                HttpResourceStage::Headers,
                HTTP_MAX_HEADER_FIELD_BYTES,
                Some(field),
                HttpResourceUnit::Bytes,
                None,
            )));
        }
        total += field;
        normalized
            .entry(name.as_str().to_string())
            .and_modify(|existing| {
                existing.push_str(", ");
                existing.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }
    if total > HTTP_MAX_HEADER_BYTES {
        return Err(TransportError::resource(resource_failure(
            "resource.adapter.http.headers",
            url,
            HttpResourceStage::Headers,
            HTTP_MAX_HEADER_BYTES,
            Some(total),
            HttpResourceUnit::Bytes,
            None,
        )));
    }
    Ok(normalized)
}

fn validate_response(url: &str, response: &HttpResponse) -> Result<(), HttpResourceFailure> {
    ensure_https_endpoint(url)?;
    validate_headers(url, &response.headers)?;
    validate_content_length(url, &response.headers)?;
    if response.body.len() as u64 > HTTP_MAX_BODY_BYTES {
        return Err(resource_failure(
            "resource.adapter.http.body_bytes",
            url,
            HttpResourceStage::Body,
            HTTP_MAX_BODY_BYTES,
            Some(response.body.len() as u64),
            HttpResourceUnit::Bytes,
            Some(HttpQuarantine {
                endpoint: url.to_string(),
                received_at: response.received_at,
                byte_count: HTTP_MAX_BODY_BYTES,
                failure_code: "resource.adapter.http.body_bytes",
            }),
        ));
    }
    if (200..300).contains(&response.status) {
        validate_media(url, &response.headers)?;
    }
    Ok(())
}

fn validate_headers(
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<(), HttpResourceFailure> {
    if headers.len() as u64 > HTTP_MAX_HEADERS {
        return Err(resource_failure(
            "resource.adapter.http.headers",
            url,
            HttpResourceStage::Headers,
            HTTP_MAX_HEADERS,
            Some(headers.len() as u64),
            HttpResourceUnit::Headers,
            None,
        ));
    }
    let mut total = 0_u64;
    for (name, value) in headers {
        let field = name.len() as u64 + value.len() as u64;
        if field > HTTP_MAX_HEADER_FIELD_BYTES {
            return Err(resource_failure(
                "resource.adapter.http.header_field_bytes",
                url,
                HttpResourceStage::Headers,
                HTTP_MAX_HEADER_FIELD_BYTES,
                Some(field),
                HttpResourceUnit::Bytes,
                None,
            ));
        }
        total += field;
    }
    if total > HTTP_MAX_HEADER_BYTES {
        return Err(resource_failure(
            "resource.adapter.http.headers",
            url,
            HttpResourceStage::Headers,
            HTTP_MAX_HEADER_BYTES,
            Some(total),
            HttpResourceUnit::Bytes,
            None,
        ));
    }
    Ok(())
}

fn validate_content_length(
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<(), HttpResourceFailure> {
    if let Some(value) = header(headers, "content-length") {
        if let Ok(length) = value.trim().parse::<u64>() {
            if length > HTTP_MAX_BODY_BYTES {
                return Err(resource_failure(
                    "resource.adapter.http.body_bytes",
                    url,
                    HttpResourceStage::Body,
                    HTTP_MAX_BODY_BYTES,
                    Some(length),
                    HttpResourceUnit::Bytes,
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn validate_media(
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<(), HttpResourceFailure> {
    if let Some(encoding) = header(headers, "content-encoding") {
        if !encoding.trim().is_empty() && !encoding.eq_ignore_ascii_case("identity") {
            return Err(resource_failure(
                "resource.adapter.http.content_encoding",
                url,
                HttpResourceStage::Media,
                0,
                None,
                HttpResourceUnit::Bytes,
                None,
            ));
        }
    }
    let media = header(headers, "content-type")
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if !(media.eq_ignore_ascii_case("application/json")
        || media.to_ascii_lowercase().ends_with("+json"))
    {
        return Err(resource_failure(
            "resource.adapter.http.media_type",
            url,
            HttpResourceStage::Media,
            0,
            None,
            HttpResourceUnit::Bytes,
            None,
        ));
    }
    Ok(())
}

fn header<'a>(headers: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
