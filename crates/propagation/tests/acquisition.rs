use std::{cell::RefCell, collections::BTreeMap, collections::VecDeque};

use antennabench_propagation::{
    AcquisitionError, ConditionalRequest, HttpRequest, HttpResponse, HttpTransport,
    NoaaSwpcAdapter, OneShotAcquisition, ProductAcquisition, SwpcProduct, TransportError,
};
use chrono::{TimeZone, Utc};

const F107: &[u8] = include_bytes!("../../../fixtures/noaa-swpc/f107.json");
const ESTIMATED_KP: &[u8] = include_bytes!("../../../fixtures/noaa-swpc/estimated-kp.json");

#[derive(Default)]
struct FakeTransport {
    requests: RefCell<Vec<HttpRequest>>,
    responses: RefCell<VecDeque<Result<HttpResponse, TransportError>>>,
}

impl FakeTransport {
    fn with_responses(responses: Vec<Result<HttpResponse, TransportError>>) -> Self {
        Self {
            requests: RefCell::new(Vec::new()),
            responses: RefCell::new(responses.into()),
        }
    }
}

impl HttpTransport for FakeTransport {
    fn get(&self, request: &HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.borrow_mut().push(request.clone());
        self.responses
            .borrow_mut()
            .pop_front()
            .expect("fake response is configured")
    }
}

fn captured_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 13, 21, 25, 0)
        .single()
        .unwrap()
}

fn response(status: u16, body: &[u8]) -> Result<HttpResponse, TransportError> {
    Ok(HttpResponse {
        received_at: captured_at(),
        status,
        headers: BTreeMap::from([
            ("ETag".to_string(), "\"test-etag\"".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ]),
        body: body.to_vec(),
    })
}

#[test]
fn one_shot_acquisition_uses_exact_endpoint_and_conditional_headers() {
    let transport = FakeTransport::with_responses(vec![response(200, F107)]);
    let adapter = NoaaSwpcAdapter::new(transport);
    let conditional = ConditionalRequest {
        etag: Some("\"previous\"".to_string()),
        last_modified: Some("Sun, 13 Jul 2026 20:00:00 GMT".to_string()),
    };

    let outcome = adapter
        .acquire_product(
            SwpcProduct::SolarFluxF107,
            "session-test",
            Some(&conditional),
        )
        .unwrap();

    let OneShotAcquisition::Selected(parsed) = outcome else {
        panic!("expected a selected observation");
    };
    assert_eq!(parsed.record.solar_flux_f107, Some(103.0));
    assert_eq!(parsed.record.meta.timestamp, captured_at());
    assert_eq!(parsed.http.etag.as_deref(), Some("\"test-etag\""));
    let requests = adapter.transport().requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, SwpcProduct::SolarFluxF107.endpoint());
    assert_eq!(requests[0].headers["accept"], "application/json");
    assert_eq!(requests[0].headers["if-none-match"], "\"previous\"");
    assert_eq!(
        requests[0].headers["if-modified-since"],
        "Sun, 13 Jul 2026 20:00:00 GMT"
    );
}

#[test]
fn not_modified_and_http_transport_failures_are_typed() {
    let not_modified =
        NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![response(304, b"")]));
    assert!(matches!(
        not_modified.acquire_product(SwpcProduct::SolarFluxF107, "session-test", None),
        Ok(OneShotAcquisition::NotModified {
            product: SwpcProduct::SolarFluxF107,
            ..
        })
    ));

    let http_error = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![response(503, b"")]));
    assert!(matches!(
        http_error.acquire_product(SwpcProduct::SolarFluxF107, "session-test", None),
        Err(AcquisitionError::HttpStatus { status: 503, .. })
    ));

    let offline = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Err(TransportError {
        message: "offline".to_string(),
    })]));
    assert!(matches!(
        offline.acquire_product(SwpcProduct::SolarFluxF107, "session-test", None),
        Err(AcquisitionError::Transport { .. })
    ));
}

#[test]
fn snapshot_attempts_both_products_when_one_fails() {
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![
        response(503, b""),
        response(200, ESTIMATED_KP),
    ]));

    let outcomes = adapter.acquire_snapshot("session-test", &BTreeMap::new());

    assert_eq!(outcomes.len(), 2);
    assert!(matches!(
        outcomes[0],
        ProductAcquisition::Failed {
            product: SwpcProduct::SolarFluxF107,
            ..
        }
    ));
    assert!(matches!(
        outcomes[1],
        ProductAcquisition::Completed(OneShotAcquisition::Selected(_))
    ));
    assert_eq!(adapter.transport().requests.borrow().len(), 2);
}
