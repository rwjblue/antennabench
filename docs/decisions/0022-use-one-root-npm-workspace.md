# 0022: Use One Root npm Workspace For JavaScript Tooling

Date: 2026-07-17

## Decision

AntennaBench uses one private npm workspace at the repository root with exactly
two members: `apps/desktop` and `apps/hosted`. One root `package-lock.json` is
the complete reproducible JavaScript dependency graph, and `npm ci` at the
repository root is the ordinary local and CI installation boundary.

Each workspace owns the direct dependencies imported by its source or tests.
The hosted workspace continues to own its Cloudflare runtime, TypeScript,
Vitest, and Wrangler dependencies. The desktop workspace owns only development
test dependencies: Vitest, its V8 coverage provider, and jsdom. The root
manifest orchestrates cross-workspace commands and owns the reviewed npm
install-script allowlist, but does not absorb workspace source dependencies.
All direct versions remain exact.

Vitest uses its current `projects` configuration. Pure desktop state, bridge,
model, form, and injected-controller behavior runs in a Node project. Renderer,
element-registry, form, contextual-help, focus, event, dynamic-child,
accessibility, and report-frame contracts run in a jsdom project that loads the
real checked-in desktop HTML. Hosted Worker fake-binding tests remain a third
Node project. V8 coverage reports lines, branches, and functions without an
arbitrary percentage gate; named behavior is authoritative.

The desktop production boundary does not change. Tauri still serves
`apps/desktop/frontend` directly through the static `frontendDist` setting.
That directory contains checked-in HTML, CSS, SVG, and native ES modules and has
no install, build, bundle, transpile, or generated-asset step. The desktop
package manifest has no production dependencies or production scripts, Tauri
has no frontend pre-build command, and Node/test dependencies are outside the
packaged frontend input.

## Context

Issues #102 through #105 intentionally established dependency-free modules and
tests while the frontend boundaries were still being extracted. That choice
made the state, bridge, controller, element registry, renderer, and platform
seams explicit, but standard DOM emulation eventually grew into hand-written
element, document, selector, attribute, class-list, event, focus, and child-tree
implementations across the desktop suite. Maintaining those incomplete browser
implementations became more costly and less truthful than using jsdom for test
behavior.

The hosted application already required an exact-pinned npm/Vitest graph and a
lockfile. Consolidating it with desktop test tooling gives the repository one
reviewable graph, one Dependabot entry, one audit boundary, and one cache key.
This later cost-based decision supersedes only the earlier test-tooling choice;
it preserves the module architecture and every Rust/Tauri authority boundary
established by #102 through #105.

## Consequences

- Contributors install JavaScript dependencies once from the repository root.
- Nested npm lockfiles and undeclared manifests fail supply-chain validation.
- Dependabot reviews the complete workspace graph from `/`, and the root audit
  fails at the ADR 0012 moderate-or-higher policy boundary.
- Desktop tests carry development dependencies, but desktop production and
  release construction do not gain a Node runtime or frontend build step.
- Astro, React, bundling, generated frontend assets, and browser/WebDriver
  automation remain separate decisions and are not implied by this workspace.
