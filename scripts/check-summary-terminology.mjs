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
  ["scripts/validate-hosted-site.mjs", new Set(["/sample-report/compact/"])],
]);
const violations = [];

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

if (violations.length > 0) {
  console.error("Deprecated Summary-artifact terminology remains outside the compatibility allowlist:");
  console.error(violations.join("\n"));
  process.exit(1);
}

console.log("Summary terminology is canonical; compatibility aliases are explicitly allowlisted.");
