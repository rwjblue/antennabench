# Development Guide

AntennaBench is a Rust workspace with a Tauri 2 desktop app and a small
framework-free JavaScript frontend. Tool versions and routine commands are
managed by [Mise](https://mise.jdx.dev/).

## Set Up

macOS is the supported desktop development platform. Install Xcode Command Line
Tools and Mise, then install the pinned Rust, Node, and Cargo tools:

```bash
xcode-select --install
mise install
```

Launch the desktop application with:

```bash
mise run desktop:dev
```

The first build may take a while. Stop the development process with Control-C.

## Common Commands

```bash
mise run desktop:test        # focused desktop and frontend tests
mise run desktop:e2e         # unattended setup-to-export workflow
mise run desktop:build       # debug application build without packaging
mise run hosted:test         # optional hosted foundation
mise run desktop:release-test
mise run desktop:publication-test
mise run ci                  # complete routine repository checks
```

The full CI task checks toolchain and supply-chain policy, formatting, Clippy,
the Rust workspace, frontend state, hosted code, and the unattended desktop
workflow. Tests use synthetic or reduced fixtures and do not require WSJT-X,
radio hardware, NOAA, Cloudflare, or another live service.

## Repository Layout

- `apps/desktop/` contains the Tauri backend and static frontend.
- `apps/hosted/` contains the optional Cloudflare foundation.
- `crates/` contains bundle, storage, adapter, analysis, and report code.
- `fixtures/` contains redistribution-safe test inputs.
- `docs/` contains product guides, technical references, and decisions.
- `.mise/tasks/` is the command source of truth for local and CI workflows.

## Contributing

Use Jujutsu (`jj`) for version control and do not create a worktree unless one
is explicitly requested. Repository-specific instructions live in
`AGENTS.md`.

GitHub Issues track unfinished work and decisions. A focused issue defines the
approved outcome and acceptance criteria; the [Work Tracking](work-tracking.md)
guide explains milestones, labels, dependencies, and completion evidence.

Keep changes focused, update maintained documentation when behavior changes,
and run verification proportional to the change. Rust behavior should pass
formatting, Clippy with warnings denied, and tests before completion.

## More Detail

- [Development Technical Reference](development-reference.md) documents coding
  conventions, test coverage, CI, desktop internals, and release artifacts.
- [Desktop Releases](releasing.md) is the owner and user release runbook.
- [Supply-Chain Updates](supply-chain.md) is the dependency and tool-update
  procedure.
- [Architecture Overview](architecture.md) explains the system boundaries.
