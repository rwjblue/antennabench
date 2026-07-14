import assert from "node:assert/strict";
import test from "node:test";

import fs from "node:fs";

import {
  validateDenyPolicyText,
  validateExceptions,
  validateFreshGate,
  validateRepository,
} from "./validate-dependency-exceptions.mjs";

const TODAY = new Date("2026-07-14T00:00:00Z");
const LOCK = '[[package]]\nname = "example-crate"\nversion = "1.2.3"\n';
const DENY = '[advisories]\nignore = ["RUSTSEC-2026-0001"]\n[bans]\n';

function validException(overrides = {}) {
  return {
    id: "example-direct-vulnerability",
    category: "vulnerability",
    severity: "high",
    identity: "RUSTSEC-2026-0001",
    package: { name: "example-crate", version: "1.2.3" },
    reachability: "The affected API is not called by the desktop application.",
    rationale: "No fixed compatible release is available during this review window.",
    mitigation: "Keep the feature disabled and update through the linked tracking issue.",
    owner: "@rwjblue",
    issue: "https://github.com/rwjblue/antennabench/issues/999",
    approved_on: "2026-07-01",
    expires_on: "2026-07-31",
    enforcement_reference: "RUSTSEC-2026-0001",
    ...overrides,
  };
}

function validate(exception, options = {}) {
  return validateExceptions(
    { version: 1, exceptions: [exception] },
    { today: TODAY, lockText: LOCK, denyText: DENY, ...options },
  );
}

test("accepts a complete in-scope time-bounded exception", () => {
  assert.deepEqual(validate(validException()), []);
});

test("rejects expired, overlong, and future exception windows", () => {
  assert.ok(validate(validException({ expires_on: "2026-07-13" })).some((error) => /expired/.test(error)));
  assert.ok(validate(validException({ expires_on: "2026-08-01" })).some((error) => /30-day/.test(error)));
  assert.ok(
    validate(validException({ approved_on: "2026-07-15", expires_on: "2026-07-20" })).some(
      (error) => /future/.test(error),
    ),
  );
});

test("rejects stale, unenforced, duplicate, and malicious exception records", () => {
  assert.ok(validate(validException(), { lockText: "" }).some((error) => /Cargo.lock/.test(error)));
  assert.ok(validate(validException(), { denyText: "" }).some((error) => /deny.toml/.test(error)));
  assert.ok(
    validateExceptions(
      { version: 1, exceptions: [validException(), validException()] },
      { today: TODAY, lockText: LOCK, denyText: DENY },
    ).some((error) => /unique/.test(error)),
  );
  assert.ok(validate(validException({ category: "malicious" })).some((error) => /cannot be waived/.test(error)));
  assert.ok(
    validateExceptions(
      { version: 1, exceptions: [] },
      { today: TODAY, lockText: LOCK, denyText: DENY },
    ).some((error) => /no tracked exception/.test(error)),
  );
});

test("rejects missing review evidence and non-exact version scope", () => {
  for (const field of ["reachability", "rationale", "mitigation", "owner", "issue"]) {
    assert.ok(validate(validException({ [field]: "" })).length > 0, field);
  }
  assert.ok(
    validate(validException({ package: { name: "example-crate", version: ">=1.2" } })).some(
      (error) => /exact semantic version/.test(error),
    ),
  );
  assert.ok(
    validate(validException({ enforcement_reference: "example-crate@1.2.3" })).some(
      (error) => /advisory identity/.test(error),
    ),
  );
});

test("pins the complete advisory, source, wildcard, duplicate, and license matrix", () => {
  const policy = fs.readFileSync("deny.toml", "utf8");
  assert.deepEqual(validateDenyPolicyText(policy), []);
  for (const mutation of [
    ["unsound = \"all\"", "unsound = \"workspace\""],
    ["unmaintained = \"workspace\"", "unmaintained = \"none\""],
    ["unknown-git = \"deny\"", "unknown-git = \"warn\""],
    ["multiple-versions = \"warn\"", "multiple-versions = \"allow\""],
    ["\"MPL-2.0\"", "\"GPL-3.0\""],
  ]) {
    assert.ok(validateDenyPolicyText(policy.replace(...mutation)).length > 0, mutation[0]);
  }
  assert.ok(
    validateDenyPolicyText(policy.replace('  "Zlib",', '  "Zlib",\n  "GPL-3.0",')).some(
      (error) => /unreviewed global license/.test(error),
    ),
  );
});

test("fresh advisory and release workflow failures cannot be suppressed", () => {
  const valid = {
    advisoryTask: "set -euo pipefail\ncargo deny --locked check advisories\n",
    releaseTask:
      "mise run supply-chain\nmise run dependency-policy\nmise run advisory-fresh\n",
    workflow: `on:
  pull_request:
    paths:
      - "Cargo.lock"
      - "Cargo.toml"
  push:
    branches:
      - main
  schedule:
    - cron: "17 11 * * *"
  workflow_dispatch:
  workflow_call:
jobs:
  preflight:
    steps:
      - run: mise run release-preflight
`,
  };
  assert.deepEqual(validateFreshGate(valid), []);
  assert.ok(
    validateFreshGate({
      ...valid,
      advisoryTask: "set -euo pipefail\ncargo deny --locked check advisories || true\n",
    }).length > 0,
  );
  assert.ok(validateFreshGate({ ...valid, workflow: valid.workflow.replace("  schedule:\n", "") }).length > 0);
  assert.ok(
    validateFreshGate({ ...valid, releaseTask: valid.releaseTask.replace("mise run advisory-fresh\n", "") })
      .length > 0,
  );
});

test("the repository has no invalid or expired exceptions", () => {
  assert.deepEqual(validateRepository(process.cwd(), TODAY), []);
});
