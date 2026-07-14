# 0012: Use Combined Supply-Chain Maintenance Gates

Date: 2026-07-14

## Decision

AntennaBench will use a combined GitHub-native and Rust-native dependency and
CI supply-chain baseline.

GitHub provides dependency inventory, alerts, reviewed update pull requests,
pull-request dependency review, Rust and workflow code scanning, and
repository enforcement. A pinned cargo-deny installation provides the
repository-owned advisory, license, source, wildcard, and duplicate policy
against the committed Cargo lockfile.

All external GitHub Actions and remote reusable workflows use reviewed
full-length commit SHAs. Routine Cargo and Action updates arrive weekly through
Dependabot, but never merge automatically. Security updates and fresh advisory
checks retain a faster independent path.

Repository-owner settings and rulesets are explicit follow-up work. This
decision does not enable a setting, create a credential, or weaken the signed
release boundary.

## Current Inventory

The repository was inspected on 2026-07-14:

- seven Rust workspace packages share one committed Cargo.lock;
- the lockfile has 452 package stanzas: seven workspace/path packages, 445
  registry packages, and no git-sourced packages;
- crates.io is the only external Rust package source;
- the desktop depends on Tauri, and the propagation adapter depends on
  network-facing reqwest/rustls code;
- Rust 1.96.1 is the exact routine compiler and Rust 1.89 is the separately
  enforced compatibility floor;
- cargo-tauri 2.11.4 is exact in Mise, but Cargo-backed tool installation still
  needs an explicit locked-install guarantee;
- Node runs dependency-free frontend tests and timing helpers, but no
  package.json or Node lockfile exists and the Node runtime is not yet pinned;
- the only workflow has seven external Action uses across actions/checkout,
  jdx/mise-action, and actions/upload-artifact;
- every Action currently uses a moving major tag rather than an immutable SHA;
- ordinary workflow permission is contents: read, with no release credentials;
- routine jobs use moving ubuntu-latest, macos-latest, and windows-latest
  runner aliases;
- no Dependabot configuration, Rust advisory/policy task, dependency-review
  job, CodeQL setup, or repository ruleset is present; and
- secret scanning and push protection are enabled, while the issue audit
  records Dependabot alerts/security updates as disabled.

The public-repository dependency graph is a GitHub-managed feature. Cargo.lock
and Cargo.toml are supported Cargo inputs, and workflow uses entries are
recognized as GitHub Actions dependencies.

Neither cargo-audit nor cargo-deny was installed in the inspected environment.
This decision therefore does not claim that the current lockfile has no known
advisory. The first implementation must record a fresh complete baseline
instead of treating absence of a local tool as a clean result.

## Dependency Surfaces

### Rust Packages

Cargo.toml declares direct requirements and Cargo.lock identifies the complete
resolved graph used by application builds. Both remain version controlled.
Updates include the manifest and lockfile diff in one focused pull request.

Workspace path dependencies are trusted repository code. crates.io is the only
allowed external registry. A git dependency, alternate registry, vendored
tree, or binary-only crate source requires a focused security and licensing
review before it can be admitted.

Cargo.lock is never regenerated incidentally inside unrelated work. A focused
update records the requested direct or transitive packages, before/after
versions, advisory or maintenance reason, important feature changes, duplicate
effects, MSRV impact, and verification.

### Node And Tauri Tooling

There is currently no npm dependency graph. Node is only a test/runtime tool.
The implementation pins one exact active LTS Node release in Mise so local and
CI JavaScript tests do not inherit an arbitrary runner default.

Adding package.json or another ecosystem manifest requires its complete
lockfile, a Dependabot or equivalent update entry, license/source policy, and
the same untrusted pull-request boundary before the first dependency lands.

Rust, Node, cargo-tauri, cargo-deny, and future repository-installed tools use
exact reviewed versions. A Cargo-backed tool is installed with the published
release lockfile or an equivalently verified upstream artifact. Floating
latest tool requests and unverified installer pipes are not permitted.

### Runner And System Packages

Routine CI moves from latest aliases to dated GA OS labels. These labels still
receive GitHub runner-image patch updates; GitHub does not expose immutable
standard-runner image references.

The project accepts that managed-image and OS repository patch channel for
ordinary CI, logs the resolved image/version, and reviews dated-label
deprecations. It does not claim bit-for-bit CI reproducibility. Release jobs
retain the explicit native runner labels and artifact metadata selected by
ADR 0007.

Linux Tauri system packages continue to come from the selected runner OS
repositories. A new third-party apt, Homebrew, Chocolatey, script, or package
repository requires the same focused source review as a new language registry.

## GitHub Update And Review Baseline

### Dependabot

The repository commits a version-2 Dependabot configuration for:

- Cargo at the workspace root; and
- GitHub Actions at the repository root.

Both run weekly. Routine patch and minor updates may be grouped per ecosystem;
major updates remain individual. Each ecosystem permits at most five open
routine update pull requests. Security update pull requests are not delayed
behind an unrelated routine group.

Dependabot pull requests run the same checks as any other untrusted pull
request. They receive no release environment, Apple credential, repository
write token, attestation permission, or automatic merge authority.

Mise tool pins and rust-toolchain.toml are not assumed to be covered by Cargo
Dependabot. The maintainer reviews exact Rust, Node, cargo-tauri, cargo-deny,
and dated runner pins monthly and opens focused updates when needed.

### Pull-Request Dependency Review

A SHA-pinned dependency-review action runs only on pull requests with
contents: read. It fails when a change introduces a known vulnerability of
moderate severity or higher. Low-severity additions remain visible and enter
the normal triage clock.

The action is an introduction gate, not the only inventory. Dependabot and
fresh RustSec checks cover vulnerabilities disclosed after a dependency is
already on main.

License enforcement remains repository-owned through cargo-deny because its
allowlist, exceptions, and complete target graph need deterministic review.

### CodeQL

The owner enables GitHub-managed CodeQL default setup for Rust and GitHub
Actions workflows. It runs on pull requests, pushes to main, and GitHub's
managed schedule without repository or release secrets.

Default setup is preferred because GitHub manages the supported CodeQL bundle,
language extraction, and upload permissions. If it cannot cover both selected
languages, the project creates a focused advanced-setup issue. An advanced
workflow would SHA-pin the CodeQL action and grant security-events: write only
to the scanning job.

CodeQL supplements compiler, Clippy, tests, dependency review, and human review.
It is not evidence that the application has no vulnerabilities.

## Immutable GitHub Actions

A full-length commit SHA is the only accepted external Action or remote
reusable-workflow reference. The same line carries a human-readable upstream
release tag comment. Review verifies that the commit belongs to the upstream
repository and corresponds to the named signed or verified release.

Dependabot updates both the SHA and same-line release comment. Its pull request
must show the upstream release notes and pass the untrusted checks before
merge.

Local repository actions may use relative paths. Container actions require an
immutable digest and a reviewed source; mutable image tags are forbidden.

The repository check rejects branches, tags, short SHAs, unpinned containers,
and remote workflow references that are not full immutable identities.
Repository Actions settings enforce the same policy after the owner follow-up
lands.

Routine jobs keep contents: read. Any additional permission is job-scoped and
documented. Credentialed release actions are limited to GitHub-owned Actions or
an explicitly allowlisted, reviewed third party, all SHA-pinned. No untrusted
pull request can reach those jobs.

## Rust-Native Policy

### Tool And Cadence

The repository pins one exact cargo-deny release in Mise and installs it from
the release's published lockfile or verified artifact. Version 0.19.4 was
current during this decision; a newer version is acceptable only as a focused
reviewed update with compatibility evidence.

The policy evaluates all targets and features represented in Cargo.lock.
Deterministic license, source, bans, wildcard, and duplicate checks run on
every pull request and main push. A fresh RustSec advisory check runs for
dependency-changing pull requests, every main push, daily on a schedule, and
again in the non-secret release preflight.

A required fresh advisory check fails closed when the database cannot be
updated. A transient network failure may be rerun, but it is not converted
into a successful audit.

### Advisory And Maintenance Policy

Known vulnerabilities, malicious-package notices, and unsound advisories are
denied throughout the resolved graph. Newly introduced yanked versions are
denied.

An unmaintained direct/workspace dependency is a blocking finding. A
transitive unmaintained notice is reported and tracked, but does not
automatically fail every ordinary build when the project cannot directly
replace it. The maintainer evaluates reachability, upstream plans, and
alternatives on the cadence below.

Informational notices remain visible and require classification. They are not
silently discarded merely because their advisory category is not a
vulnerability.

### Source, Wildcard, And Duplicate Policy

Workspace paths and the crates.io registry are allowed. Unknown registries and
git dependencies fail. A source exception names the exact repository,
revision, owner, review reason, and expiry; a branch reference is never enough.

External wildcard requirements fail. Workspace path relationships may use the
Cargo path form that metadata represents as a wildcard because their code is
in the same reviewed revision.

Duplicate versions are warnings with inclusion paths rather than a blanket
failure. The current Tauri graph legitimately resolves multiple semver-
incompatible versions. A dependency update must explain material new
duplicates, but this policy does not encourage risky patch overrides solely to
make a count reach zero.

### License Policy

The initial automatically allowed SPDX set is:

- 0BSD;
- Apache-2.0;
- Apache-2.0 WITH LLVM-exception;
- BSD-2-Clause;
- BSD-3-Clause;
- BSL-1.0;
- CC0-1.0;
- ISC;
- MIT;
- MPL-2.0;
- Unicode-3.0; and
- Zlib.

The implementation inventories every current crate license before enabling the
gate. A license outside this set is not broadly added merely to turn CI green.
It receives an exact crate/version/expression review and a named exception or
a deliberate allowlist amendment.

Automated metadata checking is not legal advice and cannot prove that every
source file has correct licensing. It is a change detector and review gate.

## Exceptions

Every advisory, license, source, yanked, or maintenance exception is
machine-checkable and contains:

- stable advisory, crate, source, or license identity;
- exact affected version or range;
- whether the code is direct, transitive, build-time, development-only, or
  packaged;
- reachability and impact analysis;
- mitigation and removal plan;
- responsible owner;
- linked GitHub issue;
- approval date; and
- expiry date.

Critical/high and unsound exceptions expire within 30 days. Moderate/low,
unmaintained, yanked, source, and license exceptions expire within 90 days.
Renewal requires new evidence and a new review; dates are never extended
silently.

Expired exceptions fail. An advisory ignore that no longer matches the graph
also fails so stale suppressions are removed.

A known malicious package, unexplained source, mutable CI reference, missing
release signature, or release credential exposure cannot be waived into a
public release.

## Ownership And Response Targets

The repository owner is the initial alert and exception owner. If another
maintainer assumes a finding, the issue records that transfer. GitHub
notifications, scheduled workflow failures, and the Security tab are checked
at least each business day while a release is being prepared.

| Finding | Triage target | Remediation or containment target |
| --- | ---: | ---: |
| Malicious package or critical vulnerability | same business day | contain within 24 hours; patched release or removal within 72 hours |
| High or reachable unsoundness | 1 business day | 7 calendar days |
| Moderate | 3 business days | 30 calendar days |
| Low | 10 business days | 90 calendar days |
| Direct unmaintained or yanked | 10 business days | replacement/retention plan within 90 days |
| Transitive unmaintained or material duplicate | monthly review | quarterly plan review |

If a target cannot be met, the time-bounded exception documents why, how users
are protected, and when the next decision occurs. A release is withheld when
the release gate fails even if ordinary local development can continue.

An Action or tool compromise triggers pinning/removal, revocation of exposed
tokens or credentials, an audit of affected workflow runs and release assets,
and the withdrawal/replacement policy in ADR 0007.

## Update Procedure

A routine dependency pull request:

1. identifies the requested packages, Actions, tools, or runner labels;
2. reviews upstream release notes and provenance;
3. inspects manifest, lockfile, features, source, license, duplicate, and MSRV
   changes;
4. runs the Rust 1.89 compatibility job and Rust 1.96.1 full suite;
5. runs dependency review, cargo-deny, CodeQL, and action-pin validation;
6. records any exception with an issue and expiry; and
7. merges only through the required main ruleset.

Broad cargo update output is not hidden in feature work. A full lockfile
refresh, when needed, is its own pull request with the same review evidence.

Routine patch/minor groups may merge together when failures remain attributable.
A major dependency, Action, Rust compiler, Node runtime, cargo-tauri,
cargo-deny, or runner-OS update is focused and independently reversible.

## Blocking Matrix

### Pull Requests

All pull requests require existing CI/MSRV checks, action-pin/tool-pin policy,
deterministic cargo-deny license/source/bans checks, dependency review, and
CodeQL.

A dependency-changing pull request also requires a fresh RustSec advisory
check. Moderate-or-higher introduced vulnerabilities block dependency review;
the Rust policy may be stricter for malicious, unsound, yanked, source, and
license findings.

No pull-request job receives release secrets or write permission.

### Main And Schedule

Main pushes repeat the policy and fresh advisory checks. A daily read-only
advisory schedule detects disclosures that occur without a code change. GitHub
Dependabot alerts/security updates and managed CodeQL scans provide independent
notification and remediation paths.

A scheduled failure opens or updates focused tracked work; it does not mutate
manifests or suppress findings automatically.

### Releases

The release workflow performs a fresh, non-secret supply-chain preflight before
the desktop-release environment or Apple credentials can be reached. It checks
the exact Cargo.lock, tool pins, Action pins, CodeQL/main status, exceptions,
and advisory database for the tagged commit.

Any failure or expired exception stops publication. The credentialed workflow
cannot disable, skip, or downgrade the preflight. Artifact signing,
notarization, checksums, provenance, draft promotion, and immutable publication
remain governed by ADR 0007 and issues #35/#36.

The formal SBOM remains deferred by ADR 0007. Cargo.lock, the exact tool and
source revision, release manifest, checksums, and attestations remain required
evidence.

## Owner-Managed Settings

After implementation checks have stable names, the owner:

- enables Dependabot alerts and security updates without auto-merge;
- enables CodeQL default setup for Rust and GitHub Actions;
- enables private vulnerability reporting;
- preserves secret scanning and push protection;
- keeps the default workflow token read-only and forbids Actions from
  approving pull requests;
- limits Actions to GitHub-authored and explicitly allowlisted repositories;
- requires full Action commit SHAs; and
- creates a main ruleset requiring pull requests, signed commits, no
  force-push/deletion, and every selected status check.

The single-maintainer ruleset uses satisfiable review requirements rather than
claiming independent approval that does not exist. Any owner emergency bypass
is rare, audited, and documented in the affected issue.

Issue #60 owns these mutations. Agents must not change them incidentally while
implementing code checks.

## Release-Issue Coordination

Issue #35 consumes the exact toolchain, locked Cargo graph, dated runner, and
policy result in its artifact manifest. It does not treat a moving tool or
runner default as release input.

Issue #36:

- SHA-pins every Action;
- runs the fresh supply-chain preflight before credential access;
- keeps build, scan, attestation, and release permissions job-scoped;
- never exposes Apple or GitHub release authority to pull requests or
  Dependabot;
- records the exact dependency/tool/action inputs with the release evidence;
  and
- follows ADR 0007 when a compromised input requires withdrawal.

The closed release decision #34 remains unchanged: this policy strengthens its
maintenance and preflight boundary without selecting new platforms, assets,
credentials, or publication behavior.

## Verification

Implementation includes deterministic tests that:

- reject a mutable Action tag, branch, short SHA, container tag, and remote
  reusable workflow;
- accept a full upstream SHA with a same-line release comment;
- validate Dependabot Cargo and Actions configuration;
- prove dependency review has read-only permission and receives no secrets;
- reject unknown registry, git source, external wildcard, unknown license,
  vulnerability, malicious, unsound, yanked, expired exception, and stale
  ignore fixtures;
- retain duplicate and transitive-unmaintained inclusion paths as visible
  diagnostics;
- prove an advisory database fetch failure cannot become a successful fresh
  audit;
- prove pull-request, daily, and release jobs have only their selected
  permissions;
- prove the release credential job depends on a successful fresh preflight;
  and
- verify exact tool and dated runner pins.

No test introduces a real vulnerable dependency on main, contacts a private
registry, or requires a release secret.

## Alternatives Considered

### GitHub-Native Baseline Only

Dependabot, dependency review, and CodeQL provide strong hosted coverage with
low repository maintenance. This was rejected as the only layer because
license/source policy, exact exception expiry, offline review, and release
preflight should remain repository-owned and reproducible.

### Rust-Native Baseline Only

cargo-deny can evaluate the complete Cargo graph with explicit policy. It was
rejected as the only layer because it does not maintain Action references,
enforce pull-request dependency diffs, provide CodeQL, or enable GitHub's
continuous alert/update path.

### Combined Baseline

This is selected. The overlap between RustSec, GitHub Advisory Database, and
dependency review is intentional independent detection rather than redundant
authority. RustSec states that GitHub imports its advisories; different
ingestion and execution paths still catch configuration or timing gaps.

### Mutable Major Action Tags

Moving major tags receive fixes automatically, but the referenced code can
change without repository review. Full SHAs plus Dependabot preserve update
flow while making every executed revision explicit.

### Immediate Auto-Merge

Auto-merge would reduce maintenance work but could combine an upstream
regression, MSRV change, toolchain change, or Action compromise with trusted
release work. It is deferred until the project has enough evidence to define
safe categories.

### Block Every Duplicate And Unmaintained Transitive Crate

This looks strict but creates noisy gates in a large platform framework graph
that the project cannot always resolve directly. Direct unmaintained
dependencies block; transitive findings and duplicates remain visible,
time-owned maintenance work.

## Consequences

- Dependency and workflow changes become explicit reviewed inputs.
- The project gains independent hosted alerting and repository-owned policy.
- Routine dependency work creates a small continuing maintenance obligation.
- Advisory database or GitHub service outages can temporarily block a fresh
  dependency or release gate rather than silently passing it.
- Some current licenses or maintenance notices may require focused baseline
  issues before cargo-deny can become required.
- Dated runners still receive managed patch updates; the project is
  reproducible in selected tools and source, not bit-for-bit runner images.
- Release credentials remain downstream of non-secret supply-chain checks.

## References

- [Decision issue #44](https://github.com/rwjblue/antennabench/issues/44)
- [Action/update implementation #58](https://github.com/rwjblue/antennabench/issues/58)
- [Rust policy implementation #59](https://github.com/rwjblue/antennabench/issues/59)
- [Owner settings #60](https://github.com/rwjblue/antennabench/issues/60)
- [Release decision 0007](0007-ship-separate-signed-macos-release-archives.md)
- [GitHub secure use reference](https://docs.github.com/en/actions/reference/security/secure-use)
- [GitHub Dependabot version updates](https://docs.github.com/en/code-security/concepts/supply-chain-security/dependabot-version-updates)
- [GitHub dependency review](https://docs.github.com/en/code-security/concepts/supply-chain-security/dependency-review)
- [GitHub dependency graph ecosystems](https://docs.github.com/en/code-security/reference/supply-chain-security/dependency-graph-supported-package-ecosystems)
- [GitHub CodeQL](https://docs.github.com/en/code-security/concepts/code-scanning/codeql/codeql-code-scanning)
- [RustSec advisory database](https://rustsec.org/)
- [cargo-deny checks](https://embarkstudios.github.io/cargo-deny/checks/)
- [GitHub-hosted runner images](https://github.com/actions/runner-images)

