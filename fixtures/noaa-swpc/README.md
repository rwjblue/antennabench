# NOAA SWPC adapter fixtures

These small response fixtures capture the field shapes returned by the two NOAA
SWPC products approved in ADR 0005. They were recorded on 2026-07-13 and reduced
to representative observations so tests do not depend on the live rolling
endpoints.

- `f107.json` preserves the array, `flux`, and UTC `time_tag` shape from the
  observed 10.7 cm solar radio flux summary.
- `estimated-kp.json` preserves the integer `kp_index`, provisional decimal
  `estimated_kp`, display `kp`, and UTC `time_tag` fields from the one-minute
  planetary K-index product. Tests ensure AntennaBench selects `estimated_kp`,
  not `kp_index`.

The fixtures are source inputs, not claims about current conditions. Live
responses are never required by the test suite.
