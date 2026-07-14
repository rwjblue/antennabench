# Development

This repo uses Rust, Cargo, and Jujutsu (`jj`).

The desktop shell also uses Tauri 2 and plain JavaScript. Node is used only for
the frontend's dependency-free state tests, supply-chain policy validation, and
desktop test timing. No Node package manifest or lockfile exists. Rust, Node,
and cargo-tauri are exact Mise pins; Cargo-backed tools are installed with their
published lockfiles.

## Version Control

Use `jj` workflows. Do not create worktrees unless explicitly requested.

Project-local instructions live in `AGENTS.md`. Future agent planning artifacts
under `docs/superpowers/` are ignored and should remain local unless a user
explicitly asks to preserve them elsewhere.

## Work Tracking

GitHub Issues are the durable source of truth for unfinished work and open
implementation decisions. The roadmap describes direction; issues define
focused outcomes, scope, non-goals, and acceptance criteria.

[Work Tracking](work-tracking.md) is the maintained human-facing guide to
milestones, tracking issues, labels, dependencies, human-required work, useful
queries, and completion evidence. This section records the agent execution
rules.

An issue is agent-ready when its outcome is unambiguous, blocking decisions are
resolved or explicitly delegated, every blocking dependency has landed, and
its acceptance criteria are objectively verifiable. The `agent-ready` label
means the issue can be handed to an agent immediately; it does not authorize
implementation by itself. Explicitly instructing an agent to implement the
issue approves that issue's scope.

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
and update any parent tracking issue. It should then review the dependent open
issues and apply `agent-ready` to newly executable work. If work is only
committed locally, the issue remains open and does not satisfy a remote
dependency.

Use the templates under `.github/ISSUE_TEMPLATE/`. Planned implementation
issues begin with `enhancement` only; apply `agent-ready` after their blocking
dependencies land. Agent-ready implementation issues begin with `agent-ready`
and `enhancement`; agent-ready technical decisions begin with `agent-ready` and
`decision`. Product/owner decisions, tracking issues, and human-validation work
deliberately do not begin agent-ready. `human-required` means an agent cannot
satisfy the issue's complete outcome without owner action, credentials, human
judgment, or real participant evidence.

## Rust Conventions

- Define shared third-party dependencies in the root `Cargo.toml`.
- Reference workspace dependencies from member crates with `workspace = true`.
- Use `thiserror` for library crate error types.
- Use `anyhow` for application code, CLIs, test harnesses, and top-level
  orchestration where typed public errors are not part of the API.
- Prefer `insta` inline snapshots for structured test output.

## Rust Toolchain Policy

The project supports one exact Rust toolchain for development, CI, and future
release builds. Rust 1.96.1 is declared by the workspace and pinned in both
`rust-toolchain.toml` and Mise so builds do not move when a new stable compiler
is published. Every package inherits the workspace declaration.

AntennaBench is an application whose build and release environment the project
controls; its internal workspace crates are not currently published as a
separately supported library surface. The project therefore does not maintain
an older minimum-supported-Rust-version compatibility promise. Run the pin
consistency check locally with:

```bash
mise run toolchain
```

Compiler updates are focused maintenance changes. They update the workspace
declaration, `rust-toolchain.toml`, and Mise together, document relevant
dependency or release effects, and pass the full quality suite. A future plan
to publish supported Rust libraries must establish a compatibility policy
before publication rather than inheriting an accidental application MSRV.
[Decision 0014](decisions/0014-use-one-pinned-rust-toolchain.md) records the
rationale for replacing the former dual-toolchain policy.

## Supply-Chain Maintenance

[Decision 0012](decisions/0012-use-combined-supply-chain-maintenance-gates.md)
selects a combined GitHub and cargo-deny baseline. External Actions use full
commit SHAs with release-tag comments. Dependabot proposes weekly Cargo and
Actions updates, and pull requests receive a read-only dependency review that
blocks newly introduced moderate-or-higher vulnerabilities.

The workflow validator rejects mutable Action or container references, moving
runner aliases, missing read-only permissions, and unowned dependency
manifests. The exact Node, Rust, cargo-tauri, and Mise workflow pins, dated GA
runner labels, Dependabot limits, and manifest maintenance policy are checked
by:

```bash
mise run supply-chain
```

The repository-owned Rust advisory, license, source, wildcard, duplicate, and
exception gates run through `mise run dependency-policy` and
`mise run advisory-fresh`. `mise run release-preflight` combines those gates
with the workflow/tool-pin checks and always fetches the RustSec database.
Dependabot alerts, CodeQL, Action restrictions, and the main ruleset still
require the explicit owner action in
[#60](https://github.com/rwjblue/antennabench/issues/60). See
[Supply-Chain Updates](supply-chain.md) for the review and update procedure.

## Verification

Before declaring Rust behavior complete, run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`mise run ci` additionally runs toolchain, workflow-input, exception-expiry,
license, source, wildcard, and duplicate policy. It deliberately does not make
ordinary CI depend on a fresh network advisory fetch; the separate read-only
Rust supply-chain workflow supplies that gate on dependency pull requests,
every main push, daily, on demand, and as the reusable release preflight.

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

`crates/storage/tests/v2_bundle.rs` treats every checked-in v1 bundle as a
migration fixture. It upgrades each source into a temporary
`.session.antennabundle`, proves the source tree is byte-identical, verifies
the v2 checkpoint and adapter backlinks, compares retained WSJT-X physical
lines, reopens the current projection, and makes/reopens a byte-identical v2
lossless copy. `crates/core/tests/v2_types.rs` pins every legacy provenance
mapping and the lowercase-ASCII identity boundaries.

`crates/core/tests/bundle_validation.rs` is the semantic rule/code matrix for
machine-ID and antenna-label boundaries, schedule sequence/window/shape rules,
universal numeric units and ranges, analysis metadata, and the explicit
adapter-owned protocol boundary. `crates/storage/tests/semantic_preflight.rs`
mutates every checked-in v1 fixture with a non-finite modeled value and proves
strict v1/v2 writes create no destination. It also proves a warning-bearing v1
source stays byte-identical, compatibility-readable, losslessly copyable, and
upgradeable when its warning is representable.

`crates/storage/src/resource.rs` pins the fixed `local-standard-v1` storage
profile and its typed diagnostic contract. Unit tests inject narrow profiles to
exercise exact-limit and first-byte-over boundaries for root JSON, JSONL lines,
record counts, JSON shape, root entries, attachments, aggregate counters, and
cooperative cancellation. Storage integration tests keep lossless copy separate
from typed projection and verify rollback, unsafe-entry rejection, v1/v2 reopen,
and byte identity.

`crates/storage/tests/live_persistence.rs` is the schema-v2 durability matrix.
It uses real OS file locks and replacement on the host platform plus injected
clock, ID, and failure hooks. The test visits every event-stream and checkpoint
write/sync/replace/verify boundary and every plan-generation file boundary,
then reopens the previous or complete next revision. Focused cases cover
cooperative and ignored locks, external-change freeze, raw-plus-normalized
batches, lost responses and idempotent retry, complete/torn/incomplete and
duplicate tails, current/previous checkpoint selection, committed corruption,
recovery attachments, interruption detection, and checkpointed export.

`crates/wsjtx/tests/all_wspr_import.rs`, `crates/wsjtx/tests/live_udp.rs`, and
`crates/propagation/tests/acquisition.rs` pin the adapter portion of that same
profile. They exercise exact and first-over offline source, line, and record
limits; cancellation and malformed-row preservation; UDP datagram, queue, rate,
client, idle-eviction, and timed fixed-size dedup boundaries; and HTTP redirect,
timeout, header, content-length, streamed body, media, encoding, quarantine, and
cancellation outcomes. All network tests use injected transports or loopback;
the verification suite never depends on a live NOAA or WSJT-X service.

Analysis/report resource tests inject tiny profiles around N-1/N/N+1 to pin
per-collection and aggregate-live entry failures, cancellation checkpoints,
full-detail versus bounded-overview projection, complete omission-family
counts, model serialization, and checked HTML escape expansion. Desktop unit
tests independently pin the 64 KiB summary and 16 MiB document IPC boundaries,
the typed foreground-busy result, prior-presentation retention, and lossless
export without a derived report. Production entry points always select the
fixed `local-standard-v1` values; injection exists only in explicit test-facing
APIs.

Standalone report-renderer tests use the same canonical sample to verify
determinism, offline-only document structure, accessible chart tables, and all
report sections. Separate hostile-string and empty-data cases pin escaping and
honest unavailable states without loading a browser or making network requests.
Mixed-quality analysis/report tests pair malformed and contradictory observations
with valid evidence, assert stable eligibility code/category/scope counts, and
prove unrelated slots and summaries remain usable. Structural antenna ambiguity
continues to fail deterministically, while non-finite SNR becomes an affected-
observation exclusion in serialized and rendered reports.

To generate the canonical sample as an untracked verification artifact:

```bash
cargo run -p antennabench-report --example render_canonical_sample -- /tmp/antennabench-sample.html
```

For documentation-only changes, inspect the rendered intent and verify the diff
is limited to the requested files.

## Continuous Integration

Pull requests and pushes to `main` run three standard GitHub-hosted jobs. Linux
is the canonical full-quality job on the single pinned toolchain: it verifies
the Rust pins agree, installs the Linux Tauri prerequisites, and runs the
remaining `mise run ci` checks, including formatting, Clippy, all workspace
targets, frontend state tests, and the unattended desktop workflow. The macOS
and Windows jobs each run the portable workspace tests, frontend state tests,
unattended desktop workflow, and `mise run desktop:build` for a native debug
`--no-bundle` compilation.

Project-local Mise tasks remain the command source of truth on every platform.
The portability jobs explicitly select `shell: bash`; on Windows, GitHub Actions
therefore uses the Bash supplied by Git for Windows instead of the PowerShell
default. Simple task wrappers intentionally remain Bash. The desktop build and
development tasks resolve Mise's Cargo and `cargo-tauri` executables in Bash;
under Git Bash they cross into native PowerShell with one canonical Windows
`Path` before Tauri starts its Cargo child processes. Developers and CI still
use the same `mise run desktop:build` and `mise run desktop:dev` entry points on
every platform. The desktop E2E task uses Node's clock rather than a
platform-specific `time` executable and records
the runner OS, elapsed seconds, exit status, and bounded phase diagnostics in
`target/desktop-e2e/last-run.log`. A failed portability job uploads that log for
seven days when it exists.

The workflow uses dated GA labels: `ubuntu-24.04`, `macos-15`, and
`windows-2025`. GitHub still updates those managed images in place. Green CI
proves that the portable contract compiled and passed on the exact image
recorded in the workflow log; it does not claim bit-for-bit reproducibility or
declare a supported release platform or architecture. Release support,
artifacts, signing, and publication remain separate decisions.

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
