# 0019: Observe Rig State Before Adding Control

## Status

Accepted.

## Context

The manual conductor and bounded WSJT-X UDP intake are now implemented. WSJT-X
status datagrams already carry dial frequency, mode, transmit mode, transmit
enabled, transmitting, decoding, and station identity. The live adapter retains
the complete datagram and keeps the latest status per admitted client, but the
desktop exposes only receiver health and does not compare that status with the
active WSPR instruction.

The bundle also has a durable provider-neutral adapter stream and a rig-record
stream. Those storage shapes reserve room for observations without making any
particular radio library canonical. No current producer writes rig records and
there is no Hamlib dependency or radio-command surface.

The first operator need is narrower than radio control: make an accidental
WSJT-X band, mode, or transmit-state mismatch visible before or during a WSPR
period. An operator must still be able to run with no radio, no status source,
or a deliberately manual station.

## Decision

The first optional rig-integration milestone is **observe and warn** using the
already admitted WSJT-X status stream. Direct rig control, including Hamlib,
frequency or mode commands, PTT, antenna switching, and automatic correction,
is deferred to a separately approved milestone.

The observation is advisory. It never blocks session start or progress, never
changes an intended or actual state record, and never claims that WSJT-X status
is direct radio read-back. Having no status source is a normal `unavailable`
state, not an error. The existing manual/no-rig conductor remains complete.

For a fresh status from the configured WSJT-X client, the Active Run surface
may warn about these concrete mismatches:

- the reported mode is not WSPR;
- the dial frequency is outside the band of the active intention;
- WSJT-X reports transmitting during a receive intention;
- Enable Tx is on during a receive intention; or
- Enable Tx is off during a transmit intention.

`transmitting = false` during a transmit intention is not itself a mismatch:
the status may have arrived before the two-minute transmit boundary. Decode
state is informational and does not establish whether Monitor is configured.
Transmit mode, split/VFO state, RF power, physical antenna selection, audio
level, and actual emitted frequency cannot be inferred reliably from the
current status and therefore produce no warning.

Identity, freshness, and lifetime are fail-closed:

- only the already configured and admitted WSJT-X client can supply status;
- a close message, receiver stop, client-generation reset, active-session
  replacement, or heartbeat expiry makes status unavailable;
- a warning is based only on a status received within the existing live-client
  heartbeat window; an implementation may display its age but must not keep a
  stale warning current; and
- unavailable or unsupported facts remain distinct from a confirmed match.

Warnings should be derived from the current conductor view and volatile
receiver state. The already committed raw WSJT-X adapter record is the audit
evidence; this slice does not duplicate every status into `rig.jsonl`, add a
schema field, or make a UI warning canonical. If a later independent rig
adapter produces normalized radio state, it may append a rig record linked to
its provider evidence under the existing bundle boundary.

## Deferred Control Contract

Any later command-capable adapter requires another decision and implementation
issue. At minimum it must:

- be disabled by default and armed explicitly for the current session;
- discover capabilities before offering an action and treat unsupported or
  stale reads as unknown;
- distinguish planned, commanded, read-back-confirmed, observed, and failed
  state in its audit trail;
- use bounded timeouts, perform a read-back after a successful set response,
  and never treat transport success as radio confirmation;
- avoid automatic retries that could create an unplanned state change;
- preserve manual fallback after refusal, disconnect, partial capability, or
  confirmation failure;
- send no PTT, transmit-enable, CW, or antenna-switch command in a
  frequency/mode milestone;
- issue no compensating "rollback" command unless the prior state was fresh,
  unambiguous, and the rollback was separately authorized; and
- drop authority and stop issuing commands on shutdown, interruption, active
  session replacement, or adapter failure. Disconnecting must not be described
  as restoring the radio.

Hamlib remains a candidate for that future adapter. `rigctld` offers a local
text protocol, capability reporting, set/get operations, and response codes,
but support varies by backend and radio. Hamlib is LGPL-2.1-or-later software;
shipping it would add a pinned native component, license notices, per-platform
packaging and signing, process or local-socket supervision, and compatibility
testing. Those costs and the safety contract above are not justified merely to
surface status AntennaBench already receives.

The operator remains responsible for radio configuration, permitted
frequencies, power, station control, and transmit authorization. AntennaBench
must describe an advisory as a setup aid rather than regulatory validation.

## Options Considered

| Option | Result | Rationale |
| --- | --- | --- |
| Observe and warn | Selected | Addresses the immediate setup-error risk using an existing bounded, auditable source and adds no hardware authority. |
| Optional Hamlib frequency/mode control | Deferred | Useful only after capability, consent, read-back, packaging, CAT ownership, and failure semantics are implemented and validated. |
| Defer all rig integration | Rejected for the first slice | Manual operation already works, but discarding fresh WSJT-X facts would leave preventable WSPR setup mistakes invisible. |

## Verification Boundary

The observe-and-warn implementation is hardware-free. Contract tests inject
official synthetic status and heartbeat datagrams, a fake clock, client close
and generation changes, and conductor intentions. The matrix covers every
listed mismatch, matching state, missing status, stale status, identity
filtering, receive/transmit transitions, and the absence of any command or rig
record mutation. Desktop tests verify that warnings are advisory and the
manual/no-rig controls remain enabled.

A future Hamlib adapter must use a fake transport for routine CI, including
unsupported capabilities, error responses, timeouts, disconnects, stale
read-back, mismatched read-back, and shutdown. Hardware interoperability tests
must be opt-in, name the tested backend/radio/version, require an operator at
the station, use a non-transmitting test configuration, and never exercise PTT
or unattended transmission.

## Consequences

- Active Run can make a small set of preventable WSPR setup mistakes visible
  without becoming a radio controller.
- Absence or loss of status degrades to the existing manual workflow.
- WSJT-X status remains companion-application state, not proof of physical rig
  or antenna state.
- Direct control remains possible behind the existing adapter and rig-record
  seams, but requires explicit new authority and its own implementation review.
- The first implementation can be tested deterministically without Hamlib,
  network services, or attached hardware.

## References

- [Hamlib project](https://hamlib.github.io/)
- [rigctld manual](https://hamlib.sourceforge.net/html/rigctld.1.html)
- [Hamlib license](https://hamlib.sourceforge.net/manuals/4.7/LICENSE.html)
- [Issue #14](https://github.com/rwjblue/antennabench/issues/14)
- [Advisory warning implementation #107](https://github.com/rwjblue/antennabench/issues/107)
