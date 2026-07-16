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
Setup also identifies automatic transmit-path public spots as coming from
WSPR.live, with an offline opt-out available as a secondary choice. There are
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

Local WSJT-X UDP reception is required when the plan includes receive periods
and optional for transmit-only runs. Before each period, Active Run separately
identifies any antenna change and whether WSJT-X Enable Tx must be turned on or
off. WSPR.live public spots are gathered automatically by default for plans
that transmit, with source and completeness disclosed in the report. A network
or adapter failure does not prevent exporting already recorded session evidence.

When the run is over, AntennaBench builds a local report and can export:

- a standalone HTML report for reading or sharing;
- the complete session bundle for archiving, reopening, or future analysis.

## What The Report Says

The report describes the evidence before it draws attention to differences. It
can show coverage by antenna, band, and slot; same-path SNR differences;
unmatched observations; time and switching-order context; and available path
distance or direction.

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

Rig control, automatic winner selection, and hosted publishing are not yet
available. See [Project Status](../README.md#project-status) for the concise
current state and the [Roadmap](roadmap.md) for direction.

For the detailed evidence rules, operational boundaries, and selected hosted
design, see the [Product Design Reference](product-design-reference.md).
