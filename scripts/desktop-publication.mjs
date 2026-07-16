import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertVersionTag,
  canonicalJson,
  readCompleteArtifactSet,
  readTargetManifest,
  verifyCompleteArtifacts,
  withAtomicDirectory,
} from "./desktop-release.mjs";

const PRODUCT = "AntennaBench";
const MAX_NOTARY_LOG_BYTES = 1_048_576;

function commandResult(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
    timeout: options.timeout ?? 60_000,
  });
  return {
    error: result.error,
    status: result.status,
    stderr: result.stderr ?? "",
    stdout: result.stdout ?? "",
  };
}

function capture(command, args, options = {}) {
  const result = commandResult(command, args, options);
  if (result.error || result.status !== 0) {
    const output = `${result.stdout}${result.stderr}`.trim();
    const reason = result.error?.message ?? `exit ${result.status}`;
    const invocation = options.sensitive ? command : `${command} ${args.join(" ")}`;
    throw new Error(`${invocation} failed (${reason})${output ? `:\n${output}` : ""}`);
  }
  return result.stdout.trim();
}

function workspaceVersion(root) {
  const cargo = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const section = cargo.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/)?.[1] ?? "";
  const version = section.match(/^version = "([^"]+)"$/m)?.[1];
  if (!version) throw new Error("Cargo.toml workspace package version is missing");
  return version;
}

export function validateTagContext({ root, tag, expectedCommit = process.env.GITHUB_SHA }) {
  const version = workspaceVersion(root);
  assertVersionTag(version, tag);
  const head = capture("git", ["rev-parse", "HEAD"], { cwd: root });
  const taggedCommit = capture("git", ["rev-parse", `${tag}^{commit}`], { cwd: root });
  if (head !== taggedCommit) throw new Error(`tag ${tag} does not identify checked-out commit ${head}`);
  if (expectedCommit && expectedCommit !== head) {
    throw new Error(`expected source ${expectedCommit} does not match checked-out commit ${head}`);
  }
  capture("git", ["merge-base", "--is-ancestor", head, "origin/main"], { cwd: root });
  return { commit: head, tag, version };
}

export async function prepareSigningInput({ input, output, target, tag, expectedCommit }) {
  const record = readTargetManifest(input);
  if (record.manifest.publishable !== false) {
    throw new Error("signing input must be the verified non-publishable artifact from the build job");
  }
  if (record.manifest.contract.target !== target || record.manifest.tag !== tag) {
    throw new Error("signing input target or tag does not match the protected signing job");
  }
  if (record.manifest.source.dirty !== false || record.manifest.source.commit !== expectedCommit) {
    throw new Error("signing input source does not match the clean tagged source revision");
  }
  const archive = path.join(input, record.manifest.artifact.filename);
  await withAtomicDirectory(output, async (staging) => {
    capture("ditto", ["-x", "-k", archive, staging], { timeout: 300_000 });
    const entries = fs.readdirSync(staging);
    if (entries.length !== 1 || entries[0] !== `${PRODUCT}.app`) {
      throw new Error(`signing input archive must contain exactly ${PRODUCT}.app`);
    }
  });
  console.log(`Prepared verified signing input at ${output}`);
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) throw new Error(`required protected-environment secret ${name} is missing`);
  return value;
}

function writePrivateFile(filename, contents) {
  fs.writeFileSync(filename, contents, { mode: 0o600 });
  fs.chmodSync(filename, 0o600);
}

function developerIdentity(keychain) {
  const output = capture("security", ["find-identity", "-v", "-p", "codesigning", keychain]);
  const identities = [...output.matchAll(/"(Developer ID Application: [^"]+)"/g)].map(
    (match) => match[1],
  );
  if (identities.length !== 1) {
    throw new Error(`certificate must contain exactly one Developer ID Application identity; found ${identities.length}`);
  }
  return identities[0];
}

function assertNoNestedCode(app) {
  for (const relative of ["Frameworks", "PlugIns", "XPCServices", "Helpers"]) {
    const directory = path.join(app, "Contents", relative);
    if (fs.existsSync(directory) && fs.readdirSync(directory).length > 0) {
      throw new Error(`unexpected nested code at Contents/${relative}; add explicit inside-out signing before release`);
    }
  }
}

export async function signAndNotarize({ app, evidenceDirectory }) {
  if (process.platform !== "darwin") throw new Error("Apple signing requires macOS");
  const certificate = requiredEnvironment("APPLE_CERTIFICATE");
  const certificatePassword = requiredEnvironment("APPLE_CERTIFICATE_PASSWORD");
  const issuer = requiredEnvironment("APPLE_API_ISSUER");
  const keyId = requiredEnvironment("APPLE_API_KEY");
  const privateKey = requiredEnvironment("APPLE_API_PRIVATE_KEY");
  if (!privateKey.includes("BEGIN PRIVATE KEY")) {
    throw new Error("APPLE_API_PRIVATE_KEY must contain the App Store Connect .p8 file contents");
  }

  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "antennabench-signing-"));
  const keychain = path.join(temporary, "release.keychain-db");
  const certificateFile = path.join(temporary, "developer-id.p12");
  const privateKeyFile = path.join(temporary, "AuthKey.p8");
  const notarizationArchive = path.join(temporary, `${PRODUCT}-notarization.zip`);
  const keychainPassword = crypto.randomBytes(32).toString("base64url");
  fs.mkdirSync(evidenceDirectory, { recursive: true });
  try {
    const certificateBytes = Buffer.from(certificate.replace(/\s/g, ""), "base64");
    if (certificateBytes.length === 0) throw new Error("APPLE_CERTIFICATE is not valid base64");
    writePrivateFile(certificateFile, certificateBytes);
    writePrivateFile(privateKeyFile, privateKey);

    capture("security", ["create-keychain", "-p", keychainPassword, keychain], { sensitive: true });
    capture("security", ["set-keychain-settings", "-lut", "21600", keychain], { sensitive: true });
    capture("security", ["unlock-keychain", "-p", keychainPassword, keychain], { sensitive: true });
    capture("security", [
      "import",
      certificateFile,
      "-k",
      keychain,
      "-P",
      certificatePassword,
      "-T",
      "/usr/bin/codesign",
    ], { sensitive: true });
    capture("security", [
      "set-key-partition-list",
      "-S",
      "apple-tool:,apple:",
      "-s",
      "-k",
      keychainPassword,
      keychain,
    ], { sensitive: true });
    const identity = developerIdentity(keychain);
    assertNoNestedCode(app);
    capture("codesign", [
      "--force",
      "--options",
      "runtime",
      "--timestamp",
      "--keychain",
      keychain,
      "--sign",
      identity,
      app,
    ], { timeout: 300_000 });
    capture("codesign", ["--verify", "--deep", "--strict", "--verbose=2", app]);

    capture("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, notarizationArchive], {
      timeout: 300_000,
    });
    const submissionText = capture("xcrun", [
      "notarytool",
      "submit",
      notarizationArchive,
      "--key",
      privateKeyFile,
      "--key-id",
      keyId,
      "--issuer",
      issuer,
      "--wait",
      "--timeout",
      "30m",
      "--output-format",
      "json",
    ], { timeout: 1_900_000, sensitive: true });
    const submission = JSON.parse(submissionText);
    fs.writeFileSync(path.join(evidenceDirectory, "notarization-submission.json"), canonicalJson(submission));
    if (!submission.id) throw new Error("notarytool did not return a submission id");
    const log = capture("xcrun", [
      "notarytool",
      "log",
      submission.id,
      "--key",
      privateKeyFile,
      "--key-id",
      keyId,
      "--issuer",
      issuer,
    ], { timeout: 300_000, sensitive: true });
    if (Buffer.byteLength(log) > MAX_NOTARY_LOG_BYTES) {
      throw new Error(`notarization log exceeds ${MAX_NOTARY_LOG_BYTES} bytes`);
    }
    fs.writeFileSync(path.join(evidenceDirectory, "notarization-log.json"), `${log}\n`);
    if (submission.status !== "Accepted") {
      throw new Error(`notarization failed with status ${submission.status ?? "unknown"}`);
    }
    capture("xcrun", ["stapler", "staple", app], { timeout: 300_000 });
    capture("xcrun", ["stapler", "validate", app], { timeout: 120_000 });
    fs.writeFileSync(
      path.join(evidenceDirectory, "signing-summary.json"),
      canonicalJson({
        identity,
        notarization_status: submission.status,
        submission_id: submission.id,
      }),
    );
    console.log(`Signed, notarized, and stapled ${app} with ${identity}`);
  } finally {
    commandResult("security", ["delete-keychain", keychain]);
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

export function planDraftMutation(existing, expectedAssets) {
  if (existing === null) return "create";
  if (!existing.isDraft) throw new Error("release already exists and is not a draft");
  const actual = existing.assets.map((asset) => asset.name).sort();
  const expected = [...expectedAssets].sort();
  if (actual.length === 0) return "resume-empty";
  if (JSON.stringify(actual) === JSON.stringify(expected)) return "verify-existing";
  throw new Error(`draft asset set is partial or mismatched: found ${actual.join(", ") || "none"}`);
}

function releaseView(tag) {
  const result = commandResult("gh", ["release", "view", tag, "--json", "assets,isDraft,tagName,url"]);
  if (result.status !== 0) {
    if (/release not found|not found/i.test(`${result.stdout}${result.stderr}`)) return null;
    throw new Error(`unable to inspect existing release: ${result.stderr.trim()}`);
  }
  return JSON.parse(result.stdout);
}

function compareDownloadedAssets(directory, complete) {
  const downloaded = readCompleteArtifactSet(directory);
  if (JSON.stringify(downloaded.entries) !== JSON.stringify(complete.entries)) {
    throw new Error("downloaded draft asset set differs from the local complete set");
  }
  for (const filename of complete.entries) {
    const local = fs.readFileSync(path.join(complete.directory, filename));
    const remote = fs.readFileSync(path.join(directory, filename));
    if (!local.equals(remote)) throw new Error(`existing draft asset ${filename} differs from local bytes`);
  }
}

export function publishDraft({ directory, notesFile, root, tag }) {
  validateTagContext({ root, tag });
  const complete = readCompleteArtifactSet(directory);
  complete.directory = directory;
  if (complete.manifest.tag !== tag) throw new Error("complete release set tag does not match requested draft");
  const existing = releaseView(tag);
  const plan = planDraftMutation(existing, complete.entries);
  if (plan === "verify-existing") {
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "antennabench-existing-draft-"));
    try {
      capture("gh", ["release", "download", tag, "--dir", temporary], { timeout: 300_000 });
      compareDownloadedAssets(temporary, complete);
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
    console.log(existing.url);
    return;
  }
  const assets = complete.entries.map((filename) => path.join(directory, filename));
  if (plan === "create") {
    capture("gh", [
      "release",
      "create",
      tag,
      ...assets,
      "--draft",
      "--verify-tag",
      "--title",
      `${PRODUCT} ${complete.manifest.version}`,
      "--notes-file",
      notesFile,
    ], { timeout: 600_000 });
  } else {
    capture("gh", ["release", "upload", tag, ...assets], { timeout: 600_000 });
    capture("gh", ["release", "edit", tag, "--notes-file", notesFile], { timeout: 120_000 });
  }
  const created = releaseView(tag);
  if (planDraftMutation(created, complete.entries) !== "verify-existing") {
    throw new Error("draft release did not reach the exact complete asset state");
  }
  console.log(created.url);
}

export function writeReleaseNotes({ filename, root, tag }) {
  const context = validateTagContext({ root, tag });
  const repository = process.env.GITHUB_REPOSITORY ?? "rwjblue/antennabench";
  const text = `# ${PRODUCT} ${context.version}\n\n` +
    `Source: [${context.commit}](https://github.com/${repository}/commit/${context.commit})\n\n` +
    `This draft contains separate macOS 15+ archives for Apple silicon and Intel Macs. ` +
    `Download the ZIP matching your Mac, verify the checksums and GitHub attestation, then extract it and move ${PRODUCT}.app to Applications.\n\n` +
    "```sh\n" +
    `shasum -a 256 -c ${PRODUCT}-${context.version}-SHA256SUMS\n` +
    `gh attestation verify ${PRODUCT}-${context.version}-aarch64-apple-darwin.zip --repo ${repository}\n` +
    `gh attestation verify ${PRODUCT}-${context.version}-x86_64-apple-darwin.zip --repo ${repository}\n` +
    "```\n\n" +
    "Known limitations: macOS 15 or later is required; Windows, Linux, automatic updates, the Mac App Store, and package-manager installation are not included.\n\n" +
    "This is a private draft verification candidate. Stable publication requires explicit owner promotion after clean-system install, launch, and canonical open/report/export/reopen verification.\n";
  fs.mkdirSync(path.dirname(filename), { recursive: true });
  fs.writeFileSync(filename, text);
}

export async function verifyDraft({ directory, root, tag, target }) {
  validateTagContext({ root, tag });
  const existing = releaseView(tag);
  if (!existing?.isDraft) throw new Error("expected an existing draft release");
  fs.rmSync(directory, { recursive: true, force: true });
  fs.mkdirSync(directory, { recursive: true });
  capture("gh", ["release", "download", tag, "--dir", directory], { timeout: 300_000 });
  const complete = await verifyCompleteArtifacts({ directory, inspectTarget: target });
  if (complete.manifest.tag !== tag) throw new Error("downloaded release manifest tag mismatch");
  const repository = process.env.GITHUB_REPOSITORY;
  if (!repository) throw new Error("GITHUB_REPOSITORY is required for attestation verification");
  for (const filename of complete.entries) {
    capture("gh", ["attestation", "verify", path.join(directory, filename), "--repo", repository], {
      timeout: 300_000,
    });
  }
  console.log(`Verified downloaded draft ${tag} for ${target}`);
}

function parseArguments(argv) {
  const [command, ...rest] = argv;
  const options = { command };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (!key.startsWith("--")) throw new Error(`unexpected argument: ${key}`);
    options[key.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = rest[++index];
  }
  return options;
}

function requireOption(options, name) {
  if (!options[name]) throw new Error(`missing --${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`);
  return options[name];
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const root = process.cwd();
  if (options.command === "validate-tag") {
    validateTagContext({ root, tag: requireOption(options, "tag") });
  } else if (options.command === "prepare") {
    await prepareSigningInput({
      input: path.resolve(requireOption(options, "input")),
      output: path.resolve(requireOption(options, "output")),
      target: requireOption(options, "target"),
      tag: requireOption(options, "tag"),
      expectedCommit: requireOption(options, "expectedCommit"),
    });
  } else if (options.command === "sign") {
    await signAndNotarize({
      app: path.resolve(requireOption(options, "app")),
      evidenceDirectory: path.resolve(requireOption(options, "evidence")),
    });
  } else if (options.command === "notes") {
    writeReleaseNotes({ filename: path.resolve(requireOption(options, "output")), root, tag: requireOption(options, "tag") });
  } else if (options.command === "publish-draft") {
    publishDraft({
      directory: path.resolve(requireOption(options, "input")),
      notesFile: path.resolve(requireOption(options, "notes")),
      root,
      tag: requireOption(options, "tag"),
    });
  } else if (options.command === "verify-draft") {
    await verifyDraft({
      directory: path.resolve(requireOption(options, "output")),
      root,
      tag: requireOption(options, "tag"),
      target: requireOption(options, "target"),
    });
  } else {
    throw new Error("usage: desktop-publication.mjs validate-tag|prepare|sign|notes|publish-draft|verify-draft [options]");
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
