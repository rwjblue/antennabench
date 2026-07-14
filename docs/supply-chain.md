# Supply-Chain Updates

[Decision 0012](decisions/0012-use-combined-supply-chain-maintenance-gates.md)
defines the project policy. This page is the operating procedure for the
repository-owned Action, runner, tool, and dependency-maintenance checks.

## Routine Pull Requests

Dependabot checks Cargo and GitHub Actions every Monday. Each ecosystem permits
at most five open update pull requests. Patch and minor version updates may be
grouped; major and security updates remain independent. Updates are never
merged automatically.

For every update:

1. Read the upstream release notes and identify relevant behavior, security,
   feature, platform, and minimum-toolchain changes.
2. Inspect manifest and lockfile changes, including new sources, licenses,
   features, duplicate versions, and build or runtime reachability.
3. Run `mise run ci`. Dependency-changing pull requests must also pass the
   read-only GitHub dependency-review job.
4. Keep the update focused. A major dependency, Action, Rust, Node, Mise,
   cargo-tauri, or runner update is independently reviewable and reversible.
5. Record any exception under the ownership, evidence, issue, and expiry rules
   in Decision 0012. Never weaken a gate merely to merge an update.

## GitHub Action Pins

Every external `uses:` reference is a full 40-hex commit SHA followed by the
corresponding upstream release tag in a same-line comment. Before accepting an
update, verify in the upstream repository that the release tag resolves to the
reviewed commit and inspect GitHub's signature/verification result for that tag
or commit. Review the release notes and the Action repository diff between the
old and new commits. Dependabot's proposed SHA and comment are inputs to this
review, not proof by themselves.

Local Actions may use relative paths. Container Actions and workflow container
images require immutable `sha256` digests. Remote reusable workflows follow
the same full-SHA and release-comment rule as Actions. Branches, tags, short
SHAs, moving container tags, and `*-latest` runners are rejected by
`mise run supply-chain`.

When updating an Action manually, change the SHA and release comment together,
then run:

```bash
mise run supply-chain
mise run ci
```

## Tool And Runner Pins

Node, Rust, and cargo-tauri use exact reviewed versions in `.mise/config.toml`;
the Tauri CLI install is lockfile-backed. The CI workflow also pins the Mise
release. Review these pins monthly because Cargo Dependabot does not own them.
The validator requires exact versions and fails if the convention drifts.

Routine jobs use dated GA runner labels. GitHub can update a dated managed
image in place, so workflow logs remain the evidence for the actual image used.
Runner-label changes receive focused release-note and portability review.

## Adding A Dependency Ecosystem

`.github/dependency-policy.json` owns every recognized dependency manifest.
Adding a package manifest requires, in the same focused change:

- a complete committed lockfile;
- an explicit update mechanism such as a Dependabot entry;
- documented source, license, advisory, and exception policy; and
- coverage in the supply-chain validator and untrusted pull-request checks.

The guard deliberately fails when it discovers a known manifest without a
matching policy entry. Extend the known-manifest list when adopting an
ecosystem whose manifest name is not already recognized.

## Rust Dependency Policy

cargo-deny 0.19.4 is an exact Mise pin. The Cargo backend is configured with
`locked = true`: source installation uses the release lockfile, while supported
platforms may use cargo-binstall's checksum-verified upstream release artifact.
The installed binary must report `cargo-deny 0.19.4`.

`deny.toml` evaluates every Cargo-resolved target and enables all features. The
baseline is:

- vulnerabilities, malicious/notice advisories, and unsound advisories fail;
- yanked versions fail;
- an unmaintained direct workspace dependency fails, while an inherited
  transitive unmaintained crate remains visible for tracked remediation;
- external wildcard requirements fail, while path-only relationships among the
  repository's explicitly non-published workspace crates are allowed;
- crates.io and workspace paths are the only standing sources; unknown
  registries and every git source fail;
- the ADR 0012 permissive license set is allowed, with unknown licenses denied;
  and
- duplicate versions are warnings with inverse dependency paths rather than a
  blanket failure for the cross-platform Tauri graph.

Run the deterministic gate with:

```bash
mise run dependency-policy
```

The 2026-07-14 baseline evaluates 472 locked packages, all from workspace paths
or crates.io. It has 32 duplicate-version warning families, no source errors,
and one exact license exception: `webpki-roots 1.0.8` under
CDLA-Permissive-2.0, tracked by
[#82](https://github.com/rwjblue/antennabench/issues/82) through 2026-10-12.
The baseline updated `url` to 2.5.8 and `idna` to 1.1.0 to remove
RUSTSEC-2024-0421 instead of suppressing it.

## Fresh Advisory And Release Gate

`mise run advisory-fresh` validates exceptions and runs a locked cargo-deny
advisory check with fetching enabled. It does not use `--offline`,
`--disable-fetch`, or a successful fallback, so failure to clone or update the
RustSec database fails the task. The database must also be no more than one day
old whenever fetching is explicitly disabled for diagnosis.

The read-only `Fresh Rust supply-chain preflight` workflow runs:

- on a pull request that changes Cargo manifests, Cargo.lock, policy, tool pins,
  or the gate itself;
- on every push to `main`;
- daily at 11:17 UTC;
- through an explicit manual dispatch; and
- through `workflow_call` for a future release workflow.

It has only `contents: read`, references no repository or release secrets, and
runs `mise run release-preflight`, which checks workflow/tool pins,
deterministic Rust policy, exception expiry, and a fresh RustSec database in
that order. A future credentialed release job must call the reusable workflow
and declare it as a successful dependency before it can reach an environment or
credential. The clearly named scheduled run is visible in Actions and GitHub's
scheduled-workflow failure notifications; the read-only job does not mutate an
issue to manufacture a notification.

The initial fresh audit has one temporary unsound exception:
RUSTSEC-2024-0429 for Linux-only `glib 0.18.5`, inherited from Tauri's GTK
stack. The repository has no direct use of the affected iterator API, but does
not claim it is unreachable throughout the framework. The exception is tracked
by [#83](https://github.com/rwjblue/antennabench/issues/83), expires on
2026-08-13, and becomes a hard failure when expired or unused.

## Focused Lockfile Updates

Do not refresh Cargo.lock incidentally. A focused dependency update must:

1. name the direct or transitive packages and the maintenance, advisory, or
   compatibility reason;
2. use a narrow `cargo update -p <package> --precise <version>` when possible;
3. inspect every added, removed, and changed package, feature, source, license,
   duplicate path, and Rust-version implication;
4. run `mise run dependency-policy`, `mise run advisory-fresh`, and
   `mise run ci`; and
5. record the before/after versions, findings, exceptions, and verification in
   the focused issue or pull request.

A full lockfile refresh is its own change. It never hides inside unrelated
feature work.

## Exceptions And Response

`.github/dependency-exceptions.json` is the review record; every exception also
has an exact enforcement reference in `deny.toml`. The validator requires a
unique identity, exact locked crate version, category/severity, reachability,
rationale, mitigation, GitHub owner, repository issue, approval date, expiry,
and a live matching policy entry. Malicious packages and mutable CI inputs have
no permitted category and cannot be waived.

Critical/high and unsound exceptions last at most 30 days. Moderate/low,
unmaintained, yanked, source, and license exceptions last at most 90 days.
Expired records, packages no longer in Cargo.lock, missing policy references,
unused advisory ignores, and unused license/source exceptions fail.

Response targets are:

| Finding | Triage target | Remediation or containment target |
| --- | ---: | ---: |
| Malicious package or critical vulnerability | same business day | contain within 24 hours; patch/remove within 72 hours |
| High or reachable unsoundness | 1 business day | 7 calendar days |
| Moderate | 3 business days | 30 calendar days |
| Low | 10 business days | 90 calendar days |
| Direct unmaintained or yanked | 10 business days | replacement/retention plan within 90 days |
| Transitive unmaintained or material duplicate | monthly review | quarterly plan review |

To renew an exception, update its evidence and approval/expiry dates, add a
fresh issue comment explaining why remediation is still unavailable, and rerun
the complete release preflight. Dates are never extended silently. To close an
exception, remove both the JSON record and its precise deny.toml allowance,
show that the package/finding is absent or fixed, run the complete preflight,
and close the tracking issue with the lockfile diff and verification evidence.
