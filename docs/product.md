# Product

AntennaBench is a local-first app for comparing and profiling antennas using
WSPR observations.

The first product target is a desktop workflow that helps an operator run a
controlled WSPR session, preserve the evidence, and generate conservative
reports. The app should favor honest evidence quality over simple winner claims.

## Core Workflow

The intended workflow is:

1. Record station basics such as callsign, grid, and power.
2. Define one or more antennas with freeform labels and optional installation
   details.
3. Define a schedule of WSPR slots across bands and antenna labels.
4. Record operator events such as switched, missed slot, bad slot, notes, and
   session end.
5. Ingest local and external observations.
6. Align observations to planned slots, preserving confidence and uncertainty.
7. Export a portable session bundle.
8. Generate reports from the bundle.

## V1 Bias

V1 should prioritize collecting trustworthy local evidence over building a
large public community surface.

Default mode is whole-station A/B testing. TX-focused, RX-focused, and
single-antenna profiling modes are part of the data model and can grow from the
same bundle shape.

WSJT-X companion mode is the expected first integration path. Native WSPR,
mobile operation, deeper rig control, public search, and hosted publishing are
later layers.
