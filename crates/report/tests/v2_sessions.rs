use std::path::Path;

use antennabench_core::{
    codes, AlignedSlotStatus, EventTimeBasisV2, MutationMember, OperatorEventPayloadV2,
    OperatorEventV2,
};
use antennabench_report::{build_report, build_report_with_validation, render_standalone_html};
use antennabench_storage::BundleStore;

#[test]
fn upgraded_v2_fixtures_analyze_and_render_through_the_shared_projection() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles");
    let mut sources = std::fs::read_dir(fixtures)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".session.wsprabundle"))
        .collect::<Vec<_>>();
    sources.sort();

    for source in sources {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("upgraded.session.antennabundle");
        let store = BundleStore::new(&source)
            .upgrade_v1_to_v2(destination)
            .unwrap();
        let bundle = store.read_normalized_validated().unwrap();
        let report = build_report(&bundle).unwrap();
        let html = render_standalone_html(&report).unwrap();
        assert_eq!(report.context.session_id, bundle.manifest.session_id);
        assert!(html.contains(&bundle.manifest.session_id));
    }
}

#[test]
fn v2_conflicting_operator_facts_are_disclosed_and_conservatively_excluded() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/session-bundles");
    let source = fixtures.join("analysis-rich-whole-station.session.wsprabundle");
    let temp = tempfile::tempdir().unwrap();
    let baseline = temp.path().join("baseline.session.antennabundle");
    let baseline_store = BundleStore::new(source)
        .upgrade_v1_to_v2(&baseline)
        .unwrap();
    let mut bundle = baseline_store.read_v2().unwrap();
    let template = bundle.events[0].clone();
    bundle.events.push(OperatorEventV2 {
        meta: antennabench_core::RecordMetaV2 {
            mutation: MutationMember {
                mutation_id: "mutation-conflicting-missed".into(),
                member_index: 0,
                member_count: 1,
            },
            recorded_at: template.meta.recorded_at + chrono::Duration::seconds(1),
            ..template.meta
        },
        event_id: "conflicting-missed".into(),
        occurred_at: template.occurred_at + chrono::Duration::seconds(1),
        time_basis: EventTimeBasisV2::ObservedNow,
        uncertainty_seconds: None,
        slot_id: Some("slot-001".into()),
        payload: OperatorEventPayloadV2::SlotMissed {
            reason: Some("operator reported uncertainty".into()),
        },
    });
    for observation in bundle
        .observations
        .iter_mut()
        .filter(|observation| observation.slot_id.as_deref() == Some("slot-001"))
    {
        observation.slot_label = None;
        observation.slot_confidence = Some(0.0);
    }
    BundleStore::refresh_v2_checkpoint(&mut bundle).unwrap();

    let authored = temp.path().join("conflict.session.antennabundle");
    let store = BundleStore::new(&authored);
    store.write_v2(&bundle).unwrap();
    let (bundle, validation) = store.read_for_analysis().unwrap();
    let report = build_report_with_validation(&bundle, &validation).unwrap();

    let slot = report
        .evidence
        .slots
        .iter()
        .find(|slot| slot.slot_id == "slot-001")
        .unwrap();
    assert_eq!(slot.status, AlignedSlotStatus::ConflictingEvidence);
    assert_eq!(slot.actual_label, None);
    assert!(report
        .eligibility_exclusions
        .iter()
        .any(|exclusion| exclusion.code == codes::V2_EVENT_CONFLICT));
}
