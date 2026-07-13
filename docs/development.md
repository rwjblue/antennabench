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
mise run desktop:test
mise run desktop:build
mise run desktop:dev
```

`desktop:test` exercises the Rust open-session orchestration and pure JavaScript
workflow transitions. `desktop:build` builds a debug application without
producing installer bundles, and `desktop:dev` launches the static shell with
Tauri's development server.

For a manual launch smoke check, run `mise run desktop:dev`, confirm the window
opens on Session setup, and navigate to Import / export. Choose
`fixtures/session-bundles/canonical-sample-report.session.wsprabundle`. The UI
should show its loading state, navigate to Local report, summarize the selected
session, and display the complete standalone report. Return to Import / export,
start another open, and cancel the native picker; the active report should stay
available and cancellation should be shown as a normal outcome. Selecting a
directory that is not a `.session.wsprabundle` should instead show a friendly
selection error with technical context. Stop the development process with
Control-C and use `jj status` to confirm the source fixture was not changed.

The main webview capability allows only `open_session_bundle` and the read-only
`active_session_report`. The first command owns native directory selection and
all filesystem/domain work, returning a small summary; the second returns only
the already-derived report document. No dialog-plugin or filesystem-plugin
permission is granted to JavaScript; this is intentional even though the native
dialog plugin is registered. The local report is loaded into a sandboxed frame
and neither the shell nor report is given network authority.

## Documentation Updates

When implementation changes project behavior, update the relevant evergreen doc
in the same change when practical.

Prefer documenting stable concepts, invariants, and workflows. Avoid copying
detailed implementation-plan steps or generated code into evergreen docs.
