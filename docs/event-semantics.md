# Operator Event Semantics

This technical reference defines how AntennaBench records session lifecycle,
operator actions, corrections, and observation eligibility. For the user-facing
workflow, start with the [Product Overview](product.md).

Schema-v2 through schema-v5 operator events are append-only evidence. The schedule
says what was planned; only explicit effective operator evidence says what
actually happened. The pure reducers are implemented independently of storage,
Tauri, sockets, and hardware. The checkpoint writer, shipped manual conductor,
and every reader use the same rules.

## Time And Identity

Every event contains:

- a stable event ID and mutation membership;
- trusted `recorded_at` capture time;
- best-known `occurred_at` time;
- `observed_now`, `operator_reported`, or `recovery_system` time basis;
- optional uncertainty in seconds; and
- an optional planned-slot reference where the payload permits one.

Append order is authoritative. Timestamps and lexicographic ID order never
choose lifecycle or correction precedence.

## Lifecycle

The valid durable transitions are:

```text
ready -> running -> interrupted -> running
  |         |             |          |
  |         +-------------+----------+-> ended
  +---------+-------------+----------+-> abandoned
draft ---------------------------------> abandoned
```

Start moves `ready` to `running`. Explicit interruption and
recovery-detected interruption move `running` to `interrupted`; both remain
auditable. Resume moves only `interrupted` to `running`. End is valid from
`running` or `interrupted`. Abandon is valid from any nonterminal state. Ended
and abandoned sessions reject every later lifecycle transition and operator
fact.

Mutation validation compares the caller's expected checkpoint revision before
reducing a proposed event. A stale revision, duplicate start, resume without an
interruption, duplicate ID, or post-terminal event fails without changing the
effective state.

The desktop conductor issues a bounded action token for the displayed revision.
Rust binds its mutation/event identity and first-submission time, so a duplicate
click, exact retry, or lost response returns the committed mutation rather than
appending a second fact. A different action using a committed token conflicts;
an unused token for an older revision is stale. Restart recovery records one
`recovery_system` interruption before exposing resume/end actions when the last
verified lifecycle was running.

## Operator Evidence

Schema v3 adds two non-correctable operational facts for operator-paced WSPR:

- `antenna_switch_started` closes the current half-open antenna-occupancy
  interval; and
- `wspr_cycle_armed` records the intended cycle, actual antenna, readiness
  action time, and backend-selected WSPR boundary.

Intentions must be armed in their stored order. Readiness never backdates a
cycle and reusing or skipping an intention is invalid. Interruption, detected
recovery, end, and abandon also close occupancy because continued antenna use
is no longer known.

The correctable payloads are:

- `antenna_state_confirmed`, with an explicit actual antenna label;
- schema-v3 `signal_state_confirmed`, with actual frequency, mode, transmitted
  identity, optional power, and cadence adherence kept separate from the plan;
- `slot_missed`, meaning no trustworthy slot action/actual state was confirmed;
- `slot_bad`, with the reason evidence is intentionally ineligible; and
- `note_added`, which does not affect eligibility by itself.

Actual antenna is never inferred from `slot_id`. It may intentionally differ
from the planned label. Missing confirmation remains unknown.

## Corrections

`event_corrected` targets one earlier correctable event in the same session. A
correction either retracts the target or supplies a complete typed replacement,
including occurrence time, basis, uncertainty, slot, and payload. Corrections
never edit or remove the original record.

Successive corrections to the same original follow committed append order; the
latest valid correction determines the effective view. The original event and
every correction remain available as history. A correction cannot target a
future event, itself, another correction, or a lifecycle event. Invalid
corrections produce a typed diagnostic and leave the previous effective view
unchanged.

## Alignment And Eligibility

For operator-paced WSPR, a cycle is attributable only when one recorded antenna
occupancy covers the complete half-open transmission interval. The nominal
cycle starts one second into an even UTC minute and its 162 symbols occupy
110.592 seconds. Routine schema-v3 operation closes the prior occupancy when
the next antenna readiness event is recorded. Historical switch-start events
remain readable: a recorded switch at the exact transmission end is valid,
while an earlier one leaves the cycle's antenna unknown. Public and local spots
use only these fully covered actual cycles.

Schema-v1 alignment retains its historical planned-label behavior. Schema-v2
alignment requires explicit actual state:

- one effective confirmation supplies its actual label and switch time;
- no confirmation yields `unknown_actual_state`;
- one missed or bad fact yields its corresponding excluded state; and
- multiple active confirmation, missed, or bad facts yield
  `conflicting_evidence`.

Unknown and conflicting slots never receive an invented antenna label.
Observations in those slots are conservatively excluded as missing or
contradictory evidence. The report retains the planned label, effective actual
label when known, slot status, and eligibility diagnostic.

## Version Compatibility

Version-1 files and fixtures are unchanged. Their wire type does not gain
schema-v2 fields. Explicit v1-to-v2 upgrade converts legacy events to typed
payloads, preserves their projected behavior, and records a recovery-system
interruption when a legacy session was started but never ended. There is no
v2-to-v1 downgrade and no live mutation of a v1 bundle.

Schema v3 retains the same lifecycle, append-order, correction, retry, and
actual-antenna rules while adding correctable signal-state confirmation. V1 or
v2 upgrade to v3 never invents a signal confirmation, and no schema downgrade
is supported.

Schema v4 retains the v3 event family and adds required WSPR intent direction.
Existing schema-v3 intentions remain readable with unknown direction; readers
and upgrades do not infer receive or transmit from surrounding evidence.

Schema v5 requires every `wspr_cycle_armed` event to record a readiness basis.
Historical and manual ready actions are `operator_confirmed`. A
`command_verified` event names one switch rig record and one independent
verification rig record. Both must precede the event as members of the same
three-member mutation, exit zero, have their expected roles, and match the
complete session/intention/target context, including the durable experiment
mode used for `{mode}` interpolation. Switch success alone is never
physical confirmation. Failed or uncertain command records are valid evidence
without an armed event and therefore create no antenna occupancy.
Review-required local-profile workflows always follow successful command
attempts with the existing named readiness action, so their durable basis
remains `operator_confirmed`; successful command records stay associated by
intention context as supporting diagnostics. Review-disabled workflows create
`command_verified` only from one successful switch and one successful
independent verification committed atomically with that event. Invocation may
be operator-triggered or Rust-automatic without changing those evidence rules.

Explicit v1-v4 upgrade maps historical armed cycles to
`operator_confirmed` and invents no controller or invocation facts. There is no
schema downgrade.
