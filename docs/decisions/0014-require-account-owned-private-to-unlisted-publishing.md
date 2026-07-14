# 0014: Require Account-Owned Private-To-Unlisted Publishing

Date: 2026-07-14

## Decision

AntennaBench will require an authenticated account before any user bundle is
uploaded to the hosted service. Each hosted report has exactly one account
owner and begins private. Publishing is a separate explicit action that exposes
only trusted derived report HTML through an unlisted random URL. The first
product has no anonymous upload, listed report, public directory, search,
callsign claim, ownership transfer, collaborator, or public raw-bundle feature.

The installed application and the website are independent first-class clients
of one hosted identity and publishing service. A user can enroll, sign in,
upload, preview, publish, unpublish, delete reports, manage sessions, and delete
the account entirely through either client. The desktop path must not require
launching a browser, and the web path must not require installing the desktop
application. Enrolling through either client with the same verified email
resumes the same account and ownership state.

This account boundary applies only to optional hosted operations. Opening,
capturing, validating, analyzing, rendering, and exporting local session
bundles remain account-free and offline-capable. Hosted state and identity are
derived convenience state and never become session evidence.

The first implementation will self-host Better Auth in the existing Worker and
D1 service boundary. Cloudflare Email Service sends transactional verification
codes. Passwords and social identity providers are not enabled initially.

- Both clients support short-lived email one-time codes for enrollment, sign-in,
  recovery, and recent reauthentication.
- The web client additionally supports passkeys on the service's stable relying
  party domain and uses secure same-origin cookie sessions.
- The desktop client receives a separately revocable bearer device session
  after in-app authentication. Rust owns the credential and all authenticated
  hosted requests; the webview receives only a safe account projection.
- Desktop credentials use a platform credential-store abstraction backed by
  macOS Keychain, Windows Credential Manager or DPAPI-protected storage, and a
  Secret Service-compatible Linux keyring. If secure persistent storage is
  unavailable, the session remains memory-only and is lost at application exit.
  The app never silently falls back to a plaintext file, bundle data, or browser
  storage.
- Native or WKWebView passkey support may strengthen desktop authentication
  after the platform integration is proven, but it does not block the first
  complete in-app email-code flow.

Better Auth is an implementation dependency, not the durable account identity.
Application-owned random account and report identifiers remain the references
stored in hosted control data. The implementation pins a reviewed stable
Better Auth release, audits every installed package, monitors advisories, and
keeps an explicit migration/export path rather than making provider-specific
identifiers public API.

## Two Complete Client Contracts

The web and desktop clients share server behavior rather than duplicating
identity or evidence semantics.

### Desktop Client

The desktop UI collects an email address and one-time code inside AntennaBench.
It shows the signed-in email, account status, known device sessions, owned
hosted reports, processing status, visibility, and lifecycle actions. A narrow
Rust service performs authentication, deterministic bundle packaging, upload,
status polling, retry, publication, unpublication, and deletion.

The JavaScript webview receives no general network, credential, filesystem, or
bundle-model authority. A hosted session token never enters JavaScript local
storage, the session bundle, logs, exported HTML, or UI diagnostics. Local work
does not fail or become blocked when the account session is absent, expired, or
offline; only the requested hosted action reports that authentication or the
network is unavailable.

### Web Client

The website provides the same account and report lifecycle without requiring
the desktop application. It accepts one bounded ZIP transport using the exact
`hosted-standard-v1` contract selected by ADR 0013, displays processing
diagnostics and the trusted rendered preview, and offers the same publish,
unpublish, delete, session, and account actions.

The web client never parses a bundle into a second semantic model and never
renders uploaded content itself. It submits bytes to the common admission and
canonical Rust processing pipeline. Browser and desktop uploads therefore
produce the same artifacts, ownership, diagnostics, limits, and lifecycle.

## Minimum Identity And Authentication Data

The hosted identity record contains only what the selected service needs:

- an application-owned random account ID;
- one unique verified email address and verification time;
- account status and creation/deletion times;
- passkey public credential metadata when the user enrolls passkeys;
- independently revocable web and desktop session records;
- acceptance times for the current privacy and acceptable-use notices; and
- redacted security and moderation references.

A display name, avatar, legal name, postal address, phone number, station grid,
callsign, license record, and social-provider identity are not required account
attributes. Authentication responses do not reveal whether an arbitrary email
already has an account. One-time codes expire after five minutes, have a small
fixed attempt budget, are stored hashed, rotate on resend, and are protected by
rate limits and Turnstile where interactive abuse warrants it.

Email is the recovery root. A verified email-code recovery invalidates existing
sessions and allows the user to establish new sessions and passkeys. Email
change requires recent authentication plus verification of both the current and
new addresses. Losing access to the current email is not grounds for a manual
callsign-based ownership transfer. Account deletion, email change, passkey
removal, and full session revocation require recent authentication.

Web cookies are Secure, HttpOnly, SameSite=Lax or stricter, restricted to the
minimum host/path, opaque, and bounded by idle and absolute lifetimes. Desktop
bearer sessions are random, revocable, scoped to the AntennaBench hosted API,
and never treated as a client secret embedded in the executable. Users can list
and revoke individual sessions from either client, and signing out removes the
local credential as well as revoking its server session.

## Identity-Bearing Evidence Inventory

The source bundle contains identity and location data beyond the account email.
The hosted service must not describe a report as anonymous merely because the
public URL is unlisted.

The current trusted standalone report can disclose:

- the station callsign and grid;
- exact scheduled ranges and observation timestamps;
- power, bands, antenna labels, heights, radial geometry, orientation, tuner,
  feedline, and antenna notes;
- remote reporter or heard callsigns and grids; and
- distance, azimuth, frequency, mode, SNR, and comparison measurements.

The private source archive can additionally contain operator notes, event
notes, exact adapter records, raw or near-raw WSJT-X input, rig and propagation
metadata, imported text, analysis notes, and arbitrary attachments. Those
families may expose personal details, station operating patterns, precise
location clues, third-party identifiers, or unrelated attachment content.

Before publication, both clients show the exact trusted rendered report and a
concise disclosure of these identity-bearing field families. Publication
requires an explicit confirmation after that preview. The first implementation
does not claim to redact sensitive content automatically. A user who does not
want the rendered fields disclosed must keep the report private or avoid
uploading it.

## Ownership And Callsign Semantics

The authenticated account that creates an upload ticket owns the resulting
hosted report. The server derives ownership from the authenticated session and
never accepts an owner ID, email, or callsign supplied by the client. Report IDs
and unlisted URLs are not owner capabilities.

One report has one owner initially. Co-owners, teams, account merging,
delegation, and ownership transfer are unsupported. A user who possesses a
local source bundle may upload a new independent hosted revision from another
account, but that does not transfer an existing report or URL.

Callsigns remain self-authored evidence attributes. The service does not create
an account-level callsign registry, require a callsign, reserve a callsign,
prevent another account from reporting the same callsign, or label a callsign
as verified. UI language identifies it as a callsign reported by the uploaded
evidence. A callsign never grants sign-in, recovery, ownership, deletion,
moderator authority, or priority in a dispute.

This deliberately leaves room for later regulatory or third-party verification
without representing self-assertion as proof today.

## Visibility And Stable-Link Lifecycle

The initial visibility vocabulary contains `private` and `unlisted`. Processing,
moderation, and deletion states remain separate lifecycle dimensions.

### Private

Every upload and accepted derived report starts private. Only its owner and an
explicit audited moderator break-glass path may access private hosted
artifacts. A private preview is dynamically authorized through the Worker and
is never copied into the public R2 bucket or served by the public report domain.

### Unlisted

An owner may explicitly publish the reviewed trusted HTML as unlisted. Anyone
who knows the random URL can view and copy it without authentication. Unlisted
means absent from AntennaBench listings, search, indexes, discovery feeds,
sitemaps, aggregate pages, and callsign directories; it does not mean private
or access-controlled.

Publication creates a new immutable public object and stable random URL. The
URL contains no callsign, grid, account, email, bundle digest, or session ID.
It is not reused for another report or revision.

### Unpublish, Republish, And Delete

Unpublishing prevents new application access, removes the public object, and
purges its exact cache URL before reporting success. The retired URL is never
reused. The owner may retain the private source and preview, but republishing
creates a new public object and URL. This preserves ADR 0013's immutable object
contract and avoids reviving cached or previously shared link semantics.

Deleting a report immediately prevents new owner and public access, then
reconciles removal of quarantine, accepted original, derived private artifacts,
public HTML, cache entries, and normal D1 control metadata. Completion is not
reported while a known object or cache purge remains outstanding. A permanent
random-ID tombstone prevents reuse but contains no email, callsign, grid,
content, or object bytes.

There is no `listed` state initially. Adding search, callsign directories,
public profiles, mutable collection pages, aggregate browsing, or leaderboards
requires a separate product and moderation decision.

## Raw Archive And Private Artifact Policy

Accepted original archives remain private under ADR 0013 for reproducibility,
retry, and audit. The first hosted product provides no raw archive download
endpoint, including to the owner. The local-first workflow already leaves the
operator with the source bundle and lossless export; omitting hosted download
avoids turning sensitive notes, raw adapter input, and arbitrary attachments
into another remote disclosure surface.

Originals are available only to the processor, deletion reconciler, and an
explicit audited moderator break-glass path. Routine administration and abuse
review use derived reports and metadata. Every break-glass read records actor,
reason, report ID, time, and outcome without copying raw content into logs.

A later owner-authorized, short-lived raw download may be considered separately
after its reauthentication, disclosure, audit, and cost behavior is defined.
Original archives are never placed in the public bucket or addressed by the
unlisted report URL.

## Account Deletion And Retention

Account deletion first revokes every session and prevents new uploads and
lifecycle actions. It then moves every owned report through the same complete
deletion workflow. The account is not reported deleted until report deletion
has either completed or remains truthfully represented as a retrying deletion
job that no longer permits access.

Ordinary authentication, upload, and lifecycle audit metadata expires after 90
days. It contains random identifiers, action, actor class, timestamp, reason or
result code, and relevant counters, but no bundle content or complete rendered
text. Security or moderation events follow the same default and may be held
longer only for a documented active investigation or legal obligation. When
the hold ends, normal deletion resumes.

After deletion, the service retains only non-PII identifier tombstones required
to prevent URL and idempotency reuse. It does not retain the account email,
passkeys, sessions, callsigns, grids, original archive, report HTML, or report
metadata for analytics. Aggregate operational counters must not permit account
or report reconstruction.

## Abuse, Takedown, And Moderator Authority

The first hosted product minimizes moderation surface by omitting discovery,
profiles, comments, messaging, reactions, and user-authored executable content.
It still provides a discoverable abuse form from the maintained site. The form
accepts a report URL, a bounded category and explanation, and an optional reply
email. Turnstile, per-source rate limits, duplicate suppression, and bounded
retention protect the form.

An administrator may:

- hide or unpublish a report immediately;
- quarantine or delete its hosted artifacts;
- suspend an account and revoke its sessions;
- deny new uploads from a suspended identity;
- inspect private or raw artifacts only through the audited break-glass path;
  and
- restore access or reject an abuse report after review.

An administrator may not edit report content, silently republish a report,
transfer ownership, verify or award a callsign, or change source evidence.
Every action records actor, target, reason code, time, prior state, resulting
state, and any break-glass use. Appeals use a published support email initially;
the service does not require a separate appeals application.

Deletion requests from unauthenticated people are treated as abuse or privacy
reports, not proof of ownership. Knowledge of a callsign, report ID, station
details, or unlisted URL is insufficient to gain account control.

## Cost And Operational Boundary

Self-hosting authentication in the existing Worker and D1 boundary adds no new
fixed service floor. The Workers Paid plan selected by ADR 0013 includes the
first 3,000 arbitrary-recipient Email Service messages each month; additional
messages are usage-priced. Persistent independently revocable sessions keep
routine email volume to enrollment, recovery, and occasional reauthentication.

Public report views retain ADR 0013's one-object cached path and normally invoke
no authentication, Worker, D1, Queue, or processor code. Private previews,
account pages, uploads, and lifecycle operations are dynamic and usage-based.
No identity or moderation process remains continuously running while idle.

Operational requirements include:

- authentication and email quotas separate from upload quotas;
- generic responses and timing behavior for unknown versus known emails;
- rate limits on code send, code verify, passkey, session, and recovery paths;
- bounce, suppression, delivery, failed-authentication, session-revocation, and
  abuse metrics without raw content in logs;
- dependency advisory monitoring and prompt reviewed security updates; and
- an emergency control that can stop new enrollment or uploads without
  affecting local operation or existing cached report views.

## Alternatives Considered

### Anonymous Unlisted Uploads

Anonymous upload minimizes initial friction but leaves deletion, recovery,
quotas, bans, and durable ownership dependent on bearer capabilities that can
be lost or stolen. Adding an account later also creates ambiguous report
transfer semantics. It was rejected because the optional share feature can
afford a small enrollment step and needs accountable lifecycle control from its
first user upload.

### Defer User Publishing

Serving only the maintained sample would avoid identity and moderation work.
It was rejected because the selected boundaries now make user publishing
tractable and indefinitely deferring it would leave the central hosted-sharing
value and issues #69 through #71 blocked.

### Managed Identity Provider

WorkOS AuthKit and Clerk reduce authentication implementation burden. WorkOS
currently places its durable custom-domain passkey shape behind a substantial
monthly custom-domain charge, while Clerk currently requires a paid plan for
passkeys. Both also make a third party the primary holder of account email and
session state. They were rejected for the first product because their fixed
cost or domain coupling conflicts with the low-cost self-hosted boundary.

### Cloudflare Access

Cloudflare Access is effective for protecting internal or organization-managed
applications through configured identity providers. It was rejected as the
consumer account system because it does not supply AntennaBench's user-owned
report, recovery, deletion, and public enrollment product model.

### Desktop Browser Authorization

OAuth authorization-code flow with PKCE and an external browser is appropriate
for third-party or federated identity. Making it the only desktop flow was
rejected because ordinary enrollment and hosted operations must be completable
inside AntennaBench. Embedding third-party OAuth pages in the Tauri webview was
also rejected; native OAuth best practice requires an external user agent.
Email-code authentication provides the complete first-party in-app path without
introducing passwords or social providers.

### Public Or Listed By Default

Default public discovery would maximize visibility but silently publish
identity-bearing station and third-party data and create a much larger
moderation product. It was rejected. Private is the safe default, and unlisted
publication always requires explicit review and confirmation.

### Account-Level Callsign Claims

Self-claimed unique callsigns would look authoritative without proving license
or control and would create squatting, recovery, and dispute obligations. They
were rejected. Callsigns remain non-exclusive evidence content.

### Owner Raw Downloads

An owner-authenticated download endpoint is feasible but adds a sensitive data
exfiltration surface that the local-first product does not need initially. It
was deferred rather than bundled into the minimum publishing product.

## Consequences

- User uploads have durable ownership, recovery, quotas, deletion, and
  moderation from the start.
- The website and installed application each provide a complete hosted workflow
  against one identity and publishing system.
- Desktop authentication works without a browser and preserves narrow
  Rust-owned network and credential authority across macOS, Windows, and Linux.
- The web client provides convenient passkey authentication while email codes
  remain the universal enrollment and recovery mechanism.
- Private-by-default and explicit unlisted publication reduce accidental
  disclosure but add a preview and confirmation step.
- An unlisted URL is public to its holder and cannot prevent redistribution;
  deletion cannot retract copies already downloaded.
- Callsigns carry no account or authorization meaning, avoiding false claims of
  verification while leaving future verification possible.
- Raw archives remain auditable without becoming a hosted download product.
- Authentication and moderation add security maintenance and dynamic request
  paths, but no additional fixed service cost is expected at initial scale.
- Better Auth security advisories and platform credential integrations become
  explicit maintained dependencies rather than hidden implementation details.
- Public discovery, ownership transfer, collaboration, raw download, desktop
  passkeys, and stronger callsign verification remain focused future choices.

## References

- [Hosted identity decision #12](https://github.com/rwjblue/antennabench/issues/12)
- [Hosted sharing tracker #10](https://github.com/rwjblue/antennabench/issues/10)
- [Decision 0001](0001-bundle-is-source-of-truth.md)
- [Decision 0013](0013-use-an-optional-static-hosted-sharing-adapter.md)
- [Better Auth Cloudflare D1 support](https://better-auth.com/blog/1-5)
- [Better Auth email OTP](https://better-auth.com/docs/plugins/email-otp)
- [Better Auth bearer authentication](https://better-auth.com/docs/plugins/bearer)
- [Better Auth passkeys](https://better-auth.com/docs/plugins/passkey)
- [Better Auth June 2026 security update](https://better-auth.com/blog/security-update-june-2026)
- [Cloudflare Email Service pricing](https://developers.cloudflare.com/email-service/platform/pricing/)
- [Apple passkeys in WKWebView](https://developer.apple.com/documentation/authenticationservices/supporting-passkeys)
- [NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html)
- [OAuth 2.0 for Native Apps](https://www.rfc-editor.org/rfc/rfc8252.html)
- [WorkOS pricing](https://workos.com/pricing)
- [Clerk pricing](https://clerk.com/pricing)
