# Your First Antenna Comparison

This walkthrough compares a **Vertical** with an **Inverted V** on 20 m. It
uses the ordinary manual-switching workflow, **Both (TX + RX)**, and three
complete repetitions. The 12 planned [WSPR cycles](glossary.md#wspr-cycle)
require about 24 minutes of cycle time. With automatic WSPR.live collection
enabled, AntennaBench then waits through one five-minute ingestion grace before
the final request. Switching, confirmation, eligible-cycle boundary waits,
request execution, and recovery from failure add further wall-clock time. WSPR
is the Weak Signal Propagation Reporter mode used by WSJT-X.

## Before You Start

You need:

- a Mac running macOS 15 or later;
- AntennaBench built and launched from source by following
  [Set Up On macOS](development.md#set-up-on-macos), until a signed end-user
  release is available;
- WSJT-X configured for WSPR on 20 m and kept online; and
- **Upload spots** enabled in WSJT-X so delayed public transmit and receive
  reports can reach WSPR.live.

This walkthrough assumes that you can switch safely between both antennas and
that each one is ready for the transmit power you will use. AntennaBench tells
you when to switch and when to turn WSJT-X **Enable Tx** on or off; it does not
switch the radio or antenna for this manual run.

## Run The Comparison

1. **Launch AntennaBench.** From the repository checkout, run
   `mise run desktop:dev`. The app opens on **Saved sessions**. Select **New
   session** to begin this comparison.

2. **Choose the question.** Under **What do you want to learn?**, select
   **Compare the whole station**. Leave **Experiment mode** as
   **Both (TX + RX)** and **Goal** as **General coverage**.

3. **Enter the station.** Add your **Callsign** and Maidenhead **Grid**, the
   short location code used by amateur radio services. Under **Optional station
   details**, add **Transmit power (W)** if you know it. You may type the grid
   yourself or use **Use current location** and approve the macOS location
   request.

4. **Name the antennas.** Set the Antenna A **Antenna label** to `Vertical`
   and the Antenna B label to `Inverted V`. An
   [antenna label](glossary.md#antenna-label) is the name that will identify
   that antenna throughout the run and report. Add short descriptions if they
   will help you recognize the installations later. Leave **Antenna switching
   assistant** off for this manual run.

5. **Set the run plan.** Choose **20 m** for **Band** and enter `3` for
   **Complete rounds**. Here, one complete round is one repetition: every
   antenna gets one receive cycle and one transmit cycle. The resulting plan
   should contain 12 directed WSPR cycles with about 24 minutes of required
   cycle time. Because automatic WSPR.live collection is enabled below, the
   estimate also shows one five-minute ingestion grace after the final cycle.
   This is not an exact end-to-end duration.

6. **Keep automatic public collection on.** Under **WSPR Spots**, expand
   **Offline option** and leave **Gather delayed/public WSPR.live TX and RX
   spots automatically** enabled. AntennaBench will request matching
   [public reports](glossary.md#public-report) after their WSPR windows; there
   is no separate fetch step. The final five-minute grace is best-effort timing,
   not a guarantee that WSPR.live has received every report.

7. **Review before creating anything.** Select **Review plan**.
   Check your station, antenna order, the 12-cycle sequence, and the statements
   under **This plan can describe** and **This plan cannot establish**. Fix any
   highlighted field and review again if needed. When the review is correct,
   select **Create session**. AntennaBench creates the working
   [session bundle](glossary.md#session-bundle) in the standard macOS
   application-data directory and opens **Active run**.

8. **Confirm WSJT-X, then follow one prompt at a time.** In **Before you
   start**, check the committed 20 m WSPR plan, transmit power, **Tx Pct 100%**,
   **Upload spots**, **Monitor**, and the first **Enable Tx** instruction.
   AntennaBench does not change these settings, and the check is not proof of
   radio state. Select **I configured WSJT-X for this run**, then **Start
   session**. For each planned cycle, the main prompt names the antenna and
   tells you whether **Enable Tx** should be on or off. Safely switch to the
   named antenna, make the requested WSJT-X change, and then select **Antenna
   ready**.
   AntennaBench waits for the next eligible even-minute WSPR boundary. Keep that
   antenna connected until the current cycle finishes; then repeat the
   switch → WSJT-X setting → **Antenna ready** rhythm for the next prompt.

9. **Record what actually happens.** If you cannot conduct the upcoming cycle,
   select **Skip this cycle** and optionally enter a reason. That records the
   planned cycle as missed and advances to the next one; it does not pretend the
   cycle occurred. To preserve useful context, select **Add note**, type the
   text in **Note or reason**, and select **Save entry**. For example, record
   `Rain started; checked both feedline connections.` Both actions stay in the
   bundle's evidence history.

10. **End the session.** After the final cycle, open **Run details and session
    controls** and select **End session**. AntennaBench completes the final
    automatic delayed/public WSPR.live collection before marking the session
    ended. Keep the app and WSJT-X online while it finishes. If collection
    reports an error, the run screen offers **Retry acquisition** or the explicit
    **End without public spots** choice; already recorded evidence remains in
    the working session bundle.

11. **Open the report.** Select **Local report** in the sidebar. The embedded
    report is generated locally from the latest committed bundle revision. A
    short first run may legitimately say **insufficient data**. That is the tool
    describing the available evidence honestly, and running more repetitions is
    the normal response. See [How AntennaBench Works](product.md) for the deeper
    experiment and evidence model.

    **Build and operational history** is a separate local support view above the
    scientific report. It identifies creator and later runtimes and retained
    material failures, partial outcomes, and recoveries. A reopened interrupted
    run also surfaces its latest relevant historical failure on **Active run**.
    Legacy, unavailable, capped, and failed-write states are labeled explicitly;
    an empty complete stream does not promise that every possible failure was
    recordable.

12. **Save the output you need.** The three portable choices serve different
    purposes:

    - **Export full evidence HTML** saves a standalone report with the result,
      supporting detail, and audit material. Operational support history is
      omitted by default; explicitly include only its redacted bounded view when
      the recipient needs it.
    - **Export compact summary HTML** saves a shorter standalone overview for a
      quick review or share.
    - Under **Import / export**, **Export the complete session bundle** saves the
      lossless durable record from which reports can be regenerated.

For a support request, prefer **Copy support summary** in **Build and operational
history**. Review the deterministic JSON before pasting it into an issue. It
omits station identity/grid, bundle names and paths, target identifiers,
controller output, attachments, and evidence rows by default. A complete bundle
is lossless—not sanitized—and should be shared only after reviewing all evidence
and operational records.

**Saved sessions** lists experiments created in AntennaBench's managed sessions
folder. Use **Start session**, **Continue session**, or **Resume session** for
workable sessions; use **View report** for terminal and legacy sessions. The
screen also provides **Open from another location…** for portable bundles and
Finder reveal actions for the managed folder and individual entries. Refresh
failures leave the last useful list visible, while partial-list and problem-row
messages identify entries that could not be fully inspected.

When reopening a bundle, AntennaBench checks the freshly opened lifecycle before
choosing the destination. Ready, running, and interrupted sessions return to
**Run** without automatically starting or resuming; ended, abandoned, and legacy
bundles open in **Local report**. Merely opening a report does not run crash
recovery or load acquisition and controller services.

For help interpreting the result, continue with
[Reading The Report](product.md#reading-the-report). Use the
[Operator Glossary](glossary.md) whenever a report or guide term is unfamiliar.
