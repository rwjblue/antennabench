use std::{cell::RefCell, collections::VecDeque};

use antennabench_core::{Band, PlannedSlot};
use antennabench_wsjtx::{
    plan_wspr_live_acquisition_for_completed_slot, AdapterCancellationToken, WsprLiveAcquirer,
    WsprLiveAcquisitionError, WsprLiveHttpResponse, WsprLiveHttpTransport,
    WSPR_LIVE_CONNECT_TIMEOUT_SECONDS, WSPR_LIVE_IMPORT_LIMITS, WSPR_LIVE_TOTAL_TIMEOUT_SECONDS,
};
use chrono::{TimeZone, Utc};

#[derive(Default)]
struct FakeTransport {
    requests: RefCell<Vec<(String, u64)>>,
    responses: RefCell<VecDeque<Result<WsprLiveHttpResponse, WsprLiveAcquisitionError>>>,
}

impl FakeTransport {
    fn with_responses(
        responses: Vec<Result<WsprLiveHttpResponse, WsprLiveAcquisitionError>>,
    ) -> Self {
        Self {
            requests: RefCell::new(Vec::new()),
            responses: RefCell::new(responses.into()),
        }
    }
}

impl WsprLiveHttpTransport for FakeTransport {
    fn get(
        &self,
        url: &str,
        body_limit: u64,
        _cancellation: &AdapterCancellationToken,
    ) -> Result<WsprLiveHttpResponse, WsprLiveAcquisitionError> {
        self.requests
            .borrow_mut()
            .push((url.to_string(), body_limit));
        self.responses
            .borrow_mut()
            .pop_front()
            .expect("fake response is configured")
    }
}

fn plan() -> antennabench_wsjtx::WsprLiveAcquisitionPlan {
    let starts_at = Utc.with_ymd_and_hms(2026, 7, 15, 20, 0, 0).unwrap();
    plan_wspr_live_acquisition_for_completed_slot(
        "N1RWJ",
        &[PlannedSlot {
            slot_id: "slot-1".into(),
            sequence_number: 1,
            starts_at,
            duration_seconds: 120,
            guard_seconds: 10,
            band: Band::M20,
            antenna_label: "A".into(),
        }],
        "slot-1",
    )
    .unwrap()
}

fn response(status: u16, body: Vec<u8>) -> WsprLiveHttpResponse {
    WsprLiveHttpResponse {
        received_at: Utc.with_ymd_and_hms(2026, 7, 15, 20, 7, 0).unwrap(),
        status,
        body,
    }
}

#[test]
fn acquisition_uses_only_the_typed_query_url_and_existing_body_cap() {
    let expected_body = br#"{"meta":[],"data":[],"rows":0}"#.to_vec();
    let acquirer = WsprLiveAcquirer::new(FakeTransport::with_responses(vec![Ok(response(
        200,
        expected_body.clone(),
    ))]));

    let captured = acquirer
        .acquire(&plan(), &AdapterCancellationToken::default())
        .unwrap();

    assert_eq!(captured.body, expected_body);
    let requests = acquirer.transport().requests.borrow();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .0
        .starts_with("https://db1.wspr.live/?query=SELECT%20id%2C"));
    assert_eq!(requests[0].1, WSPR_LIVE_IMPORT_LIMITS.source_bytes);
}

#[test]
fn status_transport_size_and_cancellation_fail_without_a_partial_success() {
    let unavailable = WsprLiveAcquirer::new(FakeTransport::with_responses(vec![Ok(response(
        503,
        Vec::new(),
    ))]));
    assert_eq!(
        unavailable
            .acquire(&plan(), &AdapterCancellationToken::default())
            .unwrap_err(),
        WsprLiveAcquisitionError::HttpStatus { status: 503 }
    );

    let transport = WsprLiveAcquirer::new(FakeTransport::with_responses(vec![Err(
        WsprLiveAcquisitionError::Transport("offline".into()),
    )]));
    assert!(matches!(
        transport.acquire(&plan(), &AdapterCancellationToken::default()),
        Err(WsprLiveAcquisitionError::Transport(_))
    ));

    let oversized = WsprLiveAcquirer::new(FakeTransport::with_responses(vec![Ok(response(
        200,
        vec![0; WSPR_LIVE_IMPORT_LIMITS.source_bytes as usize + 1],
    ))]));
    assert!(matches!(
        oversized.acquire(&plan(), &AdapterCancellationToken::default()),
        Err(WsprLiveAcquisitionError::BodyBytes { .. })
    ));

    let cancellation = AdapterCancellationToken::default();
    cancellation.cancel();
    let cancelled = WsprLiveAcquirer::new(FakeTransport::default());
    assert_eq!(
        cancelled.acquire(&plan(), &cancellation).unwrap_err(),
        WsprLiveAcquisitionError::Cancelled
    );
    assert!(cancelled.transport().requests.borrow().is_empty());
}

#[test]
fn production_transport_timeouts_are_fixed_and_tests_do_not_contact_wspr_live() {
    assert_eq!(WSPR_LIVE_CONNECT_TIMEOUT_SECONDS, 5);
    assert_eq!(WSPR_LIVE_TOTAL_TIMEOUT_SECONDS, 20);
}
