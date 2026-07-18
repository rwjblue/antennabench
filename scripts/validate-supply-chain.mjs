import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const FULL_SHA = /^[0-9a-f]{40}$/;
const RELEASE_COMMENT = /^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const CONTAINER_DIGEST = /@sha256:[0-9a-f]{64}$/;
const KNOWN_MANIFESTS = new Set([
  "Cargo.toml",
  "package.json",
  "pnpm-workspace.yaml",
  "pyproject.toml",
  "requirements.txt",
  "go.mod",
  "Gemfile",
]);

export function validateUsesText(text, source = "workflow") {
  const errors = [];
  for (const [index, line] of text.split(/\r?\n/).entries()) {
    const uses = line.match(/^\s*-?\s*uses:\s*([^\s#]+)(?:\s+#\s*(\S.*?))?\s*$/);
    if (uses) {
      const reference = uses[1];
      const comment = uses[2];
      if (reference.startsWith("./")) continue;
      if (reference.startsWith("docker://")) {
        if (!CONTAINER_DIGEST.test(reference)) {
          errors.push(`${source}:${index + 1}: container action must use a sha256 digest`);
        }
        continue;
      }
      const at = reference.lastIndexOf("@");
      const target = at === -1 ? "" : reference.slice(at + 1);
      if (!FULL_SHA.test(target)) {
        errors.push(`${source}:${index + 1}: external uses reference must use a full 40-hex SHA`);
      }
      if (!comment || !RELEASE_COMMENT.test(comment)) {
        errors.push(`${source}:${index + 1}: pinned uses reference needs a same-line release tag comment`);
      }
    }

    const image = line.match(/^\s*image:\s*([^\s#]+)/);
    if (image && !CONTAINER_DIGEST.test(image[1])) {
      errors.push(`${source}:${index + 1}: workflow container image must use a sha256 digest`);
    }
    if (/^\s*(?:runs-on|os):\s*.*-latest\s*(?:#.*)?$/.test(line)) {
      errors.push(`${source}:${index + 1}: routine runner must use a dated GA label`);
    }
  }
  return errors;
}

export function validateManifestCoverage(manifests, policy) {
  const errors = [];
  for (const manifest of manifests) {
    const covered = policy.ecosystems.some((ecosystem) =>
      ecosystem.manifest_globs.some((glob) => globRegex(glob).test(manifest)),
    );
    if (!covered) errors.push(`${manifest}: dependency manifest has no maintenance policy entry`);
  }
  return errors;
}

export function validateDependabotText(text, source = ".github/dependabot.yml") {
  const errors = [];
  for (const ecosystem of ["cargo", "github-actions", "npm"]) {
    const block = text
      .split(/\n(?=  - package-ecosystem:)/)
      .find((candidate) => candidate.includes(`package-ecosystem: ${ecosystem}`));
    if (!block) {
      errors.push(`${source}: missing ${ecosystem} updates`);
      continue;
    }
    if (!/^\s+interval:\s*weekly\s*$/m.test(block)) {
      errors.push(`${source}: ${ecosystem} updates must run weekly`);
    }
    if (!/^\s+open-pull-requests-limit:\s*5\s*$/m.test(block)) {
      errors.push(`${source}: ${ecosystem} updates must cap open pull requests at five`);
    }
    if (!/^\s+applies-to:\s*version-updates\s*$/m.test(block)) {
      errors.push(`${source}: ${ecosystem} routine groups must exclude security updates`);
    }
    for (const updateType of ["minor", "patch"]) {
      if (!new RegExp(`^\\s+- ${updateType}\\s*$`, "m").test(block)) {
        errors.push(`${source}: ${ecosystem} routine group must include ${updateType} updates`);
      }
    }
    if (/^\s+- major\s*$/m.test(block)) {
      errors.push(`${source}: ${ecosystem} major updates must remain individual`);
    }
  }
  return errors;
}

export function validateNpmWorkspace(manifests, lockfiles, lock) {
  const errors = [];
  const expectedWorkspaces = ["apps/desktop", "apps/hosted"];
  const root = manifests["package.json"];
  if (!root) return ["package.json: root npm workspace manifest is missing"];
  if (root.private !== true) errors.push("package.json: root npm workspace must be private");
  if (JSON.stringify(root.workspaces) !== JSON.stringify(expectedWorkspaces)) {
    errors.push("package.json: npm workspaces must explicitly contain apps/desktop and apps/hosted");
  }
  if (lockfiles.length !== 1 || lockfiles[0] !== "package-lock.json") {
    errors.push("npm workspace must have exactly one root package-lock.json and no nested lockfiles");
  }
  if (!lock?.packages) return [...errors, "package-lock.json: npm lockfile packages map is missing"];

  for (const [manifestPath, manifest] of Object.entries(manifests)) {
    const lockKey = manifestPath === "package.json" ? "" : path.posix.dirname(manifestPath);
    const locked = lock.packages[lockKey];
    if (!locked) {
      errors.push(`${manifestPath}: workspace is missing from package-lock.json`);
      continue;
    }
    for (const section of ["dependencies", "devDependencies", "optionalDependencies"]) {
      const declared = manifest[section] ?? {};
      for (const [name, version] of Object.entries(declared)) {
        if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
          errors.push(`${manifestPath}: ${section}.${name} must use an exact reviewed version`);
        }
      }
      if (!sameRecord(declared, locked[section] ?? {})) {
        errors.push(`${manifestPath}: ${section} disagrees with package-lock.json`);
      }
    }
  }
  return errors;
}

export function validateDependencyReviewText(
  text,
  source = ".github/workflows/dependency-review.yml",
) {
  const errors = [];
  const onBlock = text.match(/^on:\s*\n((?: {2}.*(?:\n|$))*)/m)?.[1] ?? "";
  const triggers = [...onBlock.matchAll(/^ {2}([\w-]+):/gm)].map((match) => match[1]);
  if (triggers.length !== 1 || triggers[0] !== "pull_request") {
    errors.push(`${source}: dependency review must run only on pull requests`);
  }
  if (!/uses:\s*actions\/dependency-review-action@[0-9a-f]{40}\s+#\s+v\S+/.test(text)) {
    errors.push(`${source}: missing immutable dependency-review action`);
  }
  if (!/^\s+fail-on-severity:\s*moderate\s*$/m.test(text)) {
    errors.push(`${source}: dependency review must fail at moderate severity`);
  }
  return errors;
}

export function validateReleaseWorkflowText(
  text,
  source = ".github/workflows/desktop-release.yml",
) {
  const errors = [];
  const required = [
    [/^  push:\s*\n    tags:\s*\n      - "v\*"\s*$/m, "must trigger only from v* tag pushes"],
    [/^  sign:\s*$/m, "must have an isolated signing job"],
    [/^    environment: desktop-release\s*$/m, "signing must use the desktop-release environment"],
    [/mise run desktop:release-assemble[\s\S]*?--require-publishable/, "must require publishable target manifests"],
    [/uses: actions\/attest@[0-9a-f]{40}\s+#\s+v\S+/, "must generate immutable GitHub attestations"],
    [/^      id-token: write\s*$/m, "attestation job must have OIDC write permission"],
    [/^      attestations: write\s*$/m, "attestation job must have attestation write permission"],
    [/^      contents: write\s*$/m, "one release mutation job must have contents write permission"],
    [/mise run desktop:publication-publish-draft/, "must use the fail-closed draft publication task"],
    [/mise run desktop:publication-verify-draft/, "must re-download and verify draft bytes"],
  ];
  for (const [pattern, message] of required) {
    if (!pattern.test(text)) errors.push(`${source}: ${message}`);
  }
  for (const forbidden of ["pull_request:", "workflow_dispatch:", "--clobber", "gh release publish", "--prerelease"]) {
    if (text.includes(forbidden)) errors.push(`${source}: forbidden release path token ${forbidden}`);
  }
  if ((text.match(/^    environment: desktop-release\s*$/gm) ?? []).length !== 1) {
    errors.push(`${source}: only the signing job may receive the protected environment`);
  }
  if ((text.match(/^      contents: write\s*$/gm) ?? []).length !== 1) {
    errors.push(`${source}: contents write must be limited to one draft-publication job`);
  }
  if ((text.match(/^      id-token: write\s*$/gm) ?? []).length !== 1) {
    errors.push(`${source}: OIDC write must be limited to one attestation job`);
  }
  const secretNames = [...text.matchAll(/secrets\.([A-Z0-9_]+)/g)].map((match) => match[1]);
  const expectedSecrets = [
    "APPLE_API_ISSUER",
    "APPLE_API_KEY",
    "APPLE_API_PRIVATE_KEY",
    "APPLE_CERTIFICATE",
    "APPLE_CERTIFICATE_PASSWORD",
  ];
  if (JSON.stringify([...new Set(secretNames)].sort()) !== JSON.stringify(expectedSecrets)) {
    errors.push(`${source}: protected signing secret inventory is not exact`);
  }
  const signStart = text.search(/^  sign:\s*$/m);
  const nextJob = signStart === -1
    ? null
    : [...text.matchAll(/^  [a-z][a-z0-9_-]*:\s*$/gm)].find((match) => match.index > signStart);
  const signEnd = nextJob?.index ?? -1;
  if (signStart === -1 || signEnd <= signStart || /secrets\./.test(text.slice(0, signStart) + text.slice(signEnd))) {
    errors.push(`${source}: Apple secrets must be referenced only inside the signing job`);
  }
  return errors;
}

export function validateHostedSiteDeployWorkflowText(
  text,
  source = ".github/workflows/hosted-site-deploy.yml",
) {
  const errors = [];
  const required = [
    [/^  push:\s*\n    branches: \[main\]\s*$/m, "must deploy pushes only from main"],
    [/^  workflow_dispatch:\s*$/m, "must support reviewed-revision redeployment"],
    [/^      source_revision:\s*$/m, "must require an explicit rollback source revision"],
    [/^        required: true\s*$/m, "rollback source revision must be required"],
    [/^    environment:\s*\n      name: production\s*$/m, "deploy job must use the production environment"],
    [/git merge-base --is-ancestor "\$GITHUB_SHA" origin\/main/, "must reject source outside main history"],
    [/mise run hosted:test/, "must validate the locked hosted workspace before deploy"],
    [/npm run deploy:site --workspace @antennabench\/hosted/, "must use the hosted workspace deploy command"],
  ];
  for (const [pattern, message] of required) {
    if (!pattern.test(text)) errors.push(`${source}: ${message}`);
  }
  if (text.includes("pull_request:")) {
    errors.push(`${source}: production deployment must not run for pull requests`);
  }
  if ((text.match(/^    environment:\s*$/gm) ?? []).length !== 1) {
    errors.push(`${source}: exactly one job may receive the production environment`);
  }
  const secretNames = [...text.matchAll(/secrets\.([A-Z0-9_]+)/g)].map((match) => match[1]);
  const expectedSecrets = ["CLOUDFLARE_ACCOUNT_ID", "CLOUDFLARE_API_TOKEN"];
  if (JSON.stringify([...new Set(secretNames)].sort()) !== JSON.stringify(expectedSecrets)) {
    errors.push(`${source}: Cloudflare secret inventory is not exact`);
  }
  const deployStart = text.indexOf("- name: Deploy static site");
  if (deployStart === -1 || /secrets\./.test(text.slice(0, deployStart))) {
    errors.push(`${source}: Cloudflare secrets must be scoped only to the final deploy step`);
  }
  return errors;
}

export function validateRepository(root) {
  const errors = [];
  const workflowRoot = path.join(root, ".github", "workflows");
  for (const file of fs.readdirSync(workflowRoot).filter((name) => /\.ya?ml$/.test(name)).sort()) {
    const relative = `.github/workflows/${file}`;
    const text = fs.readFileSync(path.join(workflowRoot, file), "utf8");
    errors.push(...validateUsesText(text, relative));
    if (!/^permissions:\s*\n\s{2}contents:\s*read\s*$/m.test(text)) {
      errors.push(`${relative}: workflow must declare top-level contents: read`);
    }
    if (file === "desktop-release.yml") {
      errors.push(...validateReleaseWorkflowText(text, relative));
    } else if (file === "hosted-site-deploy.yml") {
      errors.push(...validateHostedSiteDeployWorkflowText(text, relative));
    } else if (/secrets\./.test(text)) {
      errors.push(`${relative}: ordinary workflow must not reference repository secrets`);
    }
  }

  const dependabotPath = path.join(root, ".github", "dependabot.yml");
  const dependabot = fs.readFileSync(dependabotPath, "utf8");
  errors.push(...validateDependabotText(dependabot));

  const dependencyReviewPath = path.join(workflowRoot, "dependency-review.yml");
  const dependencyReview = fs.readFileSync(dependencyReviewPath, "utf8");
  errors.push(...validateDependencyReviewText(dependencyReview));

  const policy = JSON.parse(
    fs.readFileSync(path.join(root, ".github", "dependency-policy.json"), "utf8"),
  );
  const manifests = walk(root)
    .filter((file) => KNOWN_MANIFESTS.has(path.basename(file)))
    .map((file) => path.relative(root, file).split(path.sep).join("/"))
    .sort();
  errors.push(...validateManifestCoverage(manifests, policy));
  const packageManifests = Object.fromEntries(
    manifests
      .filter((file) => path.basename(file) === "package.json")
      .map((file) => [file, JSON.parse(fs.readFileSync(path.join(root, file), "utf8"))]),
  );
  const npmLockfiles = walk(root)
    .filter((file) => path.basename(file) === "package-lock.json")
    .map((file) => path.relative(root, file).split(path.sep).join("/"))
    .sort();
  const rootNpmLock = fs.existsSync(path.join(root, "package-lock.json"))
    ? JSON.parse(fs.readFileSync(path.join(root, "package-lock.json"), "utf8"))
    : null;
  errors.push(...validateNpmWorkspace(packageManifests, npmLockfiles, rootNpmLock));
  for (const ecosystem of policy.ecosystems) {
    if (!fs.existsSync(path.join(root, ecosystem.lockfile))) {
      errors.push(`${ecosystem.name}: declared lockfile ${ecosystem.lockfile} is missing`);
    }
    const [, packageEcosystem, directory] = ecosystem.update.split(":");
    if (
      !dependabot.includes(`package-ecosystem: ${packageEcosystem}`) ||
      !dependabot.includes(`directory: ${directory}`)
    ) {
      errors.push(`${ecosystem.name}: declared update mechanism is missing from Dependabot`);
    }
    if (!ecosystem.policy?.trim()) {
      errors.push(`${ecosystem.name}: policy description must not be empty`);
    }
  }

  const mise = fs.readFileSync(path.join(root, ".mise", "config.toml"), "utf8");
  for (const pattern of [
    /^node = "\d+\.\d+\.\d+"$/m,
    /^rust = \{ version = "\d+\.\d+\.\d+", components = \["rustfmt", "clippy"\] \}$/m,
    /^"cargo:cargo-deny" = \{ version = "\d+\.\d+\.\d+", locked = true \}$/m,
    /^"cargo:tauri-cli" = \{ version = "\d+\.\d+\.\d+", locked = true \}$/m,
  ]) {
    if (!pattern.test(mise)) errors.push(".mise/config.toml: tools must use exact reviewed pins");
  }
  const ci = fs.readFileSync(path.join(workflowRoot, "ci.yml"), "utf8");
  if (
    !/uses:\s*jdx\/mise-action@[0-9a-f]{40}\s+#\s+v\S+\n\s+with:\n\s+version:\s*\d+\.\d+\.\d+/.test(
      ci,
    )
  ) {
    errors.push(".github/workflows/ci.yml: Mise must use an exact reviewed release");
  }
  return errors;
}

function globRegex(glob) {
  const escaped = glob.replace(/[.+^${}()|[\]\\]/g, "\\$&").replaceAll("*", "[^/]*");
  return new RegExp(`^${escaped}$`);
}

function sameRecord(left, right) {
  const normalize = (record) => Object.fromEntries(Object.entries(record).sort(([a], [b]) => a.localeCompare(b)));
  return JSON.stringify(normalize(left)) === JSON.stringify(normalize(right));
}

function walk(root) {
  const output = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    if ([".git", "target", "node_modules"].includes(entry.name)) continue;
    const full = path.join(root, entry.name);
    if (entry.isDirectory()) output.push(...walk(full));
    else output.push(full);
  }
  return output;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = process.cwd();
  const errors = validateRepository(root);
  if (errors.length > 0) {
    for (const error of errors) console.error(error);
    process.exitCode = 1;
  } else {
    console.log("Supply-chain pins and maintenance coverage are valid");
  }
}
