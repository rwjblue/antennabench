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
      ],
      "comparison": {
        "availability": "no_eligible_blocks",
        "left_label": "A",
        "right_label": "B",
        "delta_orientation": {
          "minuend_label": "B",
          "subtrahend_label": "A"
        },
        "diagnostics": {
          "block_count": 2,
          "eligible_block_count": 0,
          "invalid_block_count": 2,
          "left_then_right_block_count": 0,
          "right_then_left_block_count": 0,
          "paired_row_count": 0,
          "unique_path_count": 0,
          "unmatched_left_count": 0,
          "unmatched_right_count": 0,
          "missing_snr_left_count": 0,
          "missing_snr_right_count": 0,
          "missing_or_invalid_mode_count": 0,
          "ambiguous_path_count": 0,
          "exact_duplicate_count": 0,
          "conflicting_duplicate_group_count": 0,
          "excluded_observation_count": 3
        },
        "blocks": [
          {
            "block_index": 0,
            "band": "20m",
            "first_slot_id": "slot-001",
            "first_sequence_number": 1,
            "first_starts_at": "2026-07-09T20:00:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-002",
            "second_sequence_number": 2,
            "second_starts_at": "2026-07-09T20:02:00Z",
            "second_label": null,
            "second_status": "bad",
            "order": null,
            "eligibility": "missing_actual_label"
          },
          {
            "block_index": 1,
            "band": "20m",
            "first_slot_id": "slot-003",
            "first_sequence_number": 3,
            "first_starts_at": "2026-07-09T20:04:00Z",
            "first_label": null,
            "first_status": "missed",
            "second_slot_id": "slot-004",
            "second_sequence_number": 4,
            "second_starts_at": "2026-07-09T20:06:00Z",
            "second_label": "B",
            "second_status": "late_switch",
            "order": null,
            "eligibility": "missing_actual_label"
          }
        ],
        "overlap_rows": [],
        "timeline_rows": [
          {
            "block_index": 0,
            "block_eligible": false,
            "sequence_number": 1,
            "slot_id": "slot-001",
            "starts_at": "2026-07-09T20:00:00Z",
            "band": "20m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 1,
            "excluded_observation_count": 0,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 0,
            "block_eligible": false,
            "sequence_number": 2,
            "slot_id": "slot-002",
            "starts_at": "2026-07-09T20:02:00Z",
            "band": "20m",
            "actual_label": null,
            "side": null,
            "status": "bad",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 1,
            "block_eligible": false,
            "sequence_number": 3,
            "slot_id": "slot-003",
            "starts_at": "2026-07-09T20:04:00Z",
            "band": "20m",
            "actual_label": null,
            "side": null,
            "status": "missed",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 1,
            "block_eligible": false,
            "sequence_number": 4,
            "slot_id": "slot-004",
            "starts_at": "2026-07-09T20:06:00Z",
            "band": "20m",
            "actual_label": "B",
            "side": "right",
            "status": "late_switch",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          }
        ],
        "paired_rows": [],
        "path_summaries": [],
        "strata": []
      },
      "solar_context": {
        "algorithm": {
          "algorithm_id": "noaa-gml-fractional-year",
          "algorithm_version": 1,
          "coordinate_method": "maidenhead-cell-center-v1"
        },
        "rows": []
      }
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
      ],
      "comparison": {
        "availability": "no_matched_paths",
        "left_label": "A",
        "right_label": "B",
        "delta_orientation": {
          "minuend_label": "B",
          "subtrahend_label": "A"
        },
        "diagnostics": {
          "block_count": 2,
          "eligible_block_count": 1,
          "invalid_block_count": 1,
          "left_then_right_block_count": 1,
          "right_then_left_block_count": 0,
          "paired_row_count": 0,
          "unique_path_count": 0,
          "unmatched_left_count": 0,
          "unmatched_right_count": 0,
          "missing_snr_left_count": 0,
          "missing_snr_right_count": 0,
          "missing_or_invalid_mode_count": 0,
          "ambiguous_path_count": 0,
          "exact_duplicate_count": 0,
          "conflicting_duplicate_group_count": 0,
          "excluded_observation_count": 3
        },
        "blocks": [
          {
            "block_index": 0,
            "band": "20m",
            "first_slot_id": "slot-001",
            "first_sequence_number": 1,
            "first_starts_at": "2026-07-09T19:00:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-002",
            "second_sequence_number": 2,
            "second_starts_at": "2026-07-09T19:02:00Z",
            "second_label": "B",
            "second_status": "switched",
            "order": "left_then_right",
            "eligibility": "eligible"
          },
          {
            "block_index": 1,
            "band": "20m",
            "first_slot_id": "slot-003",
            "first_sequence_number": 3,
            "first_starts_at": "2026-07-09T19:26:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": null,
            "second_sequence_number": null,
            "second_starts_at": null,
            "second_label": null,
            "second_status": null,
            "order": null,
            "eligibility": "incomplete_same_band_run"
          }
        ],
        "overlap_rows": [],
        "timeline_rows": [
          {
            "block_index": 0,
            "block_eligible": true,
            "sequence_number": 1,
            "slot_id": "slot-001",
            "starts_at": "2026-07-09T19:00:00Z",
            "band": "20m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 0,
            "block_eligible": true,
            "sequence_number": 2,
            "slot_id": "slot-002",
            "starts_at": "2026-07-09T19:02:00Z",
            "band": "20m",
            "actual_label": "B",
            "side": "right",
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 1,
            "block_eligible": false,
            "sequence_number": 3,
            "slot_id": "slot-003",
            "starts_at": "2026-07-09T19:26:00Z",
            "band": "20m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 0,
            "usable_observation_count": 0,
            "excluded_observation_count": 0,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          }
        ],
        "paired_rows": [],
        "path_summaries": [],
        "strata": []
      },
      "solar_context": {
        "algorithm": {
          "algorithm_id": "noaa-gml-fractional-year",
          "algorithm_version": 1,
          "coordinate_method": "maidenhead-cell-center-v1"
        },
        "rows": []
      }
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
