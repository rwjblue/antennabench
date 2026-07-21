use std::collections::BTreeMap;

use antennabench_analysis::{
    summarize_bundle, summarize_bundle_with_activity, ComparisonAvailability, PathDirection,
};
use antennabench_core::{
    v2::{
        AcquisitionChannelId, AdapterDisposition, AdapterId, AdapterInput, AdapterReasonId,
        AdapterRecordV2, EventTimeBasisV2, MutationMember, NormalizedRecordKind,
        NormalizedRecordLink, ObservationRecordV2, PlanGenerationV2, Provenance, ProviderId,
        RecordMetaV2, SessionLifecycleV2, SourceId,
    },
    v3::{
        BundleFilesV3, BundleManifestV3, BundleV3Contents, OperatorEventPayloadV3, OperatorEventV3,
        RecordMetaV3, ScheduleV3, SessionStateV3, WsprCycleDirection, WsprCycleIntentV3,
    },
    v5::{AntennaControlPolicyV5, WsprReadinessBasisV5},
    validate_bundle_report, AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band,
    ExperimentMode, ObservationKind, SessionGoal, Station, SCHEMA_VERSION_V5,
};
use antennabench_report::{
    build_report_with_snapshot_and_activity, render_compact_summary_html, render_standalone_html,
    render_standalone_html_with_resources, ReportAzimuthSector, ReportCancellationToken,
    ReportCommonOpportunityMapGroup, ReportDistanceBin, ReportError, ReportResourceLimits,
    ReportSnapshotContext, REPORT_RESOURCE_LIMITS,
};
use chrono::{DateTime, Duration, TimeZone, Utc};

const SESSION_ID: &str = "session-synthetic-wspr-live-field-shape";
const FIRST_SLOT_ID: &str = "wspr-cycle-a";
const SECOND_SLOT_ID: &str = "wspr-cycle-b";
const FIRST_SPOT_COUNT: usize = 145;
const SECOND_SPOT_COUNT: usize = 43;
const OVERLAPPING_REPORTER_COUNT: usize = 33;

#[test]
fn confirmed_source_cycles_survive_projection_analysis_and_both_reports() {
    let durable = field_shape_fixture();
    assert_eq!(durable.observations.len(), 188);
    assert!(durable.observations.iter().all(|observation| {
        observation.meta.recorded_at > utc(12, 2, 1)
            && observation.slot_id.is_none()
            && observation.slot_label.is_none()
            && observation.slot_confidence.is_none()
    }));

    let current = durable.into_current();
    let bundle = &current.bundle;
    let first_count = current
        .bundle
        .observations
        .iter()
        .filter(|observation| observation.slot_id.as_deref() == Some(FIRST_SLOT_ID))
        .count();
    let second_count = current
        .bundle
        .observations
        .iter()
        .filter(|observation| observation.slot_id.as_deref() == Some(SECOND_SLOT_ID))
        .count();
    assert_eq!((first_count, second_count), (145, 43));
    assert!(bundle.observations.iter().all(|observation| {
        observation.slot_confidence == Some(0.95)
            && matches!(
                observation.meta.timestamp,
                timestamp if timestamp == utc(12, 0, 1) || timestamp == utc(12, 2, 1)
            )
    }));

    let summary = summarize_bundle(bundle).expect("field-shape fixture should analyze");
    assert_eq!(summary.overall.observation_counts.total, 188);
    assert_eq!(summary.overall.observation_counts.usable, 188);
    assert_eq!(summary.overall.observation_counts.excluded, 0);
    assert_eq!(
        summary.comparison.availability,
        ComparisonAvailability::DescriptivePairsAvailable
    );
    assert_eq!(summary.comparison.paired_rows.len(), 33);
    assert_eq!(summary.comparison.diagnostics.unique_path_count, 33);

    let directions = BTreeMap::from([
        (FIRST_SLOT_ID.to_string(), PathDirection::Transmit),
        (SECOND_SLOT_ID.to_string(), PathDirection::Transmit),
    ]);
    let activity = summarize_bundle_with_activity(bundle, &current.adapter_records, &directions)
        .expect("field-shape activity should analyze")
        .reporter_activity;
    assert_eq!(activity.cycle_rates.len(), 2);
    assert_eq!(activity.cycle_rates[0].active_reporter_count, 200);
    assert_eq!(activity.cycle_rates[0].heard_reporter_count, 145);
    assert_eq!(activity.cycle_rates[1].active_reporter_count, 180);
    assert_eq!(activity.cycle_rates[1].heard_reporter_count, 43);
    assert_eq!(activity.paired_rates[0].active_in_both_count, 180);
    assert_eq!(activity.paired_rates[0].heard_both_count, 33);
    assert_eq!(activity.paired_rates[0].left_only_count, 112);
    assert_eq!(activity.paired_rates[0].right_only_count, 10);
    assert_eq!(activity.paired_rates[0].heard_neither_count, 25);
    assert_eq!(
        activity.joint_summaries[0].unique_active_receiver_count,
        180
    );
    assert_eq!(
        activity.joint_summaries[0].receiver_block_opportunity_count,
        180
    );

    let report = build_report_with_snapshot_and_activity(
        bundle,
        &validate_bundle_report(bundle),
        &current.adapter_records,
        ReportSnapshotContext::default(),
    )
    .expect("field-shape fixture should build a report");
    assert_eq!(report.evidence.overall.observation_counts.usable, 188);
    assert_eq!(report.comparison.paired_rows.len(), 33);
    assert_eq!(report.coverage_maps.len(), 1);
    assert_eq!(report.common_opportunity_maps.len(), 1);
    let common = &report.common_opportunity_maps[0];
    assert_eq!(common.unique_common_active_receiver_count, 180);
    assert_eq!(common.receiver_block_opportunity_count, 180);
    assert_eq!(common.located_unique_receiver_count, 179);
    assert_eq!(common.located_receiver_block_opportunity_count, 179);
    assert_eq!(common.location_unavailable_unique_receiver_count, 1);
    assert_eq!(
        common.location_unavailable_receiver_block_opportunity_count,
        1
    );
    assert_eq!(
        common
            .distance_cells
            .iter()
            .map(|cell| (
                cell.heard_both_count,
                cell.left_only_count,
                cell.right_only_count,
                cell.heard_neither_count,
            ))
            .collect::<Vec<_>>(),
        vec![(33, 112, 0, 0), (0, 0, 0, 24), (0, 0, 0, 0), (0, 0, 10, 0)]
    );
    assert_eq!(report.coverage_overlap.len(), 1);
    let overlap = &report.coverage_overlap[0];
    let observed_overlap = overlap.observed.as_ref().unwrap();
    assert_eq!(
        (
            observed_overlap.left_only_unique_path_count,
            observed_overlap.shared_unique_path_count,
            observed_overlap.right_only_unique_path_count,
            observed_overlap.total_system_unique_path_count,
        ),
        (112, 33, 10, 155)
    );
    let opportunity_overlap = overlap.common_opportunity.as_ref().unwrap();
    assert_eq!(
        (
            opportunity_overlap.heard_both_count,
            opportunity_overlap.left_only_count,
            opportunity_overlap.right_only_count,
            opportunity_overlap.heard_neither_count,
        ),
        (33, 112, 10, 25)
    );
    assert_eq!(report.coverage_maps[0].panels.len(), 2);
    assert!(report.coverage_maps[0]
        .panels
        .iter()
        .all(|panel| panel.unmapped_reporter_count == 1));
    assert!(report.coverage_maps[0].panels.iter().all(|panel| {
        panel
            .cells
            .iter()
            .any(|cell| cell.state == antennabench_report::ReportCoverageState::Heard)
            && panel
                .cells
                .iter()
                .any(|cell| cell.state == antennabench_report::ReportCoverageState::ActiveNotHeard)
    }));
    let full = render_standalone_html(&report).expect("full report should render");
    let compact = render_compact_summary_html(&report).expect("compact report should render");
    let resource_error = render_standalone_html_with_resources(
        &report,
        ReportResourceLimits::testing(25_000, 8 * 1024 * 1024, full.len() as u64 - 1),
        &ReportCancellationToken::default(),
    )
    .expect_err("high-row activity template loops must honor the checked writer byte limit");
    assert!(matches!(
        resource_error,
        ReportError::Resource(ref error)
            if error.diagnostic.code == "resource.report.html_bytes"
    ));
    let cancellation = ReportCancellationToken::default();
    cancellation.cancel();
    assert!(matches!(
        render_standalone_html_with_resources(&report, REPORT_RESOURCE_LIMITS, &cancellation),
        Err(ReportError::Resource(ref error))
            if error.diagnostic.code == "resource.operation.cancelled"
    ));
    for html in [&full, &compact] {
        assert!(!html.contains("0 usable"));
        assert!(!html.contains("No matched paths"));
        assert!(html.contains("Which antenna was heard more often by the same active receivers?"));
        assert!(html.contains("This section asks a narrower question than raw path counts"));
        assert!(html.contains("Each receiver-block is one shared opportunity"));
        assert!(html.contains("Heard by both contributes to both antenna rates"));
        assert!(html.contains("class=\"chart detection-rate-chart\""));
        assert!(html.contains("145 / 200 (72.5%)"));
        assert!(html.contains("43 / 180 (23.9%)"));
        assert!(html.contains("Joint detection outcomes by separate comparison group"));
        assert!(html.contains("Receiver-block opportunities"));
        assert!(html.contains("<td>33</td><td>112</td><td>10</td><td>25</td>"));
        assert!(html.contains("80.6% / 23.9%"));
        assert!(html.contains("Per-block joint detection outcome audit"));
        assert!(html.contains("Complete band-qualified census"));
        assert!(html.contains("Common-opportunity detection"));
        assert!(html.contains("Heard neither"));
        assert!(html.contains("Common-opportunity distance and bearing cells"));
        assert!(html.contains("Repeatability limited:"));
        assert!(
            html.contains("not an inferential uncertainty statement")
                || html.contains("not a separate coverage score")
        );
        assert!(html
            .contains("Location unavailable: 1 unique receivers / 1 receiver-block opportunities"));
        assert!(
            html.contains("This is not the all-path observed profile")
                || html
                    .contains("separate from the all-path observed distance and direction profile")
                || html.contains("separate from the all-path observed footprint")
        );
        for category in [
            "Near / local proxy (under 500 km)",
            "Regional (500–1499 km)",
            "Longer path (1500–2999 km)",
            "DX-oriented (3000 km and above)",
        ] {
            assert!(html.contains(category));
        }
        assert!(!html.contains("0–1 Mm"));
        assert!(!html.contains("3–8 Mm"));
        assert!(!html.contains("<script"));
    }
    assert!(full.contains("Coverage overlap and repeatability"));
    assert!(full.contains("Observed complementarity"));
    assert!(full.contains("Opportunity-conditioned complementarity"));
    assert!(
        full.contains("Using both antennas produced <strong>155</strong> unique observed paths")
    );
    assert!(full.contains("<td>112</td><td>33</td><td>10</td><td>155</td>"));
    assert!(compact.contains("Observed footprint"));
    assert!(!compact.contains("<h2 id=\"coverage-overlap-title\">"));
    assert!(compact.contains("Review whether observed paths repeated across blocks"));
    assert!(!compact.contains("Observed complementarity"));
    assert_common_visual_has_accessible_rows(&full, common);
    assert!(full.contains("activity-summary-field-shape"));
    assert!(full.contains("activity-first-000"));
    assert!(full.contains("id=\"coverage-grid-0\" checked"));
    assert!(full.contains("coverage-grid-view"));
    assert!(full.contains("coverage-polar-view"));
    assert!(full.contains("@media print"));
    assert_common_visual_has_accessible_rows(&compact, common);
    assert_eq!(
        compact
            .matches("class=\"common-opportunity-rate-cell")
            .count(),
        64
    );
    assert!(compact.contains("Common-opportunity distance and bearing cells"));
    assert!(!compact.contains("<svg class=\"coverage-world\""));
    assert!(compact.contains("For this General coverage run"));
    assert!(compact.contains("detection among 180 common-active receiver opportunities"));
    assert!(compact.contains("unique observed paths"));
    assert!(
        compact.contains("These results describe this session, not a universal antenna ranking.")
    );
    assert_eq!(
        compact
            .matches("<dl class=\"facts answer-metrics\">")
            .count(),
        1
    );
    assert!(compact.contains("<details class=\"goal-help\">"));
    assert!(compact.contains("Question availability and limits"));
}

fn assert_common_visual_has_accessible_rows(html: &str, group: &ReportCommonOpportunityMapGroup) {
    let sector_labels = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    for (sector_index, sector) in ReportAzimuthSector::ALL.iter().enumerate() {
        for distance in ReportDistanceBin::ALL {
            let facts = group
                .polar_cells
                .iter()
                .find(|cell| cell.bearing_sector == *sector && cell.distance_bin == distance)
                .map(|cell| &cell.facts);
            let unique = facts.map_or(0, |cell| cell.unique_common_active_receiver_count);
            let opportunities = facts.map_or(0, |cell| cell.receiver_block_opportunity_count);
            let both = facts.map_or(0, |cell| cell.heard_both_count);
            let left_only = facts.map_or(0, |cell| cell.left_only_count);
            let right_only = facts.map_or(0, |cell| cell.right_only_count);
            let neither = facts.map_or(0, |cell| cell.heard_neither_count);
            let sector_label = sector_labels[sector_index];
            let distance_label = distance.label();
            assert!(html.contains(&format!(
                "<tr><td>{sector_label}</td><td>{distance_label}</td><td>{unique}</td><td>{opportunities}</td><td>{both}</td><td>{left_only}</td><td>{right_only}</td><td>{neither}</td>"
            )));
        }
    }
    assert_eq!(
        html.matches("class=\"common-opportunity-rate-cell").count(),
        64
    );
    assert!(html.contains("tabindex=\"0\" aria-label="));
    assert!(html.contains("title=\""));
    assert!(html.contains("Show exact distance and bearing data"));
    assert!(!html.contains("<linearGradient"));
}

#[test]
fn absent_census_renders_coverage_unknown_without_inventing_zero_activity() {
    let mut durable = field_shape_fixture();
    durable
        .adapter_records
        .retain(|record| !record.record_type.starts_with("wspr_live_activity_census"));
    let current = durable.into_current();
    let report = build_report_with_snapshot_and_activity(
        &current.bundle,
        &validate_bundle_report(&current.bundle),
        &current.adapter_records,
        ReportSnapshotContext::default(),
    )
    .unwrap();

    for html in [
        render_standalone_html(&report).unwrap(),
        render_compact_summary_html(&report).unwrap(),
    ] {
        assert!(html.contains("Activity coverage unknown"));
        assert!(!html.contains("id=\"reporter-activity\""));
        assert!(!html.contains("0 / 0 (0.0%)"));
    }
}

#[test]
fn zero_matched_paths_still_render_useful_common_opportunity_geography() {
    let mut durable = field_shape_fixture();
    durable.observations.retain(|observation| {
        !observation
            .observation_id
            .starts_with("wspr-cycle-b-spot-0")
    });
    durable
        .adapter_records
        .retain(|record| !record.record_id.starts_with("wspr-cycle-b-adapter-0"));
    let current = durable.into_current();
    let report = build_report_with_snapshot_and_activity(
        &current.bundle,
        &validate_bundle_report(&current.bundle),
        &current.adapter_records,
        ReportSnapshotContext::default(),
    )
    .unwrap();

    assert_eq!(
        report.comparison.availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert!(report.comparison.paired_rows.is_empty());
    let common = &report.common_opportunity_maps[0];
    assert_eq!(
        common
            .distance_cells
            .iter()
            .map(|cell| (
                cell.heard_both_count,
                cell.left_only_count,
                cell.right_only_count,
                cell.heard_neither_count,
            ))
            .collect::<Vec<_>>(),
        vec![(0, 145, 0, 0), (0, 0, 0, 24), (0, 0, 0, 0), (0, 0, 10, 0)]
    );
    let overlap = report.coverage_overlap[0].observed.as_ref().unwrap();
    assert_eq!(
        (
            overlap.left_only_unique_path_count,
            overlap.shared_unique_path_count,
            overlap.right_only_unique_path_count,
            overlap.total_system_unique_path_count,
        ),
        (145, 0, 10, 155)
    );

    for html in [
        render_standalone_html(&report).unwrap(),
        render_compact_summary_html(&report).unwrap(),
    ] {
        assert!(html.contains("No same-path SNR comparison: no matched paths"));
        assert!(html.contains("Common-opportunity detection"));
        assert!(html.contains("Most pronounced recorded cell"));
        assert!(html.contains("session-scoped common-opportunity evidence"));
        assert!(html.contains("Separate antenna detection-rate maps"));
    }
    let compact = render_compact_summary_html(&report).unwrap();
    assert!(compact.contains("Observed footprint"));
    assert!(compact.contains("<strong>145</strong><small>"));
    assert!(compact.contains("<strong>10</strong><small>"));
}

#[test]
fn truncated_census_caveat_qualifies_each_cycle_and_paired_rate() {
    let mut durable = field_shape_fixture();
    let summary = durable
        .adapter_records
        .iter_mut()
        .find(|record| record.record_id == "activity-summary-field-shape")
        .unwrap();
    summary.disposition = AdapterDisposition::PartiallyNormalized;
    let AdapterInput::Inline { data, .. } = &mut summary.input else {
        panic!("activity summary must be inline")
    };
    let mut value: serde_json::Value = serde_json::from_str(data).unwrap();
    value["truncated"] = serde_json::Value::Bool(true);
    *data = serde_json::to_string(&value).unwrap();
    let current = durable.into_current();
    let report = build_report_with_snapshot_and_activity(
        &current.bundle,
        &validate_bundle_report(&current.bundle),
        &current.adapter_records,
        ReportSnapshotContext::default(),
    )
    .unwrap();
    let compact = render_compact_summary_html(&report).unwrap();

    assert!(report
        .common_opportunity_maps
        .iter()
        .all(|group| group.coverage == antennabench_analysis::ReporterActivityCoverage::Truncated));
    assert!(compact.contains("Truncated census — capture limit may reduce the denominator"));
}

#[test]
fn paired_rate_heatmap_states_are_distinct_and_keyboard_accessible() {
    let durable = field_shape_fixture();
    let current = durable.into_current();
    let mut report = build_report_with_snapshot_and_activity(
        &current.bundle,
        &validate_bundle_report(&current.bundle),
        &current.adapter_records,
        ReportSnapshotContext::default(),
    )
    .unwrap();
    let cells = &mut report.common_opportunity_maps[0].polar_cells;
    cells[0].facts.receiver_block_opportunity_count = 10;
    cells[0].facts.left_heard_count = 4;
    cells[0].facts.right_heard_count = 4;
    cells[0].facts.left_detection_rate = Some(0.4);
    cells[0].facts.right_detection_rate = Some(0.4);
    cells[1].facts.receiver_block_opportunity_count = 0;
    cells[1].facts.left_heard_count = 0;
    cells[1].facts.right_heard_count = 0;
    cells[1].facts.left_detection_rate = None;
    cells[1].facts.right_detection_rate = None;
    cells[2].facts.receiver_block_opportunity_count = 3;
    cells[2].facts.left_heard_count = 1;
    cells[2].facts.right_heard_count = 2;
    cells[2].facts.left_detection_rate = Some(1.0 / 3.0);
    cells[2].facts.right_detection_rate = Some(2.0 / 3.0);

    let compact = render_compact_summary_html(&report).unwrap();
    for state in [
        "rate-zero",
        "zero-opportunities",
        "low-support",
        "rate-unavailable",
    ] {
        assert!(compact.contains(state), "missing heatmap state: {state}");
    }
    assert_eq!(
        compact
            .matches("class=\"common-opportunity-rate-cell")
            .count(),
        64
    );
    assert_eq!(compact.matches("tabindex=\"0\" aria-label=").count(), 64);
    assert!(compact.contains("Rate unavailable; not zero detection"));
    assert!(compact.contains("no located common-opportunity cell"));
    assert!(compact.contains("@media(max-width:760px)"));
    assert!(compact.contains("@media print"));
}

fn field_shape_fixture() -> BundleV3Contents {
    let first_cycle = utc(12, 0, 1);
    let second_cycle = utc(12, 2, 1);
    let captured_at = utc(12, 7, 2);
    let mut adapter_records = Vec::with_capacity(FIRST_SPOT_COUNT + SECOND_SPOT_COUNT + 381);
    let mut observations = Vec::with_capacity(FIRST_SPOT_COUNT + SECOND_SPOT_COUNT);

    append_cycle_spots(
        &mut adapter_records,
        &mut observations,
        FIRST_SLOT_ID,
        first_cycle - Duration::seconds(1),
        captured_at,
        0..FIRST_SPOT_COUNT,
    );
    append_activity_census(
        &mut adapter_records,
        first_cycle - Duration::seconds(1),
        second_cycle - Duration::seconds(1),
        captured_at,
    );
    append_cycle_spots(
        &mut adapter_records,
        &mut observations,
        SECOND_SLOT_ID,
        second_cycle - Duration::seconds(1),
        captured_at,
        (0..OVERLAPPING_REPORTER_COUNT).chain(FIRST_SPOT_COUNT..155),
    );

    BundleV3Contents {
        manifest: BundleManifestV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            created_at: utc(11, 59, 0),
            app_version: "synthetic-fixture".into(),
            files: BundleFilesV3::default(),
            creator_runtime_context_id: None,
        },
        session_state: SessionStateV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            revision: 192,
            lifecycle: SessionLifecycleV2::Ended,
            wspr_live_acquisition_enabled: true,
            active_plan: PlanGenerationV2 {
                generation_id: "synthetic-plan".into(),
                station_sha256: String::new(),
                antennas_sha256: String::new(),
                schedule_sha256: String::new(),
                root_sha256: String::new(),
            },
            streams: BTreeMap::new(),
            last_committed_mutation_id: Some("session-ended".into()),
            active_runtime_context_id: None,
            diagnostics_status: None,
        },
        station: Station {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            callsign: "N0CALL".into(),
            grid: "AA00".into(),
            power_watts: Some(5.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            antennas: vec![antenna("A"), antenna("B")],
        },
        schedule: ScheduleV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::GeneralCoverage,
            antenna_control: Some(AntennaControlPolicyV5::Manual),
            signal_plans: Vec::new(),
            wspr_cycle_intents: vec![
                cycle_intent(FIRST_SLOT_ID, 1, "A"),
                cycle_intent(SECOND_SLOT_ID, 2, "B"),
            ],
            slots: Vec::new(),
        },
        events: vec![
            event(
                "session-started",
                utc(11, 59, 20),
                None,
                OperatorEventPayloadV3::SessionStarted { note: None },
            ),
            event(
                "cycle-a-armed",
                utc(11, 59, 50),
                Some(FIRST_SLOT_ID),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "A".into(),
                    cycle_starts_at: first_cycle,
                    readiness: Some(WsprReadinessBasisV5::OperatorConfirmed),
                },
            ),
            event(
                "cycle-b-armed",
                utc(12, 1, 55),
                Some(SECOND_SLOT_ID),
                OperatorEventPayloadV3::WsprCycleArmed {
                    antenna_label: "B".into(),
                    cycle_starts_at: second_cycle,
                    readiness: Some(WsprReadinessBasisV5::OperatorConfirmed),
                },
            ),
            event(
                "session-ended",
                utc(12, 3, 55),
                None,
                OperatorEventPayloadV3::SessionEnded { reason: None },
            ),
        ],
        observations,
        adapter_records,
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
        runtime_contexts: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn append_activity_census(
    adapter_records: &mut Vec<AdapterRecordV2>,
    first_cycle: DateTime<Utc>,
    second_cycle: DateTime<Utc>,
    captured_at: DateTime<Utc>,
) {
    adapter_records.push(activity_record(
        "activity-summary-field-shape",
        "wspr_live_activity_census_summary",
        captured_at,
        serde_json::json!({
            "window_start": first_cycle,
            "window_end": second_cycle + Duration::minutes(2),
            "selected_bands": [Band::M20],
            "truncated": false,
            "counts": { "malformed": 0 }
        }),
    ));
    for reporter_index in 0..200 {
        adapter_records.push(activity_record(
            &format!("activity-first-{reporter_index:03}"),
            "wspr_live_activity_census",
            captured_at,
            serde_json::json!({
                "cycle_time": first_cycle,
                "band": Band::M20,
                "reporter": synthetic_callsign(reporter_index),
                "reporter_grid": activity_grid(reporter_index)
            }),
        ));
    }
    for reporter_index in 0..180 {
        adapter_records.push(activity_record(
            &format!("activity-second-{reporter_index:03}"),
            "wspr_live_activity_census",
            captured_at,
            serde_json::json!({
                "cycle_time": second_cycle,
                "band": Band::M20,
                "reporter": synthetic_callsign(reporter_index),
                "reporter_grid": activity_grid(reporter_index)
            }),
        ));
    }
}

fn activity_grid(reporter_index: usize) -> Option<&'static str> {
    match reporter_index {
        0..=32 => Some("AA00aa"),
        33..=144 => Some("AA00bb"),
        145..=154 => Some("AJ00aa"),
        155..=178 => Some("AB00aa"),
        179 => None,
        _ => Some("EE44ee"),
    }
}

fn activity_record(
    record_id: &str,
    record_type: &str,
    captured_at: DateTime<Utc>,
    data: serde_json::Value,
) -> AdapterRecordV2 {
    AdapterRecordV2 {
        meta: record_meta(captured_at, record_id, 0),
        record_id: record_id.into(),
        source_time: None,
        record_type: record_type.into(),
        disposition: AdapterDisposition::Accepted,
        reason: AdapterReasonId::new("wspr-live.activity-census").unwrap(),
        normalized_records: Vec::new(),
        input: AdapterInput::Inline {
            data: serde_json::to_string(&data).unwrap(),
            media_type: "application/json".into(),
            encoding: None,
            source_locator: Some("synthetic-field-shape.json".into()),
        },
    }
}

fn append_cycle_spots(
    adapter_records: &mut Vec<AdapterRecordV2>,
    observations: &mut Vec<ObservationRecordV2>,
    slot_id: &str,
    source_time: DateTime<Utc>,
    captured_at: DateTime<Utc>,
    reporters: impl IntoIterator<Item = usize>,
) {
    for reporter_index in reporters {
        let observation_id = format!("{slot_id}-spot-{reporter_index:03}");
        let adapter_id = format!("{slot_id}-adapter-{reporter_index:03}");
        let mutation_id = format!("{slot_id}-mutation-{reporter_index:03}");
        let reporter_call = synthetic_callsign(reporter_index);
        adapter_records.push(AdapterRecordV2 {
            meta: record_meta(captured_at, &mutation_id, 0),
            record_id: adapter_id.clone(),
            source_time: Some(source_time),
            record_type: "wspr_live_spot".into(),
            disposition: AdapterDisposition::Accepted,
            reason: AdapterReasonId::new("wspr-live.accepted").unwrap(),
            normalized_records: vec![NormalizedRecordLink {
                record_kind: NormalizedRecordKind::Observation,
                record_id: observation_id.clone(),
            }],
            input: AdapterInput::Inline {
                data: format!(r#"{{"time":"{source_time}","rx_sign":"{reporter_call}"}}"#),
                media_type: "application/json".into(),
                encoding: None,
                source_locator: Some("synthetic-field-shape.json".into()),
            },
        });
        observations.push(ObservationRecordV2 {
            meta: record_meta(captured_at, &mutation_id, 1),
            observation_id,
            adapter_record_ids: vec![adapter_id],
            observation_kind: ObservationKind::ImportedSpot,
            band: Band::M20,
            frequency_hz: Some(14_095_600),
            mode: Some("WSPR".into()),
            reporter_call: Some(reporter_call),
            heard_call: Some("N0CALL".into()),
            reporter_grid: Some("AA00".into()),
            heard_grid: Some("AA00".into()),
            distance_km: Some(1000.0 + reporter_index as f64),
            azimuth_degrees: Some((reporter_index % 360) as f64),
            snr_db: Some(-25.0 + (reporter_index % 15) as f32),
            drift_hz_per_minute: Some(0.0),
            power_watts: Some(5.0),
            slot_id: None,
            slot_label: None,
            slot_confidence: None,
            raw: serde_json::json!({
                "provider_spot_id": format!("{slot_id}-{reporter_index}"),
                "provider": "wspr-live",
                "source": "wsprnet-spots-mirror",
                "direction": "transmit",
            }),
        });
    }
}

fn record_meta(recorded_at: DateTime<Utc>, mutation_id: &str, member_index: u32) -> RecordMetaV2 {
    RecordMetaV2 {
        schema_version: SCHEMA_VERSION_V5,
        session_id: SESSION_ID.into(),
        recorded_at,
        provenance: wspr_live_provenance(),
        mutation: MutationMember {
            mutation_id: mutation_id.into(),
            member_index,
            member_count: 2,
        },
        runtime_context_id: None,
    }
}

fn wspr_live_provenance() -> Provenance {
    Provenance {
        provider_id: ProviderId::new("wspr-live").unwrap(),
        source_id: SourceId::new("wsprnet-spots-mirror").unwrap(),
        acquisition_channel: AcquisitionChannelId::new("https-query").unwrap(),
        adapter_id: AdapterId::new("antennabench.wspr-live-json").unwrap(),
        adapter_version: "synthetic-fixture".into(),
    }
}

fn event(
    event_id: &str,
    occurred_at: DateTime<Utc>,
    slot_id: Option<&str>,
    payload: OperatorEventPayloadV3,
) -> OperatorEventV3 {
    OperatorEventV3 {
        meta: RecordMetaV3 {
            schema_version: SCHEMA_VERSION_V5,
            session_id: SESSION_ID.into(),
            recorded_at: occurred_at,
            provenance: Provenance {
                provider_id: ProviderId::new("antennabench").unwrap(),
                source_id: SourceId::new("operator-evidence").unwrap(),
                acquisition_channel: AcquisitionChannelId::new("operator-entry").unwrap(),
                adapter_id: AdapterId::new("antennabench.operator").unwrap(),
                adapter_version: "synthetic-fixture".into(),
            },
            mutation: MutationMember {
                mutation_id: event_id.into(),
                member_index: 0,
                member_count: 1,
            },
            runtime_context_id: None,
        },
        event_id: event_id.into(),
        occurred_at,
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: slot_id.map(str::to_string),
        payload,
    }
}

fn cycle_intent(id: &str, sequence_number: u32, antenna_label: &str) -> WsprCycleIntentV3 {
    WsprCycleIntentV3 {
        intent_id: id.into(),
        sequence_number,
        band: Band::M20,
        antenna_label: antenna_label.into(),
        direction: Some(WsprCycleDirection::Transmit),
        signal: None,
    }
}

fn antenna(label: &str) -> Antenna {
    Antenna {
        label: label.into(),
        facets: Vec::new(),
        height_m: None,
        radial_count: None,
        radial_length_m: None,
        orientation_degrees: None,
        tuner: None,
        feedline: None,
        notes: None,
    }
}

fn synthetic_callsign(index: usize) -> String {
    let first = char::from(b'A' + u8::try_from(index / 26).unwrap());
    let second = char::from(b'A' + u8::try_from(index % 26).unwrap());
    format!("K0{first}{second}")
}

fn utc(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 19, hour, minute, second)
        .unwrap()
}
