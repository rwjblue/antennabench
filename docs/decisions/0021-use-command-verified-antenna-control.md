# 0021: Use Command-Verified Antenna Control

## Status

Accepted.

This decision narrows the control deferral in Decision 0019 for antenna
selection only. Its passive WSJT-X observation boundary and its deferral of
frequency, mode, PTT, keying, and general rig control remain unchanged.

## Context

AntennaBench currently conducts operator-paced WSPR cycles. After a completed
cycle, it names the next antenna and direction; the operator changes the
station, then presses the named ready button. That explicit readiness action
starts the next antenna-occupancy interval and selects the next eligible WSPR
boundary.

Some stations can select an antenna through a radio CAT command, a network or
serial controller, a modified relay switch, or another local program. No one
device protocol covers those cases. Building a native protocol or Hamlib
surface first would couple the product to one subset of stations and would not
serve custom controllers.

An external program is the narrow common denominator, but command control adds
authority and evidence questions that a button alone cannot answer. Process
success is not physical antenna confirmation. A crash can occur after the
external side effect but before AntennaBench commits its result. Imported
session data must never gain authority to execute a local program. Fully
automatic operation must also remain distinguishable from an operator's
confirmation in the durable record and report.

## Decision

AntennaBench will add an optional, session-armed antenna controller that invokes
locally configured programs. Manual/no-controller operation remains the
default, complete workflow and the fallback after every controller failure.

Command-controlled sessions select two independent policies during setup:

- invocation is operator-triggered or automatic when the next cycle becomes
  eligible for preparation; and
- manual review is required or command verification may authorize the cycle.

Manual review is required by default. Disabling it requires both a switch
command and a verification command. A switch command with no independent
verification can assist an operator but can never authorize automatic cycle
arming.

The switch and verification commands run for every intention because direction,
band, or another context value can change while the antenna label stays the
same. A controller program may decide that a request is already satisfied and
return successfully without changing hardware.

The durable experiment mode is part of every invocation context. Direction
alone cannot distinguish an RX-focused session from the receive intention of a
whole-station comparison, while a controller may need that distinction to
select the correct receive and transmit paths.

## Direct Process Contract

Commands execute directly as one program and an argument array. AntennaBench
does not invoke a shell on macOS, Linux, or Windows.

The macOS and Linux UI may accept a familiar one-line command field. A small,
deterministic tokenizer converts it to the canonical program and argument
array. Whitespace separates tokens, single and double quotes group text, and a
backslash escapes the following character. Quoting affects tokenization only;
no shell evaluates the result. A Windows UI may expose the program picker and
argument array directly. Both presentations produce the same stored local
model.

There is no environment-variable expansion, tilde expansion, globbing,
redirection, pipeline, command substitution, or command chaining. Text such as
`|`, `>`, `*`, `$()`, and `&&` is an ordinary argument. Setup review shows the
canonical program and indexed argument array so this behavior is visible.

The first template contract provides these placeholders:

| Placeholder | Value |
| --- | --- |
| `{antenna}` | configured AntennaBench antenna label |
| `{target}` | opaque controller target mapped to that antenna |
| `{mode}` | durable experiment mode: `whole_station_ab`, `tx_focused`, `rx_focused`, or `single_antenna_profiling` |
| `{direction}` | `receive` or `transmit` |
| `{band}` | bundle band identifier such as `20m` |
| `{frequency_hz}` | exact planned frequency when present, otherwise an empty string |
| `{sequence}` | one-based intention sequence number |
| `{intent_id}` | durable cycle-intention identifier |
| `{session_id}` | durable session identifier |
| `{callsign}` | configured station callsign |

`{{` and `}}` encode literal braces. An unknown placeholder is a setup error.
Tokenization occurs before interpolation, and each value replaces text within
one existing token. An interpolated value therefore cannot create another
argument or executable. The implementation places fixed byte limits on the
template, canonical program, argument count, individual arguments, and complete
expanded invocation.

Executable templates, argument templates, antenna-to-target mappings, and
timeouts are local application configuration. A session bundle never supplies
an executable profile and opening or importing a bundle never enables one. A
local profile must be explicitly attached and armed for that session. The
portable plan records the selected control and review policy, while actual
invocations record the controller that was used.

## Execution And Confirmation

For each pending intention, automatic control waits until the complete prior
WSPR transmission interval has ended. The first intention becomes eligible
only after the operator explicitly starts or resumes the session. Rust owns the
coordinator and process lifetime; browser timers do not issue hardware
commands.

The controller performs these steps without automatic retry:

1. invoke the switch program with the intended antenna context;
2. if it exits zero, invoke the verification program with the same context;
3. treat verification exit zero as the program's assertion that the expected
   physical antenna state is current;
4. wait for the named operator readiness action when manual review is required;
   or
5. when review is not required, durably commit command verification and arm the
   next eligible WSPR boundary.

A switch exit of zero means only that the request completed. Verification
stdout is diagnostic and is not parsed as state; the verification program owns
device-specific read-back, settling, polling, and comparison before choosing
its exit status. A nonzero exit, spawn error, signal termination, or timeout
blocks automation and arms no cycle. A timeout is explicitly uncertain because
the external side effect may already have occurred.

The UI offers retry, local controller editing, pause/end, and the existing
manual ready action. Retry is always an explicit operator action. A manual
ready action after any command history remains operator-confirmed evidence.

Controller programs must select a named target idempotently rather than toggle
unknown state. AntennaBench cannot make an external side effect exactly once:
a crash after process completion but before durable commit can cause the same
target command to run again after explicit recovery. Once the corresponding
checkpoint is committed, recovery recognizes the armed intention and does not
repeat it.

Pause, interruption, active-session replacement, controller failure, and
shutdown revoke automatic authority and prevent new invocations. AntennaBench
may terminate an in-flight child process but never claims that termination or
disconnect restored the antenna. It issues no compensating hardware command.

## Durable Evidence And Schema

Each attempted switch or verification process produces one typed rig record,
including attempts that fail before the child starts. The record retains:

- switch or verification role and controller profile name/revision;
- original program and argument templates;
- resolved program and indexed argument array;
- antenna, target, experiment mode, direction, band, optional frequency,
  sequence, intention, session, and callsign context;
- start and completion time and elapsed duration;
- exit code, spawn error, signal termination, or timeout disposition; and
- bounded stdout and stderr with encoding and explicit truncation state.

The default timeout is ten seconds and a local profile may select a value from
one through sixty seconds. Stdout and stderr are captured independently up to
64 KiB each, keeping one complete rig record within the existing 256 KiB JSONL
line boundary. Output that is not UTF-8 is represented as bounded base64.
Truncation is diagnostic completeness metadata, never a successful hardware
confirmation by itself.

Resolved commands and output become portable session evidence. Setup warns
that they may disclose usernames, paths, network addresses, or credentials.
AntennaBench does not invent silent redaction that would make the audit record
look exact when it is not.

This behavior requires schema version 5. An armed WSPR cycle records one of two
readiness bases:

- `operator_confirmed`; or
- `command_verified`, with references to one successful switch rig record and
  one successful verification rig record for the same intention and target
  context.

Strict validation requires both referenced command records to precede the
armed event, use the expected roles and matching context, and complete with
exit zero. For command-verified arming, the switch record, verification record,
and armed event commit in one checkpointed multi-stream mutation. Failed
attempt records can commit without an armed event. Manual review produces
`operator_confirmed` even when successful command attempts exist; those rig
records remain associated through their intention context.

Explicit upgrade from schema v1 through v4 maps every historical armed cycle
to `operator_confirmed` and invents no rig invocation. Older versions remain
readable and exportable. There is no schema downgrade. The report identifies
the readiness basis and shows command attempts and failures without treating a
transport or switch response as confirmation.

## Safety And Authority Boundary

The controller is disabled by default and requires explicit per-session
arming. Commands may run only for the active running session and pending
intention, after the prior transmission interval is complete. A failed,
missing, stale, or unattached controller always degrades to the manual
workflow.

A locally selected external program runs with the operator's normal account
authority. AntennaBench cannot prove that it controls only an antenna. The UI
therefore displays the exact program and arguments, requires an explicit local
profile, and never accepts executable configuration from portable evidence.

This decision adds no built-in CAT, Hamlib, serial, TCP, UDP, PTT,
transmit-enable, frequency, mode, tuner, keyer, or waveform protocol. A wrapper
may use any of those mechanisms, but AntennaBench's contract is limited to
direct process execution, bounded audit evidence, and antenna readiness. Issue
#32 subsequently selected manual/keyer-first execution and closed without adding
built-in CAT, PTT, Morse generation, keyer, waveform, or automatic retry.

## Verification Boundary

Routine tests use deterministic fake executables and clocks. They cover
tokenization, interpolation isolation, unknown placeholders, output encoding
and truncation, every termination disposition, timeout, manual override,
operator-triggered and automatic invocation, review-required and
command-verified arming, stale/recovery state, process repetition before an
uncommitted checkpoint, no repetition after commit, atomic multi-stream
failure, and the absence of shell evaluation.

Hardware interoperability tests are opt-in and human-supervised. They name the
controller, program revision, platform, and target mapping; use a
non-transmitting station state; and never substitute generated output for the
operator's hardware observation.

## Consequences

- Custom switches and radio-specific CAT wrappers share one narrow integration
  boundary without making any device protocol canonical.
- Manual, command-assisted, and fully automatic operation use one conductor
  while retaining distinct readiness evidence.
- Direct program-plus-arguments execution behaves consistently across
  platforms and avoids granting the webview or a shell general execution
  authority.
- Full automation requires new schema, storage, reporting, local profile,
  process supervision, and recovery work; it is not only a frontend feature.
- Portable evidence is sufficient to debug an invocation, but portable bundles
  cannot cause commands to execute.
- Exactly-once external effects remain impossible, so repeat-safe target
  selection is a documented controller requirement.

## Alternatives Rejected

### Add native K4 CAT or one switch protocol first

That would provide a polished path for a small device set while leaving custom
controllers unsupported. Native integrations may be added later behind the
same evidence and conductor boundary.

### Execute the command through a shell

A shell adds platform-specific quoting, expansion, pipelines, profile loading,
and injection behavior that AntennaBench does not need. Direct process
execution keeps the canonical invocation explicit and auditable.

### Treat switch-process success as antenna confirmation

Transport or command acceptance does not establish physical state. Automatic
arming requires an independent verification program; otherwise an operator
must confirm readiness.

### Store executable profiles in the session bundle

Portable executable configuration would let imported evidence request local
code execution and would bind a session to machine-specific paths. Profiles
remain local while invocation evidence is portable.

### Reuse schema v4 without a readiness basis

Older readers and reports would present automated readiness as an operator
action. A fail-closed schema boundary preserves the distinction between human
confirmation and command verification.

## References

- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0010](0010-checkpoint-append-only-live-session-mutations.md)
- [Decision 0011](0011-use-a-fixed-bounded-local-resource-profile.md)
- [Decision 0019](0019-observe-rig-state-before-control.md)
- [First optional rig-control decision #14](https://github.com/rwjblue/antennabench/issues/14)
- [Non-WSPR transmit execution decision #32](https://github.com/rwjblue/antennabench/issues/32)
- [Schema-v5 antenna-control evidence #108](https://github.com/rwjblue/antennabench/issues/108)
- [Local direct-process controller profiles #109](https://github.com/rwjblue/antennabench/issues/109)
- [Automatic antenna-control conductor #110](https://github.com/rwjblue/antennabench/issues/110)
