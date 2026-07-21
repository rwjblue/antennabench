use std::path::PathBuf;

use antennabench_analysis::{ComparisonAvailability, EvidenceQuality, ObservationCounts};
use antennabench_core::{
    normalize_bundle,
    v2::SessionLifecycleV2,
    v3::WsprCycleDirection,
    v5::{
        AntennaControlDispositionV5, AntennaControlOutputEncodingV5, AntennaControlOutputV5,
        AntennaControlRoleV5,
    },
    AlignedSlotStatus, Band, ExperimentMode, ObservationKind, RecordSource, SessionGoal,
};
use antennabench_report::{
    build_report, render_compact_summary_html, render_standalone_html,
    render_standalone_html_with_operational_history, render_standalone_html_with_options,
    ControllerEvidenceHandling, ReportAcquisitionWorkflowStatus, ReportAdapterEvidence,
    ReportAntennaControlAttempt, ReportAzimuthSector, ReportDistanceBin, ReportEventCorrection,
    ReportEventCorrectionAction, ReportImportedEvidence, ReportLifecycleEvent,
    ReportLifecycleEventKind, ReportNotice, ReportOperatorEvent, ReportOperatorEventKind,
    ReportOverviewLimitation, ReportOverviewLocationCell, ReportOverviewPathDelta,
    ReportProviderCompleteness, ReportSnapshotContext, ReportWsprAttribution, ReportWsprCycle,
    ReportWsprReadinessBasis, SamePathSignalAnswerability, SessionReport, StandaloneHtmlOptions,
};
use antennabench_storage::BundleStore;
use chrono::Duration;

#[test]
fn renders_the_canonical_report_as_deterministic_offline_html() {
    let report = canonical_report();

    let first = render_standalone_html(&report).unwrap();
    let second = render_standalone_html(&report).unwrap();

    assert_eq!(first, second);
    assert!(first.starts_with("<!doctype html><html lang=\"en\">"));
    assert!(first.ends_with("</main></body></html>"));
    assert!(first.contains("<style>"));
    assert!(first.contains("Content-Security-Policy"));
    assert!(first.contains("default-src 'none'"));
    assert!(!first.contains("<script"));
    assert!(!first.contains("http://"));
    assert!(!first.contains("https://"));
    assert!(!first.contains("src=\""));

    for section in [
        "What did the run show?",
        "Observed reach",
        "Run quality",
        "Audit appendix",
        "Session context",
        "Schedule overview",
        "Evidence overview",
        "Antenna evidence",
        "Band evidence",
        "Slot evidence",
    ] {
        assert!(first.contains(section), "missing section: {section}");
    }
    for caption in [
        "Planned slots",
        "Path overlap and missingness data",
        "Data-quality timeline details",
        "Antenna SNR chart data",
        "Evidence by antenna",
        "Band evidence chart data",
        "Evidence details by band",
        "Slot evidence chart data",
        "Evidence details by slot",
    ] {
        assert!(first.contains(&format!("<caption>{caption}</caption>")));
    }
    assert!(first.matches("aria-hidden=\"true\"").count() >= 5);
    assert!(first.contains("<dt>Usable</dt><dd>19</dd>"));
    assert!(first.contains("<dt>Excluded</dt><dd>7</dd>"));
    assert!(first.contains("<dt>Scheduled bands</dt><dd>40 m, 20 m</dd>"));
    assert!(first.contains("<dt>Scheduled slots</dt><dd>12</dd>"));
    assert!(first.contains("This report is descriptive and does not select an antenna winner."));
    assert!(first.contains("<nav class=\"question-nav\" aria-label=\"Report questions\">"));
    for anchor in [
        "what-run-show",
        "reach-unique-paths",
        "distance-direction",
        "run-quality",
        "audit-appendix",
    ] {
        assert!(first.contains(&format!("href=\"#{anchor}\"")));
        assert!(first.contains(&format!("id=\"{anchor}\"")));
    }
    for unavailable_anchor in ["same-path-signal", "reporter-activity"] {
        assert!(!first.contains(&format!("href=\"#{unavailable_anchor}\"")));
        assert!(!first.contains(&format!("id=\"{unavailable_anchor}\"")));
    }
    assert!(first.contains("<details class=\"audit-disclosure\">"));
    assert!(first.contains("details:not([open])>:not(summary){display:none!important}"));
    assert!(first.contains("break-after:page"));
}

#[test]
fn operational_history_requires_explicit_full_report_inclusion_and_is_escaped() {
    let report = canonical_report();
    let support = r#"{"schema":"antennabench_support_summary.v1","code":"resource.jsonl_line_bytes","unsafe":"</pre><script>alert(1)</script>"}"#;
    let default_full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();
    let included = render_standalone_html_with_operational_history(
        &report,
        ControllerEvidenceHandling::Complete,
        support,
    )
    .unwrap();

    for private_default in [default_full, compact] {
        assert!(!private_default.contains("Operational support history"));
        assert!(!private_default.contains("resource.jsonl_line_bytes"));
    }
    assert!(included.contains("Operational support history"));
    assert!(included.contains("Explicitly included at export"));
    assert!(included.contains("resource.jsonl_line_bytes"));
    assert!(included.contains("&#60;/pre&#62;&#60;script&#62;alert(1)&#60;/script&#62;"));
    assert!(!included.contains("<script>alert(1)</script>"));
}

#[test]
fn compact_summary_reuses_full_report_facts_without_audit_detail() {
    let mut report = paired_report(true);
    report.snapshot.checkpoint_revision = Some(27);
    report
        .snapshot
        .antenna_control_attempts
        .push(ReportAntennaControlAttempt {
            record_id: "compact-omitted-command".into(),
            role: AntennaControlRoleV5::Verification,
            controller_profile_name: "test switch".into(),
            controller_profile_revision: "v1".into(),
            resolved_program: "/bin/test-switch".into(),
            resolved_arguments: vec!["--secret-audit-argument".into()],
            intent_id: "intent-1".into(),
            antenna: "A".into(),
            target: "relay-a".into(),
            mode: ExperimentMode::WholeStationAb,
            started_at: "2026-07-14T22:00:00Z".parse().unwrap(),
            completed_at: "2026-07-14T22:00:01Z".parse().unwrap(),
            elapsed_milliseconds: 1_000,
            disposition: AntennaControlDispositionV5::Exit { code: 0 },
            stdout: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: "controller audit output".into(),
                truncated: false,
            },
            stderr: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: String::new(),
                truncated: false,
            },
        });

    let full = render_standalone_html(&report).unwrap();
    let first = render_compact_summary_html(&report).unwrap();
    let second = render_compact_summary_html(&report).unwrap();

    assert_eq!(first, second, "compact bytes are deterministic");
    assert!(first.contains("AntennaBench compact share summary"));
    assert!(first.contains("Not the full audit report"));
    assert!(first.contains("Content-Security-Policy"));
    assert!(first.contains("default-src 'none'"));
    assert!(first.contains(".compact-summary .overview{break-after:auto}"));
    assert!(first.contains(".compact-summary .question-section{break-before:auto}"));
    assert!(!first.contains("<script"));
    assert!(!first.contains("http://"));
    assert!(!first.contains("https://"));
    assert!(!first.contains("src=\""));

    let scope_fact = format!(
        "Station <strong>{}</strong> at <strong>{}</strong>",
        report.overview.scope.station.callsign, report.overview.scope.station.grid
    );
    assert!(full.contains(&scope_fact));
    assert!(first.contains(&scope_fact));
    for shared_fact in [
        "Signed values:",
        "Supported by this run",
        "Shared-path signal",
        "Observed reach",
        "Run quality and answerability",
    ] {
        assert!(full.contains(shared_fact), "full output lost {shared_fact}");
        assert!(
            first.contains(shared_fact),
            "compact output lost {shared_fact}"
        );
    }
    assert!(first.contains("committed revision <strong>27</strong>"));
    let stratum = &report.overview.strata[0];
    let count_fact = format!(
        "{} / {}",
        stratum.unique_path_count, stratum.paired_row_count
    );
    assert!(full.contains(&count_fact));
    assert!(first.contains(&count_fact));
    assert!(first.contains("no rows are sampled here"));
    for omitted in [
        "Audit appendix",
        "Antenna-control command attempts",
        "controller audit output",
        "--secret-audit-argument",
        "Derived solar context",
        "Matched-pair difference data",
    ] {
        assert!(!first.contains(omitted), "compact output leaked {omitted}");
    }
}

#[test]
fn consolidates_standing_caveats_in_one_shared_reading_panel() {
    let report = paired_report(true);
    let full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();
    assert_eq!(full.matches("id=\"reading-guide-title\"").count(), 1);
    assert!(
        full.find("id=\"reading-guide-title\"").unwrap()
            < full.find("id=\"what-run-show-title\"").unwrap()
    );
    assert!(compact.contains(
        "<details class=\"panel reading-guide\"><summary>How to read this report</summary>"
    ));
    assert!(!compact.contains("<details class=\"panel reading-guide\" open"));
    for html in [&full, &compact] {
        for caveat in [
            "A missing public report is missing evidence, never a zero-strength signal, unless a band-qualified activity census proves that reporter was active for that cycle.",
            "This report describes evidence; it does not select a winner or prove one antenna is better.",
            "Each comparison group (direction × band × mode × kind × source) is analyzed separately and never combined.",
            "Alternating antennas reduces but does not eliminate time and propagation effects.",
        ] {
            assert_eq!(html.matches(caveat).count(), 1, "repeated caveat: {caveat}");
        }
        assert!(!html.contains("Unmatched paths are not zero-SNR measurements"));
        assert!(!html.contains("Adjacent switched slots reduce elapsed time"));
        assert!(!html.contains("strata are not pooled"));
        assert!(!html.contains("not a strength grade or winner"));
    }
}

#[test]
fn compact_summary_escapes_unavailable_and_bounded_reports() {
    let mut unavailable = canonical_report();
    unavailable.overview.scope.session_id = "<compact & session>".into();
    unavailable.overview.scope.station.callsign = "<call>".into();
    unavailable.overview.strata.clear();
    let unavailable_html = render_compact_summary_html(&unavailable).unwrap();
    assert!(unavailable_html.contains("&#60;compact &#38; session&#62;"));
    assert!(unavailable_html.contains("No comparison groups are available"));
    assert!(!unavailable_html.contains("<compact & session>"));

    let mut bounded = paired_report(true);
    bounded.completeness = antennabench_report::ReportCompleteness::BoundedOverview;
    bounded.comparison.paired_rows.clear();
    let bounded_html = render_compact_summary_html(&bounded).unwrap();
    assert!(bounded_html.contains("Bounded overview"));
    assert!(bounded_html.contains("no rows are sampled"));
}

#[test]
fn renders_revision_lifecycle_and_recorded_adapter_gap_disclosures() {
    let mut report = canonical_report();
    report.snapshot = ReportSnapshotContext {
        checkpoint_revision: Some(17),
        lifecycle: Some(SessionLifecycleV2::Interrupted),
        lifecycle_events: vec![ReportLifecycleEvent {
            kind: ReportLifecycleEventKind::InterruptionDetected,
            occurred_at: "2026-07-14T22:00:00Z".parse().unwrap(),
            detail: Some("recovered <without inventing evidence>".into()),
        }],
        operator_events: Vec::new(),
        wspr_cycles: Vec::new(),
        antenna_control_attempts: Vec::new(),
        adapter_evidence: ReportAdapterEvidence {
            record_count: 9,
            accepted_count: 5,
            malformed_count: 1,
            unsupported_count: 0,
            filtered_count: 1,
            duplicate_count: 1,
            conflict_count: 1,
            partially_normalized_count: 1,
            gap_count: 1,
            workflow_status: ReportAcquisitionWorkflowStatus::Incomplete,
            provider_completeness: ReportProviderCompleteness::Unknown,
            imports: vec![ReportImportedEvidence {
                provider_id: "wspr-live".into(),
                source_id: "wsprnet-spots-mirror".into(),
                captured_at: "2026-07-14T22:05:00Z".parse().unwrap(),
                window_start: "2026-07-14T21:00:00Z".parse().unwrap(),
                window_end: "2026-07-14T22:00:00Z".parse().unwrap(),
                selected_bands: vec![Band::M20],
                total_count: 6,
                accepted_count: 2,
                malformed_count: 1,
                filtered_count: 1,
                unsupported_count: 0,
                duplicate_count: 1,
                conflict_count: 1,
                observations_created: 2,
                provider_completeness: ReportProviderCompleteness::Unknown,
            }],
        },
    };

    let html = render_standalone_html(&report).unwrap();
    assert!(html.contains("Committed session snapshot"));
    assert!(html.contains("<dt>Checkpoint revision</dt><dd>17</dd>"));
    assert!(html.contains("Interrupted / in progress"));
    assert!(html.contains("1 recorded acquisition gap"));
    assert!(html.contains("Best-effort public collection retained rows for 1 recorded requested window(s); recorded acquisition gaps remain"));
    assert!(html.contains("Public-source boundary"));
    assert!(
        html.contains("the upstream mirror does not provide an independent completeness guarantee")
    );
    assert!(!html.contains("unknown source completeness"));
    assert!(html.contains("wspr-live / wsprnet-spots-mirror"));
    assert!(html.contains("half-open window"));
    assert!(html.contains("Lifecycle and interruption history"));
    assert!(html.contains("recovered &#60;without inventing evidence&#62;"));
}

#[test]
fn renders_successful_public_collection_without_turning_a_source_boundary_into_a_gap() {
    let mut report = canonical_report();
    report.snapshot = ReportSnapshotContext {
        checkpoint_revision: Some(19),
        lifecycle: Some(SessionLifecycleV2::Ended),
        lifecycle_events: Vec::new(),
        operator_events: Vec::new(),
        wspr_cycles: Vec::new(),
        antenna_control_attempts: Vec::new(),
        adapter_evidence: ReportAdapterEvidence {
            record_count: 6,
            accepted_count: 4,
            malformed_count: 1,
            unsupported_count: 0,
            filtered_count: 1,
            duplicate_count: 0,
            conflict_count: 0,
            partially_normalized_count: 0,
            gap_count: 0,
            workflow_status: ReportAcquisitionWorkflowStatus::Completed,
            provider_completeness: ReportProviderCompleteness::Unknown,
            imports: vec![
                ReportImportedEvidence {
                    provider_id: "wspr-live".into(),
                    source_id: "wsprnet-spots-mirror".into(),
                    captured_at: "2026-07-14T22:05:00Z".parse().unwrap(),
                    window_start: "2026-07-14T21:00:00Z".parse().unwrap(),
                    window_end: "2026-07-14T22:00:00Z".parse().unwrap(),
                    selected_bands: vec![Band::M20],
                    total_count: 6,
                    accepted_count: 4,
                    malformed_count: 1,
                    filtered_count: 1,
                    unsupported_count: 0,
                    duplicate_count: 0,
                    conflict_count: 0,
                    observations_created: 4,
                    provider_completeness: ReportProviderCompleteness::Unknown,
                },
                ReportImportedEvidence {
                    provider_id: "wsjtx-udp".into(),
                    source_id: "direct-local".into(),
                    captured_at: "2026-07-14T22:05:01Z".parse().unwrap(),
                    window_start: "2026-07-14T21:00:00Z".parse().unwrap(),
                    window_end: "2026-07-14T22:00:00Z".parse().unwrap(),
                    selected_bands: vec![Band::M20],
                    total_count: 1,
                    accepted_count: 1,
                    malformed_count: 0,
                    filtered_count: 0,
                    unsupported_count: 0,
                    duplicate_count: 0,
                    conflict_count: 0,
                    observations_created: 1,
                    provider_completeness: ReportProviderCompleteness::Known,
                },
            ],
        },
    };

    let html = render_standalone_html(&report).unwrap();
    assert!(html.contains("Collection completed; no recorded acquisition gaps"));
    assert!(
        html.contains("Best-effort public collection completed for 1 recorded requested window(s)")
    );
    assert!(html.contains("best-effort WSPR.live request-window collection; upstream mirror has no independent completeness guarantee"));
    assert!(html.contains("wsjtx-udp / direct-local"));
    assert!(html.contains("upstream completeness guarantee recorded"));
    assert!(!html.contains("Complete within recorded adapter scope"));
}

#[test]
fn acquisition_workflow_gap_and_provider_completeness_render_as_independent_facts() {
    let workflow_statuses = [
        ReportAcquisitionWorkflowStatus::NotConfigured,
        ReportAcquisitionWorkflowStatus::Completed,
        ReportAcquisitionWorkflowStatus::Incomplete,
    ];
    let provider_completeness = [
        ReportProviderCompleteness::Known,
        ReportProviderCompleteness::Unknown,
        ReportProviderCompleteness::Unsupported,
    ];

    for workflow_status in workflow_statuses {
        for gap_count in [0, 2] {
            for provider_completeness in provider_completeness {
                let mut report = canonical_report();
                report.snapshot.adapter_evidence.record_count =
                    usize::from(workflow_status != ReportAcquisitionWorkflowStatus::NotConfigured);
                report.snapshot.adapter_evidence.gap_count = gap_count;
                report.snapshot.adapter_evidence.workflow_status = workflow_status;
                report.snapshot.adapter_evidence.provider_completeness = provider_completeness;

                let full = render_standalone_html(&report).unwrap();
                let compact = render_compact_summary_html(&report).unwrap();
                if gap_count > 0 {
                    assert!(full.contains("2 recorded acquisition gaps"));
                    assert!(compact.contains("2 recorded acquisition gap(s)"));
                    continue;
                }

                match workflow_status {
                    ReportAcquisitionWorkflowStatus::NotConfigured => {
                        assert!(full.contains("No acquisition workflow was configured"));
                        assert!(compact.contains("No configured acquisition"));
                    }
                    ReportAcquisitionWorkflowStatus::Incomplete => {
                        assert!(full.contains("Recorded acquisition is incomplete"));
                        assert!(compact.contains("Recorded acquisition incomplete"));
                    }
                    ReportAcquisitionWorkflowStatus::Completed => {
                        assert!(full.contains("Collection completed"));
                        assert!(compact.contains("Collection completed"));
                        assert!(!full.contains("Recorded acquisition is incomplete"));
                        assert!(!compact.contains("Recorded acquisition incomplete"));
                        let provider_text = match provider_completeness {
                            ReportProviderCompleteness::Known => {
                                "provider completeness is recorded as known"
                            }
                            ReportProviderCompleteness::Unknown => {
                                "upstream completeness is not independently guaranteed"
                            }
                            ReportProviderCompleteness::Unsupported => {
                                "provider completeness is unsupported"
                            }
                        };
                        assert!(full.to_ascii_lowercase().contains(provider_text));
                        assert!(compact.contains(provider_text));
                    }
                }
            }
        }
    }
}

#[test]
fn renders_readiness_basis_and_bounded_command_diagnostics() {
    let mut report = canonical_report();
    let ready_at = "2026-07-14T22:01:00Z".parse().unwrap();
    report.snapshot = ReportSnapshotContext {
        checkpoint_revision: Some(18),
        lifecycle: Some(SessionLifecycleV2::Running),
        lifecycle_events: Vec::new(),
        operator_events: Vec::new(),
        wspr_cycles: vec![ReportWsprCycle {
            intent_id: "intent-1".into(),
            sequence_number: 1,
            band: Band::M20,
            direction: Some(WsprCycleDirection::Transmit),
            planned_antenna: "A".into(),
            actual_antenna: Some("A".into()),
            ready_at: Some(ready_at),
            starts_at: Some("2026-07-14T22:02:01Z".parse().unwrap()),
            transmission_ends_at: Some("2026-07-14T22:03:51.592Z".parse().unwrap()),
            attribution: ReportWsprAttribution::Attributable,
            readiness_basis: Some(ReportWsprReadinessBasis::CommandVerified),
        }],
        antenna_control_attempts: vec![ReportAntennaControlAttempt {
            record_id: "verify-record".into(),
            role: AntennaControlRoleV5::Verification,
            controller_profile_name: "switch <profile>".into(),
            controller_profile_revision: "revision-7".into(),
            resolved_program: "/opt/bin/verify".into(),
            resolved_arguments: vec!["--target".into(), "relay-a".into()],
            intent_id: "intent-1".into(),
            antenna: "A".into(),
            target: "relay-a".into(),
            mode: ExperimentMode::TxFocused,
            started_at: ready_at,
            completed_at: ready_at,
            elapsed_milliseconds: 0,
            disposition: AntennaControlDispositionV5::Exit { code: 0 },
            stdout: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Utf8,
                data: "sensitive-controller-stdout".into(),
                truncated: false,
            },
            stderr: AntennaControlOutputV5 {
                encoding: AntennaControlOutputEncodingV5::Base64,
                data: "AAE=".into(),
                truncated: true,
            },
        }],
        adapter_evidence: ReportAdapterEvidence::default(),
    };

    let mut continued = report.snapshot.wspr_cycles[0].clone();
    continued.intent_id = "intent-2".into();
    continued.sequence_number = 2;
    continued.readiness_basis = Some(ReportWsprReadinessBasis::Continued);
    report.snapshot.wspr_cycles.push(continued);

    let html = render_standalone_html(&report).unwrap();
    assert!(html.contains("Command verified"));
    assert!(html.contains("Continued readiness"));
    assert!(html.contains("Antenna-control command attempts"));
    assert!(html.contains("Transmit-focused"));
    assert!(html.contains("switch &#60;profile&#62;"));
    assert!(html.contains("truncated=true"));
    assert!(html.contains("[1]=&#34;relay-a&#34;"));

    let explicit_complete = render_standalone_html_with_options(
        &report,
        StandaloneHtmlOptions {
            controller_evidence: ControllerEvidenceHandling::Complete,
        },
    )
    .unwrap();
    assert_eq!(
        explicit_complete, html,
        "the explicit default preserves today's output"
    );

    let report_json = serde_json::to_string(&report).unwrap();
    let omitted = render_standalone_html_with_options(
        &report,
        StandaloneHtmlOptions {
            controller_evidence: ControllerEvidenceHandling::OmittedAtExport,
        },
    )
    .unwrap();
    assert_eq!(serde_json::to_string(&report).unwrap(), report_json);
    for sensitive in [
        "/opt/bin/verify",
        "--target",
        "relay-a",
        "sensitive-controller-stdout",
        "AAE=",
    ] {
        assert!(
            !omitted.contains(sensitive),
            "omitted report leaked {sensitive}"
        );
    }
    assert!(omitted.contains("Controller command details omitted at export"));
    assert_eq!(omitted.matches("Omitted at export").count(), 5);
    for retained in [
        "verify-record",
        "Verification",
        "intent-1",
        "switch &#60;profile&#62;",
        "revision-7",
        "Exit code 0",
        "0 ms elapsed",
        "Command verified",
    ] {
        assert!(omitted.contains(retained), "omitted report lost {retained}");
    }
}

#[test]
fn escapes_every_untrusted_report_string() {
    let mut report = paired_report(true);
    let hostile = "\"><script>alert('x') & imported</script>".to_string();

    report.context.session_id = hostile.clone();
    report.overview.scope.session_id = hostile.clone();
    report.context.station.callsign = hostile.clone();
    report.context.station.grid = hostile.clone();
    report.overview.scope.station.callsign = hostile.clone();
    report.overview.scope.station.grid = hostile.clone();
    report.overview.scope.antenna_labels = vec![hostile.clone(), hostile.clone()];
    for antenna in &mut report.context.antennas {
        antenna.label = hostile.clone();
        antenna.facets = vec![hostile.clone()];
        antenna.tuner = Some(hostile.clone());
        antenna.feedline = Some(hostile.clone());
        antenna.notes = Some(hostile.clone());
    }
    for slot in &mut report.context.schedule.slots {
        slot.slot_id = hostile.clone();
        slot.planned_label = hostile.clone();
    }
    for antenna in &mut report.evidence.antennas {
        antenna.antenna_label = hostile.clone();
    }
    for slot in &mut report.evidence.slots {
        slot.slot_id = hostile.clone();
        slot.planned_label = hostile.clone();
        slot.actual_label = Some(hostile.clone());
    }
    for row in &mut report.chart_data.antenna_snr {
        row.antenna_label = hostile.clone();
    }
    for row in &mut report.chart_data.slot_evidence_counts {
        row.slot_id = hostile.clone();
        row.planned_label = hostile.clone();
        row.actual_label = Some(hostile.clone());
    }
    report.comparison.left_label = Some(hostile.clone());
    report.comparison.right_label = Some(hostile.clone());
    if let Some(orientation) = &mut report.comparison.delta_orientation {
        orientation.minuend_label = hostile.clone();
        orientation.subtrahend_label = hostile.clone();
    }
    if let Some(orientation) = &mut report.overview.scope.delta_orientation {
        orientation.minuend_label = hostile.clone();
        orientation.subtrahend_label = hostile.clone();
    }
    for block in &mut report.comparison.blocks {
        block.first_slot_id = hostile.clone();
        block.first_label = Some(hostile.clone());
        block.second_slot_id = Some(hostile.clone());
        block.second_label = Some(hostile.clone());
    }
    for row in &mut report.comparison.overlap_rows {
        row.remote_path = hostile.clone();
    }
    for row in &mut report.comparison.timeline_rows {
        row.slot_id = hostile.clone();
        row.actual_label = Some(hostile.clone());
    }
    for row in &mut report.comparison.paired_rows {
        row.remote_path = hostile.clone();
        row.left_observation_id = hostile.clone();
        row.right_observation_id = hostile.clone();
        row.left_slot_id = hostile.clone();
        row.right_slot_id = hostile.clone();
        row.left_remote_grid = Some(hostile.clone());
        row.right_remote_grid = Some(hostile.clone());
    }
    for row in &mut report.comparison.path_summaries {
        row.remote_path = hostile.clone();
    }
    for row in &mut report.solar_context.rows {
        row.remote_path = hostile.clone();
        for observation in [&mut row.left, &mut row.right] {
            observation.observation_id = hostile.clone();
            observation.station.endpoint_id = hostile.clone();
            observation.remote.endpoint_id = hostile.clone();
            observation.station.grid = Some(hostile.clone());
            observation.remote.grid = Some(hostile.clone());
        }
    }

    let html = render_standalone_html(&report).unwrap();

    assert!(!html.contains(&hostile));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("</script>"));
    assert!(
        html.contains("&#34;&#62;&#60;script&#62;alert(&#39;x&#39;) &#38; imported&#60;/script&#62;")
    );
}

#[test]
fn renders_distinct_escaped_antenna_labels_without_mutating_report_data() {
    let mut report = paired_report(true);
    let left_label = "<Vertical & 1>";
    let right_label = "Loop > Beam";
    report.comparison.left_label = Some(left_label.into());
    report.comparison.right_label = Some(right_label.into());
    report.overview.scope.antenna_labels = vec![left_label.into(), right_label.into()];
    for orientation in [
        report.comparison.delta_orientation.as_mut(),
        report.overview.scope.delta_orientation.as_mut(),
    ]
    .into_iter()
    .flatten()
    {
        orientation.subtrahend_label = left_label.into();
        orientation.minuend_label = right_label.into();
    }
    let before = serde_json::to_vec(&report).unwrap();

    let full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();

    assert_eq!(serde_json::to_vec(&report).unwrap(), before);
    for html in [&full, &compact] {
        assert!(html.contains(
            "Positive values mean Loop &#62; Beam was stronger; negative values mean &#60;Vertical &#38; 1&#62; was stronger."
        ));
        assert!(html.contains("&#60;Vertical &#38; 1&#62; only"));
        assert!(html.contains("Loop &#62; Beam only"));
        assert!(!html.contains(left_label));
        assert!(!html.contains(right_label));
    }
    assert!(full.contains(
        "<th scope=\"col\">&#60;Vertical &#38; 1&#62; usable</th><th scope=\"col\">Loop &#62; Beam usable</th>"
    ));
    assert!(full.contains(
        "<th scope=\"col\">Unmatched — &#60;Vertical &#38; 1&#62; / Loop &#62; Beam</th>"
    ));
    for expected in [
        "Unmatched — &#60;Vertical &#38; 1&#62;",
        "Missing SNR — Loop &#62; Beam",
        "&#60;Vertical &#38; 1&#62; then Loop &#62; Beam",
        "Loop &#62; Beam then &#60;Vertical &#38; 1&#62;",
    ] {
        assert!(
            full.contains(expected),
            "missing labeled audit text: {expected}"
        );
    }
    for legacy in [
        "Unmatched left",
        "Unmatched right",
        "Missing SNR left",
        "Missing SNR right",
        "Left then right",
        "Right then left",
        "right minus left",
        "right − left",
    ] {
        assert!(!full.contains(legacy), "leaked display-side term: {legacy}");
    }
}

#[test]
fn renders_empty_and_unavailable_data_as_explicit_states() {
    let mut report = canonical_report();
    report.context.scheduled_time_range = None;
    report.context.antennas.clear();
    report.context.bands.clear();
    report.context.schedule.slot_count = 0;
    report.context.schedule.slots.clear();
    report.evidence.evidence_quality = EvidenceQuality::Insufficient;
    report.evidence.overall.observation_counts = ObservationCounts {
        total: 0,
        usable: 0,
        excluded: 0,
    };
    report.evidence.overall.exclusions.clear();
    report.evidence.overall.usable_observation_kinds = Default::default();
    report.evidence.overall.snr = None;
    report.evidence.antennas.clear();
    report.evidence.bands.clear();
    report.evidence.slots.clear();
    report.chart_data.antenna_snr.clear();
    report.chart_data.band_evidence_counts.clear();
    report.chart_data.slot_evidence_counts.clear();
    report.notices = vec![
        ReportNotice::NoScheduledSlots,
        ReportNotice::NoUsableObservations,
        ReportNotice::NoUsableSnrSamples,
    ];

    let html = render_standalone_html(&report).unwrap();

    for state in [
        "No scheduled slots are available; schedule comparisons cannot be shown.",
        "No observations met the current evidence rules; no usable counts are inferred.",
        "No usable SNR samples are available; SNR statistics are shown as unavailable.",
        "No scheduled time range",
        "No antennas are present in this report.",
        "No antenna SNR rows are available.",
        "No per-antenna evidence is available.",
        "No per-band evidence is available.",
        "No per-slot evidence is available.",
        "Not available",
    ] {
        assert!(html.contains(state), "missing empty state: {state}");
    }
    assert!(!html.contains("NaN"));
    assert!(!html.contains("Infinity"));
}

#[test]
fn renders_every_evidence_coverage_value_with_non_comparative_explanation() {
    for (coverage, label) in [
        (EvidenceQuality::Insufficient, "Insufficient"),
        (EvidenceQuality::Weak, "Weak"),
        (EvidenceQuality::Moderate, "Moderate"),
    ] {
        let mut report = canonical_report();
        report.evidence.evidence_quality = coverage;
        report.evidence.antennas[0].evidence_quality = coverage;

        let html = render_standalone_html(&report).unwrap();

        assert!(html.contains(&format!(
            "Evidence coverage: <span class=\"badge\">{label}</span>"
        )));
        assert!(html.contains(&format!("<td>{label}</td>")));
        assert!(html.contains(
            "Coverage reflects usable observations and contributing slots; it is not evidence that one antenna is better."
        ));
        assert!(!html.contains("Evidence quality"));
        assert!(!html.contains("<th scope=\"col\">Quality</th>"));
    }
}

#[test]
fn renders_plain_language_headline_for_every_comparison_availability() {
    for (availability, expected) in [
        (
            ComparisonAvailability::NotApplicable,
            "This session profiles one antenna. Comparative signal and detection questions do not apply; review its recorded footprint and repetition evidence when available.",
        ),
        (
            ComparisonAvailability::UnsupportedComparisonShape,
            "An A/B comparison needs exactly two antenna labels; this session recorded a different comparison shape. Use exactly two labels for a future A/B session.",
        ),
        (
            ComparisonAvailability::NoEligibleBlocks,
            "This run did not complete a usable back-to-back pair of cycles on both antennas, so no matched comparison was possible. To make a future run answerable, complete more repetitions.",
        ),
        (
            ComparisonAvailability::NoMatchedPaths,
            "For this General coverage run",
        ),
    ] {
        let mut report = paired_report(false);
        report.comparison.availability = availability;
        report.overview.comparison_availability = availability;

        for html in [
            render_standalone_html(&report).unwrap(),
            render_compact_summary_html(&report).unwrap(),
        ] {
            let answer = plain_language_answer_from(&html);
            assert!(answer.contains(expected), "missing headline for {availability:?}");
            if availability == ComparisonAvailability::NoMatchedPaths {
                assert!(answer.contains("unique observed paths"));
                assert!(answer.contains("not a universal antenna ranking"));
            }
            for forbidden in ["winner", "significant", "better antenna", "confidence"] {
                assert!(
                    !answer.to_ascii_lowercase().contains(forbidden),
                    "headline used prohibited claim language: {answer}"
                );
            }
        }
    }

    let report = paired_report(true);
    for html in [
        render_standalone_html(&report).unwrap(),
        render_compact_summary_html(&report).unwrap(),
    ] {
        let answer = plain_language_answer_from(&html);
        assert!(answer.contains("For this General coverage run"));
        assert!(answer.contains("a +1.5 dB median across 2 shared paths"));
        assert!(answer
            .contains("These results describe this session, not a universal antenna ranking."));
    }
    assert!(render_standalone_html(&report)
        .unwrap()
        .contains("For scale:</strong> a 3 dB difference"));
    assert!(!render_compact_summary_html(&report)
        .unwrap()
        .contains("For scale:</strong> a 3 dB difference"));
}

#[test]
fn no_matched_paths_leads_with_separate_nonzero_reach_facts_in_full_and_compact_reports() {
    let report = canonical_report();
    assert_eq!(
        report.overview.comparison_availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert!(report.evidence.overall.observation_counts.usable > 0);
    assert_ne!(
        report.overview.answerability.same_path_signal,
        SamePathSignalAnswerability::Available
    );

    let full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();
    let full_answer = plain_language_answer_from(&full);
    let compact_answer = plain_language_answer_from(&compact);
    assert_eq!(full_answer, compact_answer);
    for html in [&full, &compact] {
        assert!(html.contains("Answered by this run: Observed reach"));
        assert!(html.contains("No same-path SNR comparison"));
        assert!(!html.contains("id=\"same-path-signal\""));
        assert!(!html.contains("href=\"#same-path-signal\""));
    }
    assert!(full_answer.starts_with("Headline evidence is shown separately for "));
    assert!(full_answer.contains("comparison groups below and is not pooled"));
    assert_eq!(
        full_answer
            .matches("not a universal antenna ranking")
            .count(),
        1
    );
    let supported_group_count = report
        .overview
        .strata
        .iter()
        .filter(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                > 0
        })
        .count();
    for html in [&full, &compact] {
        assert_eq!(
            html.matches("class=\"headline-group-answer\"").count(),
            supported_group_count
        );
        assert!(html.contains("1 versus 1 unique observed paths"));
        assert!(html.contains("2 versus 0 unique observed paths"));
    }
    for prohibited in [
        "winner",
        "superiority",
        "coverage score",
        "equivalent",
        "inferred zero",
    ] {
        assert!(!full_answer.to_ascii_lowercase().contains(prohibited));
    }
}

#[test]
fn no_matched_paths_with_zero_usable_observations_does_not_claim_reach_evidence() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/inconclusive-sample-report.session.wsprabundle");
    let mut bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("canonical sample should be valid");
    bundle.observations.clear();
    let report = build_report(&bundle).unwrap();
    assert_eq!(
        report.overview.comparison_availability,
        ComparisonAvailability::NoMatchedPaths
    );
    assert_eq!(report.evidence.overall.observation_counts.usable, 0);

    let full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();
    let full_answer = plain_language_answer_from(&full);
    assert_eq!(full_answer, plain_language_answer_from(&compact));
    assert_eq!(
        full_answer,
        "No usable observations were recorded, so this run has no reach evidence and no same-path signal delta to summarize."
    );
    assert!(!full_answer.contains("Both"));
    assert!(!full_answer.contains("unique path"));
}

#[test]
fn zero_median_headline_stays_descriptive_without_implying_equivalence() {
    let mut report = paired_report(true);
    let ReportOverviewPathDelta::Available {
        minimum_delta_right_minus_left_db,
        maximum_delta_right_minus_left_db,
        ..
    } = report.overview.strata[0].path_delta
    else {
        panic!("paired report should have an available path delta");
    };
    report.overview.strata[0].path_delta = ReportOverviewPathDelta::Available {
        minimum_delta_right_minus_left_db,
        median_path_delta_right_minus_left_db: 0.0,
        maximum_delta_right_minus_left_db,
    };

    let html = render_standalone_html(&report).unwrap();
    let answer = plain_language_answer_from(&html);

    assert!(answer.contains("a 0 dB shared-path median across 2 paths"));
    assert!(answer.contains("available headline measures were tied"));
    for prohibited in ["equivalent", "too close", "winner", "better antenna"] {
        assert!(!answer.to_ascii_lowercase().contains(prohibited));
    }
}

#[test]
fn zero_public_wspr_spots_adds_only_setup_guidance() {
    let normal = render_standalone_html(&canonical_report()).unwrap();
    assert!(!plain_language_answer_from(&normal).contains("check WSPR upload settings"));

    let mut report = canonical_report();
    for row in report.overview.strata.iter_mut().filter(|row| {
        row.stratum.observation_kind == ObservationKind::PublicReport
            && row.stratum.mode.as_str() == "WSPR"
    }) {
        row.paired_row_count = 0;
        row.unmatched_left_count = 0;
        row.unmatched_right_count = 0;
        row.missing_snr_left_count = 0;
        row.missing_snr_right_count = 0;
        row.excluded_observation_count = 0;
    }

    let html = render_standalone_html(&report).unwrap();
    let answer = plain_language_answer_from(&html);

    assert!(answer.contains(
        "No public WSPR spots were recorded; check WSPR upload settings before the next run."
    ));
    for prohibited in ["winner", "significant", "better antenna", "confidence"] {
        assert!(!answer.to_ascii_lowercase().contains(prohibited));
    }
}

#[test]
fn keeps_answerability_headlines_short_and_all_diagnostics_disclosed() {
    let report = paired_report(true);
    let html = render_standalone_html(&report).unwrap();
    let table_start = html
        .find("<table class=\"answerability-table\">")
        .expect("headline answerability table should render");
    let table_end = table_start
        + html[table_start..]
            .find("</table>")
            .expect("headline answerability table should close");
    let headline_table = &html[table_start..table_end];

    assert_eq!(headline_table.matches("<th scope=\"col\">").count(), 4);
    for heading in [
        "Comparison group",
        "Availability",
        "Matched pairs",
        "Blocks",
    ] {
        assert!(headline_table.contains(&format!("<th scope=\"col\">{heading}</th>")));
    }
    for diagnostic in [
        "Unique matched paths",
        "Unmatched —",
        "Missing SNR —",
        "Excluded",
        "Duplicates",
        "Conflicts",
    ] {
        assert!(!headline_table.contains(diagnostic));
    }

    let disclosure_start = html
        .find("<summary>Review per-group answerability diagnostics</summary>")
        .expect("diagnostic disclosure should render");
    assert!(table_end < disclosure_start);
    let disclosure_end = disclosure_start
        + html[disclosure_start..]
            .find("</details>")
            .expect("diagnostic disclosure should close");
    let disclosure = &html[disclosure_start..disclosure_end];
    for diagnostic in [
        "Detailed answerability diagnostics by comparison group",
        "Unique matched paths",
        "A→B / B→A",
        "Unmatched — A / B",
        "Missing SNR — A / B",
        "Excluded",
        "Duplicates",
        "Conflicts",
    ] {
        assert!(
            disclosure.contains(diagnostic),
            "missing diagnostic: {diagnostic}"
        );
    }
}

#[test]
fn renders_answer_first_order_unavailable_states_and_visible_limitations() {
    let mut report = paired_report(true);
    report.overview.comparison_availability = ComparisonAvailability::NotApplicable;
    report.overview.answerability = Default::default();
    report.overview.scope.delta_orientation = None;
    report.overview.strata.clear();
    report.overview.limitations = vec![ReportOverviewLimitation::ComparisonNotApplicable];
    report.snapshot.adapter_evidence.workflow_status = ReportAcquisitionWorkflowStatus::Incomplete;
    report.snapshot.adapter_evidence.gap_count = 2;
    report.notices.push(ReportNotice::DetailOmitted {
        family: antennabench_report::ReportDetailFamily::PairedObservations,
        row_count: 42,
    });

    let html = render_standalone_html(&report).unwrap();
    let omission = html
        .find("full paired observation detail is omitted")
        .unwrap();
    let run_quality = html.find("id=\"run-quality\"").unwrap();
    let first_details = html.find("<details").unwrap();

    assert!(!html.contains("id=\"same-path-signal\""));
    assert!(run_quality < omission);
    assert!(html.find("2 recorded acquisition gaps").unwrap() < first_details);
    assert!(html.contains("Delta orientation:</strong> unavailable"));
    assert!(html.contains("No comparison groups are available for this run."));
    assert!(html.contains("Comparative questions: not applicable to single-antenna profiling."));
    assert!(!html.contains("Winner:"));
    assert!(!html.contains("antenna gain"));
}

#[test]
fn goal_lenses_reorder_the_same_facts_in_full_and_compact_reports() {
    let general = paired_report_for_goal(true, SessionGoal::GeneralCoverage);
    let dx = paired_report_for_goal(true, SessionGoal::Dx);
    let weak_signal = paired_report_for_goal(true, SessionGoal::WeakSignalReliability);

    for candidate in [&dx, &weak_signal] {
        assert_eq!(candidate.evidence, general.evidence);
        assert_eq!(candidate.comparison, general.comparison);
        assert_eq!(candidate.reporter_activity, general.reporter_activity);
        assert_eq!(candidate.coverage_maps, general.coverage_maps);
        assert_eq!(
            candidate.common_opportunity_maps,
            general.common_opportunity_maps
        );
        assert_eq!(candidate.coverage_overlap, general.coverage_overlap);
        assert_eq!(candidate.solar_context, general.solar_context);
        assert_eq!(candidate.chart_data, general.chart_data);
        assert_eq!(
            candidate.overview.answerability,
            general.overview.answerability
        );
        assert_eq!(candidate.overview.strata, general.overview.strata);
    }

    for compact in [false, true] {
        let render = |report: &SessionReport| {
            if compact {
                render_compact_summary_html(report)
            } else {
                render_standalone_html(report)
            }
        };
        let general_html = render(&general).unwrap();
        let dx_html = render(&dx).unwrap();
        let (general_sections, dx_sections): (&[&str], &[&str]) = if compact {
            (
                &["same-path-signal", "observed-footprint"],
                &["observed-footprint", "same-path-signal"],
            )
        } else {
            (
                &[
                    "same-path-signal",
                    "reach-unique-paths",
                    "distance-direction",
                    "coverage-overlap",
                ],
                &[
                    "distance-direction",
                    "same-path-signal",
                    "coverage-overlap",
                    "reach-unique-paths",
                ],
            )
        };
        let general_positions = section_positions(&general_html, general_sections);
        let dx_positions = section_positions(&dx_html, dx_sections);
        assert!(general_positions.is_sorted());
        assert!(dx_positions.is_sorted());
        for section in general_sections {
            assert!(general_html.contains(&format!("id=\"{section}\"")));
            assert!(dx_html.contains(&format!("id=\"{section}\"")));
        }
        assert!(dx_html.contains("DX-oriented (3000 km and above)"));
        assert!(dx_html.contains("Every other available distance category remains visible"));
    }
}

#[test]
fn nvis_and_single_profile_wording_stays_within_the_predeclared_contract() {
    let nvis = paired_report_for_goal(true, SessionGoal::NvisLocal);
    for html in [
        render_standalone_html(&nvis).unwrap(),
        render_compact_summary_html(&nvis).unwrap(),
    ] {
        assert!(html.contains("NVIS-oriented distance proxy"));
        assert!(html.contains("Distance does not establish NVIS propagation"));
        for bin in ReportDistanceBin::ALL {
            assert!(html.contains(bin.label()));
        }
    }

    let single = single_antenna_report();
    for html in [
        render_standalone_html(&single).unwrap(),
        render_compact_summary_html(&single).unwrap(),
    ] {
        assert!(html.contains("This session profiles one antenna."));
        assert!(html.contains("Comparative signal and detection questions do not apply"));
        assert!(!html.contains("A/B"));
        assert!(!html.contains("antenna winner"));
        assert!(!html.contains("id=\"same-path-signal\""));
        assert!(!html.contains("id=\"reporter-activity\""));
    }
}

#[test]
fn renders_complete_accessible_paired_diagnostics_without_conclusions() {
    let mut report = paired_report(true);
    report.comparison.diagnostics.unmatched_left_count = 3;
    report.comparison.diagnostics.unmatched_right_count = 1;
    report.comparison.diagnostics.missing_snr_left_count = 2;
    report.comparison.diagnostics.exact_duplicate_count = 4;
    report
        .comparison
        .diagnostics
        .conflicting_duplicate_group_count = 2;

    let html = render_standalone_html(&report).unwrap();

    for section in [
        "Coverage and data-quality counts",
        "Path overlap and missingness",
        "Data-quality timeline",
        "Matched-pair difference distribution",
        "Matched SNR over time",
        "Distance and azimuth path context",
        "Comparison-group descriptive summaries",
    ] {
        assert!(html.contains(section), "missing section: {section}");
    }
    for caption in [
        "Path overlap and missingness data",
        "Data-quality timeline details",
        "Matched-pair difference data",
        "Matched SNR over time data",
        "Observed distance path-context data",
        "Observed azimuth path-context data",
        "Comparison-group summary data",
    ] {
        assert!(html.contains(&format!("<caption>{caption}</caption>")));
    }
    for fact in [
        "Signed values:",
        "Positive values mean B was stronger; negative values mean A was stronger.",
        "TX path · 20 m · WSPR · Local decode · WSJT-X log",
        "A then B",
        "B then A",
        "Unmatched — A",
        "Missing SNR — A",
        "Missing or invalid mode",
        "Exact duplicates collapsed",
        "Conflicting duplicate groups",
        "Alternating antennas reduces but does not eliminate time and propagation effects.",
    ] {
        assert!(html.contains(fact), "missing fact: {fact}");
    }
    for prohibited in [
        "statistically significant",
        "confidence interval",
        "equivalent antennas",
        "better antenna",
    ] {
        assert!(!html.contains(prohibited));
    }
}

#[test]
fn renders_bounded_same_path_and_reach_views_with_equivalent_tables() {
    let mut report = paired_report(true);
    report.overview.scope.delta_orientation = Some(antennabench_analysis::DeltaOrientation {
        subtrahend_label: "Antenna <negative> with a deliberately long operator label".into(),
        minuend_label: "Antenna & positive with another deliberately long label".into(),
    });
    let negative_path = {
        let mut path = report.overview.strata[0].path_median_deltas[0].clone();
        path.remote_path = "K3NEGATIVE".to_string();
        path.median_delta_right_minus_left_db = -3.0;
        path
    };
    report.overview.strata[0]
        .path_median_deltas
        .push(negative_path);
    let html = render_standalone_html(&report).unwrap();

    assert!(
        html.contains("Positive values mean Antenna &#38; positive with another deliberately long label was stronger; negative values mean Antenna &#60;negative&#62; with a deliberately long operator label was stronger.")
    );
    assert_eq!(
        html.matches("class=\"path-distribution-dot-group\"")
            .count(),
        3
    );
    for class in ["path-dot-negative", "path-dot-zero", "path-dot-positive"] {
        assert!(html.contains(class), "missing {class} distribution state");
    }
    assert!(html.contains("viewBox=\"0 0 720 220\""));
    assert!(html.contains("class=\"path-distribution-tick\""));
    assert!(html.contains("class=\"path-distribution-tick-label\""));
    assert!(html.contains("K3NEGATIVE: -3 dB median across"));
    assert!(html.contains("tabindex=\"0\" role=\"img\" aria-label=\"K3NEGATIVE:"));
    assert!(!html.contains("class=\"path-distribution-iqr\""));
    assert!(!html.contains("class=\"path-distribution-median\""));
    assert!(html.contains("<dt>Tied at 0 dB</dt><dd>1</dd>"));
    assert!(html.contains("<dt>Middle half</dt>"));
    assert!(!html.contains(" style=\""));
    assert!(html.contains("a 0 dB dot is retained as a true zero"));
    assert!(html.contains("<caption>One path-median signed SNR delta per remote path"));
    assert!(html.contains("<details class=\"audit-disclosure path-detail-disclosure\"><summary>Review exact remote paths and matched-pair counts"));
    assert!(html.contains("See which paths contributed, how many matched pairs support each path median, and the exact delta behind each dot."));
    assert!(html.contains("<td>K1PAIR</td><td>2</td>"));
    assert!(html.contains("<td>K2SPARSE</td><td>1</td><td>0 dB</td>"));
    assert!(html.contains("A-only and B-only paths remain visible"));
    assert!(html.contains("<caption>Unique remote-path reach counts"));
    assert!(html.contains("class=\"coverage-polar path-distribution-chart\""));
    assert!(html.contains("@media(max-width:620px)"));
    assert!(html.contains("@media print"));

    let compact = render_compact_summary_html(&report).unwrap();
    assert_eq!(
        compact
            .matches("class=\"path-distribution-dot-group\"")
            .count(),
        3
    );
    assert!(compact.contains("Review exact remote paths and matched-pair counts"));
    assert!(compact.contains("<td>K2SPARSE</td><td>1</td><td>0 dB</td>"));
}

#[test]
fn renders_missing_and_unavailable_same_path_states_without_zeroing_them() {
    let mut report = paired_report(true);
    let row = &mut report.overview.strata[0];
    row.path_median_deltas.clear();
    row.missing_snr_left_count = 2;
    row.missing_snr_right_count = 1;
    row.reach = Default::default();

    let html = render_standalone_html(&report).unwrap();

    assert!(
        html.contains("No usable same-path signal reports are available across 1 comparison group")
    );
    assert!(html.contains("Missing SNR remains separate (A: 2, B: 1). This is not a 0 dB result"));
    assert!(html.contains("No usable path-reach signal reports in 1 of 1 comparison groups"));
    assert!(!html.contains("absent reach is not zero-SNR evidence"));
}

#[test]
fn collapses_empty_strata_without_hiding_mixed_availability() {
    let empty_report = canonical_report();
    let empty_html = render_standalone_html(&empty_report).unwrap();

    assert_eq!(empty_report.overview.strata.len(), 8);
    assert!(empty_html.contains("No path delta in 8 comparison groups"));
    assert_eq!(empty_html.matches("<div class=\"path-strip\"").count(), 0);
    let populated_reach_strata = empty_report
        .overview
        .strata
        .iter()
        .filter(|row| {
            row.reach.left_only_unique_path_count
                + row.reach.both_unique_path_count
                + row.reach.right_only_unique_path_count
                > 0
        })
        .count();
    assert_eq!(
        empty_html.matches("<div class=\"reach-strip\"").count(),
        populated_reach_strata
    );
    assert_eq!(
        empty_html
            .matches("<div class=\"location-context\"")
            .count(),
        0
    );
    assert!(empty_html.matches("No observed paired paths").count() <= 4);
    assert!(empty_html.split_whitespace().count() < 10_000);

    let mut mixed_report = paired_report(true);
    let mut empty_stratum = mixed_report.overview.strata[0].clone();
    empty_stratum.stratum.direction = antennabench_analysis::PathDirection::Receive;
    empty_stratum.stratum.band = Band::M40;
    empty_stratum.stratum.observation_kind = ObservationKind::PublicReport;
    empty_stratum.stratum.source = RecordSource::Wsprnet;
    empty_stratum.path_delta = ReportOverviewPathDelta::Unavailable;
    empty_stratum.path_median_deltas.clear();
    empty_stratum.unique_path_count = 0;
    empty_stratum.paired_row_count = 0;
    empty_stratum.contributing_block_count = 0;
    empty_stratum.reach = Default::default();
    empty_stratum.observed_profile = Default::default();
    empty_stratum.location_context.paths.clear();
    empty_stratum.location_context.missing_location_path_count = 0;
    empty_stratum
        .location_context
        .inconsistent_location_path_count = 0;
    mixed_report.overview.strata.push(empty_stratum);

    let mixed_html = render_standalone_html(&mixed_report).unwrap();
    assert_eq!(
        mixed_html
            .matches("<div class=\"path-distribution\"")
            .count(),
        1
    );
    assert_eq!(mixed_html.matches("<div class=\"reach-strip\"").count(), 1);
    assert_eq!(
        mixed_html
            .matches("<section aria-labelledby=\"path-context-")
            .count(),
        1
    );
    assert!(mixed_html.contains("No usable same-path signal reports in 1 of 2 comparison groups"));
    assert!(mixed_html.contains("No usable path-reach signal reports in 1 of 2 comparison groups"));
    assert!(mixed_html.contains("No located matched paths in 1 of 2 comparison groups"));
    assert!(mixed_html.contains("RX path · 40 m · WSPR · Public report · WSPRnet"));
    assert!(mixed_html.contains("never combined"));

    let compact_html = render_compact_summary_html(&mixed_report).unwrap();
    assert!(compact_html.contains("No path delta in 1 comparison group"));
    assert!(compact_html.contains("No usable same-path path-median delta in 1 of 2"));
    assert!(compact_html.contains("No usable observed footprint in 1 of 2 comparison groups"));
    assert!(compact_html.contains("never combined"));
}

#[test]
fn renders_stratified_location_context_missingness_and_concentration() {
    let mut report = paired_report(true);
    report.comparison.paired_rows[0].left_remote_grid = None;
    report.comparison.paired_rows[0].right_remote_grid = None;
    report.comparison.paired_rows[0].left_distance_km = None;
    report.comparison.paired_rows[0].right_distance_km = None;
    report.comparison.paired_rows[0].left_azimuth_degrees = None;
    report.comparison.paired_rows[0].right_azimuth_degrees = None;
    let mut second_stratum = report.comparison.paired_rows[1].clone();
    second_stratum.stratum.direction = antennabench_analysis::PathDirection::Receive;
    second_stratum.stratum.band = Band::M40;
    second_stratum.stratum.observation_kind = ObservationKind::PublicReport;
    second_stratum.stratum.source = RecordSource::Wsprnet;
    second_stratum.remote_path = "K9SECOND".to_string();
    second_stratum.left_distance_km = Some(900.0);
    second_stratum.right_distance_km = Some(905.0);
    second_stratum.left_azimuth_degrees = Some(10.0);
    second_stratum.right_azimuth_degrees = Some(12.0);
    report.comparison.paired_rows.push(second_stratum);

    let html = render_standalone_html(&report).unwrap();

    assert!(html.contains("Location unavailable"));
    assert!(html.contains("Unique paths in stratum"));
    assert!(html.contains("Unique paths with location"));
    assert!(html.contains("Most populated 45° display sector"));
    assert!(html.contains("TX path · 20 m · WSPR · Local decode · WSJT-X log"));
    assert!(html.contains("RX path · 40 m · WSPR · Public report · WSPRnet"));
    assert!(html.contains("<caption>Observed distance path-context data</caption>"));
    assert!(html.contains("<caption>Observed azimuth path-context data</caption>"));
    assert!(html.contains("Distance and azimuth describe only the remote paths observed"));
    assert!(!html.contains("<script"));
    assert!(!html.contains("http://"));
    assert!(!html.contains("https://"));

    let empty = render_standalone_html(&canonical_report()).unwrap();
    assert!(empty.contains("id=\"distance-direction\""));
    assert!(empty.contains("Receiver/transmitter availability may have changed"));
}

#[test]
fn renders_fixed_path_context_tables_with_equivalent_visual_states() {
    let mut report = paired_report(true);
    let context = &mut report.overview.strata[0].location_context;
    context.distance_bins = ReportDistanceBin::ALL
        .into_iter()
        .map(|category| ReportOverviewLocationCell {
            category,
            unique_located_path_count: usize::from(category == ReportDistanceBin::Under500Km),
            paired_row_count: usize::from(category == ReportDistanceBin::Under500Km),
            median_path_delta_right_minus_left_db: (category == ReportDistanceBin::Under500Km)
                .then_some(0.0),
        })
        .collect();
    context.azimuth_sectors = ReportAzimuthSector::ALL
        .into_iter()
        .map(|category| ReportOverviewLocationCell {
            category,
            unique_located_path_count: usize::from(category == ReportAzimuthSector::NorthEast),
            paired_row_count: usize::from(category == ReportAzimuthSector::NorthEast),
            median_path_delta_right_minus_left_db: (category == ReportAzimuthSector::NorthEast)
                .then_some(0.0),
        })
        .collect();

    let html = render_standalone_html(&report).unwrap();

    assert!(html.contains("not a radiation pattern, propagation model, or causal conclusion"));
    assert!(html.contains("Fixed distance bins for observed paired paths"));
    assert!(html.contains("Fixed 45° azimuth sectors for observed paired paths"));
    assert!(html.contains("0 dB (near-zero)"));
    assert!(html.contains("Sparse evidence: 1 path(s), 1 row(s)"));
    assert!(html.contains("No observed paired paths"));
    assert!(html.matches("Near / local proxy (under 500 km)").count() >= 2);
    assert!(html.matches("NE (22.5°–67.5°)").count() >= 2);
}

#[test]
fn renders_all_path_profiles_in_full_and_compact_without_overclaiming() {
    let report = canonical_report();
    let full = render_standalone_html(&report).unwrap();
    let compact = render_compact_summary_html(&report).unwrap();

    assert!(full.contains("Observed distance and direction profile"));
    assert!(full.contains("Side-by-side observed distance distribution"));
    assert!(full.contains("Side-by-side observed azimuth distribution"));
    assert!(full.contains("Observed-path composition within each distance category"));
    assert!(full.contains("Receiver/transmitter availability may have changed"));
    assert!(full.contains("not a controlled detection comparison"));
    assert!(full.contains("not a radiation pattern"));
    assert!(full.contains("Near / local distance is a practical proxy only"));
    assert!(!full.contains("measured NVIS"));
    assert!(!full.contains("universal advantage"));
    for expected in [
        "Observed footprint",
        "Observed unique paths by distance",
        "Observed unique paths by direction",
        "Paired bars share one scale",
        "Unlike common-active receiver detection",
        "Review observed distance and direction profile",
        "Review exact unique observed-path rows",
    ] {
        assert!(
            compact.contains(expected),
            "missing compact footprint: {expected}"
        );
    }
    let profile_disclosure = compact
        .find("<summary>Review observed distance and direction profile</summary>")
        .expect("observed profile is disclosed");
    assert!(
        profile_disclosure
            < compact
                .find("Observed unique paths by distance")
                .expect("distance profile remains available")
    );
    assert!(!compact.contains("Observed complementarity"));
    assert!(!compact.contains("<h2 id=\"coverage-overlap-title\">"));
    assert!(compact.contains("Review whether observed paths repeated across blocks"));
    assert!(full.contains("Exact unique observed-path records"));
    assert!(full.contains("Review shared-path distance and direction context"));
    assert!(compact.contains("Exact unique observed-path records"));
    assert!(!compact.contains("Review exact paired-row distance"));
}

#[test]
fn renders_derived_solar_context_with_method_and_non_causal_caveat() {
    let report = paired_report(true);
    let html = render_standalone_html(&report).unwrap();

    for expected in [
        "Derived solar context",
        "noaa-gml-fractional-year",
        "maidenhead-cell-center-v1",
        "Derived station and remote-endpoint solar context",
        "They are not captured propagation observations",
        "do not establish a cause",
        "Station:",
        "Remote:",
    ] {
        assert!(html.contains(expected), "missing solar context: {expected}");
    }
    assert!(!html.contains("solar score"));
    assert!(!html.contains("caused the difference"));
}

#[test]
fn run_quality_state_matrix_has_exact_accessible_audit_equivalents() {
    let mut report = canonical_report();
    let rows = &mut report.overview.timeline;
    rows[0].status = AlignedSlotStatus::Switched;
    rows[1].status = AlignedSlotStatus::LateSwitch;
    rows[2].status = AlignedSlotStatus::UnknownActualState;
    rows[2].actual_antenna = None;
    rows[2].attribution = Some(ReportWsprAttribution::UnknownAntennaOccupancy);
    rows[3].status = AlignedSlotStatus::Missed;
    rows[4].status = AlignedSlotStatus::Bad;
    rows[5].readiness_basis = Some(ReportWsprReadinessBasis::CommandVerified);
    rows[5].attribution = Some(ReportWsprAttribution::Attributable);
    let corrected_at = rows[5].planned_starts_at + Duration::seconds(12);
    let correction = ReportOperatorEvent {
        event_id: "event-correction-audit".into(),
        occurred_at: corrected_at,
        slot_id: None,
        affected_slot_id: Some(rows[5].item_id.clone()),
        kind: ReportOperatorEventKind::EventCorrected,
        detail: Some("Actual antenna corrected from operator log".into()),
        correction: Some(ReportEventCorrection {
            target_event_id: "event-original-audit".into(),
            action: ReportEventCorrectionAction::Replaced,
            reason: "Operator reviewed the station log".into(),
            applied: true,
        }),
    };
    rows[5].event_history.push(correction.clone());
    report.snapshot.operator_events.push(correction);
    let rejected_correction = ReportOperatorEvent {
        event_id: "event-rejected-correction".into(),
        occurred_at: rows[0].planned_starts_at + Duration::seconds(8),
        slot_id: None,
        affected_slot_id: Some(rows[0].item_id.clone()),
        kind: ReportOperatorEventKind::EventCorrected,
        detail: None,
        correction: Some(ReportEventCorrection {
            target_event_id: "missing-target".into(),
            action: ReportEventCorrectionAction::Retracted,
            reason: "Target was not present".into(),
            applied: false,
        }),
    };
    rows[0].event_history.push(rejected_correction.clone());
    report.snapshot.operator_events.push(rejected_correction);
    let interrupted_at = rows[6].planned_starts_at + Duration::seconds(20);
    report.snapshot.lifecycle = Some(SessionLifecycleV2::Abandoned);
    report.snapshot.lifecycle_events.extend([
        ReportLifecycleEvent {
            kind: ReportLifecycleEventKind::Interrupted,
            occurred_at: interrupted_at,
            detail: Some("Power interruption".into()),
        },
        ReportLifecycleEvent {
            kind: ReportLifecycleEventKind::Resumed,
            occurred_at: interrupted_at + Duration::seconds(40),
            detail: Some("Power restored".into()),
        },
        ReportLifecycleEvent {
            kind: ReportLifecycleEventKind::Abandoned,
            occurred_at: rows[7].planned_ends_at,
            detail: Some("Run abandoned after recurrence".into()),
        },
    ]);
    report.snapshot.adapter_evidence.workflow_status = ReportAcquisitionWorkflowStatus::Incomplete;
    report.snapshot.adapter_evidence.gap_count = 1;
    report.snapshot.adapter_evidence.malformed_count = 2;
    report.snapshot.adapter_evidence.duplicate_count = 3;
    report.snapshot.adapter_evidence.conflict_count = 1;

    let html = render_standalone_html(&report).unwrap();

    for state in [
        "Switched",
        "Late switch",
        "Unknown occupancy",
        "Missed",
        "Bad",
        "Corrected",
        "Interrupted",
        "Resumed",
        "Abandoned",
        "Command verified",
        "Full antenna occupancy recorded",
        "Explicit acquisition gap",
        "2 malformed",
        "3 duplicate",
        "1 conflict",
    ] {
        assert!(html.contains(state), "missing matrix state: {state}");
    }
    for exact in [
        "event-correction-audit",
        "event-original-audit",
        "event-rejected-correction",
        "not applied",
        "Actual antenna corrected from operator log",
        "Operator reviewed the station log",
        "Power interruption",
        "Power restored",
        "Run abandoned after recurrence",
        "Complete operator note and correction history",
        "Every excluded observation retained by the report projection",
    ] {
        assert!(html.contains(exact), "missing exact audit detail: {exact}");
    }
    for class in [
        "state-late",
        "state-missed",
        "state-bad",
        "state-unknown",
        "state-interrupted",
        "state-corrected",
    ] {
        assert!(html.contains(class), "missing non-color state: {class}");
    }
    assert_eq!(
        html.matches("<details class=\"state-corrected\">").count(),
        1
    );
    assert!(html.contains("@media print{.answerability-table{display:block}"));
}

fn plain_language_answer_from(html: &str) -> &str {
    let marker = "<p class=\"answer plain-language-answer\">";
    let start = html
        .find(marker)
        .map(|index| index + marker.len())
        .expect("plain-language answer should render");
    let end = start
        + html[start..]
            .find("</p>")
            .expect("plain-language answer should close");
    &html[start..end]
}

fn canonical_report() -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/inconclusive-sample-report.session.wsprabundle");
    let bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("canonical sample should be valid");
    build_report(&bundle).expect("canonical sample should build report data")
}

fn paired_report(balanced_order: bool) -> SessionReport {
    paired_report_for_goal(balanced_order, SessionGoal::GeneralCoverage)
}

fn paired_report_for_goal(balanced_order: bool, goal: SessionGoal) -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let mut bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("minimal sample should be valid");
    bundle.events.clear();
    bundle.schedule.goal = goal;
    if balanced_order {
        bundle.schedule.slots[2].antenna_label = "B".to_string();
        bundle.schedule.slots[3].antenna_label = "A".to_string();
    }
    let template = bundle.observations[0].clone();
    bundle.observations = [
        ("pair-1-left", 0, "K1PAIR", -22.0),
        ("pair-1-right", 1, "K1PAIR", -19.0),
        ("pair-2-right", 2, "K1PAIR", -18.0),
        ("pair-2-left", 3, "K1PAIR", -21.0),
        ("sparse-left", 0, "K2SPARSE", -20.0),
        ("sparse-right", 1, "K2SPARSE", -20.0),
    ]
    .into_iter()
    .map(|(id, slot_index, remote, snr)| {
        let slot = &bundle.schedule.slots[slot_index];
        let mut observation = template.clone();
        observation.observation_id = id.to_string();
        observation.meta.timestamp = slot.starts_at + Duration::seconds(30);
        observation.band = slot.band;
        observation.reporter_call = Some(remote.to_string());
        observation.heard_call = Some(bundle.station.callsign.clone());
        observation.snr_db = Some(snr);
        observation.slot_id = None;
        observation.slot_label = None;
        observation.slot_confidence = None;
        observation
    })
    .collect();
    let bundle = normalize_bundle(bundle);
    build_report(&bundle).expect("paired sample should build a report")
}

fn single_antenna_report() -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let mut bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("minimal sample should be valid");
    bundle.events.clear();
    bundle.schedule.mode = ExperimentMode::SingleAntennaProfiling;
    bundle.schedule.goal = SessionGoal::SingleAntennaProfiling;
    bundle.antennas.antennas.truncate(1);
    let label = bundle.antennas.antennas[0].label.clone();
    for slot in &mut bundle.schedule.slots {
        slot.antenna_label.clone_from(&label);
    }
    let bundle = normalize_bundle(bundle);
    build_report(&bundle).expect("single-antenna sample should build a report")
}

fn section_positions(html: &str, ids: &[&str]) -> Vec<usize> {
    ids.iter()
        .map(|id| {
            html.find(&format!("<section id=\"{id}\""))
                .unwrap_or_else(|| panic!("missing section {id}"))
        })
        .collect()
}
