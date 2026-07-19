import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const PRODUCT = "AntennaBench";
const BUNDLE_IDENTIFIER = "com.rwjblue.antennabench";
const MINIMUM_MACOS = "15.0";
const TARGETS = Object.freeze({
  "aarch64-apple-darwin": Object.freeze({
    architecture: "arm64",
    runner: "macos-15",
  }),
  "x86_64-apple-darwin": Object.freeze({
    architecture: "x86_64",
    runner: "macos-15-intel",
  }),
});
const TARGET_ORDER = Object.freeze(Object.keys(TARGETS));
const STABLE_SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function targetContract(target) {
  const contract = TARGETS[target];
  if (!contract) throw new Error(`unsupported release target: ${target}`);
  return contract;
}

export function assertStableVersion(version) {
  if (!STABLE_SEMVER.test(version)) {
    throw new Error(`release version must be stable MAJOR.MINOR.PATCH: ${version}`);
  }
  return version;
}

export function assertVersionTag(version, tag) {
  assertStableVersion(version);
  if (tag !== undefined && tag !== null && tag !== `v${version}`) {
    throw new Error(`release tag ${tag} does not match version ${version}`);
  }
}

export function resolveReleaseIdentityTag(version, tag) {
  assertStableVersion(version);
  const resolved = tag ?? `v${version}`;
  assertVersionTag(version, resolved);
  return resolved;
}

export function archiveName(version, target) {
  assertStableVersion(version);
  targetContract(target);
  return `${PRODUCT}-${version}-${target}.zip`;
}

export function canonicalJson(value) {
  return `${JSON.stringify(sortValue(value), null, 2)}\n`;
}

function sortValue(value) {
  if (Array.isArray(value)) return value.map(sortValue);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort((left, right) => Buffer.from(left).compare(Buffer.from(right)))
        .map((key) => [key, sortValue(value[key])]),
    );
  }
  return value;
}

export function checksumLines(entries) {
  return [...entries]
    .sort(([left], [right]) => Buffer.from(left).compare(Buffer.from(right)))
    .map(([filename, digest]) => `${digest}  ${filename}`)
    .join("\n")
    .concat("\n");
}

export function validateStagedEntries(directory, expected) {
  const actual = fs.readdirSync(directory).sort(bytewise);
  const wanted = [...expected].sort(bytewise);
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(
      `staged asset set mismatch: expected ${wanted.join(", ")}; found ${actual.join(", ")}`,
    );
  }
}

export async function withAtomicDirectory(finalDirectory, builder) {
  const parent = path.dirname(finalDirectory);
  fs.mkdirSync(parent, { recursive: true });
  fs.rmSync(finalDirectory, { recursive: true, force: true });
  const staging = fs.mkdtempSync(path.join(parent, ".staging-"));
  try {
    await builder(staging);
    fs.renameSync(staging, finalDirectory);
  } finally {
    fs.rmSync(staging, { recursive: true, force: true });
  }
}

function bytewise(left, right) {
  return Buffer.from(left).compare(Buffer.from(right));
}

function sha256File(filename) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filename));
  return hash.digest("hex");
}

function capture(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
    timeout: options.timeout ?? 30_000,
  });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
  if (result.error || result.status !== 0) {
    const reason = result.error?.message ?? `exit ${result.status}`;
    throw new Error(`${command} ${args.join(" ")} failed (${reason})${output ? `:\n${output}` : ""}`);
  }
  return output;
}

function captureResult(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
    timeout: options.timeout ?? 30_000,
  });
  return {
    ok: !result.error && result.status === 0,
    output: `${result.stdout ?? ""}${result.stderr ?? ""}`.trim(),
  };
}

async function runBounded(command, args, { cwd, env, timeoutMs, label }) {
  await new Promise((resolve, reject) => {
    const detached = process.platform !== "win32";
    const child = spawn(command, args, {
      cwd,
      env,
      detached,
      stdio: "inherit",
    });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      try {
        if (detached) process.kill(-child.pid, "SIGTERM");
        else child.kill("SIGTERM");
      } catch {
        // The process may have exited between the timer and the signal.
      }
      setTimeout(() => {
        try {
          if (detached) process.kill(-child.pid, "SIGKILL");
          else child.kill("SIGKILL");
        } catch {
          // The process group is already gone.
        }
      }, 5_000).unref();
    }, timeoutMs);

    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (timedOut) reject(new Error(`${label} exceeded ${timeoutMs / 1000}s and was terminated`));
      else if (code !== 0) reject(new Error(`${label} failed with ${signal ?? `exit ${code}`}`));
      else resolve();
    });
  });
}

function workspaceVersion(root) {
  const cargo = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const section = cargo.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/)?.[1] ?? "";
  const version = section.match(/^version = "([^"]+)"$/m)?.[1];
  if (!version) throw new Error("Cargo.toml workspace package version is missing");
  return assertStableVersion(version);
}

function readToolPins(root) {
  const mise = fs.readFileSync(path.join(root, ".mise", "config.toml"), "utf8");
  const pins = {
    cargo_deny: mise.match(/^"cargo:cargo-deny" = \{ version = "([^"]+)"/m)?.[1],
    cargo_tauri: mise.match(/^"cargo:tauri-cli" = \{ version = "([^"]+)"/m)?.[1],
    node: mise.match(/^node = "([^"]+)"$/m)?.[1],
    rust: mise.match(/^rust = \{ version = "([^"]+)"/m)?.[1],
  };
  for (const [tool, version] of Object.entries(pins)) {
    if (!version) throw new Error(`missing exact ${tool} pin in .mise/config.toml`);
  }
  return pins;
}

function validateTauriContract(root) {
  const config = JSON.parse(
    fs.readFileSync(path.join(root, "apps", "desktop", "tauri.conf.json"), "utf8"),
  );
  if (Object.hasOwn(config, "version")) {
    throw new Error("tauri.conf.json must inherit the Cargo workspace version");
  }
  if (config.productName !== PRODUCT || config.identifier !== BUNDLE_IDENTIFIER) {
    throw new Error("Tauri product name or bundle identifier does not match the release contract");
  }
  if (JSON.stringify(config.bundle?.targets) !== JSON.stringify(["app"])) {
    throw new Error('Tauri bundle targets must be exactly ["app"]');
  }
  if (config.bundle?.macOS?.minimumSystemVersion !== MINIMUM_MACOS) {
    throw new Error(`Tauri minimum macOS version must be ${MINIMUM_MACOS}`);
  }
}

function assertPinnedTool(actualOutput, expected, name) {
  const actual = actualOutput.match(/\d+\.\d+\.\d+/)?.[0];
  if (actual !== expected) throw new Error(`${name} mismatch: expected ${expected}, found ${actual}`);
  return actualOutput.split(/\r?\n/, 1)[0];
}

function sourceEvidence(root) {
  const commit = capture("git", ["rev-parse", "HEAD"], { cwd: root });
  if (process.env.GITHUB_SHA && process.env.GITHUB_SHA !== commit) {
    throw new Error(`GITHUB_SHA ${process.env.GITHUB_SHA} does not match checked-out commit ${commit}`);
  }
  const dirty = capture("git", ["status", "--porcelain"], { cwd: root }).length > 0;
  return { commit, dirty };
}

function validateHost(target, runnerLabel) {
  if (process.platform !== "darwin") throw new Error("desktop release artifacts require macOS");
  const contract = targetContract(target);
  const machine = capture("uname", ["-m"]);
  if (machine !== contract.architecture) {
    throw new Error(`native host architecture ${machine} does not match ${target}`);
  }
  if (runnerLabel !== "local" && runnerLabel !== contract.runner) {
    throw new Error(`runner ${runnerLabel} does not match ${target}; expected ${contract.runner}`);
  }
  if (process.env.RUNNER_ARCH) {
    const expectedRunnerArch = contract.architecture === "arm64" ? "ARM64" : "X64";
    if (process.env.RUNNER_ARCH !== expectedRunnerArch) {
      throw new Error(`RUNNER_ARCH ${process.env.RUNNER_ARCH} does not match ${target}`);
    }
  }
}

function plistValue(plist, key) {
  return capture("plutil", ["-extract", key, "raw", "-o", "-", plist]);
}

function inspectSignature(app, trustMode) {
  const details = captureResult("codesign", ["-d", "--verbose=4", app]);
  let classification = "unsigned";
  if (details.ok && /Signature=adhoc/.test(details.output)) classification = "ad-hoc";
  else if (details.ok && /Authority=Developer ID Application:/.test(details.output)) {
    classification = "developer-id";
  } else if (details.ok) classification = "other";

  if (trustMode === "release") {
    if (classification !== "developer-id") {
      throw new Error(`release mode requires Developer ID signing; found ${classification}`);
    }
    if (!/flags=.*runtime/.test(details.output)) {
      throw new Error("release signature is missing hardened runtime");
    }
    if (!/^Timestamp=/m.test(details.output) || /^Timestamp=none$/m.test(details.output)) {
      throw new Error("release signature is missing a secure timestamp");
    }
    capture("codesign", ["--verify", "--deep", "--strict", "--verbose=2", app]);
    capture("xcrun", ["stapler", "validate", app], { timeout: 120_000 });
    capture("spctl", ["--assess", "--type", "execute", "--verbose=4", app], {
      timeout: 120_000,
    });
  }

  return {
    authorities: [...details.output.matchAll(/^Authority=(.+)$/gm)].map((match) => match[1]),
    classification,
    gatekeeper: trustMode === "release" ? "accepted" : "not-checked-non-publishable",
    hardened_runtime: trustMode === "release",
    notarization: trustMode === "release" ? "stapled-and-validated" : "not-checked-non-publishable",
    publishable: trustMode === "release",
    secure_timestamp: trustMode === "release",
  };
}

function assertPublishableSignature(signature, source) {
  const requiredTrust = {
    classification: "developer-id",
    gatekeeper: "accepted",
    hardened_runtime: true,
    notarization: "stapled-and-validated",
    publishable: true,
    secure_timestamp: true,
  };
  for (const [field, expected] of Object.entries(requiredTrust)) {
    if (signature?.[field] !== expected) {
      throw new Error(`${source} publishable signature evidence has invalid ${field}`);
    }
  }
  if (!Array.isArray(signature.authorities) || !signature.authorities[0]?.startsWith("Developer ID Application:")) {
    throw new Error(`${source} publishable signature evidence is missing its Developer ID authority`);
  }
}

function inspectApp(app, { target, version, trustMode }) {
  const contract = targetContract(target);
  const plist = path.join(app, "Contents", "Info.plist");
  if (!fs.existsSync(plist)) throw new Error(`application Info.plist is missing: ${plist}`);
  const metadata = {
    build_version: plistValue(plist, "CFBundleVersion"),
    bundle_identifier: plistValue(plist, "CFBundleIdentifier"),
    executable: plistValue(plist, "CFBundleExecutable"),
    minimum_macos: plistValue(plist, "LSMinimumSystemVersion"),
    product_name: plistValue(plist, "CFBundleName"),
    short_version: plistValue(plist, "CFBundleShortVersionString"),
  };
  const expected = {
    build_version: version,
    bundle_identifier: BUNDLE_IDENTIFIER,
    minimum_macos: MINIMUM_MACOS,
    product_name: PRODUCT,
    short_version: version,
  };
  for (const [field, wanted] of Object.entries(expected)) {
    if (metadata[field] !== wanted) {
      throw new Error(`embedded ${field} mismatch: expected ${wanted}, found ${metadata[field]}`);
    }
  }

  const executable = path.join(app, "Contents", "MacOS", metadata.executable);
  if (!fs.existsSync(executable)) throw new Error(`application executable is missing: ${executable}`);
  const architectures = capture("lipo", ["-archs", executable]).split(/\s+/).filter(Boolean);
  if (architectures.length !== 1 || architectures[0] !== contract.architecture) {
    throw new Error(
      `executable architecture mismatch: expected ${contract.architecture}, found ${architectures.join(",")}`,
    );
  }
  const buildMetadata = capture("xcrun", ["vtool", "-show-build", executable]);
  const minimumVersions = [...buildMetadata.matchAll(/^\s*minos\s+([^\s]+)$/gm)].map(
    (match) => match[1],
  );
  if (minimumVersions.length === 0 || minimumVersions.some((value) => value !== MINIMUM_MACOS)) {
    throw new Error(
      `Mach-O minimum macOS mismatch: expected ${MINIMUM_MACOS}, found ${minimumVersions.join(",") || "none"}`,
    );
  }
  return {
    architecture: contract.architecture,
    metadata,
    signature: inspectSignature(app, trustMode),
  };
}

function buildInputs(root, target, runnerLabel, policy) {
  const pins = readToolPins(root);
  if (process.versions.node !== pins.node) {
    throw new Error(`Node mismatch: expected ${pins.node}, found ${process.versions.node}`);
  }
  const rustc = capture("rustc", ["--version"]);
  const cargoTauri = capture("cargo-tauri", ["--version"]);
  const cargoDeny = capture("cargo-deny", ["--version"]);
  assertPinnedTool(rustc, pins.rust, "rustc");
  assertPinnedTool(cargoTauri, pins.cargo_tauri, "cargo-tauri");
  assertPinnedTool(cargoDeny, pins.cargo_deny, "cargo-deny");
  return {
    cargo_deny: cargoDeny,
    cargo_lock_sha256: sha256File(path.join(root, "Cargo.lock")),
    cargo_tauri: cargoTauri,
    node: process.version,
    policy,
    runner: {
      image_os: process.env.ImageOS ?? null,
      image_version: process.env.ImageVersion ?? null,
      label: runnerLabel,
      os_version: capture("sw_vers", ["-productVersion"]),
    },
    rustc,
    target,
  };
}

async function stageApp({ root, app, target, tag, runnerLabel, trustMode, inputs }) {
  const version = workspaceVersion(root);
  assertVersionTag(version, tag);
  const source = sourceEvidence(root);
  if (trustMode === "release" && source.dirty) {
    throw new Error("publishable staging requires a clean source checkout");
  }
  const appEvidence = inspectApp(app, { target, version, trustMode });
  const outputRoot = path.join(root, "target", "desktop-release");
  const finalDirectory = path.join(
    outputRoot,
    appEvidence.signature.publishable ? "publishable" : "non-publishable",
    target,
  );
  const filename = archiveName(version, target);

  await withAtomicDirectory(finalDirectory, async (staging) => {
    const archive = path.join(staging, filename);
    capture("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, archive], {
      timeout: 300_000,
    });
    const extracted = path.join(staging, ".verify-extracted");
    fs.mkdirSync(extracted);
    try {
      capture("ditto", ["-x", "-k", archive, extracted], { timeout: 300_000 });
      validateStagedEntries(extracted, [`${PRODUCT}.app`]);
      const extractedEvidence = inspectApp(path.join(extracted, `${PRODUCT}.app`), {
        target,
        version,
        trustMode,
      });
      if (canonicalJson(extractedEvidence) !== canonicalJson(appEvidence)) {
        throw new Error("extracted archive evidence differs from the source application");
      }
    } finally {
      fs.rmSync(extracted, { recursive: true, force: true });
    }

    const artifact = {
      filename,
      sha256: sha256File(archive),
      size: fs.statSync(archive).size,
    };
    const manifest = {
      app: appEvidence,
      artifact,
      build_inputs: inputs,
      contract: {
        bundle_identifier: BUNDLE_IDENTIFIER,
        minimum_macos: MINIMUM_MACOS,
        product: PRODUCT,
        runner: targetContract(target).runner,
        target,
      },
      generated_at: new Date().toISOString(),
      publishable: appEvidence.signature.publishable,
      schema_version: 1,
      source,
      state: "complete",
      tag: tag ?? null,
      version,
    };
    fs.writeFileSync(path.join(staging, "artifact-manifest.json"), canonicalJson(manifest));
    const expected = [filename, "artifact-manifest.json"];
    if (!manifest.publishable) {
      fs.writeFileSync(
        path.join(staging, "NON_PUBLISHABLE.txt"),
        "This artifact is unsigned or ad-hoc and must not be attached to a GitHub Release.\n",
      );
      expected.push("NON_PUBLISHABLE.txt");
    }
    validateStagedEntries(staging, expected);
  });
  console.log(`Staged ${trustMode} ${target} artifact at ${finalDirectory}`);
  return finalDirectory;
}

async function buildTarget({ root, target, tag, runnerLabel }) {
  validateHost(target, runnerLabel);
  validateTauriContract(root);
  const version = workspaceVersion(root);
  const releaseIdentityTag = resolveReleaseIdentityTag(version, tag);
  if (process.env.GITHUB_REF_TYPE === "tag") {
    assertVersionTag(version, process.env.GITHUB_REF_NAME);
    if (releaseIdentityTag !== process.env.GITHUB_REF_NAME) {
      throw new Error(`requested tag ${releaseIdentityTag} does not match GITHUB_REF_NAME ${process.env.GITHUB_REF_NAME}`);
    }
  }

  await runBounded("mise", ["run", "toolchain"], {
    cwd: root,
    env: process.env,
    timeoutMs: 120_000,
    label: "toolchain preflight",
  });
  await runBounded("mise", ["run", "release-preflight"], {
    cwd: root,
    env: process.env,
    timeoutMs: 600_000,
    label: "release supply-chain preflight",
  });
  const policy = {
    command: "mise run release-preflight",
    status: "passed",
  };
  const inputs = buildInputs(root, target, runnerLabel, policy);
  const source = sourceEvidence(root);
  if (source.dirty) throw new Error("official release build requires a clean source checkout");

  await runBounded("rustup", ["target", "add", target], {
    cwd: root,
    env: process.env,
    timeoutMs: 300_000,
    label: `install Rust target ${target}`,
  });
  const app = path.join(
    root,
    "target",
    target,
    "release",
    "bundle",
    "macos",
    `${PRODUCT}.app`,
  );
  fs.rmSync(app, { recursive: true, force: true });
  await runBounded(
    "cargo-tauri",
    ["build", "--target", target, "--bundles", "app", "--ci", "--no-sign"],
    {
      cwd: path.join(root, "apps", "desktop"),
      env: {
        ...process.env,
        MACOSX_DEPLOYMENT_TARGET: MINIMUM_MACOS,
        ANTENNABENCH_BUILD_CHANNEL: "official_release",
        ANTENNABENCH_SOURCE_COMMIT: source.commit,
        ANTENNABENCH_SOURCE_STATE: "clean",
        ANTENNABENCH_RELEASE_TAG: releaseIdentityTag,
        ANTENNABENCH_TARGET_TRIPLE: target,
        ANTENNABENCH_BUILD_ARCHITECTURE: target.split("-", 1)[0],
      },
      timeoutMs: 1_800_000,
      label: `release build for ${target}`,
    },
  );
  if (!fs.existsSync(app)) throw new Error(`Tauri did not produce ${app}`);
  return stageApp({
    root,
    app,
    target,
    tag: releaseIdentityTag,
    runnerLabel,
    trustMode: "local",
    inputs,
  });
}

export function readTargetManifest(directory) {
  const filename = path.join(directory, "artifact-manifest.json");
  const manifest = JSON.parse(fs.readFileSync(filename, "utf8"));
  if (manifest.schema_version !== 1 || manifest.state !== "complete") {
    throw new Error(`${filename} is not a complete schema-v1 artifact manifest`);
  }
  targetContract(manifest.contract?.target);
  assertVersionTag(manifest.version, manifest.tag);
  const expectedName = archiveName(manifest.version, manifest.contract.target);
  if (manifest.artifact?.filename !== expectedName) {
    throw new Error(`${filename} contains an untruthful artifact filename`);
  }
  const signature = manifest.app?.signature;
  if (manifest.publishable !== signature?.publishable) {
    throw new Error(`${filename} publishable state disagrees with its signature evidence`);
  }
  if (manifest.publishable) {
    assertPublishableSignature(signature, filename);
    if (manifest.source?.dirty !== false || manifest.tag === null) {
      throw new Error(`${filename} publishable input requires a clean tagged source`);
    }
  }
  const archive = path.join(directory, expectedName);
  if (!fs.existsSync(archive)) throw new Error(`${archive} is missing`);
  if (
    fs.statSync(archive).size !== manifest.artifact.size ||
    sha256File(archive) !== manifest.artifact.sha256
  ) {
    throw new Error(`${archive} does not match its artifact manifest`);
  }
  const expectedEntries = [expectedName, "artifact-manifest.json"];
  if (!manifest.publishable) expectedEntries.push("NON_PUBLISHABLE.txt");
  validateStagedEntries(directory, expectedEntries);
  return { directory, manifest };
}

export function readCompleteArtifactSet(directory) {
  const entries = fs.readdirSync(directory).sort(bytewise);
  const releaseManifestName = entries.find((entry) => entry.endsWith("-release-manifest.json"));
  const checksumName = entries.find((entry) => entry.endsWith("-SHA256SUMS"));
  if (!releaseManifestName || !checksumName) {
    throw new Error("complete release set is missing its manifest or SHA256SUMS file");
  }
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, releaseManifestName), "utf8"));
  if (
    manifest.schema_version !== 1 ||
    manifest.state !== "complete" ||
    manifest.publishable !== true
  ) {
    throw new Error(`${releaseManifestName} is not a complete publishable schema-v1 release manifest`);
  }
  assertVersionTag(manifest.version, manifest.tag);
  if (manifest.product !== PRODUCT || manifest.bundle_identifier !== BUNDLE_IDENTIFIER) {
    throw new Error(`${releaseManifestName} does not match the application release contract`);
  }
  if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length !== TARGET_ORDER.length) {
    throw new Error(`${releaseManifestName} must contain exactly ${TARGET_ORDER.length} artifacts`);
  }
  const targets = manifest.artifacts.map((artifact) => artifact.target).sort(bytewise);
  if (JSON.stringify(targets) !== JSON.stringify([...TARGET_ORDER].sort(bytewise))) {
    throw new Error(`${releaseManifestName} does not contain the exact native target set`);
  }
  const expectedArchives = manifest.artifacts.map((artifact) => archiveName(manifest.version, artifact.target));
  for (const artifact of manifest.artifacts) {
    if (artifact.filename !== archiveName(manifest.version, artifact.target)) {
      throw new Error(`${releaseManifestName} contains an invalid artifact filename`);
    }
    const filename = path.join(directory, artifact.filename);
    if (!fs.existsSync(filename)) throw new Error(`${artifact.filename} is missing`);
    if (fs.statSync(filename).size !== artifact.size || sha256File(filename) !== artifact.sha256) {
      throw new Error(`${artifact.filename} does not match the release manifest`);
    }
    assertPublishableSignature(artifact.app?.signature, releaseManifestName);
    if (
      artifact.app?.metadata?.short_version !== manifest.version ||
      artifact.app?.metadata?.build_version !== manifest.version
    ) {
      throw new Error(`${releaseManifestName} artifact version evidence does not match the release`);
    }
  }
  validateStagedEntries(directory, [...expectedArchives, releaseManifestName, checksumName]);

  const checksumText = fs.readFileSync(path.join(directory, checksumName), "utf8");
  const expectedChecksums = checksumLines([
    ...manifest.artifacts.map((artifact) => [artifact.filename, artifact.sha256]),
    [releaseManifestName, sha256File(path.join(directory, releaseManifestName))],
  ]);
  if (checksumText !== expectedChecksums) {
    throw new Error(`${checksumName} does not exactly match the final release bytes`);
  }
  return { checksumName, entries, manifest, releaseManifestName };
}

export async function verifyCompleteArtifacts({ directory, inspectTarget }) {
  const complete = readCompleteArtifactSet(directory);
  if (inspectTarget) {
    const contract = targetContract(inspectTarget);
    validateHost(inspectTarget, contract.runner);
    const artifact = complete.manifest.artifacts.find((candidate) => candidate.target === inspectTarget);
    const extracted = fs.mkdtempSync(path.join(path.dirname(directory), ".verify-release-"));
    try {
      capture("ditto", ["-x", "-k", path.join(directory, artifact.filename), extracted], {
        timeout: 300_000,
      });
      validateStagedEntries(extracted, [`${PRODUCT}.app`]);
      const evidence = inspectApp(path.join(extracted, `${PRODUCT}.app`), {
        target: inspectTarget,
        version: complete.manifest.version,
        trustMode: "release",
      });
      if (canonicalJson(evidence) !== canonicalJson(artifact.app)) {
        throw new Error(`${artifact.filename} final application evidence differs from its manifest`);
      }
    } finally {
      fs.rmSync(extracted, { recursive: true, force: true });
    }
  }
  console.log(`Verified complete release set at ${directory}`);
  return complete;
}

export async function assembleArtifacts({ root, inputs, requirePublishable = false }) {
  const records = inputs.map(readTargetManifest);
  if (records.length !== TARGET_ORDER.length) {
    throw new Error(`release assembly requires exactly ${TARGET_ORDER.length} target artifacts`);
  }
  records.sort((left, right) =>
    TARGET_ORDER.indexOf(left.manifest.contract.target) -
    TARGET_ORDER.indexOf(right.manifest.contract.target),
  );
  if (records.map((record) => record.manifest.contract.target).join(",") !== TARGET_ORDER.join(",")) {
    throw new Error(`release assembly requires targets ${TARGET_ORDER.join(", ")}`);
  }
  for (const field of ["version", "tag"]) {
    const values = new Set(records.map((record) => record.manifest[field]));
    if (values.size !== 1) throw new Error(`target artifact ${field} values disagree`);
  }
  const commits = new Set(records.map((record) => record.manifest.source.commit));
  if (commits.size !== 1) throw new Error("target artifact source commits disagree");
  const publishable = records.every((record) => record.manifest.publishable === true);
  if (requirePublishable && !publishable) {
    throw new Error("release assembly requires signed, notarized, publishable target artifacts");
  }

  const version = records[0].manifest.version;
  const outputRoot = path.join(root, "target", "desktop-release");
  const finalDirectory = path.join(
    outputRoot,
    publishable ? "publishable" : "non-publishable",
    "complete",
  );
  await withAtomicDirectory(finalDirectory, async (staging) => {
    const artifacts = [];
    for (const record of records) {
      const { filename } = record.manifest.artifact;
      fs.copyFileSync(path.join(record.directory, filename), path.join(staging, filename));
      artifacts.push({
        ...record.manifest.artifact,
        app: record.manifest.app,
        build_inputs: record.manifest.build_inputs,
        target: record.manifest.contract.target,
      });
    }
    const releaseManifestName = `${PRODUCT}-${version}-release-manifest.json`;
    const releaseManifest = {
      artifacts,
      bundle_identifier: BUNDLE_IDENTIFIER,
      minimum_macos: MINIMUM_MACOS,
      product: PRODUCT,
      publishable,
      schema_version: 1,
      source_commit: records[0].manifest.source.commit,
      state: "complete",
      tag: records[0].manifest.tag,
      version,
    };
    fs.writeFileSync(path.join(staging, releaseManifestName), canonicalJson(releaseManifest));
    const sumEntries = artifacts.map((artifact) => [artifact.filename, artifact.sha256]);
    sumEntries.push([
      releaseManifestName,
      sha256File(path.join(staging, releaseManifestName)),
    ]);
    const checksumName = `${PRODUCT}-${version}-SHA256SUMS`;
    fs.writeFileSync(path.join(staging, checksumName), checksumLines(sumEntries));
    for (const [filename, digest] of sumEntries) {
      if (sha256File(path.join(staging, filename)) !== digest) {
        throw new Error(`self-verification failed for ${filename}`);
      }
    }
    const expected = [...artifacts.map((artifact) => artifact.filename), releaseManifestName, checksumName];
    if (!publishable) {
      fs.writeFileSync(
        path.join(staging, "NON_PUBLISHABLE.txt"),
        "This assembled set contains unsigned or ad-hoc applications and must not be published.\n",
      );
      expected.push("NON_PUBLISHABLE.txt");
    }
    validateStagedEntries(staging, expected);
  });
  console.log(`Assembled ${publishable ? "publishable" : "non-publishable"} release set at ${finalDirectory}`);
  return finalDirectory;
}

function parseArguments(argv) {
  const [command, ...rest] = argv;
  const options = { command, inputs: [] };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (key === "--require-publishable") options.requirePublishable = true;
    else if (key === "--input") options.inputs.push(rest[++index]);
    else if (key.startsWith("--")) options[camelCase(key.slice(2))] = rest[++index];
    else throw new Error(`unexpected argument: ${key}`);
  }
  return options;
}

function camelCase(value) {
  return value.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
}

function requireOption(options, name) {
  if (!options[name]) throw new Error(`missing --${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`);
  return options[name];
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const root = process.cwd();
  if (options.command === "build") {
    await buildTarget({
      root,
      target: requireOption(options, "target"),
      tag: options.tag,
      runnerLabel: options.runnerLabel ?? "local",
    });
  } else if (options.command === "stage") {
    validateTauriContract(root);
    const target = requireOption(options, "target");
    const runnerLabel = options.runnerLabel ?? "local";
    const trustMode = options.trustMode ?? "local";
    if (!["local", "release"].includes(trustMode)) throw new Error(`unsupported trust mode: ${trustMode}`);
    validateHost(target, runnerLabel);
    const inputs = buildInputs(root, target, runnerLabel, {
      command: "external credentialed publication preflight",
      status: trustMode === "release" ? "passed-by-caller" : "not-required-local-restage",
    });
    await stageApp({
      root,
      app: path.resolve(requireOption(options, "app")),
      target,
      tag: options.tag,
      runnerLabel,
      trustMode,
      inputs,
    });
  } else if (options.command === "assemble") {
    await assembleArtifacts({
      root,
      inputs: options.inputs.map((input) => path.resolve(input)),
      requirePublishable: options.requirePublishable,
    });
  } else if (options.command === "verify") {
    await verifyCompleteArtifacts({
      directory: path.resolve(requireOption(options, "input")),
      inspectTarget: options.target,
    });
  } else {
    throw new Error("usage: desktop-release.mjs build|stage|assemble|verify [options]");
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
