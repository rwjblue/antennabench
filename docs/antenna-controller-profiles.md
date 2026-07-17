# Local Antenna Controller Profiles

AntennaBench can optionally assist an operator by running a machine-local
antenna switch program and, when configured, a separate verification program.
This is an operator-triggered aid: the existing named **antenna ready** action
still arms every WSPR cycle and records `operator_confirmed` readiness.

Manual operation is the default and remains complete after every missing
profile, process error, timeout, verification failure, interruption, or edit.
There is no automatic retry.

## Local Configuration And Portable Evidence

Profiles are stored only in the platform application-data directory. A profile
contains a name and revision, switch and optional verification templates, and
a timeout from one through sixty seconds. The per-session association maps
every scheduled antenna label to one opaque target. Associations may be saved
locally, but executable authority is volatile: opening or recovering a bundle
never attaches or arms a profile. The operator must explicitly attach and arm
it again.

Session bundles contain only the portable antenna-control policy and evidence
from commands that actually ran. They never contain executable paths, target
mappings, or timeouts. Opening or importing a bundle therefore cannot grant it
process authority.

Resolved programs, indexed arguments, stdout, and stderr become portable
evidence. They may disclose local paths, usernames, network addresses, or
credentials. Setup displays this warning before creation and previews every
normalized invocation with its experiment mode and direction.

## Command Entry And Expansion

On macOS and Linux, the setup UI accepts one command line. AntennaBench's small
tokenizer recognizes only whitespace, single- or double-quote grouping, and a
backslash that escapes the next character. On Windows, setup presents the
program and ordered argument array separately. Both inputs normalize to the
same program-plus-arguments model.

No shell is used. Environment variables, `~`, globs, redirects, pipes,
substitutions, and chaining are not expanded. Text such as `*`, `>`, `|`,
`$()`, and `&&` remains ordinary argument text.

Templates support `{antenna}`, `{target}`, `{mode}`, `{direction}`, `{band}`,
`{frequency_hz}`, `{sequence}`, `{intent_id}`, `{session_id}`, and `{callsign}`.
`{{` and `}}` produce literal braces. Unknown or malformed placeholders fail
setup review. Tokenization happens first and interpolation happens inside each
existing token, so a value containing whitespace, quotes, braces, or shell
metacharacters cannot create another argument or executable.

## Active Run

The direct-process action is available only when all of these remain true:

- the active bundle is a running schema-v5 session whose portable policy is
  command-controlled, operator-triggered, and manual-review-required;
- the prior committed WSPR transmission interval has ended;
- Rust issued the action token for the current revision and pending intention;
- a local profile revision and complete antenna-target mapping were explicitly
  attached and armed for that session; and
- the saved profile still has the attached revision.

Rust derives the complete interpolation context from the active bundle. The
webview submits only the action token, checkpoint revision, and pending
intention identity; it cannot choose an executable at invocation time.

The switch program runs first. Verification runs only after switch exit zero.
Switch or verification success is diagnostic evidence, not physical antenna
confirmation. Active Run always waits for the named operator-ready action.
Retry is explicit. Editing a profile creates a new revision and revokes its
current arm until the operator reviews and attaches it again.

Each process has the configured one-to-sixty-second timeout and independent
64 KiB stdout/stderr capture. UTF-8 is retained as text; other bytes use base64.
Truncation, spawn failure, exit code, signal termination, and timeout remain
explicit. Every attempt commits through the schema-v5 rig-evidence boundary,
including failures. These failure-only commits do not arm a cycle or change
antenna occupancy.

Interruption, end, abandon, active-session replacement, profile replacement,
and application shutdown revoke in-memory authority and stop new invocations.
An in-flight child may be terminated, but AntennaBench never claims that child
termination restored the hardware and issues no compensating command.

The checked-in [Elecraft K4/QK4 example](../examples/rig-control/elecraft-k4/README.md)
shows a protocol-specific wrapper behind this protocol-neutral boundary.
