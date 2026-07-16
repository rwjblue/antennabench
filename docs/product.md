# Product Overview

AntennaBench helps radio operators run more trustworthy antenna experiments.
It turns an A/B test from a collection of log files and handwritten switch
times into a guided session with a durable record and a report that stays close
to the evidence.

The first focus is WSPR, where changing propagation and different reporting
paths make casual comparisons easy to overstate. AntennaBench helps control the
experiment and makes its limitations visible.

## What You Do

Before transmitting, describe the station and antennas and choose the timing
for a repeatable schedule. During the session, AntennaBench shows the
current and next slot. You confirm the antenna actually in use and can mark a
slot missed or bad, add a note, or correct an earlier entry.

Observation sources are optional. You can run the session manually, receive
local WSPR decodes from WSJT-X, and bring in supported public observations. A
network or adapter failure does not prevent you from recording operator actions
or exporting the session.

When the run is over, AntennaBench builds a local report and can export:

- a standalone HTML report for reading or sharing;
- the complete session bundle for archiving, reopening, or future analysis.

## What The Report Says

The report describes the evidence before it draws attention to differences. It
can show coverage by antenna, band, and slot; same-path SNR differences;
unmatched observations; time and switching-order context; and available path
distance or direction.

It also says what is missing. A planned switch is not treated as a confirmed
switch. A missing decode is not treated as a zero-strength signal. Conflicting,
damaged, or ineligible records remain disclosed instead of quietly improving
the result.

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
system for location to estimate a six-character station grid; it stores the resulting
grid, never the raw coordinates, and manual entry remains available.

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
