use std::{collections::BTreeMap, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use thiserror::Error;

use crate::{
    parse_response, ConditionalRequest, HttpMetadata, ParseError, ParsedSwpcRecord, SwpcProduct,
};

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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{message}")]
pub struct TransportError {
    pub message: String,
}

pub trait HttpTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError>;
}

#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self, TransportError> {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("AntennaBench/", env!("CARGO_PKG_VERSION")))
            .build()
            .map(|client| Self { client })
            .map_err(|error| TransportError {
                message: error.to_string(),
            })
    }
}

impl HttpTransport for ReqwestTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = self.client.get(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.send().map_err(|error| TransportError {
            message: error.to_string(),
        })?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_string(), value.to_string()))
            })
            .collect();
        let body = response
            .bytes()
            .map_err(|error| TransportError {
                message: error.to_string(),
            })?
            .to_vec();
        Ok(HttpResponse {
            received_at: Utc::now(),
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
        let response = self
            .transport
            .get(&request)
            .map_err(|source| AcquisitionError::Transport { product, source })?;
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
