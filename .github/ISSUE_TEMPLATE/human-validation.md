---
name: Human validation
about: Gather structured human or real-world evidence without treating it as implementation
title: "Validate: "
labels: "enhancement, human-required"
assignees: ""
---

## Outcome

State the product question that requires human judgment, participation, or
real-world operation.

## Entry criteria

- Required implementation and documentation have landed.
- Blocking safety, privacy, and feedback-intake decisions are resolved.
- The exact artifact or revision under test is identified.

## Participant profile

Describe the minimum relevant experience and aggregate cohort. Do not require
public disclosure of participant identity.

## Protocol

Define the tasks, environments, session matrix, observations, and fixed
questions that make feedback comparable.

## Privacy and evidence handling

- Define consent, access, sanitization, retention, and deletion.
- Keep sensitive station/session evidence out of public issue bodies.
- State which aggregate evidence is sufficient for completion.

## Stop-test criteria

- Evidence loss, false durable state, unsafe operation, privacy exposure, or a
  materially misleading product claim

## Non-goals

- Implementation work that belongs in a focused issue
- Claims the participant evidence cannot support
- Automatic telemetry or unrelated product expansion

## Acceptance criteria

- [ ] Entry criteria and the participant/session matrix are satisfied.
- [ ] The protocol is followed against a recorded artifact or revision.
- [ ] Material findings receive focused issues or explicit dispositions.
- [ ] Blocking findings are fixed and retested or consciously accepted.
- [ ] Sensitive evidence is deleted or retained according to policy.
- [ ] The parent tracker records the aggregate result and evidence limits.

## Dependencies

- Tracked by: Parent tracking issue
- Depends on: Required implementation and policy issues

## Completion evidence

Record the tested artifact, aggregate participant/environment/session matrix,
protocol results, finding dispositions, retests, privacy actions, readiness
recommendation, and limits of what the evidence proves.
