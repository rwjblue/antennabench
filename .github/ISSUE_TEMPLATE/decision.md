---
name: Agent-ready technical decision
about: Hand a bounded technical choice directly to an implementation agent
title: "Decision: "
labels: "agent-ready, decision"
assignees: ""
---

Use this template only when every blocking dependency has landed and an agent
may resolve the choice now. Use the product/owner decision template when the
selection itself requires human judgment or authority.

## Decision needed

State the question that must be resolved.

## Context

Explain why the decision matters, what already exists, and which future work it
affects.

## Options

### Option A

Describe benefits, costs, and constraints.

### Option B

Describe benefits, costs, and constraints.

## Decision criteria

- Compatibility and interoperability
- Complexity and maintenance cost
- Offline and failure behavior
- Durable data and migration implications
- Testing and observability

## Acceptance criteria

- [ ] Relevant primary sources and current repository behavior are documented.
- [ ] Viable options and tradeoffs are compared.
- [ ] A recommendation is recorded with rationale.
- [ ] A durable architectural decision is promoted to an ADR when warranted.
- [ ] Follow-up implementation issues are created or updated.

## Implementation discretion

The assigned agent may perform read-only research and create local experiments.
The agent must not ship product behavior or make external infrastructure changes
unless this issue explicitly authorizes them.

## References

- Relevant docs, ADRs, APIs, fixtures, and related issues

## Result

Record the selected option, rationale, consequences, and follow-up issues. Close
this issue and update any parent tracking issue once the decision is durable.
