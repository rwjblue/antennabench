use std::path::PathBuf;

use antennabench_analysis::EvidenceQuality;
use antennabench_report::{
    build_report, ReportNotice, ReportQuestionAnswerability, SessionReport,
    UsableObservationKindCounts,
};
use antennabench_storage::BundleStore;

#[test]
fn reports_the_minimal_whole_station_fixture() {
    let bundle = fixture_bundle("minimal-whole-station.session.wsprabundle");
    let report = build_report(&bundle).expect("minimal fixture should produce a report");

    assert_eq!(report.context.session_id, bundle.manifest.session_id);
    assert_eq!(report.evidence.overall.observation_counts.total, 5);
    assert_eq!(report.evidence.overall.observation_counts.usable, 2);
    assert_eq!(report.evidence.overall.observation_counts.excluded, 3);
    assert!(report.notices.is_empty());

    insta::assert_json_snapshot!(report, @r#"
    {
      "overview": {
        "scope": {
          "session_id": "session-2026-07-09-n1rwj-20m",
          "station": {
            "callsign": "N1RWJ",
            "grid": "FN42",
            "power_watts": 5.0
          },
          "goal": "general_coverage",
          "experiment_mode": "whole_station_ab",
          "bands": [
            "20m"
          ],
          "antenna_labels": [
            "A",
            "B"
          ],
          "observed_directions": [
            "transmit",
            "receive"
          ],
          "delta_orientation": {
            "minuend_label": "B",
            "subtrahend_label": "A"
          }
        },
        "goal_lens": {
          "goal": "general_coverage",
          "priority": [
            "shared_path_signal",
            "common_opportunity_detection",
            "observed_reach",
            "geographic_profile",
            "repeatability"
          ],
          "emphasized_distance_bins": [],
          "practical_meaning": "Prioritizes shared-path signal, common listening opportunities, observed reach, distance and bearing, and repeatability. No universal antenna winner is selected."
        },
        "lifecycle": {
          "checkpoint_revision": null,
          "state": "not_recorded"
        },
        "answerability": {
          "same_path_signal": "no_eligible_blocks",
          "paired_detectability": "no_eligible_blocks",
          "observed_reach": "no_usable_paths",
          "geographic_profile": "no_located_paths",
          "repeatability": "insufficient_repetition"
        },
        "comparison_availability": "no_eligible_blocks",
        "strata": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 2,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 0,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 0
            },
            "observed_profile": {
              "left": null,
              "right": null,
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 0,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 0
            },
            "observed_profile": {
              "left": null,
              "right": null,
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          }
        ],
        "timeline": [
          {
            "item_id": "slot-001",
            "sequence_number": 1,
            "block_index": 0,
            "block_eligibility": "missing_actual_label",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-09T20:00:00Z",
            "planned_ends_at": "2026-07-09T20:02:00Z",
            "actual_starts_at": "2026-07-09T20:00:03Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 1,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-001",
                "occurred_at": "2026-07-09T20:00:03Z",
                "slot_id": "slot-001",
                "affected_slot_id": "slot-001",
                "kind": "switched",
                "detail": "A connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-002",
            "sequence_number": 2,
            "block_index": 0,
            "block_eligibility": "missing_actual_label",
            "band": "20m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": null,
            "planned_starts_at": "2026-07-09T20:02:00Z",
            "planned_ends_at": "2026-07-09T20:04:00Z",
            "actual_starts_at": null,
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "bad",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-002",
                "occurred_at": "2026-07-09T20:02:20Z",
                "slot_id": "slot-002",
                "affected_slot_id": "slot-002",
                "kind": "slot_bad",
                "detail": "High SWR during B slot",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-003",
            "sequence_number": 3,
            "block_index": 1,
            "block_eligibility": "missing_actual_label",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": null,
            "planned_starts_at": "2026-07-09T20:04:00Z",
            "planned_ends_at": "2026-07-09T20:06:00Z",
            "actual_starts_at": null,
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "missed",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-003",
                "occurred_at": "2026-07-09T20:04:00Z",
                "slot_id": "slot-003",
                "affected_slot_id": "slot-003",
                "kind": "slot_missed",
                "detail": "Operator missed the A switch prompt",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-004",
            "sequence_number": 4,
            "block_index": 1,
            "block_eligibility": "missing_actual_label",
            "band": "20m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-09T20:06:00Z",
            "planned_ends_at": "2026-07-09T20:08:00Z",
            "actual_starts_at": "2026-07-09T20:06:25Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "late_switch",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-004",
                "occurred_at": "2026-07-09T20:06:25Z",
                "slot_id": "slot-004",
                "affected_slot_id": "slot-004",
                "kind": "switched",
                "detail": "B connected late",
                "correction": null
              }
            ]
          }
        ],
        "limitations": [
          {
            "kind": "no_eligible_blocks"
          }
        ]
      },
      "context": {
        "session_id": "session-2026-07-09-n1rwj-20m",
        "station": {
          "callsign": "N1RWJ",
          "grid": "FN42",
          "power_watts": 5.0
        },
        "experiment_mode": "whole_station_ab",
        "goal": "general_coverage",
        "scheduled_time_range": {
          "starts_at": "2026-07-09T20:00:00Z",
          "ends_at": "2026-07-09T20:08:00Z"
        },
        "antennas": [
          {
            "label": "A",
            "facets": [
              "vertical"
            ],
            "height_m": 7.0,
            "radial_count": 16,
            "radial_length_m": 5.0,
            "orientation_degrees": null,
            "tuner": "manual",
            "feedline": "RG-8X",
            "notes": "Temporary ground-mounted vertical"
          },
          {
            "label": "B",
            "facets": [
              "dipole"
            ],
            "height_m": 9.0,
            "radial_count": null,
            "radial_length_m": null,
            "orientation_degrees": 70.0,
            "tuner": null,
            "feedline": "RG-58",
            "notes": "Inverted vee"
          }
        ],
        "bands": [
          "20m"
        ],
        "schedule": {
          "slot_count": 4,
          "slots": [
            {
              "slot_id": "slot-001",
              "sequence_number": 1,
              "starts_at": "2026-07-09T20:00:00Z",
              "ends_at": "2026-07-09T20:02:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-002",
              "sequence_number": 2,
              "starts_at": "2026-07-09T20:02:00Z",
              "ends_at": "2026-07-09T20:04:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "B"
            },
            {
              "slot_id": "slot-003",
              "sequence_number": 3,
              "starts_at": "2026-07-09T20:04:00Z",
              "ends_at": "2026-07-09T20:06:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-004",
              "sequence_number": 4,
              "starts_at": "2026-07-09T20:06:00Z",
              "ends_at": "2026-07-09T20:08:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "B"
            }
          ]
        }
      },
      "evidence": {
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
          "usable_observation_kinds": {
            "local_decode": 1,
            "public_report": 1,
            "imported_spot": 0
          },
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
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 0,
                "imported_spot": 0
              },
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 1,
                "imported_spot": 0
              },
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
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 1,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T20:00:00Z",
            "ends_at": "2026-07-09T20:02:00Z",
            "usable_start": "2026-07-09T20:00:15Z",
            "switch_event_id": "event-001",
            "switch_timestamp": "2026-07-09T20:00:03Z",
            "switch_delay_seconds": 3,
            "evidence": {
              "observation_counts": {
                "total": 1,
                "usable": 1,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T20:02:00Z",
            "ends_at": "2026-07-09T20:04:00Z",
            "usable_start": "2026-07-09T20:02:15Z",
            "switch_event_id": null,
            "switch_timestamp": null,
            "switch_delay_seconds": null,
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T20:04:00Z",
            "ends_at": "2026-07-09T20:06:00Z",
            "usable_start": "2026-07-09T20:04:15Z",
            "switch_event_id": null,
            "switch_timestamp": null,
            "switch_delay_seconds": null,
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T20:06:00Z",
            "ends_at": "2026-07-09T20:08:00Z",
            "usable_start": "2026-07-09T20:06:25Z",
            "switch_event_id": "event-004",
            "switch_timestamp": "2026-07-09T20:06:25Z",
            "switch_delay_seconds": 25,
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 1,
                "imported_spot": 0
              },
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
      },
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
        "strata": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 2,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          }
        ],
        "observed_path_profiles": []
      },
      "solar_context": {
        "algorithm": {
          "algorithm_id": "noaa-gml-fractional-year",
          "algorithm_version": 1,
          "coordinate_method": "maidenhead-cell-center-v1"
        },
        "rows": []
      },
      "chart_data": {
        "antenna_snr": [
          {
            "antenna_label": "A",
            "usable_observation_count": 1,
            "snr": {
              "sample_count": 1,
              "min_db": -18.0,
              "median_db": -18.0,
              "mean_db": -18.0,
              "max_db": -18.0
            }
          },
          {
            "antenna_label": "B",
            "usable_observation_count": 1,
            "snr": {
              "sample_count": 1,
              "min_db": -19.0,
              "median_db": -19.0,
              "mean_db": -19.0,
              "max_db": -19.0
            }
          }
        ],
        "band_evidence_counts": [
          {
            "band": "20m",
            "observation_counts": {
              "total": 5,
              "usable": 2,
              "excluded": 3
            },
            "usable_observation_kinds": {
              "local_decode": 1,
              "public_report": 1,
              "imported_spot": 0
            }
          }
        ],
        "slot_evidence_counts": [
          {
            "slot_id": "slot-001",
            "sequence_number": 1,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-002",
            "sequence_number": 2,
            "band": "20m",
            "planned_label": "B",
            "actual_label": null,
            "status": "bad",
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            }
          },
          {
            "slot_id": "slot-003",
            "sequence_number": 3,
            "band": "20m",
            "planned_label": "A",
            "actual_label": null,
            "status": "missed",
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            }
          },
          {
            "slot_id": "slot-004",
            "sequence_number": 4,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "late_switch",
            "observation_counts": {
              "total": 2,
              "usable": 1,
              "excluded": 1
            }
          }
        ]
      },
      "notices": [],
      "snapshot": {
        "checkpoint_revision": null,
        "lifecycle": null,
        "lifecycle_events": [],
        "operator_events": [
          {
            "event_id": "event-001",
            "occurred_at": "2026-07-09T20:00:03Z",
            "slot_id": "slot-001",
            "affected_slot_id": "slot-001",
            "kind": "switched",
            "detail": "A connected",
            "correction": null
          },
          {
            "event_id": "event-002",
            "occurred_at": "2026-07-09T20:02:20Z",
            "slot_id": "slot-002",
            "affected_slot_id": "slot-002",
            "kind": "slot_bad",
            "detail": "High SWR during B slot",
            "correction": null
          },
          {
            "event_id": "event-003",
            "occurred_at": "2026-07-09T20:04:00Z",
            "slot_id": "slot-003",
            "affected_slot_id": "slot-003",
            "kind": "slot_missed",
            "detail": "Operator missed the A switch prompt",
            "correction": null
          },
          {
            "event_id": "event-004",
            "occurred_at": "2026-07-09T20:06:25Z",
            "slot_id": "slot-004",
            "affected_slot_id": "slot-004",
            "kind": "switched",
            "detail": "B connected late",
            "correction": null
          }
        ],
        "adapter_evidence": {
          "record_count": 0,
          "accepted_count": 0,
          "malformed_count": 0,
          "unsupported_count": 0,
          "filtered_count": 0,
          "duplicate_count": 0,
          "partially_normalized_count": 0,
          "gap_count": 0,
          "workflow_status": "not_configured",
          "provider_completeness": "unsupported"
        }
      },
      "exclusion_records": [
        {
          "observation_id": "obs-002",
          "reason": "bad_slot",
          "timestamp": "2026-07-09T20:02:50Z",
          "band": "20m",
          "observation_kind": "public_report",
          "source": "wsprnet",
          "mode": "WSPR",
          "slot_id": "slot-002",
          "assigned_label": null,
          "assignment_confidence": 0.0
        },
        {
          "observation_id": "obs-003",
          "reason": "missed_slot",
          "timestamp": "2026-07-09T20:04:40Z",
          "band": "20m",
          "observation_kind": "local_decode",
          "source": "wsjtx_log",
          "mode": "WSPR",
          "slot_id": "slot-003",
          "assigned_label": null,
          "assignment_confidence": 0.0
        },
        {
          "observation_id": "obs-004",
          "reason": "before_observed_switch",
          "timestamp": "2026-07-09T20:06:10Z",
          "band": "20m",
          "observation_kind": "public_report",
          "source": "wsprnet",
          "mode": "WSPR",
          "slot_id": "slot-004",
          "assigned_label": null,
          "assignment_confidence": 0.1
        }
      ]
    }
    "#);
}

#[test]
fn older_serialized_reports_default_missing_question_answerability() {
    let report =
        build_report(&fixture_bundle("minimal-whole-station.session.wsprabundle")).unwrap();
    let mut value = serde_json::to_value(report).unwrap();
    value
        .pointer_mut("/overview")
        .and_then(serde_json::Value::as_object_mut)
        .unwrap()
        .remove("answerability");

    let restored: SessionReport = serde_json::from_value(value).unwrap();
    assert_eq!(
        restored.overview.answerability,
        ReportQuestionAnswerability::default()
    );
}

#[test]
fn reports_only_observations_from_the_wsjtx_hardening_fixture() {
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

    let report = build_report(&bundle).expect("hardening fixture should produce a report");

    assert_eq!(report.evidence.overall.observation_counts.total, 3);
    assert_eq!(report.evidence.overall.observation_counts.usable, 0);
    assert_eq!(report.evidence.overall.observation_counts.excluded, 3);
    assert_eq!(report.evidence.overall.snr, None);
    assert_eq!(
        report.notices,
        vec![
            ReportNotice::NoUsableObservations,
            ReportNotice::NoUsableSnrSamples,
        ]
    );

    insta::assert_json_snapshot!(report, @r#"
    {
      "overview": {
        "scope": {
          "session_id": "session-wsjtx-import-hardening",
          "station": {
            "callsign": "N1RWJ",
            "grid": "FN42",
            "power_watts": 5.0
          },
          "goal": "general_coverage",
          "experiment_mode": "whole_station_ab",
          "bands": [
            "20m"
          ],
          "antenna_labels": [
            "A",
            "B"
          ],
          "observed_directions": [
            "receive"
          ],
          "delta_orientation": {
            "minuend_label": "B",
            "subtrahend_label": "A"
          }
        },
        "goal_lens": {
          "goal": "general_coverage",
          "priority": [
            "shared_path_signal",
            "common_opportunity_detection",
            "observed_reach",
            "geographic_profile",
            "repeatability"
          ],
          "emphasized_distance_bins": [],
          "practical_meaning": "Prioritizes shared-path signal, common listening opportunities, observed reach, distance and bearing, and repeatability. No universal antenna winner is selected."
        },
        "lifecycle": {
          "checkpoint_revision": null,
          "state": "not_recorded"
        },
        "answerability": {
          "same_path_signal": "no_matched_paths",
          "paired_detectability": "activity_coverage_unknown",
          "observed_reach": "no_usable_paths",
          "geographic_profile": "no_located_paths",
          "repeatability": "insufficient_repetition"
        },
        "comparison_availability": "no_matched_paths",
        "strata": [
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 0,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 0
            },
            "observed_profile": {
              "left": null,
              "right": null,
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 2,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 0,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 0
            },
            "observed_profile": {
              "left": null,
              "right": null,
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          }
        ],
        "timeline": [
          {
            "item_id": "slot-001",
            "sequence_number": 1,
            "block_index": 0,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-09T19:00:00Z",
            "planned_ends_at": "2026-07-09T19:02:00Z",
            "actual_starts_at": "2026-07-09T19:00:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-001",
                "occurred_at": "2026-07-09T19:00:00Z",
                "slot_id": "slot-001",
                "affected_slot_id": "slot-001",
                "kind": "switched",
                "detail": null,
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-002",
            "sequence_number": 2,
            "block_index": 0,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-09T19:02:00Z",
            "planned_ends_at": "2026-07-09T19:04:00Z",
            "actual_starts_at": "2026-07-09T19:02:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 0,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-002",
                "occurred_at": "2026-07-09T19:02:00Z",
                "slot_id": "slot-002",
                "affected_slot_id": "slot-002",
                "kind": "switched",
                "detail": null,
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-003",
            "sequence_number": 3,
            "block_index": 1,
            "block_eligibility": "incomplete_same_band_run",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-09T19:26:00Z",
            "planned_ends_at": "2026-07-09T19:28:00Z",
            "actual_starts_at": "2026-07-09T19:26:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 0,
            "usable_observation_count": 0,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-003",
                "occurred_at": "2026-07-09T19:26:00Z",
                "slot_id": "slot-003",
                "affected_slot_id": "slot-003",
                "kind": "switched",
                "detail": null,
                "correction": null
              }
            ]
          }
        ],
        "limitations": [
          {
            "kind": "no_matched_paths"
          }
        ]
      },
      "context": {
        "session_id": "session-wsjtx-import-hardening",
        "station": {
          "callsign": "N1RWJ",
          "grid": "FN42",
          "power_watts": 5.0
        },
        "experiment_mode": "whole_station_ab",
        "goal": "general_coverage",
        "scheduled_time_range": {
          "starts_at": "2026-07-09T19:00:00Z",
          "ends_at": "2026-07-09T19:28:00Z"
        },
        "antennas": [
          {
            "label": "A",
            "facets": [
              "vertical"
            ],
            "height_m": null,
            "radial_count": null,
            "radial_length_m": null,
            "orientation_degrees": null,
            "tuner": null,
            "feedline": null,
            "notes": null
          },
          {
            "label": "B",
            "facets": [
              "dipole"
            ],
            "height_m": null,
            "radial_count": null,
            "radial_length_m": null,
            "orientation_degrees": null,
            "tuner": null,
            "feedline": null,
            "notes": null
          }
        ],
        "bands": [
          "20m"
        ],
        "schedule": {
          "slot_count": 3,
          "slots": [
            {
              "slot_id": "slot-001",
              "sequence_number": 1,
              "starts_at": "2026-07-09T19:00:00Z",
              "ends_at": "2026-07-09T19:02:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-002",
              "sequence_number": 2,
              "starts_at": "2026-07-09T19:02:00Z",
              "ends_at": "2026-07-09T19:04:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "B"
            },
            {
              "slot_id": "slot-003",
              "sequence_number": 3,
              "starts_at": "2026-07-09T19:26:00Z",
              "ends_at": "2026-07-09T19:28:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            }
          ]
        }
      },
      "evidence": {
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
          "usable_observation_kinds": {
            "local_decode": 0,
            "public_report": 0,
            "imported_spot": 0
          },
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T19:00:00Z",
            "ends_at": "2026-07-09T19:02:00Z",
            "usable_start": "2026-07-09T19:00:15Z",
            "switch_event_id": "event-001",
            "switch_timestamp": "2026-07-09T19:00:00Z",
            "switch_delay_seconds": 0,
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T19:02:00Z",
            "ends_at": "2026-07-09T19:04:00Z",
            "usable_start": "2026-07-09T19:02:15Z",
            "switch_event_id": "event-002",
            "switch_timestamp": "2026-07-09T19:02:00Z",
            "switch_delay_seconds": 0,
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
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
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
            "starts_at": "2026-07-09T19:26:00Z",
            "ends_at": "2026-07-09T19:28:00Z",
            "usable_start": "2026-07-09T19:26:15Z",
            "switch_event_id": "event-003",
            "switch_timestamp": "2026-07-09T19:26:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 0,
                "usable": 0,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 0
              },
              "snr": null
            }
          }
        ]
      },
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
        "strata": [
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 2,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          }
        ],
        "observed_path_profiles": []
      },
      "solar_context": {
        "algorithm": {
          "algorithm_id": "noaa-gml-fractional-year",
          "algorithm_version": 1,
          "coordinate_method": "maidenhead-cell-center-v1"
        },
        "rows": []
      },
      "chart_data": {
        "antenna_snr": [
          {
            "antenna_label": "A",
            "usable_observation_count": 0,
            "snr": null
          },
          {
            "antenna_label": "B",
            "usable_observation_count": 0,
            "snr": null
          }
        ],
        "band_evidence_counts": [
          {
            "band": "40m",
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            },
            "usable_observation_kinds": {
              "local_decode": 0,
              "public_report": 0,
              "imported_spot": 0
            }
          },
          {
            "band": "20m",
            "observation_counts": {
              "total": 2,
              "usable": 0,
              "excluded": 2
            },
            "usable_observation_kinds": {
              "local_decode": 0,
              "public_report": 0,
              "imported_spot": 0
            }
          }
        ],
        "slot_evidence_counts": [
          {
            "slot_id": "slot-001",
            "sequence_number": 1,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            }
          },
          {
            "slot_id": "slot-002",
            "sequence_number": 2,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "observation_counts": {
              "total": 1,
              "usable": 0,
              "excluded": 1
            }
          },
          {
            "slot_id": "slot-003",
            "sequence_number": 3,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 0,
              "usable": 0,
              "excluded": 0
            }
          }
        ]
      },
      "notices": [
        "no_usable_observations",
        "no_usable_snr_samples"
      ],
      "snapshot": {
        "checkpoint_revision": null,
        "lifecycle": null,
        "lifecycle_events": [],
        "operator_events": [
          {
            "event_id": "event-001",
            "occurred_at": "2026-07-09T19:00:00Z",
            "slot_id": "slot-001",
            "affected_slot_id": "slot-001",
            "kind": "switched",
            "detail": null,
            "correction": null
          },
          {
            "event_id": "event-002",
            "occurred_at": "2026-07-09T19:02:00Z",
            "slot_id": "slot-002",
            "affected_slot_id": "slot-002",
            "kind": "switched",
            "detail": null,
            "correction": null
          },
          {
            "event_id": "event-003",
            "occurred_at": "2026-07-09T19:26:00Z",
            "slot_id": "slot-003",
            "affected_slot_id": "slot-003",
            "kind": "switched",
            "detail": null,
            "correction": null
          }
        ],
        "adapter_evidence": {
          "record_count": 0,
          "accepted_count": 0,
          "malformed_count": 0,
          "unsupported_count": 0,
          "filtered_count": 0,
          "duplicate_count": 0,
          "partially_normalized_count": 0,
          "gap_count": 0,
          "workflow_status": "not_configured",
          "provider_completeness": "unsupported"
        }
      },
      "exclusion_records": [
        {
          "observation_id": "edge-cases-obs-000001",
          "reason": "guard_time",
          "timestamp": "2026-07-09T19:00:00Z",
          "band": "20m",
          "observation_kind": "local_decode",
          "source": "wsjtx_log",
          "mode": "WSPR",
          "slot_id": "slot-001",
          "assigned_label": "A",
          "assignment_confidence": 0.25
        },
        {
          "observation_id": "edge-cases-obs-000003",
          "reason": "guard_time",
          "timestamp": "2026-07-09T19:02:00Z",
          "band": "20m",
          "observation_kind": "local_decode",
          "source": "wsjtx_log",
          "mode": "WSPR",
          "slot_id": "slot-002",
          "assigned_label": "B",
          "assignment_confidence": 0.25
        },
        {
          "observation_id": "edge-cases-obs-000015",
          "reason": "band_mismatch",
          "timestamp": "2026-07-09T19:26:00Z",
          "band": "40m",
          "observation_kind": "local_decode",
          "source": "wsjtx_log",
          "mode": "WSPR",
          "slot_id": null,
          "assigned_label": null,
          "assignment_confidence": 0.0
        }
      ]
    }
    "#);
}

#[test]
fn reports_the_analysis_rich_whole_station_fixture() {
    let bundle = fixture_bundle("analysis-rich-whole-station.session.wsprabundle");
    let report = build_report(&bundle).expect("analysis-rich fixture should produce a report");

    assert_eq!(report.evidence.evidence_quality, EvidenceQuality::Moderate);
    assert_eq!(report.evidence.overall.observation_counts.total, 14);
    assert_eq!(report.evidence.overall.observation_counts.usable, 12);
    assert_eq!(report.evidence.overall.observation_counts.excluded, 2);
    assert!(report.notices.is_empty());

    assert_eq!(report.evidence.antennas.len(), 2);
    for antenna in &report.evidence.antennas {
        assert_eq!(antenna.evidence_quality, EvidenceQuality::Moderate);
        assert_eq!(antenna.contributing_slot_count, 4);
        assert_eq!(antenna.evidence.observation_counts.usable, 6);
        assert_eq!(
            antenna.evidence.usable_observation_kinds,
            UsableObservationKindCounts {
                local_decode: 2,
                public_report: 2,
                imported_spot: 2,
            }
        );
        assert_eq!(
            antenna
                .evidence
                .snr
                .as_ref()
                .expect("each antenna should have SNR statistics")
                .sample_count,
            5
        );
    }

    assert_eq!(report.evidence.bands.len(), 2);
    assert!(report
        .evidence
        .bands
        .iter()
        .all(|band| band.evidence.observation_counts.usable == 6));

    insta::assert_json_snapshot!(report, @r#"
    {
      "overview": {
        "scope": {
          "session_id": "session-2026-07-12-n1rwj-analysis-rich",
          "station": {
            "callsign": "N1RWJ",
            "grid": "FN42",
            "power_watts": 5.0
          },
          "goal": "general_coverage",
          "experiment_mode": "whole_station_ab",
          "bands": [
            "40m",
            "20m"
          ],
          "antenna_labels": [
            "A",
            "B"
          ],
          "observed_directions": [
            "transmit",
            "receive"
          ],
          "delta_orientation": {
            "minuend_label": "B",
            "subtrahend_label": "A"
          }
        },
        "goal_lens": {
          "goal": "general_coverage",
          "priority": [
            "shared_path_signal",
            "common_opportunity_detection",
            "observed_reach",
            "geographic_profile",
            "repeatability"
          ],
          "emphasized_distance_bins": [],
          "practical_meaning": "Prioritizes shared-path signal, common listening opportunities, observed reach, distance and bearing, and repeatability. No universal antenna winner is selected."
        },
        "lifecycle": {
          "checkpoint_revision": null,
          "state": "not_recorded"
        },
        "answerability": {
          "same_path_signal": "no_matched_paths",
          "paired_detectability": "activity_coverage_unknown",
          "observed_reach": "available",
          "geographic_profile": "available",
          "repeatability": "available"
        },
        "comparison_availability": "no_matched_paths",
        "strata": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 1,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 1,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 0
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 1,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 0,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 1
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 1,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 1
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 1,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 1
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 1,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 1
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "availability": "no_finite_paired_paths",
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "path_delta": {
              "availability": "unavailable"
            },
            "path_median_deltas": [],
            "reach": {
              "left_only_unique_path_count": 1,
              "both_unique_path_count": 0,
              "right_only_unique_path_count": 1
            },
            "observed_profile": {
              "left": {
                "side": "left",
                "antenna_label": "A",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "right": {
                "side": "right",
                "antenna_label": "B",
                "unique_path_count": 1,
                "located_path_count": 1,
                "missing_location_path_count": 0,
                "inconsistent_location_path_count": 0,
                "distance_bins": [
                  {
                    "category": "under500_km",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km500_to1499",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "km1500_to2999",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "km3000_and_above",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ],
                "azimuth_sectors": [
                  {
                    "category": "north",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_east",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "south_west",
                    "unique_path_count": 1,
                    "observation_count": 1
                  },
                  {
                    "category": "west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  },
                  {
                    "category": "north_west",
                    "unique_path_count": 0,
                    "observation_count": 0
                  }
                ]
              },
              "distance_composition": [
                {
                  "category": "under500_km",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km500_to1499",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 1
                },
                {
                  "category": "km1500_to2999",
                  "left_only_unique_path_count": 1,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                },
                {
                  "category": "km3000_and_above",
                  "left_only_unique_path_count": 0,
                  "shared_unique_path_count": 0,
                  "right_only_unique_path_count": 0
                }
              ],
              "composition_location_unavailable_count": 0
            },
            "location_context": {
              "paths": [],
              "distance_bins": [
                {
                  "category": "under500_km",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km500_to1499",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km1500_to2999",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "km3000_and_above",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "azimuth_sectors": [
                {
                  "category": "north",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_east",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "south_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                },
                {
                  "category": "north_west",
                  "unique_located_path_count": 0,
                  "paired_row_count": 0,
                  "median_path_delta_right_minus_left_db": null
                }
              ],
              "missing_location_path_count": 0,
              "inconsistent_location_path_count": 0
            }
          }
        ],
        "timeline": [
          {
            "item_id": "slot-001",
            "sequence_number": 1,
            "block_index": 0,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-12T20:00:00Z",
            "planned_ends_at": "2026-07-12T20:02:00Z",
            "actual_starts_at": "2026-07-12T20:00:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-001",
                "occurred_at": "2026-07-12T20:00:00Z",
                "slot_id": "slot-001",
                "affected_slot_id": "slot-001",
                "kind": "switched",
                "detail": "A connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-002",
            "sequence_number": 2,
            "block_index": 0,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-12T20:02:00Z",
            "planned_ends_at": "2026-07-12T20:04:00Z",
            "actual_starts_at": "2026-07-12T20:02:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-002",
                "occurred_at": "2026-07-12T20:02:00Z",
                "slot_id": "slot-002",
                "affected_slot_id": "slot-002",
                "kind": "switched",
                "detail": "B connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-003",
            "sequence_number": 3,
            "block_index": 1,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-12T20:04:00Z",
            "planned_ends_at": "2026-07-12T20:06:00Z",
            "actual_starts_at": "2026-07-12T20:04:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-003",
                "occurred_at": "2026-07-12T20:04:00Z",
                "slot_id": "slot-003",
                "affected_slot_id": "slot-003",
                "kind": "switched",
                "detail": "A connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-004",
            "sequence_number": 4,
            "block_index": 1,
            "block_eligibility": "eligible",
            "band": "20m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-12T20:06:00Z",
            "planned_ends_at": "2026-07-12T20:08:00Z",
            "actual_starts_at": "2026-07-12T20:06:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 1,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-004",
                "occurred_at": "2026-07-12T20:06:00Z",
                "slot_id": "slot-004",
                "affected_slot_id": "slot-004",
                "kind": "switched",
                "detail": "B connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-005",
            "sequence_number": 5,
            "block_index": 2,
            "block_eligibility": "eligible",
            "band": "40m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-12T20:08:00Z",
            "planned_ends_at": "2026-07-12T20:10:00Z",
            "actual_starts_at": "2026-07-12T20:08:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-005",
                "occurred_at": "2026-07-12T20:08:00Z",
                "slot_id": "slot-005",
                "affected_slot_id": "slot-005",
                "kind": "switched",
                "detail": "A connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-006",
            "sequence_number": 6,
            "block_index": 2,
            "block_eligibility": "eligible",
            "band": "40m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-12T20:10:00Z",
            "planned_ends_at": "2026-07-12T20:12:00Z",
            "actual_starts_at": "2026-07-12T20:10:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-006",
                "occurred_at": "2026-07-12T20:10:00Z",
                "slot_id": "slot-006",
                "affected_slot_id": "slot-006",
                "kind": "switched",
                "detail": "B connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-007",
            "sequence_number": 7,
            "block_index": 3,
            "block_eligibility": "eligible",
            "band": "40m",
            "direction": null,
            "planned_antenna": "A",
            "actual_antenna": "A",
            "planned_starts_at": "2026-07-12T20:12:00Z",
            "planned_ends_at": "2026-07-12T20:14:00Z",
            "actual_starts_at": "2026-07-12T20:12:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 1,
            "excluded_observation_count": 0,
            "event_history": [
              {
                "event_id": "event-007",
                "occurred_at": "2026-07-12T20:12:00Z",
                "slot_id": "slot-007",
                "affected_slot_id": "slot-007",
                "kind": "switched",
                "detail": "A connected",
                "correction": null
              }
            ]
          },
          {
            "item_id": "slot-008",
            "sequence_number": 8,
            "block_index": 3,
            "block_eligibility": "eligible",
            "band": "40m",
            "direction": null,
            "planned_antenna": "B",
            "actual_antenna": "B",
            "planned_starts_at": "2026-07-12T20:14:00Z",
            "planned_ends_at": "2026-07-12T20:16:00Z",
            "actual_starts_at": "2026-07-12T20:14:00Z",
            "actual_ends_at": null,
            "readiness_basis": null,
            "attribution": null,
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "event_history": [
              {
                "event_id": "event-008",
                "occurred_at": "2026-07-12T20:14:00Z",
                "slot_id": "slot-008",
                "affected_slot_id": "slot-008",
                "kind": "switched",
                "detail": "B connected",
                "correction": null
              }
            ]
          }
        ],
        "limitations": [
          {
            "kind": "no_matched_paths"
          },
          {
            "kind": "unmatched_paths",
            "left_count": 5,
            "right_count": 5
          },
          {
            "kind": "missing_snr",
            "left_count": 1,
            "right_count": 1
          }
        ]
      },
      "context": {
        "session_id": "session-2026-07-12-n1rwj-analysis-rich",
        "station": {
          "callsign": "N1RWJ",
          "grid": "FN42",
          "power_watts": 5.0
        },
        "experiment_mode": "whole_station_ab",
        "goal": "general_coverage",
        "scheduled_time_range": {
          "starts_at": "2026-07-12T20:00:00Z",
          "ends_at": "2026-07-12T20:16:00Z"
        },
        "antennas": [
          {
            "label": "A",
            "facets": [
              "vertical",
              "ground_mounted"
            ],
            "height_m": 7.0,
            "radial_count": 16,
            "radial_length_m": 5.0,
            "orientation_degrees": 0.0,
            "tuner": "manual",
            "feedline": "RG-8X",
            "notes": "Ground-mounted quarter-wave vertical"
          },
          {
            "label": "B",
            "facets": [
              "dipole",
              "inverted_vee"
            ],
            "height_m": 9.0,
            "radial_count": null,
            "radial_length_m": null,
            "orientation_degrees": 70.0,
            "tuner": "automatic",
            "feedline": "RG-58",
            "notes": "Center-fed inverted vee"
          }
        ],
        "bands": [
          "40m",
          "20m"
        ],
        "schedule": {
          "slot_count": 8,
          "slots": [
            {
              "slot_id": "slot-001",
              "sequence_number": 1,
              "starts_at": "2026-07-12T20:00:00Z",
              "ends_at": "2026-07-12T20:02:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-002",
              "sequence_number": 2,
              "starts_at": "2026-07-12T20:02:00Z",
              "ends_at": "2026-07-12T20:04:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "B"
            },
            {
              "slot_id": "slot-003",
              "sequence_number": 3,
              "starts_at": "2026-07-12T20:04:00Z",
              "ends_at": "2026-07-12T20:06:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-004",
              "sequence_number": 4,
              "starts_at": "2026-07-12T20:06:00Z",
              "ends_at": "2026-07-12T20:08:00Z",
              "guard_seconds": 15,
              "band": "20m",
              "planned_label": "B"
            },
            {
              "slot_id": "slot-005",
              "sequence_number": 5,
              "starts_at": "2026-07-12T20:08:00Z",
              "ends_at": "2026-07-12T20:10:00Z",
              "guard_seconds": 15,
              "band": "40m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-006",
              "sequence_number": 6,
              "starts_at": "2026-07-12T20:10:00Z",
              "ends_at": "2026-07-12T20:12:00Z",
              "guard_seconds": 15,
              "band": "40m",
              "planned_label": "B"
            },
            {
              "slot_id": "slot-007",
              "sequence_number": 7,
              "starts_at": "2026-07-12T20:12:00Z",
              "ends_at": "2026-07-12T20:14:00Z",
              "guard_seconds": 15,
              "band": "40m",
              "planned_label": "A"
            },
            {
              "slot_id": "slot-008",
              "sequence_number": 8,
              "starts_at": "2026-07-12T20:14:00Z",
              "ends_at": "2026-07-12T20:16:00Z",
              "guard_seconds": 15,
              "band": "40m",
              "planned_label": "B"
            }
          ]
        }
      },
      "evidence": {
        "evidence_quality": "moderate",
        "overall": {
          "observation_counts": {
            "total": 14,
            "usable": 12,
            "excluded": 2
          },
          "exclusions": [
            {
              "reason": "guard_time",
              "count": 1
            },
            {
              "reason": "near_boundary",
              "count": 1
            }
          ],
          "usable_observation_kinds": {
            "local_decode": 4,
            "public_report": 4,
            "imported_spot": 4
          },
          "snr": {
            "sample_count": 10,
            "min_db": -25.0,
            "median_db": -18.5,
            "mean_db": -18.1,
            "max_db": -11.0
          }
        },
        "antennas": [
          {
            "antenna_label": "A",
            "contributing_slot_count": 4,
            "evidence_quality": "moderate",
            "evidence": {
              "observation_counts": {
                "total": 7,
                "usable": 6,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "guard_time",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 2,
                "public_report": 2,
                "imported_spot": 2
              },
              "snr": {
                "sample_count": 5,
                "min_db": -24.0,
                "median_db": -18.0,
                "mean_db": -18.0,
                "max_db": -12.0
              }
            }
          },
          {
            "antenna_label": "B",
            "contributing_slot_count": 4,
            "evidence_quality": "moderate",
            "evidence": {
              "observation_counts": {
                "total": 7,
                "usable": 6,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "near_boundary",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 2,
                "public_report": 2,
                "imported_spot": 2
              },
              "snr": {
                "sample_count": 5,
                "min_db": -25.0,
                "median_db": -19.0,
                "mean_db": -18.2,
                "max_db": -11.0
              }
            }
          }
        ],
        "bands": [
          {
            "band": "40m",
            "evidence": {
              "observation_counts": {
                "total": 7,
                "usable": 6,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "near_boundary",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 2,
                "public_report": 2,
                "imported_spot": 2
              },
              "snr": {
                "sample_count": 4,
                "min_db": -16.0,
                "median_db": -13.5,
                "mean_db": -13.5,
                "max_db": -11.0
              }
            }
          },
          {
            "band": "20m",
            "evidence": {
              "observation_counts": {
                "total": 7,
                "usable": 6,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "guard_time",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 2,
                "public_report": 2,
                "imported_spot": 2
              },
              "snr": {
                "sample_count": 6,
                "min_db": -25.0,
                "median_db": -20.5,
                "mean_db": -21.166666666666668,
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
            "starts_at": "2026-07-12T20:00:00Z",
            "ends_at": "2026-07-12T20:02:00Z",
            "usable_start": "2026-07-12T20:00:15Z",
            "switch_event_id": "event-001",
            "switch_timestamp": "2026-07-12T20:00:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 2,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 1,
                "imported_spot": 0
              },
              "snr": {
                "sample_count": 2,
                "min_db": -24.0,
                "median_db": -22.0,
                "mean_db": -22.0,
                "max_db": -20.0
              }
            }
          },
          {
            "slot_id": "slot-002",
            "sequence_number": 2,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "starts_at": "2026-07-12T20:02:00Z",
            "ends_at": "2026-07-12T20:04:00Z",
            "usable_start": "2026-07-12T20:02:15Z",
            "switch_event_id": "event-002",
            "switch_timestamp": "2026-07-12T20:02:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 2,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 1,
                "imported_spot": 0
              },
              "snr": {
                "sample_count": 2,
                "min_db": -25.0,
                "median_db": -23.0,
                "mean_db": -23.0,
                "max_db": -21.0
              }
            }
          },
          {
            "slot_id": "slot-003",
            "sequence_number": 3,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "starts_at": "2026-07-12T20:04:00Z",
            "ends_at": "2026-07-12T20:06:00Z",
            "usable_start": "2026-07-12T20:04:15Z",
            "switch_event_id": "event-003",
            "switch_timestamp": "2026-07-12T20:04:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 1,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "guard_time",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 1
              },
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
            "slot_id": "slot-004",
            "sequence_number": 4,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "starts_at": "2026-07-12T20:06:00Z",
            "ends_at": "2026-07-12T20:08:00Z",
            "usable_start": "2026-07-12T20:06:15Z",
            "switch_event_id": "event-004",
            "switch_timestamp": "2026-07-12T20:06:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 1,
                "usable": 1,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 1
              },
              "snr": {
                "sample_count": 1,
                "min_db": -19.0,
                "median_db": -19.0,
                "mean_db": -19.0,
                "max_db": -19.0
              }
            }
          },
          {
            "slot_id": "slot-005",
            "sequence_number": 5,
            "band": "40m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "starts_at": "2026-07-12T20:08:00Z",
            "ends_at": "2026-07-12T20:10:00Z",
            "usable_start": "2026-07-12T20:08:15Z",
            "switch_event_id": "event-005",
            "switch_timestamp": "2026-07-12T20:08:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 2,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 1,
                "imported_spot": 0
              },
              "snr": {
                "sample_count": 2,
                "min_db": -16.0,
                "median_db": -14.0,
                "mean_db": -14.0,
                "max_db": -12.0
              }
            }
          },
          {
            "slot_id": "slot-006",
            "sequence_number": 6,
            "band": "40m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "starts_at": "2026-07-12T20:10:00Z",
            "ends_at": "2026-07-12T20:12:00Z",
            "usable_start": "2026-07-12T20:10:15Z",
            "switch_event_id": "event-006",
            "switch_timestamp": "2026-07-12T20:10:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 2,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 1,
                "public_report": 0,
                "imported_spot": 1
              },
              "snr": {
                "sample_count": 2,
                "min_db": -15.0,
                "median_db": -13.0,
                "mean_db": -13.0,
                "max_db": -11.0
              }
            }
          },
          {
            "slot_id": "slot-007",
            "sequence_number": 7,
            "band": "40m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "starts_at": "2026-07-12T20:12:00Z",
            "ends_at": "2026-07-12T20:14:00Z",
            "usable_start": "2026-07-12T20:12:15Z",
            "switch_event_id": "event-007",
            "switch_timestamp": "2026-07-12T20:12:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 1,
                "usable": 1,
                "excluded": 0
              },
              "exclusions": [],
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 0,
                "imported_spot": 1
              },
              "snr": null
            }
          },
          {
            "slot_id": "slot-008",
            "sequence_number": 8,
            "band": "40m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "starts_at": "2026-07-12T20:14:00Z",
            "ends_at": "2026-07-12T20:16:00Z",
            "usable_start": "2026-07-12T20:14:15Z",
            "switch_event_id": "event-008",
            "switch_timestamp": "2026-07-12T20:14:00Z",
            "switch_delay_seconds": 0,
            "evidence": {
              "observation_counts": {
                "total": 2,
                "usable": 1,
                "excluded": 1
              },
              "exclusions": [
                {
                  "reason": "near_boundary",
                  "count": 1
                }
              ],
              "usable_observation_kinds": {
                "local_decode": 0,
                "public_report": 1,
                "imported_spot": 0
              },
              "snr": null
            }
          }
        ]
      },
      "comparison": {
        "availability": "no_matched_paths",
        "left_label": "A",
        "right_label": "B",
        "delta_orientation": {
          "minuend_label": "B",
          "subtrahend_label": "A"
        },
        "diagnostics": {
          "block_count": 4,
          "eligible_block_count": 4,
          "invalid_block_count": 0,
          "left_then_right_block_count": 4,
          "right_then_left_block_count": 0,
          "paired_row_count": 0,
          "unique_path_count": 0,
          "unmatched_left_count": 5,
          "unmatched_right_count": 5,
          "missing_snr_left_count": 1,
          "missing_snr_right_count": 1,
          "missing_or_invalid_mode_count": 0,
          "ambiguous_path_count": 0,
          "exact_duplicate_count": 0,
          "conflicting_duplicate_group_count": 0,
          "excluded_observation_count": 2
        },
        "blocks": [
          {
            "block_index": 0,
            "band": "20m",
            "first_slot_id": "slot-001",
            "first_sequence_number": 1,
            "first_starts_at": "2026-07-12T20:00:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-002",
            "second_sequence_number": 2,
            "second_starts_at": "2026-07-12T20:02:00Z",
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
            "first_starts_at": "2026-07-12T20:04:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-004",
            "second_sequence_number": 4,
            "second_starts_at": "2026-07-12T20:06:00Z",
            "second_label": "B",
            "second_status": "switched",
            "order": "left_then_right",
            "eligibility": "eligible"
          },
          {
            "block_index": 2,
            "band": "40m",
            "first_slot_id": "slot-005",
            "first_sequence_number": 5,
            "first_starts_at": "2026-07-12T20:08:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-006",
            "second_sequence_number": 6,
            "second_starts_at": "2026-07-12T20:10:00Z",
            "second_label": "B",
            "second_status": "switched",
            "order": "left_then_right",
            "eligibility": "eligible"
          },
          {
            "block_index": 3,
            "band": "40m",
            "first_slot_id": "slot-007",
            "first_sequence_number": 7,
            "first_starts_at": "2026-07-12T20:12:00Z",
            "first_label": "A",
            "first_status": "switched",
            "second_slot_id": "slot-008",
            "second_sequence_number": 8,
            "second_starts_at": "2026-07-12T20:14:00Z",
            "second_label": "B",
            "second_status": "switched",
            "order": "left_then_right",
            "eligibility": "eligible"
          }
        ],
        "overlap_rows": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "remote_path": "N2FFF",
            "left_finite_count": 1,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "remote_path": "N4JJJ",
            "left_finite_count": 0,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "remote_path": "K1III",
            "left_finite_count": 0,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 1,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "remote_path": "W1HHH",
            "left_finite_count": 0,
            "right_finite_count": 1,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "remote_path": "K9XYZ",
            "left_finite_count": 1,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "remote_path": "VE3ZZZ",
            "left_finite_count": 0,
            "right_finite_count": 1,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "remote_path": "K5CCC",
            "left_finite_count": 1,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "remote_path": "W8DDD",
            "left_finite_count": 0,
            "right_finite_count": 1,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "remote_path": "K2EEE",
            "left_finite_count": 1,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "remote_path": "K3GGG",
            "left_finite_count": 0,
            "right_finite_count": 1,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "remote_path": "K1ABC",
            "left_finite_count": 1,
            "right_finite_count": 0,
            "paired_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "remote_path": "W3AAA",
            "left_finite_count": 0,
            "right_finite_count": 1,
            "paired_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          }
        ],
        "timeline_rows": [
          {
            "block_index": 0,
            "block_eligible": true,
            "sequence_number": 1,
            "slot_id": "slot-001",
            "starts_at": "2026-07-12T20:00:00Z",
            "band": "20m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
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
            "starts_at": "2026-07-12T20:02:00Z",
            "band": "20m",
            "actual_label": "B",
            "side": "right",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 1,
            "block_eligible": true,
            "sequence_number": 3,
            "slot_id": "slot-003",
            "starts_at": "2026-07-12T20:04:00Z",
            "band": "20m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 1,
            "block_eligible": true,
            "sequence_number": 4,
            "slot_id": "slot-004",
            "starts_at": "2026-07-12T20:06:00Z",
            "band": "20m",
            "actual_label": "B",
            "side": "right",
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
            "block_index": 2,
            "block_eligible": true,
            "sequence_number": 5,
            "slot_id": "slot-005",
            "starts_at": "2026-07-12T20:08:00Z",
            "band": "40m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 2,
            "block_eligible": true,
            "sequence_number": 6,
            "slot_id": "slot-006",
            "starts_at": "2026-07-12T20:10:00Z",
            "band": "40m",
            "actual_label": "B",
            "side": "right",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 2,
            "excluded_observation_count": 0,
            "missing_snr_count": 0,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 3,
            "block_eligible": true,
            "sequence_number": 7,
            "slot_id": "slot-007",
            "starts_at": "2026-07-12T20:12:00Z",
            "band": "40m",
            "actual_label": "A",
            "side": "left",
            "status": "switched",
            "total_observation_count": 1,
            "usable_observation_count": 1,
            "excluded_observation_count": 0,
            "missing_snr_count": 1,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          },
          {
            "block_index": 3,
            "block_eligible": true,
            "sequence_number": 8,
            "slot_id": "slot-008",
            "starts_at": "2026-07-12T20:14:00Z",
            "band": "40m",
            "actual_label": "B",
            "side": "right",
            "status": "switched",
            "total_observation_count": 2,
            "usable_observation_count": 1,
            "excluded_observation_count": 1,
            "missing_snr_count": 1,
            "missing_or_invalid_mode_count": 0,
            "ambiguous_path_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0
          }
        ],
        "paired_rows": [],
        "path_summaries": [],
        "strata": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 0,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 1,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 0,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 1,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 1,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "paired_row_count": 0,
            "unique_path_count": 0,
            "contributing_block_count": 0,
            "left_then_right_block_count": 0,
            "right_then_left_block_count": 0,
            "unmatched_left_count": 1,
            "unmatched_right_count": 1,
            "missing_snr_left_count": 0,
            "missing_snr_right_count": 0,
            "excluded_observation_count": 0,
            "exact_duplicate_count": 0,
            "conflicting_duplicate_group_count": 0,
            "minimum_delta_right_minus_left_db": null,
            "median_path_delta_right_minus_left_db": null,
            "maximum_delta_right_minus_left_db": null
          }
        ],
        "observed_path_profiles": [
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "N2FFF",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN13",
                  "distance_km": 430.0,
                  "initial_bearing_degrees": 285.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  2
                ],
                "slot_ids": [
                  "slot-005"
                ],
                "observation_ids": [
                  "obs-009"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -12.0,
                  "median_db": -12.0,
                  "mean_db": -12.0,
                  "max_db": -12.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "N4JJJ",
                "location": {
                  "availability": "available",
                  "remote_grid": "FM18",
                  "distance_km": 610.0,
                  "initial_bearing_degrees": 230.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  3
                ],
                "slot_ids": [
                  "slot-008"
                ],
                "observation_ids": [
                  "obs-013"
                ],
                "snr": null
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K1III",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN32",
                  "distance_km": 140.0,
                  "initial_bearing_degrees": 246.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  3
                ],
                "slot_ids": [
                  "slot-007"
                ],
                "observation_ids": [
                  "obs-012"
                ],
                "snr": null
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "W1HHH",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN54",
                  "distance_km": 280.0,
                  "initial_bearing_degrees": 37.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  2
                ],
                "slot_ids": [
                  "slot-006"
                ],
                "observation_ids": [
                  "obs-011"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -11.0,
                  "median_db": -11.0,
                  "mean_db": -11.0,
                  "max_db": -11.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K9XYZ",
                "location": {
                  "availability": "available",
                  "remote_grid": "EN52",
                  "distance_km": 1350.0,
                  "initial_bearing_degrees": 276.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  0
                ],
                "slot_ids": [
                  "slot-001"
                ],
                "observation_ids": [
                  "obs-002"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -20.0,
                  "median_db": -20.0,
                  "mean_db": -20.0,
                  "max_db": -20.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "public_report",
              "source": "wsprnet"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "VE3ZZZ",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN03",
                  "distance_km": 640.0,
                  "initial_bearing_degrees": 294.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  0
                ],
                "slot_ids": [
                  "slot-002"
                ],
                "observation_ids": [
                  "obs-004"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -21.0,
                  "median_db": -21.0,
                  "mean_db": -21.0,
                  "max_db": -21.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K5CCC",
                "location": {
                  "availability": "available",
                  "remote_grid": "EM13",
                  "distance_km": 2380.0,
                  "initial_bearing_degrees": 247.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  1
                ],
                "slot_ids": [
                  "slot-003"
                ],
                "observation_ids": [
                  "obs-006"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -18.0,
                  "median_db": -18.0,
                  "mean_db": -18.0,
                  "max_db": -18.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "transmit",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "imported_spot",
              "source": "imported_file"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "W8DDD",
                "location": {
                  "availability": "available",
                  "remote_grid": "EN81",
                  "distance_km": 890.0,
                  "initial_bearing_degrees": 267.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  1
                ],
                "slot_ids": [
                  "slot-004"
                ],
                "observation_ids": [
                  "obs-007"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -19.0,
                  "median_db": -19.0,
                  "mean_db": -19.0,
                  "max_db": -19.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K2EEE",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN31",
                  "distance_km": 180.0,
                  "initial_bearing_degrees": 222.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  2
                ],
                "slot_ids": [
                  "slot-005"
                ],
                "observation_ids": [
                  "obs-008"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -16.0,
                  "median_db": -16.0,
                  "mean_db": -16.0,
                  "max_db": -16.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "40m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K3GGG",
                "location": {
                  "availability": "available",
                  "remote_grid": "FN20",
                  "distance_km": 320.0,
                  "initial_bearing_degrees": 217.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  2
                ],
                "slot_ids": [
                  "slot-006"
                ],
                "observation_ids": [
                  "obs-010"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -15.0,
                  "median_db": -15.0,
                  "mean_db": -15.0,
                  "max_db": -15.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "side": "left",
            "antenna_label": "A",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "K1ABC",
                "location": {
                  "availability": "available",
                  "remote_grid": "EM12",
                  "distance_km": 2500.0,
                  "initial_bearing_degrees": 250.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  0
                ],
                "slot_ids": [
                  "slot-001"
                ],
                "observation_ids": [
                  "obs-001"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -24.0,
                  "median_db": -24.0,
                  "mean_db": -24.0,
                  "max_db": -24.0
                }
              }
            ]
          },
          {
            "stratum": {
              "direction": "receive",
              "band": "20m",
              "mode": "WSPR",
              "observation_kind": "local_decode",
              "source": "wsjtx_log"
            },
            "side": "right",
            "antenna_label": "B",
            "unique_path_count": 1,
            "located_path_count": 1,
            "missing_location_path_count": 0,
            "inconsistent_location_path_count": 0,
            "paths": [
              {
                "remote_path": "W3AAA",
                "location": {
                  "availability": "available",
                  "remote_grid": "FM19",
                  "distance_km": 650.0,
                  "initial_bearing_degrees": 231.0
                },
                "block_support_count": 1,
                "slot_support_count": 1,
                "observation_count": 1,
                "block_indices": [
                  0
                ],
                "slot_ids": [
                  "slot-002"
                ],
                "observation_ids": [
                  "obs-003"
                ],
                "snr": {
                  "sample_count": 1,
                  "min_db": -25.0,
                  "median_db": -25.0,
                  "mean_db": -25.0,
                  "max_db": -25.0
                }
              }
            ]
          }
        ]
      },
      "coverage_overlap": [
        {
          "stratum": {
            "direction": "transmit",
            "band": "40m",
            "mode": "WSPR",
            "observation_kind": "public_report",
            "source": "wsprnet"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "N2FFF",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    2
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "N4JJJ",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    3
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        },
        {
          "stratum": {
            "direction": "transmit",
            "band": "40m",
            "mode": "WSPR",
            "observation_kind": "imported_spot",
            "source": "imported_file"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K1III",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    3
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "W1HHH",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    2
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        },
        {
          "stratum": {
            "direction": "transmit",
            "band": "20m",
            "mode": "WSPR",
            "observation_kind": "public_report",
            "source": "wsprnet"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K9XYZ",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    0
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "VE3ZZZ",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    0
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        },
        {
          "stratum": {
            "direction": "transmit",
            "band": "20m",
            "mode": "WSPR",
            "observation_kind": "imported_spot",
            "source": "imported_file"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K5CCC",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    1
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "W8DDD",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    1
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        },
        {
          "stratum": {
            "direction": "receive",
            "band": "40m",
            "mode": "WSPR",
            "observation_kind": "local_decode",
            "source": "wsjtx_log"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K2EEE",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    2
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K3GGG",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    2
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        },
        {
          "stratum": {
            "direction": "receive",
            "band": "20m",
            "mode": "WSPR",
            "observation_kind": "local_decode",
            "source": "wsjtx_log"
          },
          "observed": {
            "eligible_block_count": 2,
            "left_only_unique_path_count": 1,
            "shared_unique_path_count": 0,
            "right_only_unique_path_count": 1,
            "total_system_unique_path_count": 2,
            "incremental_left_path_count": 1,
            "incremental_right_path_count": 1,
            "left": {
              "side": "left",
              "antenna_label": "A",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "K1ABC",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    0
                  ]
                }
              ]
            },
            "right": {
              "side": "right",
              "antenna_label": "B",
              "unique_endpoint_count": 1,
              "path_block_observation_count": 1,
              "observed_once_path_count": 1,
              "repeated_path_count": 0,
              "block_count_distribution": [
                {
                  "observed_block_count": 1,
                  "unique_path_count": 1
                }
              ],
              "paths": [
                {
                  "remote_path": "W3AAA",
                  "observed_block_count": 1,
                  "observation_count": 1,
                  "left_then_right_block_count": 1,
                  "right_then_left_block_count": 0,
                  "block_indices": [
                    0
                  ]
                }
              ]
            }
          },
          "common_opportunity": null
        }
      ],
      "solar_context": {
        "algorithm": {
          "algorithm_id": "noaa-gml-fractional-year",
          "algorithm_version": 1,
          "coordinate_method": "maidenhead-cell-center-v1"
        },
        "rows": []
      },
      "chart_data": {
        "antenna_snr": [
          {
            "antenna_label": "A",
            "usable_observation_count": 6,
            "snr": {
              "sample_count": 5,
              "min_db": -24.0,
              "median_db": -18.0,
              "mean_db": -18.0,
              "max_db": -12.0
            }
          },
          {
            "antenna_label": "B",
            "usable_observation_count": 6,
            "snr": {
              "sample_count": 5,
              "min_db": -25.0,
              "median_db": -19.0,
              "mean_db": -18.2,
              "max_db": -11.0
            }
          }
        ],
        "band_evidence_counts": [
          {
            "band": "40m",
            "observation_counts": {
              "total": 7,
              "usable": 6,
              "excluded": 1
            },
            "usable_observation_kinds": {
              "local_decode": 2,
              "public_report": 2,
              "imported_spot": 2
            }
          },
          {
            "band": "20m",
            "observation_counts": {
              "total": 7,
              "usable": 6,
              "excluded": 1
            },
            "usable_observation_kinds": {
              "local_decode": 2,
              "public_report": 2,
              "imported_spot": 2
            }
          }
        ],
        "slot_evidence_counts": [
          {
            "slot_id": "slot-001",
            "sequence_number": 1,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 2,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-002",
            "sequence_number": 2,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 2,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-003",
            "sequence_number": 3,
            "band": "20m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 1,
              "excluded": 1
            }
          },
          {
            "slot_id": "slot-004",
            "sequence_number": 4,
            "band": "20m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-005",
            "sequence_number": 5,
            "band": "40m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 2,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-006",
            "sequence_number": 6,
            "band": "40m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 2,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-007",
            "sequence_number": 7,
            "band": "40m",
            "planned_label": "A",
            "actual_label": "A",
            "status": "switched",
            "observation_counts": {
              "total": 1,
              "usable": 1,
              "excluded": 0
            }
          },
          {
            "slot_id": "slot-008",
            "sequence_number": 8,
            "band": "40m",
            "planned_label": "B",
            "actual_label": "B",
            "status": "switched",
            "observation_counts": {
              "total": 2,
              "usable": 1,
              "excluded": 1
            }
          }
        ]
      },
      "notices": [],
      "snapshot": {
        "checkpoint_revision": null,
        "lifecycle": null,
        "lifecycle_events": [],
        "operator_events": [
          {
            "event_id": "event-001",
            "occurred_at": "2026-07-12T20:00:00Z",
            "slot_id": "slot-001",
            "affected_slot_id": "slot-001",
            "kind": "switched",
            "detail": "A connected",
            "correction": null
          },
          {
            "event_id": "event-002",
            "occurred_at": "2026-07-12T20:02:00Z",
            "slot_id": "slot-002",
            "affected_slot_id": "slot-002",
            "kind": "switched",
            "detail": "B connected",
            "correction": null
          },
          {
            "event_id": "event-003",
            "occurred_at": "2026-07-12T20:04:00Z",
            "slot_id": "slot-003",
            "affected_slot_id": "slot-003",
            "kind": "switched",
            "detail": "A connected",
            "correction": null
          },
          {
            "event_id": "event-004",
            "occurred_at": "2026-07-12T20:06:00Z",
            "slot_id": "slot-004",
            "affected_slot_id": "slot-004",
            "kind": "switched",
            "detail": "B connected",
            "correction": null
          },
          {
            "event_id": "event-005",
            "occurred_at": "2026-07-12T20:08:00Z",
            "slot_id": "slot-005",
            "affected_slot_id": "slot-005",
            "kind": "switched",
            "detail": "A connected",
            "correction": null
          },
          {
            "event_id": "event-006",
            "occurred_at": "2026-07-12T20:10:00Z",
            "slot_id": "slot-006",
            "affected_slot_id": "slot-006",
            "kind": "switched",
            "detail": "B connected",
            "correction": null
          },
          {
            "event_id": "event-007",
            "occurred_at": "2026-07-12T20:12:00Z",
            "slot_id": "slot-007",
            "affected_slot_id": "slot-007",
            "kind": "switched",
            "detail": "A connected",
            "correction": null
          },
          {
            "event_id": "event-008",
            "occurred_at": "2026-07-12T20:14:00Z",
            "slot_id": "slot-008",
            "affected_slot_id": "slot-008",
            "kind": "switched",
            "detail": "B connected",
            "correction": null
          }
        ],
        "adapter_evidence": {
          "record_count": 0,
          "accepted_count": 0,
          "malformed_count": 0,
          "unsupported_count": 0,
          "filtered_count": 0,
          "duplicate_count": 0,
          "partially_normalized_count": 0,
          "gap_count": 0,
          "workflow_status": "not_configured",
          "provider_completeness": "unsupported"
        }
      },
      "exclusion_records": [
        {
          "observation_id": "obs-005",
          "reason": "guard_time",
          "timestamp": "2026-07-12T20:04:10Z",
          "band": "20m",
          "observation_kind": "imported_spot",
          "source": "imported_file",
          "mode": "WSPR",
          "slot_id": "slot-003",
          "assigned_label": "A",
          "assignment_confidence": 0.25
        },
        {
          "observation_id": "obs-014",
          "reason": "near_boundary",
          "timestamp": "2026-07-12T20:15:55Z",
          "band": "40m",
          "observation_kind": "public_report",
          "source": "wsprnet",
          "mode": "WSPR",
          "slot_id": "slot-008",
          "assigned_label": "B",
          "assignment_confidence": 0.6
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
