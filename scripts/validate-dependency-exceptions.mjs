import fs from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const ALLOWED_CATEGORIES = new Set([
  "vulnerability",
  "unsound",
  "notice",
  "unmaintained",
  "yanked",
  "license",
  "source",
]);
const SEVERITIES = new Set(["critical", "high", "moderate", "low", "informational"]);
const DATE = /^\d{4}-\d{2}-\d{2}$/;
const EXACT_VERSION = /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const ISSUE = /^https:\/\/github\.com\/rwjblue\/antennabench\/issues\/\d+$/;
const REQUIRED_LICENSES = [
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MPL-2.0",
  "Unicode-3.0",
  "Zlib",
];

export function validateExceptions(document, { today, lockText, denyText }) {
  const errors = [];
  if (document.version !== 1 || !Array.isArray(document.exceptions)) {
    return ["dependency exceptions must use schema version 1 with an exceptions array"];
  }

  const packages = lockedPackages(lockText);
  const ids = new Set();
  for (const [index, exception] of document.exceptions.entries()) {
    const at = `exceptions[${index}]`;
    if (!isText(exception.id)) errors.push(`${at}.id must be non-empty`);
    else if (ids.has(exception.id)) errors.push(`${at}.id must be unique`);
    else ids.add(exception.id);

    if (!ALLOWED_CATEGORIES.has(exception.category)) {
      errors.push(`${at}.category is unsupported and cannot be waived`);
    }
    if (!SEVERITIES.has(exception.severity)) errors.push(`${at}.severity is invalid`);
    if (!isText(exception.identity)) errors.push(`${at}.identity must be non-empty`);
    if (!isText(exception.package?.name)) errors.push(`${at}.package.name must be non-empty`);
    if (!EXACT_VERSION.test(exception.package?.version ?? "")) {
      errors.push(`${at}.package.version must be one exact semantic version`);
    } else if (!packages.has(`${exception.package.name}@${exception.package.version}`)) {
      errors.push(`${at} does not match a package in Cargo.lock`);
    }
    const packageReference = `${exception.package?.name}@${exception.package?.version}`;
    if (["vulnerability", "unsound", "notice", "unmaintained"].includes(exception.category)) {
      if (!/^RUSTSEC-\d{4}-\d{4}$/.test(exception.identity ?? "")) {
        errors.push(`${at}.identity must be a RustSec advisory ID`);
      }
      if (exception.enforcement_reference !== exception.identity) {
        errors.push(`${at}.enforcement_reference must match its advisory identity`);
      }
    } else if (["license", "yanked"].includes(exception.category)) {
      if (exception.enforcement_reference !== packageReference) {
        errors.push(`${at}.enforcement_reference must match its exact package`);
      }
    } else if (
      exception.category === "source" &&
      exception.enforcement_reference !== exception.identity
    ) {
      errors.push(`${at}.enforcement_reference must match its exact source`);
    }

    for (const field of ["reachability", "rationale", "mitigation"]) {
      if (!isText(exception[field]) || exception[field].trim().length < 20) {
        errors.push(`${at}.${field} must contain review evidence`);
      }
    }
    if (!/^@[A-Za-z0-9-]+$/.test(exception.owner ?? "")) {
      errors.push(`${at}.owner must name a GitHub owner`);
    }
    if (!ISSUE.test(exception.issue ?? "")) {
      errors.push(`${at}.issue must link a repository issue`);
    }
    if (!isText(exception.enforcement_reference)) {
      errors.push(`${at}.enforcement_reference must identify the deny.toml entry`);
    } else if (!denyText.includes(exception.enforcement_reference)) {
      errors.push(`${at}.enforcement_reference is not present in deny.toml`);
    }

    const approved = parseDate(exception.approved_on);
    const expires = parseDate(exception.expires_on);
    if (!approved) errors.push(`${at}.approved_on must be an ISO calendar date`);
    if (!expires) errors.push(`${at}.expires_on must be an ISO calendar date`);
    if (approved && expires) {
      const days = (expires - approved) / 86_400_000;
      const maximumDays =
        exception.category === "unsound" || ["critical", "high"].includes(exception.severity)
          ? 30
          : 90;
      if (days < 0 || days > maximumDays) {
        errors.push(`${at} exceeds its ${maximumDays}-day maximum lifetime`);
      }
      if (approved > today) errors.push(`${at}.approved_on cannot be in the future`);
      if (expires < today) errors.push(`${at} is expired`);
    }
  }
  const references = new Set(document.exceptions.map((exception) => exception.enforcement_reference));
  const advisorySection = denyText.match(/\[advisories\]([\s\S]*?)\n\[bans\]/)?.[1] ?? "";
  const policyReferences = new Set(advisorySection.match(/RUSTSEC-\d{4}-\d{4}/g) ?? []);
  for (const match of advisorySection.matchAll(/crate = "([^"]+)"/g)) policyReferences.add(match[1]);
  const licenseSection = denyText.match(/\[licenses\]([\s\S]*?)\n\[sources\]/)?.[1] ?? "";
  for (const match of licenseSection.matchAll(/crate = "([^"]+)"/g)) policyReferences.add(match[1]);
  const sourcesSection = denyText.match(/\[sources\]([\s\S]*)$/)?.[1] ?? "";
  const allowedGit = sourcesSection.match(/allow-git = \[([\s\S]*?)\]/)?.[1] ?? "";
  for (const match of allowedGit.matchAll(/"([^"]+)"/g)) policyReferences.add(match[1]);
  for (const reference of policyReferences) {
    if (!references.has(reference)) {
      errors.push(`deny.toml exception ${reference} has no tracked exception record`);
    }
  }
  return errors;
}

export function validateDenyPolicyText(text, source = "deny.toml") {
  const errors = [];
  const requirements = [
    [/^all-features = true$/m, "all features must be evaluated"],
    [/^yanked = "deny"$/m, "yanked packages must fail"],
    [/^unmaintained = "workspace"$/m, "direct unmaintained packages must fail"],
    [/^unsound = "all"$/m, "unsound advisories must fail throughout the graph"],
    [/^unused-ignored-advisory = "deny"$/m, "unused advisory exceptions must fail"],
    [/^multiple-versions = "warn"$/m, "duplicates must remain visible warnings"],
    [/^wildcards = "deny"$/m, "external wildcard requirements must fail"],
    [/^allow-wildcard-paths = true$/m, "workspace path wildcards must be allowed"],
    [/^unknown-registry = "deny"$/m, "unknown registries must fail"],
    [/^unknown-git = "deny"$/m, "unknown git sources must fail"],
    [/^allow-registry = \["https:\/\/github\.com\/rust-lang\/crates\.io-index"\]$/m, "only crates.io may be allowed"],
    [/^allow-git = \[\]$/m, "git sources must have no standing allowlist"],
    [/^unused-license-exception = "deny"$/m, "unused license exceptions must fail"],
    [/^unused-allowed-source = "deny"$/m, "unused source exceptions must fail"],
  ];
  for (const [pattern, message] of requirements) {
    if (!pattern.test(text)) errors.push(`${source}: ${message}`);
  }
  const licenseSection = text.match(/\[licenses\]([\s\S]*?)\n\[sources\]/)?.[1] ?? "";
  const allowList = licenseSection.match(/allow = \[([\s\S]*?)\]\nexceptions/)?.[1] ?? "";
  const configuredLicenses = new Set([...allowList.matchAll(/"([^"]+)"/g)].map((match) => match[1]));
  for (const license of REQUIRED_LICENSES) {
    if (!configuredLicenses.delete(license)) {
      errors.push(`${source}: missing reviewed license ${license}`);
    }
  }
  for (const license of configuredLicenses) {
    errors.push(`${source}: unreviewed global license ${license}`);
  }
  return errors;
}

export function validateFreshGate({ advisoryTask, releaseTask, workflow }) {
  const errors = [];
  if (!/^set -euo pipefail$/m.test(advisoryTask)) {
    errors.push("advisory-fresh: task must fail on every command error");
  }
  if (!/^cargo deny --locked check advisories$/m.test(advisoryTask)) {
    errors.push("advisory-fresh: task must run a locked advisory check");
  }
  if (/--(?:offline|disable-fetch)|\|\||\|\s*true|;\s*true/.test(advisoryTask)) {
    errors.push("advisory-fresh: fetch or check failures must not be suppressed");
  }
  for (const task of ["supply-chain", "dependency-policy", "advisory-fresh"]) {
    if (!releaseTask.includes(`mise run ${task}`)) {
      errors.push(`release-preflight: missing ${task}`);
    }
  }
  for (const trigger of ["pull_request", "push", "schedule", "workflow_dispatch", "workflow_call"]) {
    if (!new RegExp(`^  ${trigger}:`, "m").test(workflow)) {
      errors.push(`rust-supply-chain workflow: missing ${trigger} trigger`);
    }
  }
  if (!/^\s+- main$/m.test(workflow) || !/^\s+- cron: "\d+ \d+ \* \* \*"$/m.test(workflow)) {
    errors.push("rust-supply-chain workflow: main and daily cadence must be explicit");
  }
  for (const dependencyPath of ["Cargo.lock", "Cargo.toml"]) {
    if (!workflow.includes(`"${dependencyPath}"`)) {
      errors.push(`rust-supply-chain workflow: PR paths must include ${dependencyPath}`);
    }
  }
  if (!workflow.includes("run: mise run release-preflight")) {
    errors.push("rust-supply-chain workflow: must call the complete release preflight");
  }
  return errors;
}

function isText(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function parseDate(value) {
  if (!DATE.test(value ?? "")) return undefined;
  const parsed = new Date(`${value}T00:00:00Z`);
  return Number.isNaN(parsed.valueOf()) || parsed.toISOString().slice(0, 10) !== value
    ? undefined
    : parsed;
}

function lockedPackages(lockText) {
  const packages = new Set();
  for (const match of lockText.matchAll(/\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/g)) {
    packages.add(`${match[1]}@${match[2]}`);
  }
  return packages;
}

export function validateRepository(root, today = new Date()) {
  const denyText = fs.readFileSync(path.join(root, "deny.toml"), "utf8");
  const errors = validateExceptions(
    JSON.parse(fs.readFileSync(path.join(root, ".github", "dependency-exceptions.json"), "utf8")),
    {
      today: new Date(`${today.toISOString().slice(0, 10)}T00:00:00Z`),
      lockText: fs.readFileSync(path.join(root, "Cargo.lock"), "utf8"),
      denyText,
    },
  );
  errors.push(...validateDenyPolicyText(denyText));
  errors.push(
    ...validateFreshGate({
      advisoryTask: fs.readFileSync(path.join(root, ".mise", "tasks", "advisory-fresh"), "utf8"),
      releaseTask: fs.readFileSync(path.join(root, ".mise", "tasks", "release-preflight"), "utf8"),
      workflow: fs.readFileSync(
        path.join(root, ".github", "workflows", "rust-supply-chain.yml"),
        "utf8",
      ),
    }),
  );
  return errors;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const errors = validateRepository(process.cwd());
  if (errors.length > 0) {
    for (const error of errors) console.error(error);
    process.exitCode = 1;
  } else {
    console.log("Dependency exceptions are valid and unexpired");
  }
}
