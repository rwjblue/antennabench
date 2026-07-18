# Hosted Site And Foundation Operations

The public project site and the future hosted-sharing foundation occupy the same
`apps/hosted` package but have deliberately different deployment boundaries.

- `wrangler.site.jsonc` serves only deterministic Astro output from
  `dist/site`. It has no Worker entry point, API, backend binding, admission
  state, or hosted-sharing resource.
- `wrangler.jsonc` retains the future admission-disabled `/api/*` Worker and its
  R2, D1, Queue, Durable Object, and Container declarations. Ordinary project-
  site deployment does not use this configuration.

The desktop's local session, analysis, report, and export workflows never depend
on either deployment.

## Public Project Site

### Build And Validate Without Credentials

From the repository root:

```bash
npm ci
npm run site:sample:check --workspace @antennabench/hosted
npm run site:social:check --workspace @antennabench/hosted
npm run site:build --workspace @antennabench/hosted
npm run site:check --workspace @antennabench/hosted
npm run site:dry-build --workspace @antennabench/hosted
```

`mise run hosted:test` runs those checks together with the preserved hosted
Worker tests, type drift check, strict TypeScript compilation, and future-
foundation dry builds. The sample check reruns the canonical fixture through the
trusted Rust report renderer and byte-compares the result with
`public/sample-report/index.html`. The social-card check validates the committed
PNG signature and dimensions together with its editable SVG design source.
Astro adds no hydration or client JavaScript.

The static validation checks expected pages and assets, internal links, canonical
and social metadata, security headers, the site-only Wrangler boundary, exact
Astro ownership, and the absence of React and external runtime resources.

### One-Time Owner Setup

Complete these steps before merging the first production-deploying change:

1. In Cloudflare, create a custom API token from the **Edit Cloudflare Workers**
   template. Restrict its account resource to the one account that owns
   `antennabench.com`; do not grant R2, D1, Queues, Containers, DNS, or unrelated
   account access for this static deployment.
2. In GitHub, create or update the repository's `production` environment. Enable
   required-reviewer protection for deployments and add environment secrets
   `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN`. Do not add either value as
   a repository variable, workflow input, local file, or pull-request secret.
3. After the first reviewed deployment creates the `antennabench-site` Worker,
   attach `antennabench.com` as its Workers custom domain. Confirm Cloudflare
   provisions the proxied DNS record and an active TLS certificate.
4. Ensure `www.antennabench.com` has a proxied DNS record, then add a Cloudflare
   Single Redirect with source wildcard
   `https://www.antennabench.com/*`, target
   `https://antennabench.com/${1}`, status **301**, and **Preserve query string**
   enabled. Do not reverse the canonical direction.

Cloudflare's maintained references are
[Workers GitHub Actions](https://developers.cloudflare.com/workers/ci-cd/external-cicd/github-actions/),
[Workers custom domains](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/),
and the [www-to-apex redirect example](https://developers.cloudflare.com/rules/url-forwarding/examples/redirect-www-to-root/).

### Production Deployment And Rollback

`.github/workflows/hosted-site-deploy.yml` deploys pushes to `main` through the
protected `production` environment. Pull requests never receive Cloudflare
credentials. The job reruns the locked hosted validation before invoking the
exact-pinned workspace Wrangler.

To redeploy or roll back, open **Deploy public project site** in GitHub Actions,
choose **Run workflow**, and enter the full commit SHA to deploy. The job rejects
any revision that is not already reachable from `origin/main`; a rollback is a
redeployment of a reviewed historical source revision, not an unreviewed bundle
or dashboard edit. Record the selected SHA and deployment result in the release
or incident notes.

Before the first public announcement—and after a domain, header, or deployment
change—verify:

```bash
curl --fail --silent --show-error --head https://antennabench.com/
curl --fail --silent --show-error --head https://antennabench.com/sample-report/
curl --silent --show-error --head 'https://www.antennabench.com/check/path?source=verify'
curl --silent --show-error --head https://antennabench.com/api/health
```

Confirm the apex responses have valid TLS, CSP, `nosniff`, referrer, framing,
permissions, and expected cache headers. The `www` request must return 301 with
location `https://antennabench.com/check/path?source=verify`. The site-only
`/api/health` request must return the static not-found response; it must not
expose the future hosted foundation's health inventory. Inspect the home and
how-it-works pages at narrow and desktop widths, navigate them by keyboard, open
the canonical report, and review the social card before approving production.

The production URL, domain/TLS checks, redirect result, header result, rollback
rehearsal, and final visual/copy approval are human-owned completion evidence for
#139. Never replace them with a test domain or fabricated output.

## Future Hosted-Sharing Foundation

The optional hosted application lives in `apps/hosted` and is independent from
the Rust desktop and local bundle workflow. Admission is fixed off in every
environment in this foundation. Development and production have no public
Worker route; preview alone may use its distinct `workers.dev` name for an
authorized smoke test. No upload or publication endpoint exists yet.

## Fixed Environment And Binding Matrix

| Role | Development | Preview | Production |
| --- | --- | --- | --- |
| Worker | `antennabench-hosted-development` | `antennabench-hosted-preview` | `antennabench-hosted-production` |
| Private upload R2 | `...-development-uploads-private` | `...-preview-uploads-private` | `...-production-uploads-private` |
| Private derived R2 | `...-development-derived-private` | `...-preview-derived-private` | `...-production-derived-private` |
| Public-report R2 | `...-development-reports-public` | `...-preview-reports-public` | `...-production-reports-public` |
| D1 control plane | `...-development-control` | `...-preview-control` | `...-production-control` |
| Processing Queue | `...-development-process` | `...-preview-process` | `...-production-process` |
| Dead-letter Queue | `...-development-process-dlq` | `...-preview-process-dlq` | `...-production-process-dlq` |

Every omitted prefix is `antennabench-hosted`. The three R2 roles are separate
buckets and bindings. Do not enable public development access or an R2 custom
domain on either private bucket. The `REPORTS_PUBLIC` bucket also stays private
until the trusted immutable promotion path in #70 is implemented.

The checked-in D1 UUIDs are recognizable non-resource placeholders so local
types and fake tests are deterministic. They must be replaced only with the ID
returned for the same named environment resource; never copy an ID between
environments.

## Local Verification

No Cloudflare account, network request, Container runtime, or Docker daemon is
required for the maintained contract suite:

```bash
mise run hosted:test
```

That task reuses the clean locked root npm workspace install, then runs the
hosted Vitest project, generated binding-type drift checks, strict TypeScript
compilation, and a Wrangler dry build. `--containers-rollout=none` deliberately
bundles and validates the
Worker without building the separately declared OCI image. A maintainer with a
Docker-compatible runtime can additionally run:

```bash
cd apps/hosted
npx wrangler deploy --dry-run --env= --outdir dist/worker
```

## Create One Remote Environment

Authenticate interactively with the intended Cloudflare account and confirm it
before creating anything:

```bash
cd apps/hosted
npx wrangler whoami
```

For `preview` or `production`, create the three exactly named R2 buckets, the
D1 database, processing Queue, and dead-letter Queue shown in the matrix. Use
the matching `--env` on Wrangler commands. Record the returned D1 UUID in only
that environment block. Apply the foundation migration by database name:

```bash
npx wrangler d1 migrations apply antennabench-hosted-preview-control --env preview --remote
```

Do not create routes, R2 custom domains, access keys, presigned-upload secrets,
or user-publishing credentials for this slice. Container and Durable Object
declarations are deployed with the Worker. The Container is fixed to `basic`,
at most two running instances, SSH disabled, and public Internet egress disabled
in code. Its probe stops explicitly in a `finally` path; idle compute is not an
accepted lifecycle.

## Deploy And Verify Bindings

Run the locked checks immediately before deployment, then deploy only the named
environment:

```bash
mise run hosted:test
cd apps/hosted
npx wrangler deploy --env preview
npx wrangler d1 migrations list antennabench-hosted-preview-control --env preview --remote
npx wrangler r2 bucket list
npx wrangler queues list
```

For the authorized preview smoke test, request `/api/health` and verify all
seven binding roles are `true`, the environment is `preview`, the profile is
`hosted-standard-v1`, and admission remains `false`. The response intentionally
contains no resource IDs or names. Production has no route in the foundation;
verify its binding table in Wrangler deploy output and the dashboard instead.

## Observability And Sensitive Data

Worker invocation logs are disabled because they include request URLs.
Lifecycle code may log only `event`, `stage`, `outcome`, and stable `code`.
Never log bundle bytes, callsigns, grids or coordinates, notes, filenames,
tokens, capabilities, identities, or complete report URLs. Ordinary static
views bypass the Worker and produce no application log. Production custom-log
sampling is 10%; preview and local lifecycle events are unsampled.

Inspect Queue age, dead-letter count, Container duration, R2 retained bytes, D1
rows read/written, Worker requests/CPU, and Workers Logs volume in the
Cloudflare dashboard. Configure a low monthly account budget notification
before enabling any later admission. Budget alerts notify; they are not a hard
spend stop. The hard controls remain admission state, fixed profile limits,
Queue backpressure, and the two-instance ceiling.

The reviewed planning envelope expects a fixed floor near the Workers Paid USD
$5 monthly minimum and no idle application compute. Before provisioning, and
after every pricing or profile change, compare current Workers, Containers, R2,
D1, Queues, and Logs pricing with the estimates in ADR 0013. Inspect actual
account usage after each smoke test; do not describe included usage as a cap.

## Admission Stop, Drain, And Teardown

Admission is already off in this foundation. In a later incident, deploy a
reviewed configuration with admission off before draining. Do not delete a
Queue while it has accepted jobs. Observe the processing Queue until empty,
inspect and reconcile the dead-letter Queue, stop every Container instance, and
only then remove the consumer.

Complete teardown proceeds from public exposure inward:

1. remove Worker routes and any later public R2 custom domain;
2. stop admission and drain/reconcile both Queues;
3. stop Container instances and delete obsolete Container images;
4. remove the Worker/Container deployment;
5. delete public derived objects, then private derived objects, then private
   original uploads according to the selected retention policy;
6. export any required operational audit metadata, then delete D1; and
7. delete the environment-specific Queues and R2 buckets and confirm billing
   shows no retained bytes or active compute.

Never teardown one environment by broad prefix matching. Compare every resource
name against the matrix and inspect current cost/usage after deletion.
