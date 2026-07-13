---
name: Agent-ready implementation
about: Define a bounded implementation slice that can be handed directly to an agent
title: ""
labels: ""
assignees: ""
---

## Outcome

Describe the concrete result this issue should deliver.

## Context

Explain why this matters, what already exists, and where the current source of
truth lives.

## Implementation contract

The implementation must:

- Preserve the relevant existing behavior and architectural boundaries.
- Reuse existing public APIs where their semantics already fit.
- Update maintained documentation when behavior changes.
- Avoid durable format or schema changes unless explicitly authorized here.

The implementing agent may choose internal file structure, private APIs, and
test organization consistent with repository conventions.

## Scope

- Required capability
- Required integration points
- Required tests or fixtures
- Required documentation changes

## Non-goals

- Explicitly deferred behavior
- Adjacent systems that must not change

## Acceptance criteria

- [ ] The required externally observable behavior exists.
- [ ] Important failure and edge cases are covered.
- [ ] Required fixtures or interoperability proofs exist.
- [ ] Existing behavior remains covered.
- [ ] Maintained documentation is accurate.
- [ ] `mise run ci` passes.
- [ ] The working-copy diff contains no unrelated changes.

## Dependencies

- Depends on: None
- Blocks: None

## Implementation discretion

The agent may decide internal module organization, private types, and test
decomposition without further approval.

The agent must stop and request direction if a durable schema change, material
public-behavior expansion, new external authority, or unresolved architectural
choice becomes necessary.

## References

- Relevant docs, ADRs, APIs, fixtures, and related issues

## Completion evidence

Record the delivered behavior, Jujutsu change or commit, verification commands
and results, documentation updates, and follow-up issues before closing.
