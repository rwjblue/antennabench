use std::path::PathBuf;

use antennabench_analysis::{summarize_bundle, EvidenceQuality};
use antennabench_storage::BundleStore;

#[test]
fn summarizes_the_minimal_whole_station_fixture() {
    let bundle = fixture_bundle("minimal-whole-station.session.wsprabundle");
    let summary = summarize_bundle(&bundle).expect("fixture should summarize");

    assert_eq!(summary.session_id, bundle.manifest.session_id);
    assert_eq!(summary.evidence_quality, EvidenceQuality::Insufficient);
    assert_eq!(summary.overall.observation_counts.total, 5);
    assert_eq!(summary.overall.observation_counts.usable, 2);
    assert_eq!(summary.overall.observation_counts.excluded, 3);

    insta::assert_json_snapshot!(summary, @r#"
    {
      "session_id": "session-2026-07-09-n1rwj-20m",
      "evidence_quality": "insufficient",
      "overall": {
        "observation_counts": {
          "total": 5,
          "usable": 2,
          "excluded": 3
        },
        "exclusions": [
          {
            "reason": "before_observed_switch",
            "count": 1
          },
          {
            "reason": "missed_slot",
            "count": 1
          },
          {
            "reason": "bad_slot",
            "count": 1
          }
        ],
        "usable_observation_kinds": [
          {
            "kind": "local_decode",
            "count": 1
          },
          {
            "kind": "public_report",
            "count": 1
          }
        ],
        "snr": {
          "sample_count": 2,
          "min_db": -19.0,
          "median_db": -18.5,
          "mean_db": -18.5,
          "max_db": -18.0
        }
      },
      "antennas": [
        {
          "antenna_label": "A",
          "contributing_slot_count": 1,
          "evidence_quality": "insufficient",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            },
            "exclusions": [],
            "usable_observation_kinds": [
              {
                "kind": "local_decode",
                "count": 1
              }
            ],
            "snr": {
              "sample_count": 1,
              "min_db": -18.0,
              "median_db": -18.0,
              "mean_db": -18.0,
              "max_db": -18.0
            }
          }
        },
        {
          "antenna_label": "B",
          "contributing_slot_count": 1,
          "evidence_quality": "insufficient",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            },
            "exclusions": [],
            "usable_observation_kinds": [
              {
                "kind": "public_report",
                "count": 1
              }
            ],
            "snr": {
              "sample_count": 1,
              "min_db": -19.0,
              "median_db": -19.0,
              "mean_db": -19.0,
              "max_db": -19.0
            }
          }
        }
      ],
      "bands": [
        {
          "band": "20m",
          "evidence": {
            "observation_counts": {
              "total": 5,
              "usable": 2,
              "excluded": 3
            },
            "exclusions": [
              {
                "reason": "before_observed_switch",
                "count": 1
              },
              {
                "reason": "missed_slot",
                "count": 1
              },
              {
                "reason": "bad_slot",
                "count": 1
              }
            ],
            "usable_observation_kinds": [
              {
                "kind": "local_decode",
                "count": 1
              },
              {
                "kind": "public_report",
                "count": 1
              }
            ],
            "snr": {
              "sample_count": 2,
              "min_db": -19.0,
              "median_db": -18.5,
              "mean_db": -18.5,
              "max_db": -18.0
            }
          }
        }
      ],
      "slots": [
        {
          "slot_id": "slot-001",
          "sequence_number": 1,
          "band": "20m",
          "planned_label": "A",
          "actual_label": "A",
          "status": "switched",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            },
            "exclusions": [],
            "usable_observation_kinds": [
              {
                "kind": "local_decode",
                "count": 1
              }
            ],
            "snr": {
              "sample_count": 1,
              "min_db": -18.0,
              "median_db": -18.0,
              "mean_db": -18.0,
              "max_db": -18.0
            }
          }
        },
        {
          "slot_id": "slot-002",
          "sequence_number": 2,
          "band": "20m",
          "planned_label": "B",
          "actual_label": null,
          "status": "bad",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "bad_slot",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "slot_id": "slot-003",
          "sequence_number": 3,
          "band": "20m",
          "planned_label": "A",
          "actual_label": null,
          "status": "missed",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "missed_slot",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "slot_id": "slot-004",
          "sequence_number": 4,
          "band": "20m",
          "planned_label": "B",
          "actual_label": "B",
          "status": "late_switch",
          "evidence": {
            "observation_counts": {
              "total": 2,
              "usable": 1,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "before_observed_switch",
                "count": 1
              }
            ],
            "usable_observation_kinds": [
              {
                "kind": "public_report",
                "count": 1
              }
            ],
            "snr": {
              "sample_count": 1,
              "min_db": -19.0,
              "median_db": -19.0,
              "mean_db": -19.0,
              "max_db": -19.0
            }
          }
        }
      ]
    }
    "#);
}

#[test]
fn summarizes_only_observations_from_the_wsjtx_hardening_fixture() {
    let bundle = fixture_bundle("wsjtx-import-hardening.session.wsprabundle");
    assert_eq!(bundle.observations.len(), 3);
    assert_eq!(bundle.wsjtx.len(), 14);
    assert_eq!(
        bundle
            .wsjtx
            .iter()
            .filter(|record| record.message_type == "all_wspr_malformed")
            .count(),
        11
    );

    let summary = summarize_bundle(&bundle).expect("fixture should summarize");

    assert_eq!(summary.session_id, bundle.manifest.session_id);
    assert_eq!(summary.evidence_quality, EvidenceQuality::Insufficient);
    assert_eq!(summary.overall.observation_counts.total, 3);
    assert_eq!(summary.overall.observation_counts.usable, 0);
    assert_eq!(summary.overall.observation_counts.excluded, 3);
    assert_eq!(summary.overall.snr, None);

    insta::assert_json_snapshot!(summary, @r#"
    {
      "session_id": "session-wsjtx-import-hardening",
      "evidence_quality": "insufficient",
      "overall": {
        "observation_counts": {
          "total": 3,
          "usable": 0,
          "excluded": 3
        },
        "exclusions": [
          {
            "reason": "guard_time",
            "count": 2
          },
          {
            "reason": "band_mismatch",
            "count": 1
          }
        ],
        "usable_observation_kinds": [],
        "snr": null
      },
      "antennas": [
        {
          "antenna_label": "A",
          "contributing_slot_count": 0,
          "evidence_quality": "insufficient",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "guard_time",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "antenna_label": "B",
          "contributing_slot_count": 0,
          "evidence_quality": "insufficient",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "guard_time",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        }
      ],
      "bands": [
        {
          "band": "40m",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "band_mismatch",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "band": "20m",
          "evidence": {
            "observation_counts": {
              "total": 2,
              "usable": 0,
              "excluded": 2
            },
            "exclusions": [
              {
                "reason": "guard_time",
                "count": 2
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        }
      ],
      "slots": [
        {
          "slot_id": "slot-001",
          "sequence_number": 1,
          "band": "20m",
          "planned_label": "A",
          "actual_label": "A",
          "status": "switched",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "guard_time",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "slot_id": "slot-002",
          "sequence_number": 2,
          "band": "20m",
          "planned_label": "B",
          "actual_label": "B",
          "status": "switched",
          "evidence": {
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "exclusions": [
              {
                "reason": "guard_time",
                "count": 1
              }
            ],
            "usable_observation_kinds": [],
            "snr": null
          }
        },
        {
          "slot_id": "slot-003",
          "sequence_number": 3,
          "band": "20m",
          "planned_label": "A",
          "actual_label": "A",
          "status": "switched",
          "evidence": {
            "observation_counts": {
              "total": 0,
              "usable": 0,
              "excluded": 0
            },
            "exclusions": [],
            "usable_observation_kinds": [],
            "snr": null
          }
        }
      ]
    }
    "#);
}

fn fixture_bundle(name: &str) -> antennabench_core::BundleContents {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/session-bundles")
        .join(name);
    BundleStore::new(root)
        .read_normalized_validated()
        .expect("fixture bundle should be normalized and valid")
}
