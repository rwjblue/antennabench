# Session Bundles

An AntennaBench [session bundle](glossary.md#session-bundle) is the portable
record of one antenna experiment. It keeps what you planned, what actually
happened, the observations that were collected, and the information needed to
explain the report.

Bundles are ordinary directories ending in `.session.antennabundle`. You can copy
one to another computer, archive it with station records, or reopen it in a later
version of AntennaBench.

## Bundle, Compact Summary, Or Full HTML Report?

Export the **session bundle** when you want the reusable experiment record. It can
be reopened, rechecked, and used to regenerate reports.

Export the **compact summary HTML** for a printable one- or two-page local
share summary. It carries the scoped descriptive answer, same-path and reach
facts, run quality, limitations, and the committed session revision, while
intentionally directing readers to the complete audit evidence.

Export the **full evidence HTML** when someone needs the convenient,
read-only report with its complete audit appendix. Both HTML exports are derived
from the same committed report revision; neither replaces the bundle.

## What A Bundle Keeps

A bundle can include:

- station details, antenna descriptions, and the intended run order;
- [readiness](glossary.md#readiness) actions, missed or bad
  [cycles](glossary.md#wspr-cycle), notes, interruptions, and corrections;
- [local decodes](glossary.md#local-decode) and attributed
  [public reports](glossary.md#public-report);
- original imported inputs when they are needed to establish provenance;
- optional controller attempts and bounded diagnostics; and
- enough metadata to reproduce the supported analysis and explain missing data.

AntennaBench keeps these fact types distinct. It does not turn a planned switch
into a confirmed switch, treat a missing [spot](glossary.md#spot) as a zero, or
hide a correction by rewriting the original record.

## What Is Inside?

You do not need to inspect the files to use AntennaBench, but the layout is
intentionally readable:

```text
my-test.session.antennabundle/
  manifest.json          session identity and format version
  station.json           station details
  antennas.json          antenna labels and installation notes
  schedule.json          the intended experiment
  events.jsonl           operator actions and corrections
  observations.jsonl     decodes and public reports
  adapter-records.jsonl  attributed import and collection records
  session-state.json     the latest durable checkpoint
  attachments/           larger original inputs, stored by content hash
```

The `.jsonl` files store one record per line. New facts and corrections are
appended so the history remains inspectable.

## Compatibility

New sessions currently use bundle schema v5. AntennaBench has explicit readers
and upgrade paths for earlier pre-release bundle versions; it does not silently
rewrite an older bundle in place. Use the app’s open, upgrade, and export paths
rather than editing bundle files by hand.

## Handling Bundles Safely

- Keep the bundle when you may want to revisit or reanalyze the experiment.
- Use AntennaBench to modify an active session. Manual edits can make recorded
  identities, lengths, and digests disagree.
- Treat imported attachments as evidence. AntennaBench verifies their recorded
  size and SHA-256 digest when reading them.
- Review bundles before sharing. Notes, imported source material, and controller
  diagnostics can contain station details, local paths, usernames, addresses, or
  other sensitive text.
- A damaged or ambiguous record may receive an
  [exclusion](glossary.md#exclusion) while the report still explains the
  problem. It is not silently converted into a result.

Building an importer or working on storage internals? See the
[Bundle Format Technical Reference](bundle-format-reference.md) for complete
layouts, record semantics, upgrades, validation profiles, resource limits, and
APIs.
