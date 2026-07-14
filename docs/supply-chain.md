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
