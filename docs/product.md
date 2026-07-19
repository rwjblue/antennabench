# How AntennaBench Works

AntennaBench helps radio operators run antenna comparisons that are repeatable,
auditable, and honest about uncertainty. The first supported workflow uses WSPR,
whose fixed two-minute [cycles](glossary.md#wspr-cycle) make it practical to
alternate antennas often while propagation is changing.

The app does not reduce an experiment to a single “winner.” It keeps the plan,
operator actions, observations, missing data, and corrections separate, then
builds a report that stays close to that evidence.

## Before The Run

Setup starts with **What do you want to learn?** Choose whether to compare the
whole station, ask where you are heard better, ask what you can hear better, or
profile one antenna. That choice selects an existing experiment mode and a
compatible goal default; the mode and coverage goal remain visible and editable.
It is not saved as a desired result.

Then enter:

- station details such as callsign, Maidenhead grid, and transmit power;
- one or more [antenna labels](glossary.md#antenna-label) and optional
  installation notes (two or more for an A/B comparison);
- the band, experiment direction, and number of complete repetitions; and
- whether AntennaBench should gather delayed
  [public WSPR reports](glossary.md#public-report) from WSPR.live.

The default **Both (TX + RX)** mode schedules one receive period and one transmit
period for every antenna in each repetition. The normalized review shows the
exact directed counterbalanced order, antenna-versus-direction changes, cycle
count, and required WSPR cycle time before session creation. It also states what
the plan can describe—such as same-path differences, overlap and unmatched
paths, available band/direction/distance/azimuth context, and run-quality
limits—and what it cannot establish. A run does not prove universal gain,
causal superiority, that a missing [observation](glossary.md#observation) is
zero, complete public collection, a winner, or practical equivalence.
Counterbalancing reduces but does not eliminate time and propagation confounding.

When using WSPR.live, enable **Upload spots** in WSJT-X and keep WSJT-X online.
Local WSJT-X UDP reception is optional on that path and can provide separately
attributed direct evidence. For an offline receive-capable run, local WSJT-X
reception is required.

## During The Run

Before **Start session** or **Resume session**, Active Run shows the committed
band, power context, next WSPR direction, and the WSJT-X settings the operator
must configure. The operator must acknowledge that local checklist each time a
session is created or reopened. The acknowledgement enables the action but is
not durable evidence or proof of the companion application or radio state.

AntennaBench presents one current instruction at a time:

1. Switch to the named antenna.
2. Set WSJT-X transmit enable as instructed for that period.
3. Press the named antenna’s **ready** action after the physical change is
   complete.

AntennaBench then selects the next eligible even-minute WSPR cycle. There are no
setup-time timestamps or switch deadlines. The recorded readiness action—not the
planned schedule—determines when the antenna is known to be in use.

During the run you can mark a missed or bad cycle, add notes, record corrections,
interrupt and resume the session, or continue manually after an optional adapter
or controller fails. Earlier evidence is retained rather than silently rewritten.

## Evidence Sources

A session can contain several kinds of evidence without mixing their meaning:

- **WSPR.live public reports** are gathered automatically by default for configured
  WSPR windows. Collection is best effort; the upstream mirror does not provide
  an independent completeness guarantee.
- **[Local WSJT-X decodes](glossary.md#local-decode)** arrive directly over the
  loopback interface and remain distinct from delayed public data.
- **[Imported WSPR or Reverse Beacon Network spots](glossary.md#imported-spot)**
  support bounded historical or controlled non-WSPR analysis.
- **Operator facts** include readiness actions, missed or bad cycles, notes,
  interruptions, and corrections.

Every source retains its provenance. A network or adapter failure is recorded as
a gap and does not prevent export of evidence that is already safe on disk.

## Reading The Report

The report leads with a plain-language answer, then keeps same-path signal,
reach, location context, run quality, planned-versus-actual history, exclusions,
and audit evidence available without combining unlike
[comparison groups](glossary.md#comparison-group-internally-stratum). A missing
observation is not a zero-strength signal, and “insufficient data” is a useful
outcome when the recorded session cannot support a comparison.

Use [How To Read Your AntennaBench Report](reading-your-report.md) for a
section-by-section walkthrough of the full report, all five availability states,
the compact summary, and the lossless bundle boundary. AntennaBench currently
provides descriptive evidence, not an automatic verdict.

## Local-First By Design

Creating, conducting, reviewing, and exporting a session requires no account.
The [session bundle](bundle-format.md) on your computer is the durable record; the
report is derived from it and can be regenerated.

Working sessions and reusable station preferences live in the platform-standard
application-data directory. Preferences are convenience state, not experiment
evidence. Export creates the portable bundle or standalone HTML report that you
choose to keep or share.

On macOS, **Use current location** requests one foreground location only after you
press it. AntennaBench converts that location to a six-character Maidenhead grid,
stores the grid, and does not retain the raw coordinates. Manual entry remains
available if permission is denied or the lookup fails. The native boundary and
fallbacks have deterministic coverage; the fresh-install system permission
prompt remains part of the field-alpha verification.

## Optional Controller Assistance

Advanced operators can attach a machine-local program that switches an antenna
and optionally verifies the resulting state. Setup can keep invocation
operator-triggered or let Rust prepare each intention automatically after an
explicit Start/Resume. Manual review remains on by default. When it is disabled,
an independent verification command is required and two successful commands
atomically authorize the next eligible WSPR boundary. Manual operation remains
available after any command failure.

Executable profiles and antenna mappings stay on the local computer. Portable
bundles may retain the commands and bounded diagnostics that actually ran, so
review them for paths, usernames, addresses, or credentials before sharing. See
[Local Antenna Controller Profiles](antenna-controller-profiles.md).

## Available Today

The desktop app can create and reopen sessions, run manual WSPR comparisons,
collect optional WSJT-X and WSPR.live evidence, import supported WSPR and RBN
data, recover interrupted sessions, render local reports, and export reports or
verified bundle copies.

The public information site at `antennabench.com` explains that local workflow
and serves the repository's generated canonical sample. The site is not an
account, upload, or hosted-report product, and the desktop remains fully usable
without it.

A signed end-user release, automated antenna conclusions, and hosted report
publishing are not yet available. See the
[roadmap](roadmap.md) for current direction and the
[Product Design Reference](product-design-reference.md) for implementation-level
product invariants.
