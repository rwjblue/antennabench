use std::path::PathBuf;

use antennabench_analysis::{ComparisonAvailability, EvidenceQuality, ObservationCounts};
use antennabench_core::{
    normalize_bundle, AntennaControlDispositionV5, AntennaControlOutputEncodingV5,
    AntennaControlOutputV5, AntennaControlRoleV5, Band, ExperimentMode, ObservationKind,
    RecordSource, SessionLifecycleV2, WsprCycleDirection,
};
use antennabench_report::{
    build_report, render_standalone_html, ReportAdapterEvidence, ReportAntennaControlAttempt,
    ReportImportedEvidence, ReportLifecycleEvent, ReportLifecycleEventKind, ReportNotice,
    ReportOverviewLimitation, ReportSnapshotContext, ReportWsprAttribution, ReportWsprCycle,
    ReportWsprReadinessBasis, SessionReport,
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
        "Same-path signal",
        "Reach and unique paths",
        "Distance and direction",
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
        "same-path-signal",
        "reach-unique-paths",
        "distance-direction",
        "run-quality",
        "audit-appendix",
    ] {
        assert!(first.contains(&format!("href=\"#{anchor}\"")));
        assert!(first.contains(&format!("id=\"{anchor}\"")));
    }
    assert!(first.contains("<details class=\"audit-disclosure\">"));
    assert!(first.contains("details:not([open])>:not(summary){display:none!important}"));
    assert!(first.contains("break-after:page"));
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
            evidence_complete: false,
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
                completeness_known: false,
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
    assert!(html.contains("recovered &lt;without inventing evidence&gt;"));
}

#[test]
fn renders_successful_public_collection_without_turning_a_source_boundary_into_a_gap() {
    let mut report = canonical_report();
    report.snapshot = ReportSnapshotContext {
        checkpoint_revision: Some(19),
        lifecycle: Some(SessionLifecycleV2::Ended),
        lifecycle_events: Vec::new(),
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
            evidence_complete: true,
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
                    completeness_known: false,
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
                    completeness_known: true,
                },
            ],
        },
    };

    let html = render_standalone_html(&report).unwrap();
    assert!(html.contains("No recorded acquisition gaps"));
    assert!(
        html.contains("Best-effort public collection completed for 1 recorded requested window(s)")
    );
    assert!(html.contains("best-effort WSPR.live request-window collection; upstream mirror has no independent completeness guarantee"));
    assert!(html.contains("wsjtx-udp / direct-local"));
    assert!(html.contains("upstream completeness guarantee recorded"));
    assert!(!html.contains("Complete within recorded adapter scope"));
}

#[test]
fn renders_readiness_basis_and_bounded_command_diagnostics() {
    let mut report = canonical_report();
    let ready_at = "2026-07-14T22:01:00Z".parse().unwrap();
    report.snapshot = ReportSnapshotContext {
        checkpoint_revision: Some(18),
        lifecycle: Some(SessionLifecycleV2::Running),
        lifecycle_events: Vec::new(),
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
                data: "verified".into(),
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

    let html = render_standalone_html(&report).unwrap();
    assert!(html.contains("Command verified"));
    assert!(html.contains("Antenna-control command attempts"));
    assert!(html.contains("Transmit-focused"));
    assert!(html.contains("switch &lt;profile&gt;"));
    assert!(html.contains("truncated=true"));
    assert!(html.contains("[1]=&quot;relay-a&quot;"));
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
        html.contains("&quot;&gt;&lt;script&gt;alert(&#39;x&#39;) &amp; imported&lt;/script&gt;")
    );
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
fn renders_every_comparison_availability_before_difference_output() {
    for (availability, label, explanation) in [
        (
            ComparisonAvailability::NotApplicable,
            "Not applicable",
            "Single-antenna profiling does not create an A/B comparison.",
        ),
        (
            ComparisonAvailability::UnsupportedComparisonShape,
            "Unsupported comparison shape",
            "A paired comparison requires exactly two scheduled antenna labels.",
        ),
        (
            ComparisonAvailability::NoEligibleBlocks,
            "No eligible blocks",
            "No adjacent same-band block contained one usable actual slot for each label.",
        ),
        (
            ComparisonAvailability::NoMatchedPaths,
            "No matched paths",
            "Eligible blocks exist, but no same-stratum remote path had finite SNR under both labels.",
        ),
        (
            ComparisonAvailability::DescriptivePairsAvailable,
            "Descriptive pairs available",
            "Finite same-path paired rows are available for descriptive display only.",
        ),
    ] {
        let mut report = paired_report(false);
        report.comparison.availability = availability;
        report.overview.comparison_availability = availability;

        let html = render_standalone_html(&report).unwrap();
        let availability_position = html
            .find("Comparison availability")
            .expect("availability should render");
        let difference_position = html
            .find("Paired difference distribution")
            .expect("difference section should render");

        assert!(availability_position < difference_position);
        assert!(html.contains(&format!("<span class=\"badge\">{label}</span>")));
        assert!(html.contains(explanation));
    }
}

#[test]
fn renders_answer_first_order_unavailable_states_and_visible_limitations() {
    let mut report = paired_report(true);
    report.overview.comparison_availability = ComparisonAvailability::NotApplicable;
    report.overview.scope.delta_orientation = None;
    report.overview.strata.clear();
    report.overview.limitations = vec![ReportOverviewLimitation::ComparisonNotApplicable];
    report.snapshot.adapter_evidence.evidence_complete = false;
    report.snapshot.adapter_evidence.gap_count = 2;
    report.notices.push(ReportNotice::DetailOmitted {
        family: antennabench_report::ReportDetailFamily::PairedObservations,
        row_count: 42,
    });

    let html = render_standalone_html(&report).unwrap();
    let overview = html.find("id=\"what-run-show\"").unwrap();
    let same_path = html.find("id=\"same-path-signal\"").unwrap();
    let omission = html
        .find("full paired observation detail is omitted")
        .unwrap();
    let run_quality = html.find("id=\"run-quality\"").unwrap();
    let first_details = html.find("<details").unwrap();

    assert!(overview < same_path);
    assert!(run_quality < omission);
    assert!(html.find("2 recorded acquisition gaps").unwrap() < first_details);
    assert!(html.contains("Delta orientation:</strong> unavailable"));
    assert!(html.contains("No comparison strata are available for this run."));
    assert!(html.contains("A/B comparison: not established for single-antenna profiling."));
    assert!(!html.contains("Winner:"));
    assert!(!html.contains("antenna gain"));
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
        "Paired difference distribution",
        "Paired SNR over time",
        "Distance and azimuth path context",
        "Stratum descriptive summaries",
    ] {
        assert!(html.contains(section), "missing section: {section}");
    }
    for caption in [
        "Path overlap and missingness data",
        "Data-quality timeline details",
        "Paired difference data",
        "Paired SNR over time data",
        "Observed distance path-context data",
        "Observed azimuth path-context data",
        "Stratum summary data",
    ] {
        assert!(html.contains(&format!("<caption>{caption}</caption>")));
    }
    for fact in [
        "Delta orientation:",
        "B minus A (right minus left)",
        "TX path · 20 m · WSPR · Local decode · WSJT-X log",
        "Left then right",
        "Right then left",
        "Unmatched left",
        "Missing SNR left",
        "Missing or invalid mode",
        "Exact duplicates collapsed",
        "Conflicting duplicate groups",
        "Adjacent switched slots reduce elapsed time but do not remove propagation or time confounding.",
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
    let report = paired_report(true);
    let html = render_standalone_html(&report).unwrap();

    assert!(html.contains("each value is <strong>B − A</strong> SNR in dB"));
    assert_eq!(html.matches("<span class=\"path-strip-dot\"").count(), 2);
    assert_eq!(html.matches("<span class=\"path-strip-median\"").count(), 1);
    assert!(html.contains("A finite 0 dB dot is retained as a true zero"));
    assert!(html.contains("<caption>One path-median B − A SNR delta per remote path"));
    assert!(html.contains("<td>K1PAIR</td><td>2</td>"));
    assert!(html.contains("<td>K2SPARSE</td><td>1</td><td>0 dB</td>"));
    assert!(html.contains("<strong>not</strong> zero-SNR measurements."));
    assert!(html.contains("<caption>Unique finite remote-path reach counts"));
    assert!(html.contains(".path-strip-row{grid-template-columns:1fr}"));
    assert!(html.contains("@media print"));
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

    assert!(html.contains("No finite same-path delta is available; missing SNR is retained separately (left: 2, right: 1). This is not a 0 dB result."));
    assert!(html.contains(
        "No finite path reach counts; missing SNR is retained separately (left: 2, right: 1)."
    ));
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
    assert!(empty.contains("No paired rows are available for location views."));
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

fn canonical_report() -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    let bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("canonical sample should be valid");
    build_report(&bundle).expect("canonical sample should build report data")
}

fn paired_report(balanced_order: bool) -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/minimal-whole-station.session.wsprabundle");
    let mut bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("minimal sample should be valid");
    bundle.events.clear();
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
