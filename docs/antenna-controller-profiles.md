# Local Antenna Controller Profiles

> **Audience:** advanced operators integrating a local switch or radio-control
> program. Manual operation is the default and remains fully supported.

AntennaBench can run a machine-local program to request an antenna switch and,
optionally, a separate program to verify the resulting state. Setup freezes two
choices for the [session](glossary.md#session): whether invocation is
operator-triggered or automatic,
and whether manual review remains required. Operator-triggered invocation and
manual review are the defaults.

There is no automatic retry. A missing profile, command error, timeout, failed
verification, interrupted run, or profile edit leaves the normal manual workflow
available.

## Security And Portability Model

Controller profiles are stored only in this computer’s application-data
directory. A profile contains a name, switch command, optional verification
command, and timeout. Each session maps its
[antenna labels](glossary.md#antenna-label) to opaque controller
targets such as `1`, `2`, `north`, or `loop`.

The setup screen keeps profile selection, commands, behavior, and per-antenna
target values together. It can save a new profile or update the selected one
before a session is created. Deleting a profile removes it and its remembered
session associations from this computer. Any affected session falls back to
manual switching until another local profile is selected and allowed to run.

Portable session bundles never contain an executable profile, target mapping, or
timeout. Enabling **Run a command to switch antennas** and reviewing a local
profile explicitly gives the new session temporary process authority. Opening
or recovering a bundle cannot restore that authority: you must explicitly
select a local profile and attach it to the active session again. Internally
this temporary process authority is called “armed.”

A bundle can retain the resolved program, ordered arguments, standard output,
standard error, timing, and result from commands that actually ran. That evidence
may expose local paths, usernames, network addresses, or credentials. Review it
before sharing a bundle.

When a session contains controller attempts, **Export full evidence HTML** asks
whether to include controller command details or omit the programs, arguments,
targets, and output. **Include** is the default. Choosing **Omit** leaves the
attempt, role, controller identity, result, timing, and readiness chain visible,
marks every hidden field as omitted at export, and points readers to the lossless
bundle for the complete record. It does not alter the bundle. The Summary
already omits controller output and says that the complete audit evidence remains
in the Full evidence report or bundle.

## Creating A Profile

A reusable profile has:

- one switch command;
- an optional, independent verification command;
- a timeout from 1 through 60 seconds.

The session’s local controller association also has one target mapping for every
antenna in the session.

Disabling manual review requires an independent verification command.

Use **Save profile** to keep the entered commands for this and future sessions.
Creating the session also saves the reviewed profile. Select an existing profile
to reuse or **Update profile**; use **Delete profile** to remove it from local
application data. Target values belong to the session association rather than
the reusable profile.

On macOS and Linux, the app accepts a command line and applies a small,
predictable tokenizer for whitespace, quotes, and backslash escapes. On Windows,
the program and arguments are entered separately. In both cases AntennaBench
runs the program directly—never through a shell.

Shell expansion is not available. Environment variables, `~`, globs, pipes,
redirection, substitutions, and command chaining are passed as ordinary argument
text rather than executed.

Templates can use these placeholders:

| Placeholder | Value |
| --- | --- |
| `{antenna}` | Antenna label in the session |
| `{target}` | Local target mapped to that antenna |
| `{mode}` | Experiment mode |
| `{direction}` | `receive` or `transmit` |
| `{band}` | Scheduled amateur band |
| `{frequency_hz}` | Scheduled frequency when available |
| `{sequence}` | Cycle sequence number |
| `{intent_id}` | Stable cycle-intention identity |
| `{session_id}` | Stable session identity |
| `{callsign}` | Station callsign |

Use `{{` and `}}` for literal braces. Unknown or malformed placeholders fail
setup review. Interpolation occurs within an existing argument, so a substituted
value cannot create a new argument or executable.

## During A Run

For each pending antenna instruction, Rust runs switch and then verification
back-to-back. Automatic invocation starts only after explicit **Start** or
**Resume**. Later intentions wait until the complete prior 110.592-second WSPR
transmission interval has ended, even when the antenna label repeats.
If the trusted antenna, direction, band, and signal context are all unchanged,
the Rust transition coordinator carries the existing readiness forward and
does not invoke either controller command. A changed antenna follows the normal
switch/verification policy; changed WSJT-X or controlled-signal settings remain
operator work even after an automatic antenna verification.

In review-required mode:

1. Request the configured switch command.
2. Inspect the result and, when configured, request independent verification.
3. Confirm the actual hardware state.
4. Press the named antenna’s
   [**ready** action](glossary.md#readiness) to arm the next eligible cycle.

When review is disabled, both commands must exit zero. AntennaBench commits the
two attempt records and one `command_verified` ready event atomically, using the
verification completion time to select the next eligible WSPR boundary. Command
stdout remains diagnostic and is never parsed as state.

Command success is diagnostic evidence, not proof of the physical antenna state.
A failed attempt is retained without advancing the schedule or changing antenna
occupancy. Retry is always explicit.

After a command has completed, AntennaBench retains that captured result while
waiting for the single desktop mutation permit. A concurrent WSPR.live check or
other short foreground operation cannot cause the command to run again, turn a
successful exit into a failure, or disarm the controller merely because
admission was temporarily busy. The attempt uses one stable mutation identity,
so recovery from an uncertain persistence acknowledgement cannot duplicate its
rig records or ready event. Actual process, verification, stale-authority,
cancellation, lifecycle, and durable persistence failures keep the existing
blocked, explicit-retry behavior.

Editing a profile creates a new local revision and revokes its current arm until
you review and attach it again. Interruption, session end, session replacement,
and application shutdown also revoke the in-memory authority used to start new
commands. A failure or uncertain timeout blocks automation and disarms the local
association. Retry is explicit; editing/reattaching, the manual ready action,
pause, end, report, and export remain available.

Recovery never restores process authority. If a crash happened before the
atomic [checkpoint](glossary.md#checkpoint), explicit recovery, reattachment,
and Resume may run the repeat-safe target commands again. If the checkpoint
committed, the armed intention is projected and is not repeated. A committed
successful pair in review-required mode is shown as awaiting operator review and
is also not rerun.

## Example Integration

The checked-in [Elecraft K4/QK4 example](../examples/rig-control/elecraft-k4/README.md)
shows dependency-free Node wrappers that switch and verify KAT4 ANT1/ANT2 without
using a shell.

For the implementation-level command boundary, evidence records, output limits,
and validation rules, see the [Architecture Technical Reference](architecture-reference.md)
and [Bundle Format Technical Reference](bundle-format-reference.md).
