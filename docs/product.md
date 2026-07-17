# Product Overview

AntennaBench helps radio operators run more trustworthy antenna experiments.
It turns an A/B test from a collection of log files and handwritten switch
times into a guided session with a durable record and a report that stays close
to the evidence.

The first focus is WSPR, where changing propagation and different reporting
paths make casual comparisons easy to overstate. AntennaBench helps control the
experiment and makes its limitations visible.

## What You Do

Before starting, describe the station and antennas and choose whether to test
transmit, receive, or both. BOTH is the default. One repetition visits every
configured antenna once in each selected direction, and setup shows the
resulting number of two-minute WSPR periods plus the ideal minimum time. Four
default BOTH repetitions with two antennas are 16 periods, or 32 ideal minutes.
Setup also identifies automatic bidirectional public spots as coming from
WSPR.live, with an offline opt-out available as a secondary choice. Operators
using this default online path enable WSJT-X **Upload spots** and keep WSJT-X
online. AntennaBench collects the rows returned for its configured request
windows on a best-effort basis; the upstream mirror does not provide an
independent completeness guarantee. There are
no setup-time timestamps or manual-switch deadlines. During the session, begin
the physical antenna change and press the named antenna's ready button once the
change is complete. AntennaBench does not ask when switching began or measure
its duration. The readiness action selects the next eligible even-minute WSPR
cycle and closes the prior antenna-occupancy interval at that recorded time.

Active Run leads with one current instruction and one primary action. Notes,
cycle skipping, corrections, receiver configuration, public-spot status, and
session internals remain available without competing with the next physical
step. Short contextual help is available beside selected unfamiliar concepts;
required instructions, validation, safety, and recovery guidance remain visible
without opening it.

Setup can optionally select a reusable local direct-process antenna controller,
map each antenna to an opaque target, and preview every normalized invocation.
On macOS/Linux a deterministic one-line tokenizer is available; Windows uses
an explicit program and ordered arguments. No shell or shell expansion is
used. During Active Run the operator may request switch and optional
verification commands, inspect bounded diagnostics, retry explicitly, edit and
reattach the local profile, or continue manually. Regardless of command
success, the named antenna-ready action remains the authority that arms the
cycle.

Local WSJT-X UDP reception is an optional direct/local source when WSPR.live is
enabled, and is required before a receive-capable run only when WSPR.live is
disabled. It remains useful offline and may run alongside the delayed/public
source; their provenance and analysis strata stay separate. Before each period, Active Run separately
identifies any antenna change and whether WSJT-X Enable Tx must be turned on or
off. WSPR.live TX and RX spots are gathered automatically by default for WSPR
plans. Successful configured-window collection is presented as best effort;
recorded network or adapter failures remain explicit and do not prevent
exporting already recorded session evidence.

When the run is over, AntennaBench builds a local report and can export:

- a standalone HTML report for reading or sharing;
- the complete session bundle for archiving, reopening, or future analysis.

## What The Report Says

The report describes the evidence before it draws attention to differences. It
can show coverage by antenna, band, and slot; same-path SNR differences;
unmatched observations; time and switching-order context; and available path
distance or direction.

Its renderer-neutral overview states the session scope, recorded lifecycle
state, comparison availability, and the fixed named delta orientation before
any presentation chooses a reading order. Each descriptive overview fact stays
separate by transmit/receive direction, band, normalized mode, observation
kind, and source. Headline deltas use medians of same-path medians, not a
pooled average of observations. When a comparison or path delta is unavailable,
the report records that state and its typed limitations rather than inventing a
zero or conclusion.

The standalone HTML turns that overview into the first reading page. It shows
no more than four scope facts, the full named delta orientation, one concise
row per retained comparison stratum, and explicit supported/not-established
statements before detailed diagnostics. Stable question links lead to
same-path signal, reach, distance/direction, run quality, and the audit
appendix. Native disclosures keep lifecycle, schedule, antenna, controller,
raw paired-row, solar, and per-slot evidence available without making the
default report one uninterrupted audit table. Required failures, unavailable
states, bounded-overview omissions, acquisition gaps, and important
limitations remain visible while those disclosures are closed.

For each retained stratum, the same-path view shows one zero-centered dot for
each unique remote path’s `right − left` median, plus a distinct median of
those path medians. Its accessible table carries the same exact values and
paired-row/block counts. The reach view separately shows unique finite paths
seen left-only, on both antennas, and right-only. Unmatched paths remain
operational evidence, never zero-SNR measurements; missing SNR, duplicates,
and conflicts remain separately accounted for and auditable. These report-owned
projections are bounded before rendering, so the HTML never groups unbounded
raw records to create the headline views.

It also says what is missing. Intended order is shown separately from observed
antenna use. A cycle switched before its 110.592-second transmission completed
has unknown antenna attribution. A missing decode is not treated as a
zero-strength signal. Conflicting, damaged, or ineligible records remain
disclosed instead of quietly improving the result.

AntennaBench does not currently declare a winning antenna. Evidence may be
interesting while still being too sparse or imbalanced for that claim.

## Local First

Creating, conducting, reviewing, and exporting a session works without an
account. The [session bundle](bundle-format.md) on your computer is the durable
experiment record; the report is derived from it and can be regenerated.

The desktop app keeps working sessions in its platform-standard application
data directory and remembers the last station details for the next setup.
Those preferences are convenience state, not session evidence. Export creates
the portable bundle you choose to archive or move elsewhere. Setup can ask the
macOS system for one foreground location to estimate a six-character station
grid. The request occurs only after pressing **Use current location** and may
show the system permission prompt. AntennaBench stores the resulting grid,
never the raw coordinates, and manual entry remains available after denial,
restriction, timeout, or another lookup failure.

Optional data services receive only the inputs disclosed for that integration.
Hosted sharing is planned as an explicit copy for convenience, not as a new
source of truth or a requirement for using the desktop app.

## Where It Fits Today

The primary workflow is a whole-station WSPR A/B comparison with manual antenna
switching. The data model also supports TX-focused, RX-focused, and
single-antenna profiling sessions. Controlled CW and RTTY comparisons can use
typed signal plans and offline Reverse Beacon Network imports, but remain a
more technical path.

Portable session evidence can now distinguish an operator-confirmed WSPR cycle
from a cycle authorized by independent command verification, and reports show
bounded switch/verification attempts and failures. AntennaBench now supports
optional local operator-triggered switch and verification profiles while
keeping executable configuration outside portable bundles. Automatic
switching and command-verified arming remain later work. Manual operation
remains the complete default workflow.

Automatic winner selection and hosted publishing are not yet available. See
[Project Status](../README.md#project-status) for the concise
current state and the [Roadmap](roadmap.md) for direction.

For the detailed evidence rules, operational boundaries, and selected hosted
design, see the [Product Design Reference](product-design-reference.md).
