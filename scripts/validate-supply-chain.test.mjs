import assert from "node:assert/strict";
import test from "node:test";

import {
  validateDependabotText,
  validateDependencyReviewText,
  validateHostedSiteDeployWorkflowText,
  validateManifestCoverage,
  validateNpmConfigText,
  validateNpmWorkspace,
  validateReleaseWorkflowText,
  validateRepository,
  validateUsesText,
} from "./validate-supply-chain.mjs";

const SHA = "0123456789abcdef0123456789abcdef01234567";

test("accepts immutable actions with release provenance and local actions", () => {
  assert.deepEqual(
    validateUsesText(`steps:\n  - uses: actions/checkout@${SHA} # v7.0.0\n  - uses: ./local`),
    [],
  );
});

test("rejects mutable, short, uncommented, container, and remote workflow references", () => {
  for (const reference of [
    "actions/checkout@v7 # v7.0.0",
    "actions/checkout@0123456 # v7.0.0",
    `actions/checkout@${SHA}`,
    "docker://example/image:latest",
    "owner/repo/.github/workflows/ci.yml@main # v1.0.0",
  ]) {
    assert.ok(validateUsesText(`- uses: ${reference}`).length > 0, reference);
  }
});

test("new dependency manifests require an explicit maintenance policy entry", () => {
  const policy = {
    ecosystems: [{ manifest_globs: ["Cargo.toml", "crates/*/Cargo.toml"] }],
  };
  assert.deepEqual(validateManifestCoverage(["Cargo.toml", "crates/core/Cargo.toml"], policy), []);
  assert.match(validateManifestCoverage(["package.json"], policy)[0], /no maintenance policy/);
});

test("Dependabot routine groups are weekly, bounded, and exclude security and major updates", () => {
  const valid = `version: 2
updates:
  - package-ecosystem: cargo
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
    groups:
      routine:
        applies-to: version-updates
        update-types:
          - minor
          - patch
  - package-ecosystem: github-actions
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
    groups:
      routine:
        applies-to: version-updates
        update-types:
          - minor
          - patch
  - package-ecosystem: npm
    schedule:
      interval: weekly
    open-pull-requests-limit: 5
    groups:
      routine:
        applies-to: version-updates
        update-types:
          - minor
          - patch
`;
  assert.deepEqual(validateDependabotText(valid), []);
  assert.ok(validateDependabotText(valid.replace("interval: weekly", "interval: monthly")).length);
  assert.ok(validateDependabotText(valid.replace("- patch", "- major")).length);
  assert.ok(
    validateDependabotText(valid.replace("applies-to: version-updates", "applies-to: security-updates"))
      .length,
  );
});

test("npm policy requires exact workspace pins, one root lock, and lock agreement", () => {
  const manifests = {
    "package.json": { private: true, workspaces: ["apps/desktop", "apps/hosted"] },
    "apps/desktop/package.json": { private: true, devDependencies: { vitest: "4.1.10" } },
    "apps/hosted/package.json": { private: true, dependencies: { worker: "1.2.3" } },
  };
  const lock = {
    packages: {
      "": {},
      "apps/desktop": { devDependencies: { vitest: "4.1.10" } },
      "apps/hosted": { dependencies: { worker: "1.2.3" } },
    },
  };
  assert.deepEqual(validateNpmWorkspace(manifests, ["package-lock.json"], lock), []);
  assert.ok(validateNpmWorkspace(manifests, ["apps/hosted/package-lock.json"], lock).length);
  assert.ok(validateNpmWorkspace({
    ...manifests,
    "apps/desktop/package.json": { devDependencies: { vitest: "^4.1.10" } },
  }, ["package-lock.json"], lock).length);
  assert.ok(
    validateNpmWorkspace({
      ...manifests,
      "package.json": {
        ...manifests["package.json"],
        allowScripts: { "workerd@1.2.3": true },
      },
    }, ["package-lock.json"], lock).some((error) => error.includes("without duplicating")),
  );
});

test("npm configuration preserves exact pins during automated updates", () => {
  const valid = "save-exact=true\nstrict-allow-scripts=true\n";
  assert.deepEqual(validateNpmConfigText(valid), []);
  assert.match(
    validateNpmConfigText(valid.replace("save-exact=true", "save-exact=false"))[0],
    /save exact/,
  );
  assert.match(
    validateNpmConfigText(
      valid.replace("strict-allow-scripts=true", "strict-allow-scripts=false"),
    )[0],
    /fail closed/,
  );
});

test("dependency review is pull-request-only and blocks moderate additions", () => {
  const valid = `name: Dependency review
on:
  pull_request:
permissions:
  contents: read
jobs:
  review:
    steps:
      - uses: actions/dependency-review-action@${SHA} # v5.0.0
        with:
          fail-on-severity: moderate
`;
  assert.deepEqual(validateDependencyReviewText(valid), []);
  assert.ok(validateDependencyReviewText(valid.replace("pull_request:", "push:")).length);
  assert.ok(validateDependencyReviewText(valid.replace("moderate", "high")).length);
});

test("release workflow validator pins the trusted event and credential boundary", () => {
  const valid = `name: release
on:
  push:
    tags:
      - "v*"
permissions:
  contents: read
jobs:
  sign:
    environment: desktop-release
    steps:
      - run: echo \${{ secrets.APPLE_CERTIFICATE }} \${{ secrets.APPLE_CERTIFICATE_PASSWORD }} \${{ secrets.APPLE_API_ISSUER }} \${{ secrets.APPLE_API_KEY }} \${{ secrets.APPLE_API_PRIVATE_KEY }}
  assemble:
    steps:
      - run: mise run desktop:release-assemble -- arm intel --require-publishable
  attest:
    permissions:
      contents: read
      id-token: write
      attestations: write
    steps:
      - uses: actions/attest@${SHA} # v4.1.1
  publish:
    permissions:
      contents: write
    steps:
      - run: mise run desktop:publication-publish-draft
  verify:
    steps:
      - run: mise run desktop:publication-verify-draft
`;
  assert.deepEqual(validateReleaseWorkflowText(valid), []);
  assert.ok(validateReleaseWorkflowText(valid.replace("environment: desktop-release", "environment: ci")).length);
  assert.ok(validateReleaseWorkflowText(valid.replace("  assemble:\n", "  leak:\n    run: echo ${{ secrets.APPLE_API_KEY }}\n  assemble:\n")).length);
  assert.ok(validateReleaseWorkflowText(valid.replace("  push:\n", "  pull_request:\n")).length);
  assert.ok(validateReleaseWorkflowText(valid.replace("publication-publish-draft", "gh release publish")).length);
});

test("hosted site deployment pins reviewed main source and protected credentials", () => {
  const valid = `name: site
on:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      source_revision:
        required: true
permissions:
  contents: read
jobs:
  deploy:
    environment:
      name: production
    steps:
      - run: git merge-base --is-ancestor "$GITHUB_SHA" origin/main
      - run: mise run hosted:test
      - name: Deploy static site
        run: npm run deploy:site --workspace @antennabench/hosted
        env:
          CLOUDFLARE_ACCOUNT_ID: \${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          CLOUDFLARE_API_TOKEN: \${{ secrets.CLOUDFLARE_API_TOKEN }}
`;
  assert.deepEqual(validateHostedSiteDeployWorkflowText(valid), []);
  assert.ok(validateHostedSiteDeployWorkflowText(valid.replace("branches: [main]", "branches: [feature]")).length);
  assert.ok(validateHostedSiteDeployWorkflowText(valid.replace("origin/main", "HEAD^")).length);
  assert.ok(validateHostedSiteDeployWorkflowText(valid.replace("- name: Deploy static site", "- run: echo ${{ secrets.CLOUDFLARE_API_TOKEN }}\n      - name: Deploy static site")).length);
});

test("the repository satisfies its supply-chain convention", () => {
  assert.deepEqual(validateRepository(process.cwd()), []);
});
