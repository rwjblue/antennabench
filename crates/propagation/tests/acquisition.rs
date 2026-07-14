use std::{cell::RefCell, collections::BTreeMap, collections::VecDeque};

use antennabench_propagation::{
    validate_redirect_chain, AcquisitionError, ConditionalRequest, HttpCancellationToken,
    HttpRequest, HttpResponse, HttpTransport, NoaaSwpcAdapter, OneShotAcquisition,
    ProductAcquisition, SwpcProduct, TransportError, F107_ENDPOINT, HTTP_CONNECT_TIMEOUT_SECONDS,
    HTTP_MAX_BODY_BYTES, HTTP_MAX_HEADERS, HTTP_MAX_HEADER_BYTES, HTTP_MAX_HEADER_FIELD_BYTES,
    HTTP_MAX_REDIRECTS, HTTP_TOTAL_TIMEOUT_SECONDS,
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

    let offline = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Err(
        TransportError::new("offline"),
    )]));
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

#[test]
fn response_resource_failures_are_typed_before_any_partial_body_is_parsed() {
    let mut too_many_headers = BTreeMap::new();
    for index in 0..=HTTP_MAX_HEADERS {
        too_many_headers.insert(format!("x-{index}"), "value".to_string());
    }
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(HttpResponse {
        received_at: captured_at(),
        status: 200,
        headers: too_many_headers,
        body: F107.to_vec(),
    })]));
    let error = adapter
        .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
        .unwrap_err();
    let AcquisitionError::Resource { failure, .. } = error else {
        panic!("header overflow must be a typed resource failure");
    };
    assert_eq!(failure.diagnostic.code, "resource.adapter.http.headers");
    assert!(!failure.diagnostic.complete_result);

    let mut declared = response(200, F107).unwrap();
    declared.headers.insert(
        "Content-Length".to_string(),
        (HTTP_MAX_BODY_BYTES + 1).to_string(),
    );
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(declared)]));
    let error = adapter
        .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
        .unwrap_err();
    assert!(matches!(
        error,
        AcquisitionError::Resource { ref failure, .. }
            if failure.diagnostic.code == "resource.adapter.http.body_bytes"
    ));
}

#[test]
fn media_encoding_body_and_redirect_policies_are_independently_enforced() {
    for (header, value, code) in [
        (
            "Content-Type",
            "text/html",
            "resource.adapter.http.media_type",
        ),
        (
            "Content-Encoding",
            "gzip",
            "resource.adapter.http.content_encoding",
        ),
    ] {
        let mut bad = response(200, F107).unwrap();
        bad.headers.insert(header.to_string(), value.to_string());
        let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(bad)]));
        let error = adapter
            .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
            .unwrap_err();
        assert!(matches!(
            error,
            AcquisitionError::Resource { ref failure, .. }
                if failure.diagnostic.code == code
        ));
    }

    let mut oversized = response(200, &[]).unwrap();
    oversized.body = vec![b' '; HTTP_MAX_BODY_BYTES as usize + 1];
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(oversized)]));
    let error = adapter
        .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
        .unwrap_err();
    let AcquisitionError::Resource { failure, .. } = error else {
        panic!("streamed overflow must not reach JSON parsing");
    };
    assert_eq!(failure.diagnostic.code, "resource.adapter.http.body_bytes");
    assert_eq!(failure.quarantine.unwrap().byte_count, HTTP_MAX_BODY_BYTES);

    validate_redirect_chain(
        F107_ENDPOINT,
        &["https://services.swpc.noaa.gov/redirected.json".to_string()],
    )
    .unwrap();
    assert_eq!(
        validate_redirect_chain(
            F107_ENDPOINT,
            &["https://example.com/redirected.json".to_string()]
        )
        .unwrap_err()
        .diagnostic
        .code,
        "resource.adapter.http.redirect_target"
    );
    assert_eq!(
        validate_redirect_chain(
            F107_ENDPOINT,
            &vec![F107_ENDPOINT.to_string(); HTTP_MAX_REDIRECTS as usize + 1],
        )
        .unwrap_err()
        .diagnostic
        .code,
        "resource.adapter.http.redirects"
    );
    assert_eq!(
        validate_redirect_chain(
            F107_ENDPOINT,
            &["http://services.swpc.noaa.gov/x".to_string()]
        )
        .unwrap_err()
        .diagnostic
        .code,
        "resource.adapter.http.redirect_target"
    );
}

#[test]
fn header_field_aggregate_timeouts_and_cancellation_have_fixed_boundaries() {
    assert_eq!(HTTP_CONNECT_TIMEOUT_SECONDS, 5);
    assert_eq!(HTTP_TOTAL_TIMEOUT_SECONDS, 20);

    let mut oversized_field = response(200, F107).unwrap();
    oversized_field.headers.insert(
        "x-large".to_string(),
        "x".repeat(HTTP_MAX_HEADER_FIELD_BYTES as usize),
    );
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(oversized_field)]));
    let error = adapter
        .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
        .unwrap_err();
    assert!(matches!(
        error,
        AcquisitionError::Resource { ref failure, .. }
            if failure.diagnostic.code == "resource.adapter.http.header_field_bytes"
    ));

    let mut aggregate = BTreeMap::new();
    for index in 0..HTTP_MAX_HEADERS {
        aggregate.insert(
            format!("x-{index}"),
            "x".repeat((HTTP_MAX_HEADER_BYTES / HTTP_MAX_HEADERS + 1) as usize),
        );
    }
    let adapter = NoaaSwpcAdapter::new(FakeTransport::with_responses(vec![Ok(HttpResponse {
        received_at: captured_at(),
        status: 200,
        headers: aggregate,
        body: F107.to_vec(),
    })]));
    let error = adapter
        .acquire_product(SwpcProduct::SolarFluxF107, "session-test", None)
        .unwrap_err();
    assert!(matches!(
        error,
        AcquisitionError::Resource { ref failure, .. }
            if failure.diagnostic.code == "resource.adapter.http.headers"
                && failure.diagnostic.limit == HTTP_MAX_HEADER_BYTES
    ));

    let transport = FakeTransport::with_responses(vec![response(200, F107)]);
    let adapter = NoaaSwpcAdapter::new(transport);
    let cancellation = HttpCancellationToken::default();
    cancellation.cancel();
    let error = adapter
        .acquire_product_with_cancellation(
            SwpcProduct::SolarFluxF107,
            "session-test",
            None,
            &cancellation,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        AcquisitionError::Resource { ref failure, .. }
            if failure.diagnostic.code == "resource.operation.cancelled"
    ));
    assert!(adapter.transport().requests.borrow().is_empty());
}
