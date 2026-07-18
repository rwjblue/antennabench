import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const CONFIG_PATH = path.join(".github", "module-size-budget.json");
const TEST_CONFIGURATION = /^\s*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(.*)$/;
const INLINE_MODULE = /^(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/;

export function countNonTestSourceLines(text) {
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  const testModule = findInlineTestModule(lines);
  return testModule === -1 ? lines.length : testModule;
}

function findInlineTestModule(lines) {
  for (const [index, line] of lines.entries()) {
    const configured = line.match(TEST_CONFIGURATION);
    if (!configured) continue;

    let candidate = configured[1].trim();
    for (let next = index + 1; candidate === "" && next < lines.length; next += 1) {
      const trimmed = lines[next].trim();
      if (trimmed === "" || trimmed.startsWith("//") || /^#\s*\[/.test(trimmed)) continue;
      candidate = trimmed;
    }
    if (INLINE_MODULE.test(candidate)) return index;
  }
  return -1;
}

export function validateConfiguration(config) {
  const errors = [];
  if (config?.version !== 1) {
    errors.push("module-size configuration must use schema version 1");
  }
  if (!Number.isInteger(config?.default_budget) || config.default_budget < 1) {
    errors.push("module-size default_budget must be a positive integer");
  }
  if (!Number.isInteger(config?.allowlist_slack) || config.allowlist_slack < 0) {
    errors.push("module-size allowlist_slack must be a non-negative integer");
  }
  if (!isPlainObject(config?.allowlist)) {
    errors.push("module-size allowlist must be an object keyed by repository path");
    return errors;
  }
  for (const [file, budget] of Object.entries(config.allowlist)) {
    if (!isManagedRustPath(file)) {
      errors.push(`${file}: allowlist path must be a managed Rust source file`);
    }
    if (!Number.isInteger(budget) || budget < 1) {
      errors.push(`${file}: allowlist budget must be a positive integer`);
    }
    if (Number.isInteger(config.default_budget) && budget <= config.default_budget) {
      errors.push(`${file}: allowlist budget must exceed the default budget`);
    }
  }
  return errors;
}

export function validateModuleSizes(root, config) {
  const errors = validateConfiguration(config);
  if (errors.length > 0) return { errors, measurements: [] };

  const files = managedRustFiles(root);
  const allowlist = config.allowlist;
  const measurements = [];
  const seen = new Set();

  for (const absolute of files) {
    const file = slash(path.relative(root, absolute));
    const measured = countNonTestSourceLines(fs.readFileSync(absolute, "utf8"));
    const listed = Object.hasOwn(allowlist, file);
    const budget = listed ? allowlist[file] : config.default_budget;
    seen.add(file);
    measurements.push({ file, measured, budget, listed });

    if (measured > budget) {
      errors.push(
        `${file}: measured ${measured} non-test source lines; effective budget ${budget}. ` +
          "Decompose the module, or consciously raise its allowlist entry in review.",
      );
      continue;
    }

    if (listed && measured <= config.default_budget) {
      errors.push(
        `${file}: allowlist entry is no longer needed; measured ${measured} non-test source lines ` +
          `within the default budget ${config.default_budget}. Remove the entry.`,
      );
      continue;
    }

    if (listed && budget - measured > config.allowlist_slack) {
      errors.push(
        `${file}: stale allowlist budget ${budget} exceeds the measured ${measured} non-test ` +
          `source lines by more than the ${config.allowlist_slack}-line slack. ` +
          "Lower the allowlist entry so the budget ratchets down with the module.",
      );
    }
  }

  for (const file of Object.keys(allowlist).sort()) {
    if (!seen.has(file)) {
      errors.push(`${file}: stale allowlist entry does not match a managed Rust source file`);
    }
  }

  return { errors, measurements };
}

export function validateRepository(root) {
  const configPath = path.join(root, CONFIG_PATH);
  let config;
  try {
    config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  } catch (error) {
    return {
      errors: [`${slash(CONFIG_PATH)}: could not read module-size configuration: ${error.message}`],
      measurements: [],
    };
  }
  return validateModuleSizes(root, config);
}

function managedRustFiles(root) {
  const roots = [];
  const crates = path.join(root, "crates");
  if (fs.existsSync(crates)) {
    for (const entry of fs.readdirSync(crates, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const source = path.join(crates, entry.name, "src");
      if (fs.existsSync(source)) roots.push(source);
    }
  }
  const desktop = path.join(root, "apps", "desktop", "src");
  if (fs.existsSync(desktop)) roots.push(desktop);
  return roots.flatMap(walkRustFiles).sort((left, right) => left.localeCompare(right));
}

function walkRustFiles(directory) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...walkRustFiles(absolute));
    else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(absolute);
  }
  return files;
}

function isManagedRustPath(file) {
  if (typeof file !== "string" || file !== slash(file) || path.isAbsolute(file)) return false;
  return (
    /^crates\/[^/]+\/src\/(?:.+\/)*[^/]+\.rs$/.test(file) ||
    /^apps\/desktop\/src\/(?:.+\/)*[^/]+\.rs$/.test(file)
  );
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function slash(value) {
  return value.split(path.sep).join("/");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { errors, measurements } = validateRepository(process.cwd());
  if (errors.length > 0) {
    for (const error of errors) console.error(error);
    process.exitCode = 1;
  } else {
    const exceptions = measurements.filter((measurement) => measurement.listed).length;
    console.log(
      `Module-size budget is valid: ${measurements.length} Rust files checked; ` +
        `${exceptions} managed exceptions`,
    );
  }
}
