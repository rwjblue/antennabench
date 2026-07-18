# Decision 0023: Use Static Astro For The Project Site

- Status: Accepted
- Date: 2026-07-17
- Issues: [#139](https://github.com/rwjblue/antennabench/issues/139), [#70](https://github.com/rwjblue/antennabench/issues/70), [#73](https://github.com/rwjblue/antennabench/issues/73)

## Context

The repository needs a public home before the optional hosted-sharing service is
ready. The existing hosted package already owns Cloudflare Workers Static
Assets, the future `/api/*` Worker seam, exact-pinned JavaScript tooling, and the
security and operations boundary. Its placeholder page was not a maintainable
public site, while requiring the unfinished R2, D1, Queue, Durable Object, and
Container resources would make an informational launch depend on later product
work.

The desktop remains a local-first application. A public site must explain the
current product without becoming a runtime dependency, claiming unavailable
downloads or publishing, or creating a second frontend package and lockfile.

## Decision

Astro statically generates the project site inside `apps/hosted`. The existing
root npm workspace and lockfile remain the only JavaScript dependency graph.
`apps/hosted/public` supplies passthrough assets, including the report generated
from the canonical fixture by the trusted Rust renderer. The deterministic
output is `apps/hosted/dist/site`.

The marketing site has no server rendering, Cloudflare Astro adapter, React,
hydrated components, telemetry, remote fonts, or third-party runtime resources.
Workers Static Assets serves ordinary pages without Worker execution. Explicit
headers allow only same-origin report framing and resources; scripts and network
connections are disabled.

Two Wrangler configurations have different responsibilities:

- `wrangler.site.jsonc` deploys only `dist/site`. It declares no Worker entry
  point or hosted-sharing resource.
- `wrangler.jsonc` retains the admission-disabled future API and processing
  foundation. Its static asset binding now points at the same Astro output and
  continues to run the Worker first only for `/api/*`.

This is an additive topology. Astro owns public information pages. `/app` may
later host #73's authenticated React client without replacing those pages, and
the existing Worker may handle same-origin `/api/*`. Immutable published reports
remain isolated on a separate report origin rather than becoming site assets.

Production deployment uses the exact-pinned Wrangler dependency from the hosted
workspace. GitHub Actions accepts only a commit already reachable from `main`,
runs credential-free validation before deployment, and releases through the
protected `production` environment. Cloudflare account credentials exist only
as environment secrets. The apex domain is canonical; a Cloudflare redirect
rule sends `www` to the apex while preserving path and query.

## Evidence For Changing The Placeholder

The repository now has the root workspace from Decision 0022, the canonical
sample and deterministic standalone renderer from #3 and #4, and the updated
answer-first/compact report contract from #133. Those boundaries make the
finished site a small static consumer of maintained project facts instead of an
independent product implementation.

The generated sample is byte-compared with a fresh Rust render. Site validation
checks output pages, internal links, canonical and social metadata, headers,
site-only configuration, the future `/app` and `/api/*` seams, and absence of
client JavaScript or React.

## Consequences

- A project-site deployment requires no upload, identity, processing, or report
  publication infrastructure.
- Local and CI builds compile Astro and the canonical Rust sample through the
  root workspace and Mise task graph.
- A report-renderer or fixture change that affects the public sample must
  regenerate and review the committed sample.
- Owner-controlled Cloudflare credentials, custom-domain attachment, redirect
  configuration, and final visual review remain human-required.
- User-report serving and authenticated application behavior remain owned by
  #70–#74; this decision does not make those surfaces available.

## Rejected Alternatives

- A separate Pages project or frontend package would duplicate the dependency
  and deployment boundary.
- Hand-maintained HTML would make navigation, metadata, and later public pages
  needlessly repetitive.
- Astro SSR or a Cloudflare adapter would add Worker execution to static page
  views without a product requirement.
- React for marketing content would spend client JavaScript and blur #73's
  explicit application boundary.
- Deploying the full hosted foundation would provision unfinished resources and
  could expose the placeholder health API.
