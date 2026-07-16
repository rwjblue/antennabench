import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  archiveName,
  assembleArtifacts,
  assertStableVersion,
  assertVersionTag,
  canonicalJson,
  checksumLines,
  readCompleteArtifactSet,
  targetContract,
  validateStagedEntries,
  withAtomicDirectory,
} from "./desktop-release.mjs";

test("accepts stable versions and exact v-prefixed tags", () => {
  assert.equal(assertStableVersion("0.1.0"), "0.1.0");
  assert.doesNotThrow(() => assertVersionTag("12.34.56", "v12.34.56"));
  for (const version of ["01.2.3", "1.2", "1.2.3-beta.1", "v1.2.3"]) {
    assert.throws(() => assertStableVersion(version), /stable MAJOR\.MINOR\.PATCH/);
  }
  assert.throws(() => assertVersionTag("1.2.3", "v1.2.4"), /does not match/);
});

test("target contracts make arm64 and Intel artifact names truthful", () => {
  assert.deepEqual(targetContract("aarch64-apple-darwin"), {
    architecture: "arm64",
    runner: "macos-15",
  });
  assert.deepEqual(targetContract("x86_64-apple-darwin"), {
    architecture: "x86_64",
    runner: "macos-15-intel",
  });
  assert.equal(
    archiveName("0.1.0", "aarch64-apple-darwin"),
    "AntennaBench-0.1.0-aarch64-apple-darwin.zip",
  );
  assert.equal(
    archiveName("0.1.0", "x86_64-apple-darwin"),
    "AntennaBench-0.1.0-x86_64-apple-darwin.zip",
  );
});

test("manifest JSON and checksum entries use stable bytewise ordering", () => {
  assert.equal(canonicalJson({ z: 1, a: { y: 2, b: 3 } }), '{\n  "a": {\n    "b": 3,\n    "y": 2\n  },\n  "z": 1\n}\n');
  assert.equal(
    checksumLines([
      ["z.zip", "bbb"],
      ["A.json", "aaa"],
    ]),
    "aaa  A.json\nbbb  z.zip\n",
  );
});

test("staging rejects unexpected public assets", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "desktop-release-assets-"));
  try {
    fs.writeFileSync(path.join(directory, "expected.zip"), "zip");
    assert.doesNotThrow(() => validateStagedEntries(directory, ["expected.zip"]));
    fs.writeFileSync(path.join(directory, "unexpected.dmg"), "dmg");
    assert.throws(
      () => validateStagedEntries(directory, ["expected.zip"]),
      /staged asset set mismatch/,
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("failed atomic staging leaves neither a final nor partial directory", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "desktop-release-atomic-"));
  const finalDirectory = path.join(root, "complete");
  fs.mkdirSync(finalDirectory);
  fs.writeFileSync(path.join(finalDirectory, "stale.zip"), "stale");
  await assert.rejects(
    withAtomicDirectory(finalDirectory, async (staging) => {
      fs.writeFileSync(path.join(staging, "partial.zip"), "partial");
      throw new Error("injected failure");
    }),
    /injected failure/,
  );
  assert.equal(fs.existsSync(finalDirectory), false);
  assert.deepEqual(fs.readdirSync(root), []);
  fs.rmSync(root, { recursive: true, force: true });
});

test("assembly emits the exact two-archive manifest and checksum set", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "desktop-release-assemble-"));
  try {
    const inputs = [];
    for (const target of ["aarch64-apple-darwin", "x86_64-apple-darwin"]) {
      const directory = path.join(root, "inputs", target);
      fs.mkdirSync(directory, { recursive: true });
      const filename = archiveName("0.1.0", target);
      fs.writeFileSync(path.join(directory, filename), target);
      const digest = await import("node:crypto").then(({ default: crypto }) =>
        crypto.createHash("sha256").update(target).digest("hex"),
      );
      fs.writeFileSync(
        path.join(directory, "artifact-manifest.json"),
        canonicalJson({
          app: { signature: { publishable: false } },
          artifact: { filename, sha256: digest, size: Buffer.byteLength(target) },
          build_inputs: { target },
          contract: { target },
          publishable: false,
          schema_version: 1,
          source: { commit: "0123456789abcdef0123456789abcdef01234567" },
          state: "complete",
          tag: "v0.1.0",
          version: "0.1.0",
        }),
      );
      fs.writeFileSync(path.join(directory, "NON_PUBLISHABLE.txt"), "local\n");
      inputs.push(directory);
    }
    const output = await assembleArtifacts({ root, inputs });
    validateStagedEntries(output, [
      "AntennaBench-0.1.0-SHA256SUMS",
      "AntennaBench-0.1.0-aarch64-apple-darwin.zip",
      "AntennaBench-0.1.0-release-manifest.json",
      "AntennaBench-0.1.0-x86_64-apple-darwin.zip",
      "NON_PUBLISHABLE.txt",
    ]);
    await assert.rejects(
      assembleArtifacts({ root, inputs, requirePublishable: true }),
      /requires signed, notarized, publishable/,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("assembly does not trust a forged publishable flag", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "desktop-release-trust-"));
  try {
    const target = "aarch64-apple-darwin";
    const directory = path.join(root, target);
    fs.mkdirSync(directory, { recursive: true });
    const filename = archiveName("0.1.0", target);
    fs.writeFileSync(path.join(directory, filename), target);
    const crypto = await import("node:crypto").then(({ default: value }) => value);
    fs.writeFileSync(
      path.join(directory, "artifact-manifest.json"),
      canonicalJson({
        app: { signature: { publishable: false } },
        artifact: {
          filename,
          sha256: crypto.createHash("sha256").update(target).digest("hex"),
          size: Buffer.byteLength(target),
        },
        contract: { target },
        publishable: true,
        schema_version: 1,
        source: { commit: "0123456789abcdef0123456789abcdef01234567", dirty: false },
        state: "complete",
        tag: "v0.1.0",
        version: "0.1.0",
      }),
    );
    await assert.rejects(
      assembleArtifacts({ root, inputs: [directory, directory] }),
      /publishable state disagrees/,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("complete-set verification rechecks exact publishable bytes and trust evidence", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "desktop-release-complete-"));
  try {
    const crypto = await import("node:crypto").then(({ default: value }) => value);
    const inputs = [];
    for (const target of ["aarch64-apple-darwin", "x86_64-apple-darwin"]) {
      const directory = path.join(root, "inputs", target);
      fs.mkdirSync(directory, { recursive: true });
      const filename = archiveName("0.1.0", target);
      fs.writeFileSync(path.join(directory, filename), target);
      fs.writeFileSync(
        path.join(directory, "artifact-manifest.json"),
        canonicalJson({
          app: {
            metadata: { build_version: "0.1.0", short_version: "0.1.0" },
            signature: {
              authorities: ["Developer ID Application: Example (TEAMID)"],
              classification: "developer-id",
              gatekeeper: "accepted",
              hardened_runtime: true,
              notarization: "stapled-and-validated",
              publishable: true,
              secure_timestamp: true,
            },
          },
          artifact: {
            filename,
            sha256: crypto.createHash("sha256").update(target).digest("hex"),
            size: Buffer.byteLength(target),
          },
          build_inputs: { target },
          contract: { target },
          publishable: true,
          schema_version: 1,
          source: {
            commit: "0123456789abcdef0123456789abcdef01234567",
            dirty: false,
          },
          state: "complete",
          tag: "v0.1.0",
          version: "0.1.0",
        }),
      );
      inputs.push(directory);
    }
    const output = await assembleArtifacts({ root, inputs, requirePublishable: true });
    assert.equal(readCompleteArtifactSet(output).entries.length, 4);
    fs.appendFileSync(path.join(output, "AntennaBench-0.1.0-SHA256SUMS"), "unexpected\n");
    assert.throws(() => readCompleteArtifactSet(output), /does not exactly match/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
