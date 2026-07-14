# 0007: Ship Separate Signed macOS Release Archives

Date: 2026-07-13

Toolchain policy amended by
[Decision 0014](0014-use-one-pinned-rust-toolchain.md).

## Decision

The first public AntennaBench desktop release will support macOS 15 and later
on both Apple silicon and Intel Macs. Each architecture will have its own
signed, notarized, stapled application archive:

- `aarch64-apple-darwin`, built and verified on GitHub's `macos-15` runner; and
- `x86_64-apple-darwin`, built and verified on GitHub's `macos-15-intel`
  runner.

The public install artifacts will be ZIP archives containing `AntennaBench.app`,
not DMG images or universal binaries. The exact application archive names are:

- `AntennaBench-{version}-aarch64-apple-darwin.zip`; and
- `AntennaBench-{version}-x86_64-apple-darwin.zip`.

This release contract separates repeatable non-secret artifact construction
from credentialed signing and publication. Pull requests and routine CI never
receive release credentials or mutate releases.

Windows and Linux CI remain portability evidence rather than release-support
promises. Windows, Linux, macOS universal binaries, DMG and PKG installers, the
Mac App Store, package managers, automatic updates, public prereleases, and an
SBOM are deliberately deferred.

## Supported Platform Contract

The two target triples and macOS 15.0 minimum are product claims, not whatever
architecture or operating system a floating runner happens to provide.
Release jobs therefore use the explicit `macos-15` and `macos-15-intel` labels
and fail unless the runner architecture matches the selected target.

The Tauri configuration will set `bundle.macOS.minimumSystemVersion` to `15.0`.
Each target must run the portable test suite, unattended desktop workflow, and
native release application build on its native runner. Release verification
also checks the final application's Mach-O architecture, deployment target,
bundle identifier, embedded version, signature, notarization ticket, and
Gatekeeper assessment.

Apple-silicon-only release support was rejected because this public repository
can use standard native Intel and arm64 GitHub runners without self-hosted or
paid CI. A universal application was rejected because separate artifacts make
the architecture explicit and independently testable while avoiding binary
merging, larger downloads, and more complicated signing verification.

Broader desktop releases remain possible after the project proves native
packaging, signing, installation, and release verification on those platforms.
A green Windows or Linux compile check alone is not sufficient evidence for a
public support commitment.

## Distribution Assets

Every release exposes exactly these project-produced assets:

- the two architecture-specific application ZIP archives;
- `AntennaBench-{version}-release-manifest.json`; and
- `AntennaBench-{version}-SHA256SUMS`.

The release manifest is a versioned machine-readable document containing at
least:

- manifest schema version;
- AntennaBench version and Git tag;
- source commit SHA;
- bundle identifier;
- minimum macOS version;
- exact Rust target triple and executable architecture for each application;
- public filename, byte size, and SHA-256 digest; and
- verified signing, hardened-runtime, secure-timestamp, notarization,
  stapling, and Gatekeeper state.

The checksum file uses the conventional SHA-256 format, lists the two ZIP
archives and release manifest, and is sorted bytewise by filename. It does not
list itself.

GitHub's automatically generated source archives are sufficient. AntennaBench
will not publish a duplicate project-generated source archive in this slice.
Release notes identify the Apple-silicon and Intel downloads in user-facing
language and document archive extraction, moving the app to Applications,
verification, supported macOS versions, and known limitations.

## ZIP Rather Than DMG

The release build creates the Tauri macOS application bundle without invoking
the DMG bundler. Apple accepts a ZIP archive containing a Developer ID-signed
application for notarization. Because a ZIP cannot itself be stapled, the
workflow:

1. signs the application and all nested code;
2. creates a temporary ZIP for notarization;
3. submits it with `notarytool` and checks the complete notary log;
4. staples and validates the ticket on `AntennaBench.app`;
5. creates the final public ZIP from the stapled application; and
6. extracts that final ZIP and verifies the downloaded representation.

This command-line path replaces the observed Tauri DMG Finder/AppleScript hang
with bounded and inspectable steps. A later decision may add a DMG after its
unattended construction and cleanup behavior are proven. The initial ZIP must
preserve application permissions, links, extended attributes, and bundle
structure using a macOS-aware archive tool.

## Version And Tag Authority

The root `Cargo.toml` `[workspace.package].version` is the single editable
application-version authority. The desktop crate inherits it. The duplicate
literal version in `tauri.conf.json` will be removed; when that value is absent,
Tauri uses the desktop Cargo package version.

The first release contract accepts stable Semantic Versioning values only:

- application version: `MAJOR.MINOR.PATCH`; and
- release tag: `vMAJOR.MINOR.PATCH`.

The workflow consumes an existing tag; it does not create or move one. The tag
must exactly match the Cargo version and point to a commit reachable from
`main`. Cargo metadata, the tag, Tauri metadata, `CFBundleShortVersionString`,
the release manifest, and every public filename must agree before the workflow
imports credentials or creates a draft release.

Release builds use the exact routine compiler pinned by `rust-toolchain.toml`
and Mise, rather than a moving `stable` channel. The workspace supports that
single compiler for development, CI, and release builds; it does not maintain a
separate older compatibility floor. Compiler-pin updates are reviewed and
verified like dependency updates, and a release manifest records the exact
compiler version used for each artifact.

Public prerelease versions are not supported initially. The first candidate is
validated as a private draft release for its eventual stable tag. This avoids
claiming a prerelease syntax before its Cargo, Tauri, macOS bundle-version, and
update-order semantics are defined. A future decision may add an explicit
prerelease contract.

Tags are never force-moved or reused. If an unpublished candidate is abandoned,
the next attempt uses a new application version and tag.

## Signing And Notarization

Every public application uses:

- an Apple Developer ID Application certificate;
- hardened runtime;
- a secure signing timestamp;
- Apple notarization through `notarytool`;
- a stapled notarization ticket on the application;
- strict recursive code-signature verification;
- `stapler validate`; and
- Gatekeeper assessment of the extracted final application.

Ad-hoc signing is permitted only for clearly marked local or non-secret CI
artifacts. It is never a publishable fallback. Missing, expired, revoked, or
invalid credentials stop the credentialed workflow without producing a public
asset.

Direct trusted distribution requires an owner-managed Apple Developer Program
membership. Enrollment, its current annual cost, Developer ID certificate
creation, and App Store Connect key creation remain explicit repository-owner
prerequisites. Agents and workflows must not enroll, purchase, export, or
weaken credentials to satisfy a build.

## Credential Boundary

Apple release credentials live only in a protected GitHub environment named
`desktop-release`. The environment:

- accepts only `v*` release tags;
- requires an explicit reviewer approval before credentialed jobs start;
- does not expose secrets to pull requests, forks, routine CI, or non-secret
  build jobs; and
- disallows administrative bypass when the repository plan and ownership model
  make that practical.

For a single-maintainer repository, the required review is a deliberate
promotion checkpoint rather than independent separation of duties. Self-review
may remain enabled until a second trusted maintainer exists; that limitation is
documented rather than implying two-person control.

The environment stores only the secrets needed for:

- a base64-encoded Developer ID Application `.p12` and its export password;
  and
- an App Store Connect team API issuer, key ID, and private key used by
  `notarytool`.

A revocable team API key with the Developer role is preferred over an Apple ID
and app-specific password. The workflow derives the signing identity after
certificate import, creates a random ephemeral keychain password for each run,
and deletes the temporary keychain and key material during cleanup.

Credential names, owners, creation date, rotation expectations, and revocation
procedure are documented without recording secret values. Suspected exposure
requires immediate Apple credential revocation, removal from GitHub, and an
audit of releases signed or notarized during the exposure window.

## Build And Publication State Machine

The release path is:

1. A maintainer pushes a `vMAJOR.MINOR.PATCH` tag for a reviewed commit on
   `main`.
2. Read-only, non-secret jobs build and test each target on its native runner,
   validate the version and target contract, and upload explicitly named
   internal workflow artifacts.
3. Jobs protected by the `desktop-release` environment import the Apple
   credentials only after approval, sign and notarize the exact validated
   application inputs, staple them, create the final ZIPs, and verify their
   extracted contents on the matching native runners.
4. A final least-privilege job verifies the complete asset set, creates the
   release manifest and sorted checksums, generates build-provenance
   attestations for the final bytes, and creates a draft GitHub Release.
5. The owner downloads every draft asset and verifies checksums, attestations,
   architecture, installation, first launch, and the canonical
   open/report/export/reopen workflow.
6. The owner explicitly publishes the draft as a stable release.

Release workflow permissions are job-scoped. Routine and build jobs retain
`contents: read`. OIDC and `attestations: write` are available only to the
attestation job. `contents: write` is available only to the job that creates or
updates the draft release. Apple secrets are available only to the signing and
notarization jobs.

Repository-owned Mise tasks and verification scripts remain the command source
of truth; workflow YAML orchestrates them rather than duplicating release
logic. External actions used in the credentialed path are pinned to reviewed
immutable commit SHAs where practical.

## Dependency And Workflow Preflight

[Decision 0012](0012-use-combined-supply-chain-maintenance-gates.md) adds the
maintenance boundary used by this release contract. Every external Action in
the release workflow is pinned to a reviewed full commit SHA. The tagged
revision uses exact Rust, Node, Tauri CLI, and supply-chain tool pins plus its
committed Cargo lockfile.

Before the protected desktop-release environment or Apple credentials become
reachable, a read-only job refreshes the RustSec advisory database and verifies
the action pins, dependency sources and licenses, exception expiry, CodeQL/main
status, and exact tool inputs. A failure cannot be downgraded by a credentialed
job. Dependabot and pull-request workflows never receive release secrets or
release mutation permission.

Implementation is split across #58 and #59; repository settings and required
checks require the owner action in #60. This preflight strengthens the selected
artifact and credential boundary without changing its platform, version, asset,
signing, notarization, attestation, or promotion contract.

## Integrity And Provenance

The workflow generates GitHub build-provenance attestations for the two final
application archives, release manifest, and checksum file. User and operator
documentation includes both:

- `shasum -a 256 -c AntennaBench-{version}-SHA256SUMS`; and
- `gh attestation verify <asset> -R rwjblue/antennabench`.

GitHub immutable releases will be enabled before the first public publication.
The workflow follows GitHub's draft-first sequence: create the draft, attach and
verify the complete asset set, then publish. Publication locks the associated
tag and assets and creates a release attestation in addition to the per-artifact
build provenance.

A formal SBOM is deferred. A Rust-dependency-only document would not completely
describe the Tauri application and its platform WebKit/runtime boundary. A
future SBOM must first define component coverage, format, verification, and how
the document relates to the exact packaged artifact. `Cargo.lock`, the release
manifest, checksums, and build provenance remain available in the initial
slice.

## Retry, Failure, Withdrawal, And Rollback

No workflow step silently replaces a release asset.

- Before draft creation, a failed job may be rerun for the same immutable tag
  and source commit.
- If draft creation fails partway through, automation removes the incomplete
  draft and its assets when safe, or reports the exact state requiring owner
  cleanup.
- An existing complete draft may be resumed only after its tag, commit,
  manifest, and asset digests are proven identical.
- Any mismatch fails closed instead of deleting or overwriting unexplained
  state.
- A published release is never modified in place.

A defective public release is marked withdrawn in its notes and replaced by a
new patch version. If continued distribution is unsafe, the owner may delete
the immutable release; its tag name remains permanently retired and is never
reused. Revoked Apple credentials or notarization tickets are handled as a
security incident and documented in the replacement release.

## Deferred Scope

The following are intentionally outside the first release contract:

- Windows and Linux release artifacts;
- macOS universal applications;
- support below macOS 15;
- DMG, PKG, App Store, Homebrew, and other package-manager distribution;
- public prerelease tags or channels;
- automatic update manifests and signing keys;
- a formal SBOM;
- paid or self-hosted build runners; and
- stable release publication without an explicit owner promotion.

Adding any of these changes the public support, trust, or lifecycle contract
and requires focused approval rather than incidental implementation expansion.

## Alternatives Considered

### macOS Apple Silicon Only

This is the smallest release matrix and matches the primary development host.
It was rejected because native Intel verification is available on a standard
free runner and separate assets keep the added support honest and isolated.

### macOS Universal Application

A universal application gives users one download. It was rejected because it
requires building and merging two targets, increases artifact size, complicates
architecture and signing verification, and provides less failure isolation
than two native artifacts.

### DMG Installer

A DMG offers a familiar drag-to-Applications presentation. It was rejected for
the first slice because the current Tauri DMG path hung in its interactive
Finder/AppleScript layout step. A notarized and stapled application ZIP is an
Apple-supported, bounded command-line replacement.

### Immediate Windows And Linux Releases

Multi-platform CI can expose portability problems, but it does not prove
installer construction, platform signing, clean installation, or user support.
These releases were deferred until their focused platform contracts are ready.

### Ad-Hoc Public macOS Release

Ad-hoc signing avoids Apple credentials but still requires users to bypass
macOS trust protections. It was rejected because the release track promises a
trustworthy normal installation path rather than instructions to weaken
Gatekeeper expectations.

### Workflow-Created Tags And Mutable Releases

Allowing publication automation to invent tags or replace existing assets
reduces reviewability and makes reruns dangerous. It was rejected in favor of
an existing reviewed tag, draft-first validation, and immutable publication.

## Consequences

- Issues #35 and #36 have an exact two-target artifact, version, trust, and
  promotion contract to implement.
- The first supported release is broader than the primary development host but
  remains limited to a matrix that native public runners can verify.
- Replacing DMG with ZIP removes the observed unattended packaging blocker at
  the cost of a less polished installation presentation.
- Public release remains blocked until the owner provides an active Apple
  Developer Program membership and approved credentials.
- Non-secret artifact construction and verification can proceed before those
  credentials exist.
- Public assets are attributable to an exact tag, commit, target, digest,
  signature, notarization result, and GitHub build.
- Published assets cannot be silently replaced; fixes require a new version.
- Windows and Linux CI do not create accidental support promises.
- Auto-updates and prerelease channels remain compatible future work rather
  than implicit behavior in the first release.

## References

- [Decision issue #34](https://github.com/rwjblue/antennabench/issues/34)
- [Release tracking issue #33](https://github.com/rwjblue/antennabench/issues/33)
- [Artifact construction issue #35](https://github.com/rwjblue/antennabench/issues/35)
- [Credentialed publication issue #36](https://github.com/rwjblue/antennabench/issues/36)
- [Supply-chain decision #44](https://github.com/rwjblue/antennabench/issues/44)
- [Decision 0012](0012-use-combined-supply-chain-maintenance-gates.md)
- [Tauri distribution and versioning](https://v2.tauri.app/distribute/)
- [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/)
- [Apple notarization workflow](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)
- [Apple Developer Program enrollment](https://developer.apple.com/programs/enroll/)
- [GitHub-hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub deployments and environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments)
- [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
- [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
