use std::path::PathBuf;

use antennabench_analysis::{EvidenceQuality, ObservationCounts};
use antennabench_report::{build_report, render_standalone_html, ReportNotice, SessionReport};
use antennabench_storage::BundleStore;

#[test]
fn renders_the_canonical_report_as_deterministic_offline_html() {
    let report = canonical_report();

    let first = render_standalone_html(&report);
    let second = render_standalone_html(&report);

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
        "Antenna SNR chart data",
        "Evidence by antenna",
        "Band evidence chart data",
        "Evidence details by band",
        "Slot evidence chart data",
        "Evidence details by slot",
    ] {
        assert!(first.contains(&format!("<caption>{caption}</caption>")));
    }
    assert_eq!(first.matches("aria-hidden=\"true\"").count(), 3);
    assert!(first.contains("<dt>Usable</dt><dd>19</dd>"));
    assert!(first.contains("<dt>Excluded</dt><dd>7</dd>"));
    assert!(first.contains("<dt>Scheduled bands</dt><dd>40 m, 20 m</dd>"));
    assert!(first.contains("<dt>Scheduled slots</dt><dd>12</dd>"));
    assert!(first.contains("This report is descriptive and does not select an antenna winner."));
}

#[test]
fn escapes_every_untrusted_report_string() {
    let mut report = canonical_report();
    let hostile = "\"><script>alert('x') & imported</script>".to_string();

    report.context.session_id = hostile.clone();
    report.context.station.callsign = hostile.clone();
    report.context.station.grid = hostile.clone();
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

    let html = render_standalone_html(&report);

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

    let html = render_standalone_html(&report);

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

fn canonical_report() -> SessionReport {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles/canonical-sample-report.session.wsprabundle");
    let bundle = BundleStore::new(fixture)
        .read_normalized_validated()
        .expect("canonical sample should be valid");
    build_report(&bundle).expect("canonical sample should build report data")
}
