# Development Guide

> **Audience:** contributors building, testing, or changing AntennaBench. Agent
> execution and issue-authority rules live in [AGENTS.md](../AGENTS.md) and the
> [internal work-tracking guide](work-tracking.md).

AntennaBench is a Rust workspace with a Tauri 2 desktop app and a small,
framework-free JavaScript frontend. [Mise](https://mise.jdx.dev/) installs the
pinned Rust, Node, and Cargo tools and exposes the repository’s standard tasks.

## Set Up On macOS

The complete desktop development workflow is supported on macOS 15 or later.
Install the
Xcode Command Line Tools and Mise, then clone and initialize the repository:

```bash
xcode-select --install
git clone https://github.com/rwjblue/antennabench.git
cd antennabench
mise install
npm ci
```

Launch the desktop app with:

```bash
mise run desktop:dev
```

The first build downloads dependencies and may take a while. Stop the process
with Control-C.

CI also exercises portable workspace and desktop-build behavior on Linux and
Windows, but macOS is the supported environment for interactive desktop work.

## Common Commands

```bash
mise run desktop:test        # focused desktop and frontend tests
mise run desktop:e2e         # unattended setup-to-export workflow
mise run desktop:report-browser # real-browser embedded report style/CSP check
mise run desktop:build       # debug app build without packaging
mise run hosted:test         # public site plus optional hosted foundation
mise run desktop:frontend-coverage
mise run module-size         # ratcheting Rust module-size policy
mise run desktop:release-test
mise run desktop:publication-test
mise run ci                  # complete routine repository checks
```

`mise run ci` checks tool pins and supply-chain policy, the Rust module-size
ratchet, formatting, Clippy, the Rust workspace, frontend behavior, hosted code,
the real-browser report-frame style boundary, and the unattended desktop
workflow. Tests use synthetic or reduced fixtures and do not require radio
hardware, WSJT-X, NOAA, Cloudflare, or another live service.

## Rust Module-Size Budget

`mise run module-size` measures every Rust file under `crates/*/src` and
`apps/desktop/src`. The measurement stops before the first inline module gated
by `#[cfg(test)]`, so isolated test-only imports or helpers do not hide
production code that follows, and the trailing inline test module does not
consume the production budget. Unlisted files may contain at most 1,200
measured lines.

Current oversized modules are explicit exceptions in
`.github/module-size-budget.json`. An exception is a ratchet, not a target: the
check fails when a file grows beyond its recorded budget and also when the
recorded budget is more than 10 lines above the measured file. Prefer splitting
the module; when growth is deliberate, raise the exact entry in review and
explain why. When a split lands, lower or remove its entry in the same change.
Frontend `.mjs` files are not covered by this policy.

## Repository Layout

- `apps/desktop/` contains the Tauri backend and static frontend.
- `apps/hosted/` contains the static Astro project site and the separate,
  optional admission-disabled Cloudflare sharing foundation.
- `crates/` contains bundle, storage, adapter, analysis, and report code.
- `fixtures/` contains redistribution-safe test inputs.
- `docs/` contains operator guides, contributor references, internal runbooks,
  and architecture decisions.
- `.mise/tasks/` is the command source of truth for local and CI workflows.

## Contribution Expectations

This repository uses Jujutsu (`jj`) for maintained version-control workflows.
Keep changes focused, update maintained documentation when behavior changes, and
run verification proportional to the change. Rust behavior should pass
formatting, Clippy with warnings denied, and tests before completion.

GitHub Issues track unfinished work and decisions. Maintainers and coding agents
should follow the [internal work-tracking guide](work-tracking.md); ordinary
contributors do not need its agent handoff and label protocol to understand the
codebase.

## More Detail

- [Architecture Overview](architecture.md) explains the system shape and trust
  boundaries.
- [Development Technical Reference](development-reference.md) records repository
  policy, verification coverage, CI, desktop internals, and release construction.
- [Desktop Releases](releasing.md) is the owner release runbook.
- [Supply-Chain Updates](supply-chain.md) is the dependency and tool-update
  procedure.
