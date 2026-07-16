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

The RBN tests use only the purpose-built
`crates/rbn/tests/fixtures/synthetic-current.csv` fixture. It models the
documented 13-column header and covers CW, RTTY, malformed, filtered,
unsupported-band/mode, exact duplicate, replay, conflict, schema drift, ZIP,
and resource bounds without committing third-party spot rows. The adapter,
schema-v3 persistence, and desktop state tests require no network. A disposable
compatibility check against the official 2026-07-14 daily archive confirmed
the pinned header and representative field conventions; the downloaded bytes
were not added to the repository. Re-run such checks only in disposable local
storage and never turn them into fixtures without confirmed redistribution
permission.

Desktop orchestration tests inject that same heartbeat/status/decode sequence
below the socket and exercise atomic adapter/observation persistence,
malformed/unsupported/client-mismatch/duplicate dispositions, lost
acknowledgement, stale heartbeat, resource-stop, lifecycle-stop, and restart
status without binding a port. A real WSJT-X installation is never required.

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
export without a derived report. Desktop tests also pin revision-stable frame
identity, retained presentation after refresh/export failure, exact HTML bytes,
create-new destination behavior, and checkpoint revision agreement across the
visible report and both export paths. Production entry points always select the
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

Solar-context tests independently fixture-check the deterministic approximation
against the NOAA GML Solar Geometry Calculator's uncorrected elevation for
40° N, 105° W at 2024-06-20 12:00 UTC (NOAA: 3.93770°), with a pinned maximum
0.5° fixture difference. Unit and integration cases also cover exact 0/-6/-12/-18°
category boundaries, polar day and night, leap-year UTC rollover, invalid and
non-finite coordinates, valid 4/6/8-character Maidenhead cell centers, typed
missing versus malformed grids, reordered source observations, serialized
algorithm/input metadata, and hostile report strings. The 0.5° assertion is a
fixture regression bound, not a global physical-accuracy claim; atmospheric
refraction and the finite locator cell are deliberately outside the model.

To generate the canonical sample as an untracked verification artifact:

```bash
cargo run -p antennabench-report --example render_canonical_sample -- /tmp/antennabench-sample.html
```

For documentation-only changes, inspect the rendered intent and verify the diff
is limited to the requested files.

The optional hosted foundation is verified independently with:

```bash
mise run hosted:test
```

It uses locked npm dependencies, generated Wrangler binding types, fake service
inventory tests, strict TypeScript, and a no-account dry build. The dry build
uses `--containers-rollout=none` so ordinary CI needs no Docker daemon and does
not provision or contact Cloudflare. Environment matrix tests pin distinct
private-upload, private-derived, public-report, D1, Queue/DLQ, and bounded
Container configuration. Local Rust and desktop tests never read hosted config.

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
declare a supported release platform or architecture.

The separate read-only desktop release artifact probe runs only when its
inputs change, on demand, and after matching changes reach `main`. It uses the
selected native `macos-15` arm64 and `macos-15-intel` runners, runs the portable
and unattended checks, and retains each verified non-publishable artifact input
for seven days. It cannot read release credentials or mutate tags and releases.

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
It injects deterministic setup/conductor clocks and IDs plus open/save
selections immediately below the native picker adapter. One composed seeded
scenario reviews and creates an exact checkpointed schema-v2 setup, then runs
the production manual conductor through start, a lost-response retry, explicit
actual-antenna confirmation, missed/bad/note/correction evidence, an operator
interruption/resume, synthetic WSJT-X raw evidence plus observation, a bounded
adapter gap, a torn-write failpoint, process recovery, final resume/end, report
refresh, exact standalone HTML export, checkpointed bundle export, collision
rejection, and reopen. It asserts revision identity, retry idempotency, raw hex,
effective corrections, explicit gap disclosure, terminal lifecycle, exported
checkpoint equality, and deterministic report identity. Focused scenarios also
cover cancellation, stale revisions, replacement, malformed JSON, and the
remaining recovery/resource matrices. Temporary sources and destinations are
isolated and automatically removed. The task does not launch Tauri, create a
window, open a socket, take focus, or send keyboard or pointer input.

The task streams phase diagnostics to the terminal and overwrites the bounded
`target/desktop-e2e/last-run.log` artifact on every run. It records the platform,
elapsed seconds, and final status without depending on a Unix-only timing tool.
CI runs the same task, so failures retain the selected phase, fixture path,
typed error kind, technical detail, and Rust assertion context in both the
artifact and job log.
The composed scenario records its fixed seed and result inside the temporary
scenario root; if an assertion panics, that exact root is copied to
`target/desktop-e2e-failures/<seed>/` before temporary cleanup.

On the 2026-07-13 macOS development machine, the warm task completed in 0.42 s.
The prior issue #18 foreground smoke took 3 min 49 s from application relaunch
through cleanup (2 min 49 s from opening the first picker through the final
cancellation). The unattended path is therefore more than 400 times faster
than the interactive workflow while covering the canonical behavior more
deterministically.

`desktop:test` retains the focused Rust and pure JavaScript tests.
`desktop:build` builds a debug application without producing installer bundles,
and `desktop:dev` launches the static shell with Tauri's development server.

## Desktop Release Artifact Construction

The initial release contract supports macOS 15 and later with separate native
Apple-silicon and Intel application archives. Xcode Command Line Tools and the
Mise-managed tools listed above are the only local prerequisites for the
non-secret build. Each architecture must be built on its matching native host:

```bash
# Apple silicon on macos-15 or a local arm64 Mac
mise run desktop:release-bundle -- aarch64-apple-darwin

# Intel on macos-15-intel or a local x86_64 Mac
mise run desktop:release-bundle -- x86_64-apple-darwin
```

The optional `--tag vMAJOR.MINOR.PATCH` argument fails unless it exactly matches
the Cargo workspace version. CI also passes `--runner-label macos-15` or
`--runner-label macos-15-intel`; a mismatched native machine, runner, target,
version, or tag fails before staging.

The command first runs the single-toolchain check and the fresh non-secret
`release-preflight`. It installs the explicit Rust target, then invokes Tauri
in release mode with only `--bundles app`, `--ci`, and `--no-sign`. The build
has a 30-minute timeout and never invokes the DMG bundler or Finder/AppleScript.
Tauri inherits its version from the Cargo workspace, while its configuration
pins the only bundle target to `app` and the minimum system version to macOS
15.0.

Before staging, the task verifies all of the following against the built app
and again after a `ditto` ZIP extraction:

- the product name, bundle identifier, short version, build version, and
  minimum-system metadata;
- the single Mach-O architecture and deployment target;
- the exact target-derived archive name and archive structure;
- the signature, hardened-runtime, timestamp, notarization, and Gatekeeper
  classification appropriate to the selected trust mode; and
- the archive byte size and SHA-256 digest.

The normal build deliberately skips Developer ID signing. On Apple silicon the
Mach-O may retain an ad-hoc linker signature, but the target manifest records
`publishable: false`, the directory contains `NON_PUBLISHABLE.txt`, and output
is isolated under:

```text
target/desktop-release/non-publishable/<target>/
├── AntennaBench-<version>-<target>.zip
├── artifact-manifest.json
└── NON_PUBLISHABLE.txt
```

Staging uses a temporary sibling directory. The stable directory appears only
after build, archive extraction, metadata verification, digest verification,
and exact asset-set validation all pass. A failed or interrupted attempt
removes both stale final output and partial staging output. Everything remains
under ignored `target/`; credentials, application bundles, archives, and
notarization material must never be committed.

After both native jobs have produced target directories, combine them with:

```bash
mise run desktop:release-assemble -- \
  target/desktop-release/non-publishable/aarch64-apple-darwin \
  target/desktop-release/non-publishable/x86_64-apple-darwin
```

That command requires exactly one artifact for each selected target, identical
versions, tags, and source commits, and matching manifest sizes and digests. It
atomically creates the two ZIPs,
`AntennaBench-<version>-release-manifest.json`, and the bytewise-sorted
`AntennaBench-<version>-SHA256SUMS`. The checksum file covers both archives and
the release manifest, but not itself. The assembled local set remains under
`target/desktop-release/non-publishable/complete` and cannot pass
`--require-publishable`.

The protected `v*` tag workflow owns the credentialed layer. It signs,
notarizes, and staples each `.app`, then calls `desktop:release-stage` with
`--trust-mode release` and assemble with `--require-publishable`. Release mode
fails unless Developer ID authority, hardened runtime, secure timestamp,
stapled notarization, strict code-signature validation, and Gatekeeper
assessment all pass. Only that complete output may be attached to a draft
GitHub Release. [Desktop Releases](releasing.md) is the maintained owner and
user runbook; the workflow never publishes a stable release.

Run the platform-independent parsing, naming, manifest, checksum, unexpected
asset, and failure-cleanup regressions with:

```bash
mise run desktop:release-test
mise run desktop:publication-test
```

For troubleshooting, inspect the command's first failing invariant. A host or
runner mismatch requires the selected native machine; an embedded metadata or
Mach-O deployment mismatch must be fixed at the Tauri/build boundary rather
than renamed after the fact. A missing app indicates a Tauri build failure.
Archive verification failures leave no stable target directory. Policy or
advisory failures must be resolved through the maintained supply-chain process;
they are not bypassed by the release task.

The remaining native-picker smoke is optional release/platform verification,
not routine regression testing. Run `mise run desktop:dev`, confirm the window
opens, and verify only that the open and save pickers appear, cancellation
returns to the app normally, and selecting the canonical fixture hands a local
directory path to the app. The unattended test owns open → report → export →
reopen semantics, error behavior, preservation assertions, and diagnostics; do
not repeat those checks with coordinate-driven automation. Stop the development
process with Control-C and use `jj status` to confirm the fixture was unchanged.

The main webview capability allowlists focused setup, conductor, receiver,
session-open, manual WSPR.live/RBN import, checkpoint-export,
report-read/refresh, and report-export
commands. Native selection and all filesystem/domain work remain in Rust; the
report commands return only a bounded revision-keyed presentation. No dialog-plugin or
filesystem-plugin permission is granted to JavaScript; this is intentional
even though the native dialog plugin is registered. The local report is loaded
into a sandboxed frame and neither the shell nor report is given network
authority.

The transfer screen's RBN action is available only for schema-v3 sessions that
have started. Rust derives the exact callsign, half-open schedule window, and
distinct bands from the committed bundle, owns the native ZIP picker and
bounded parsing, and commits the exact archive plus all retained dispositions
under one checkpoint. The frontend receives only the bounded outcome summary.

## Documentation Updates

When implementation changes project behavior, update the relevant evergreen doc
in the same change when practical.

Prefer documenting stable concepts, invariants, and workflows. Avoid copying
detailed implementation-plan steps or generated code into evergreen docs.
