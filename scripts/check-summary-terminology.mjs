import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const tracked = spawnSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { encoding: "utf8" },
);
if (tracked.status !== 0) {
  process.stderr.write(tracked.stderr);
  process.exit(tracked.status ?? 1);
}

const forbidden = [
  /Compact Summary/gu,
  /[Cc]ompact summary/gu,
  /[Cc]ompact (?:report|share)/gu,
  /compact-summary/gu,
  /compact_summary/gu,
  /CompactSummary/gu,
  /render_compact/gu,
  /report-compact\.css/gu,
  /\/sample-report\/compact\//gu,
];
const allowlist = new Map([
  ["crates/report/examples/render_canonical_sample.rs", new Set(["compact-summary"])],
  ["docs/reading-your-report.md", new Set(["/sample-report/compact/"])],
  ["scripts/validate-hosted-site.mjs", new Set(["/sample-report/compact/"])],
]);
const violations = [];

function requireIncludes(path, required) {
  const contents = readFileSync(path, "utf8");
  for (const text of required) {
    if (!contents.includes(text)) violations.push(`${path}: missing required text: ${text}`);
  }
  return contents;
}

function requireReadingOrder(path) {
  const contents = readFileSync(path, "utf8");
  const quick = contents.indexOf("read-summary-in-two-minutes.md");
  const detailed = contents.indexOf("reading-your-report.md");
  if (quick < 0 || detailed < 0 || quick > detailed) {
    violations.push(`${path}: the Summary quick guide must precede the Full evidence reference`);
  }
}

for (const path of tracked.stdout.split("\0").filter(Boolean)) {
  if (path === "scripts/check-summary-terminology.mjs") continue;
  let contents;
  try {
    contents = readFileSync(path, "utf8");
  } catch {
    continue;
  }
  for (const pattern of forbidden) {
    for (const match of contents.matchAll(pattern)) {
      if (allowlist.get(path)?.has(match[0])) continue;
      const line = contents.slice(0, match.index).split("\n").length;
      violations.push(`${path}:${line}: ${match[0]}`);
    }
  }
}

const quickGuidePath = "docs/read-summary-in-two-minutes.md";
const quickGuide = requireIncludes(quickGuidePath, [
  "https://antennabench.com/sample-report/summary/",
  "https://antennabench.com/sample-report/inconclusive/",
  "Paired shared-path signal",
  "Controlled common-opportunity detection",
  "Uncontrolled observed paths",
  "Principal Limitation",
  "Full evidence",
  "session bundle",
]);
const quickGuideWords = quickGuide
  .replace(/\[([^\]]+)\]\([^)]+\)/gu, "$1")
  .replace(/[`#*_]/gu, "")
  .match(/\b[\p{L}\p{N}][\p{L}\p{N}’'/-]*\b/gu)?.length ?? 0;
if (quickGuideWords < 400 || quickGuideWords > 600) {
  violations.push(`${quickGuidePath}: ${quickGuideWords} words; expected 400–600`);
}
for (const internalTerm of ["renderer", "schema", "stratum", "typed"]) {
  if (quickGuide.toLowerCase().includes(internalTerm)) {
    violations.push(`${quickGuidePath}: internal implementation term: ${internalTerm}`);
  }
}

for (const path of ["README.md", "docs/README.md", "docs/product.md", "docs/quickstart.md"]) {
  requireReadingOrder(path);
}

requireIncludes("docs/reading-your-report.md", [
  "comparison group",
  "conditional",
  "active",
  "missing",
  "comparison availability",
  "Same-Path Signal",
  "Reach And Unique Paths",
  "Coverage Overlap And Repeatability",
  "Distance And Azimuth",
  "Run Quality",
  "Audit Appendix",
  "Summary, Full Evidence, Or Bundle",
]);
requireIncludes("docs/glossary.md", [
  "## Summary",
  "## Full Evidence",
  "## Paired Shared-Path Signal",
  "## Controlled Common-Opportunity Detection",
  "## Uncontrolled Observed Paths",
]);
requireIncludes("apps/hosted/src/pages/index.astro", [
  "read-summary-in-two-minutes.md",
  "Read the two-minute guide",
]);
requireIncludes("apps/desktop/frontend/models.mjs", [
  "report_documents",
  "Summary and Full evidence",
  "principal limitation",
  "session bundle remains the durable record",
]);

if (violations.length > 0) {
  console.error("Deprecated Summary-artifact terminology remains outside the compatibility allowlist:");
  console.error(violations.join("\n"));
  process.exit(1);
}

console.log("Summary terminology is canonical; compatibility aliases are explicitly allowlisted.");
