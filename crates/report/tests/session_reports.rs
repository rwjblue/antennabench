use std::path::PathBuf;

use antennabench_analysis::EvidenceQuality;
use antennabench_report::{build_report, ReportNotice, UsableObservationKindCounts};
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
      "notices": []
    }
    "#);
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
      "notices": []
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
