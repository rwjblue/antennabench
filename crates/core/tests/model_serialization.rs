use antennabench_core::{
    AnalysisFile, AnalysisStatus, Antenna, AntennasFile, Band, BundleContents, BundleFiles,
    BundleManifest, ExperimentMode, PlannedSlot, Schedule, SessionGoal, Station,
};
use chrono::{TimeZone, Utc};
use serde_json::json;

#[test]
fn serializes_minimum_station_and_schedule_shape() {
    let station = Station {
        schema_version: 1,
        session_id: "session-001".to_string(),
        callsign: "K1ABC".to_string(),
        grid: "FN42".to_string(),
        power_watts: None,
        operator_notes: None,
    };

    assert_eq!(
        serde_json::to_value(&station).unwrap(),
        json!({
            "schema_version": 1,
            "session_id": "session-001",
            "callsign": "K1ABC",
            "grid": "FN42",
            "power_watts": null,
            "operator_notes": null
        })
    );

    let schedule = Schedule {
        schema_version: 1,
        session_id: "session-001".to_string(),
        mode: ExperimentMode::WholeStationAb,
        goal: SessionGoal::WeakSignalReliability,
        slots: vec![PlannedSlot {
            slot_id: "slot-001".to_string(),
            sequence_number: 1,
            starts_at: Utc.with_ymd_and_hms(2026, 7, 9, 20, 0, 0).unwrap(),
            duration_seconds: 900,
            guard_seconds: 30,
            band: Band::M20,
            antenna_label: "dipole".to_string(),
        }],
    };

    assert_eq!(
        serde_json::to_value(&schedule).unwrap(),
        json!({
            "schema_version": 1,
            "session_id": "session-001",
            "mode": "whole_station_ab",
            "goal": "weak_signal_reliability",
            "slots": [{
                "slot_id": "slot-001",
                "sequence_number": 1,
                "starts_at": "2026-07-09T20:00:00Z",
                "duration_seconds": 900,
                "guard_seconds": 30,
                "band": "20m",
                "antenna_label": "dipole"
            }]
        })
    );
}

#[test]
fn bundle_contents_groups_required_bundle_files() {
    let files = BundleFiles::default();

    assert_eq!(files.manifest, "manifest.json");
    assert_eq!(files.station, "station.json");
    assert_eq!(files.antennas, "antennas.json");
    assert_eq!(files.schedule, "schedule.json");
    assert_eq!(files.events, "events.jsonl");
    assert_eq!(files.observations, "observations.jsonl");
    assert_eq!(files.wsjtx, "wsjtx.jsonl");
    assert_eq!(files.rig, "rig.jsonl");
    assert_eq!(files.propagation, "propagation.jsonl");
    assert_eq!(files.analysis, "analysis.json");

    let contents = BundleContents {
        manifest: BundleManifest {
            schema_version: 1,
            session_id: "session-001".to_string(),
            created_at: Utc.with_ymd_and_hms(2026, 7, 9, 19, 0, 0).unwrap(),
            app_version: "0.1.0".to_string(),
            files,
        },
        station: Station {
            schema_version: 1,
            session_id: "session-001".to_string(),
            callsign: "K1ABC".to_string(),
            grid: "FN42".to_string(),
            power_watts: Some(100.0),
            operator_notes: None,
        },
        antennas: AntennasFile {
            schema_version: 1,
            session_id: "session-001".to_string(),
            antennas: vec![Antenna {
                label: "dipole".to_string(),
                facets: vec!["baseline".to_string()],
                height_m: Some(10.0),
                radial_count: None,
                radial_length_m: None,
                orientation_degrees: Some(90.0),
                tuner: None,
                feedline: None,
                notes: None,
            }],
        },
        schedule: Schedule {
            schema_version: 1,
            session_id: "session-001".to_string(),
            mode: ExperimentMode::WholeStationAb,
            goal: SessionGoal::Dx,
            slots: Vec::new(),
        },
        events: Vec::new(),
        observations: Vec::new(),
        wsjtx: Vec::new(),
        rig: Vec::new(),
        propagation: Vec::new(),
        analysis: AnalysisFile {
            schema_version: 1,
            session_id: "session-001".to_string(),
            generated_at: None,
            status: AnalysisStatus::NotRun,
            notes: Vec::new(),
        },
    };

    assert_eq!(contents.manifest.files.attachments_dir, "attachments");
    assert!(contents.events.is_empty());
    assert!(contents.observations.is_empty());
    assert!(contents.wsjtx.is_empty());
    assert!(contents.rig.is_empty());
    assert!(contents.propagation.is_empty());
}
