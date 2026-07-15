# Session Bundles

An AntennaBench session bundle is the portable record of one antenna
experiment. It keeps the plan, what the operator actually did, the observations
collected, and the information needed to explain a report.

Bundles are ordinary directories ending in `.session.antennabundle`. You can
copy one to another computer, archive it with your other station records, or
reopen it in a later version of AntennaBench. The app can also export a
standalone HTML report for someone who only needs to see the results.

## Why Keep A Bundle?

An antenna report is only as useful as the evidence behind it. AntennaBench
does not reduce a session to a winner or a few chart values. The bundle keeps:

- the station, antennas, and schedule you entered;
- confirmations, missed or bad slots, notes, and later corrections;
- local decodes and imported public reports;
- original adapter input when it is needed for provenance;
- enough metadata to reproduce the analysis and disclose missing evidence.

The bundle is the source of truth. Reports, charts, and search indexes can be
rebuilt from it; they do not replace it.

## What Is Inside?

You do not need to understand the files to use AntennaBench, but the layout is
deliberately inspectable. A typical bundle contains:

```text
my-test.session.antennabundle/
  manifest.json          identifies the session and format version
  station.json           station details
  antennas.json          antenna labels and installation notes
  schedule.json          the planned experiment
  events.jsonl           what happened while the session ran
  observations.jsonl     decodes and public reports
  adapter-records.jsonl  retained input and import outcomes
  session-state.json     the latest durable checkpoint
  attachments/           larger original inputs, stored by content hash
```

The `.jsonl` files are append-oriented streams: each line is one record. This
lets AntennaBench preserve corrections and recovery history instead of quietly
rewriting earlier evidence.

## Versions

Current AntennaBench sessions use one of two closely related formats:

- Schema v2 is used for WSPR and general antenna sessions.
- Schema v3 adds controlled CW and RTTY signal plans and their confirmed
  settings.

Older `.session.wsprabundle` sessions use schema v1. AntennaBench can open and
copy them without changing the source. Features that need durable live updates
require an explicit upgrade to a new `.session.antennabundle` directory.

An upgrade never overwrites the original bundle, and there is no automatic
downgrade.

## Working With Bundles Safely

- Use AntennaBench to create and update an active session. Editing stream or
  checkpoint files by hand can make their recorded identities and digests
  disagree.
- Keep the bundle export when you want the reusable experiment record. Export
  HTML when you want a convenient, read-only report to share.
- Treat imported source attachments as evidence. AntennaBench verifies their
  recorded sizes and SHA-256 digests when reading them.
- A report may exclude a damaged or ambiguous record while still explaining
  the problem. It will not silently turn missing evidence into a result.

## Technical Reference

Building an importer, inspecting validation behavior, or working on the
storage layer? See the [Bundle Format Technical Reference](bundle-format-reference.md)
for complete layouts, record semantics, upgrades, resource limits, and APIs.
