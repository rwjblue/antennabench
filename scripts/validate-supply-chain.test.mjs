import assert from "node:assert/strict";
import test from "node:test";

import {
  validateDependabotText,
  validateDependencyReviewText,
  validateManifestCoverage,
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
`;
  assert.deepEqual(validateDependabotText(valid), []);
  assert.ok(validateDependabotText(valid.replace("interval: weekly", "interval: monthly")).length);
  assert.ok(validateDependabotText(valid.replace("- patch", "- major")).length);
  assert.ok(
    validateDependabotText(valid.replace("applies-to: version-updates", "applies-to: security-updates"))
      .length,
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

test("the repository satisfies its supply-chain convention", () => {
  assert.deepEqual(validateRepository(process.cwd()), []);
});
