import assert from "node:assert/strict";
import test from "node:test";

import { planDraftMutation } from "./desktop-publication.mjs";

const ASSETS = [
  "AntennaBench-0.1.0-SHA256SUMS",
  "AntennaBench-0.1.0-aarch64-apple-darwin.zip",
  "AntennaBench-0.1.0-release-manifest.json",
  "AntennaBench-0.1.0-x86_64-apple-darwin.zip",
];

test("draft retry policy creates, resumes empty, or verifies an exact set", () => {
  assert.equal(planDraftMutation(null, ASSETS), "create");
  assert.equal(planDraftMutation({ isDraft: true, assets: [] }, ASSETS), "resume-empty");
  assert.equal(
    planDraftMutation({ isDraft: true, assets: ASSETS.map((name) => ({ name })).reverse() }, ASSETS),
    "verify-existing",
  );
});

test("draft retry policy rejects publication and partial or unexpected assets", () => {
  assert.throws(
    () => planDraftMutation({ isDraft: false, assets: ASSETS.map((name) => ({ name })) }, ASSETS),
    /not a draft/,
  );
  assert.throws(
    () => planDraftMutation({ isDraft: true, assets: [{ name: ASSETS[0] }] }, ASSETS),
    /partial or mismatched/,
  );
  assert.throws(
    () => planDraftMutation({ isDraft: true, assets: [...ASSETS, "extra.dmg"].map((name) => ({ name })) }, ASSETS),
    /partial or mismatched/,
  );
});
