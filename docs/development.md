# Development

This repo uses Rust, Cargo, and Jujutsu (`jj`).

The desktop shell also uses Tauri 2 and plain JavaScript. Node is used only for
the frontend's dependency-free state tests.

## Version Control

Use `jj` workflows. Do not create worktrees unless explicitly requested.

Project-local instructions live in `AGENTS.md`. Future agent planning artifacts
under `docs/superpowers/` are ignored and should remain local unless a user
explicitly asks to preserve them elsewhere.

## Work Tracking

GitHub Issues are the durable source of truth for unfinished work and open
implementation decisions. The roadmap describes direction; issues define
focused outcomes, scope, non-goals, and acceptance criteria.

An issue is agent-ready when its outcome is unambiguous, blocking decisions are
resolved or explicitly delegated, dependencies are identified, and its
acceptance criteria are objectively verifiable. The `agent-ready` label means
the issue can be handed to an agent; it does not authorize implementation by
itself. Explicitly instructing an agent to implement the issue approves that
issue's scope.

Agents may create ignored detailed plans while executing an approved issue. A
local plan must not be the only record of unfinished work. Material expansion
of public behavior, durable schemas, or architectural scope requires user
direction rather than an implicit issue rewrite.

When an issue is explicitly handed to an agent, the agent should confirm its
dependencies, replace `agent-ready` with `in-progress`, and inspect the current
checkout before planning or implementing. Label changes are managed by the
agent through GitHub; they are not state transitions performed by a bot.

If implementation becomes blocked on a product or architectural choice, the
agent should apply `needs-decision`, explain the concrete blocker and viable
choices, and leave the issue open. Partially implemented work must not be closed
as complete.

Before closing an implementation issue, land the work and record the delivered
behavior, Jujutsu change or commit, verification results, documentation updates,
and any follow-up issues discovered. The agent should close the focused issue
and update any parent tracking issue. If work is only committed locally, the
issue remains open.

Use the templates under `.github/ISSUE_TEMPLATE/` for agent-ready implementation
and decision work. Implementation issues begin with `agent-ready` and
`enhancement`; decision tasks begin with `agent-ready` and `decision`.

## Rust Conventions

- Define shared third-party dependencies in the root `Cargo.toml`.
- Reference workspace dependencies from member crates with `workspace = true`.
- Use `thiserror` for library crate error types.
- Use `anyhow` for application code, CLIs, test harnesses, and top-level
  orchestration where typed public errors are not part of the API.
- Prefer `insta` inline snapshots for structured test output.

## Verification

Before declaring Rust behavior complete, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The WSJT-X live adapter tests use purpose-built protocol datagrams documented
under `fixtures/wsjtx/udp/`; no operator capture or third-party spot data is
committed. Loopback UDP coverage verifies only the receiver boundary. It does
not require WSJT-X or network access during the test suite.

The optional NOAA SWPC adapter tests use reduced captured response shapes under
`fixtures/noaa-swpc/`. They cover pure F10.7 and provisional estimated-Kp
parsing, source envelopes, malformed/partial/stale input, deterministic record
selection, duplicate suppression, polling/backoff policy, conditional requests,
and best-effort failures through an injected transport. `cargo test -p
antennabench-propagation` never contacts NOAA. The live blocking transport is a
one-shot boundary for future desktop orchestration; network or source failure
must remain a typed optional outcome rather than a session failure.

The canonical sample-report bundle is purpose-built synthetic data documented
under `fixtures/session-bundles/`. Its focused integration test reads,
normalizes, validates, exports, reopens, analyzes, and builds report data from
the bundle. The test pins important scenario counts without snapshotting the
entire fixture.

Standalone report-renderer tests use the same canonical sample to verify
determinism, offline-only document structure, accessible chart tables, and all
report sections. Separate hostile-string and empty-data cases pin escaping and
honest unavailable states without loading a browser or making network requests.

To generate the canonical sample as an untracked verification artifact:

```bash
cargo run -p antennabench-report --example render_canonical_sample -- /tmp/antennabench-sample.html
```

For documentation-only changes, inspect the rendered intent and verify the diff
is limited to the requested files.

## Continuous Integration

Pull requests and pushes to `main` run three standard GitHub-hosted jobs. Linux
is the canonical full-quality job: it installs the Linux Tauri prerequisites
and runs `mise run ci`, including formatting, Clippy, all workspace targets,
frontend state tests, and the unattended desktop workflow. The macOS and
Windows jobs each run the portable workspace tests, frontend state tests,
unattended desktop workflow, and `mise run desktop:build` for a native debug
`--no-bundle` compilation.

Project-local Mise tasks remain the command source of truth on every platform.
The portability jobs explicitly select `shell: bash`; on Windows, GitHub Actions
therefore uses the Bash supplied by Git for Windows instead of the PowerShell
default. Simple task wrappers intentionally remain Bash. The desktop build and
development tasks resolve Mise's `cargo-tauri` executable before restoring the
native Windows path list required by Tauri's Cargo child processes. The desktop
E2E task uses Node's clock rather than a platform-specific `time` executable
and records
the runner OS, elapsed seconds, exit status, and bounded phase diagnostics in
`target/desktop-e2e/last-run.log`. A failed portability job uploads that log for
seven days when it exists.

The workflow uses the moving `ubuntu-latest`, `macos-latest`, and
`windows-latest` labels. As of 2026-07-13, GitHub documents the standard public
`macos-latest` runner as arm64 and `windows-latest` as x64, but those labels and
images evolve. Green CI proves that the portable contract compiled and passed
on the exact runner recorded in that workflow log; it does not declare a
supported release platform or architecture. Release support, artifacts,
signing, and publication remain separate decisions.

## Desktop Development

The currently supported desktop development platform is macOS. Install Xcode
Command Line Tools (or Xcode) before building Tauri, then let Mise install the
pinned Rust, Node, and Tauri CLI versions:

```bash
xcode-select --install
mise install
```

The desktop-specific commands are:

```bash
mise run desktop:e2e
mise run desktop:test
mise run desktop:build
mise run desktop:dev
```

`desktop:e2e` is the routine desktop workflow check for agents and developers.
It injects deterministic open/save selections immediately below the native
picker adapter, then runs the production Rust open, normalized validation,
analysis, report rendering, active-session state, lossless export verification,
and reopen path. It asserts source non-mutation and exported tree/byte equality,
and separately covers normal open/export cancellation and a typed malformed-JSON
failure. Temporary sources and destinations are isolated and automatically
removed. The task does not launch Tauri, create a window, take focus, or send
keyboard or pointer input.

The task streams phase diagnostics to the terminal and overwrites the bounded
`target/desktop-e2e/last-run.log` artifact on every run. It records the platform,
elapsed seconds, and final status without depending on a Unix-only timing tool.
CI runs the same task, so failures retain the selected phase, fixture path,
typed error kind, technical detail, and Rust assertion context in both the
artifact and job log.

On the 2026-07-13 macOS development machine, the warm task completed in 0.42 s.
The prior issue #18 foreground smoke took 3 min 49 s from application relaunch
through cleanup (2 min 49 s from opening the first picker through the final
cancellation). The unattended path is therefore more than 400 times faster
than the interactive workflow while covering the canonical behavior more
deterministically.

`desktop:test` retains the focused Rust and pure JavaScript tests.
`desktop:build` builds a debug application without producing installer bundles,
and `desktop:dev` launches the static shell with Tauri's development server.

The remaining native-picker smoke is optional release/platform verification,
not routine regression testing. Run `mise run desktop:dev`, confirm the window
opens, and verify only that the open and save pickers appear, cancellation
returns to the app normally, and selecting the canonical fixture hands a local
directory path to the app. The unattended test owns open → report → export →
reopen semantics, error behavior, preservation assertions, and diagnostics; do
not repeat those checks with coordinate-driven automation. Stop the development
process with Control-C and use `jj status` to confirm the fixture was unchanged.

The main webview capability allows only `open_session_bundle`,
`export_active_session`, and the read-only `active_session_report`. The first
two commands own native selection and all filesystem/domain work; the report
command returns only the already-derived document. No dialog-plugin or
filesystem-plugin permission is granted to JavaScript; this is intentional
even though the native dialog plugin is registered. The local report is loaded
into a sandboxed frame and neither the shell nor report is given network
authority.

## Documentation Updates

When implementation changes project behavior, update the relevant evergreen doc
in the same change when practical.

Prefer documenting stable concepts, invariants, and workflows. Avoid copying
detailed implementation-plan steps or generated code into evergreen docs.
