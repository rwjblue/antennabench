import { mkdtempSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const committedOutput = join(
  repositoryRoot,
  "apps",
  "hosted",
  "public",
  "sample-report",
  "index.html",
);
const check = process.argv.slice(2).includes("--check");
const temporaryDirectory = check
  ? mkdtempSync(join(tmpdir(), "antennabench-hosted-sample-"))
  : undefined;
const output = temporaryDirectory === undefined
  ? committedOutput
  : join(temporaryDirectory, "index.html");

try {
  mkdirSync(dirname(output), { recursive: true });
  const result = spawnSync(
    "cargo",
    [
      "run",
      "--quiet",
      "-p",
      "antennabench-report",
      "--example",
      "render_canonical_sample",
      "--",
      output,
    ],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
  if (result.status !== 0) {
    process.stderr.write(result.stdout);
    process.stderr.write(result.stderr);
    process.exit(result.status ?? 1);
  }

  if (check) {
    const generated = readFileSync(output);
    const committed = readFileSync(committedOutput);
    if (!generated.equals(committed)) {
      throw new Error(
        "apps/hosted/public/sample-report/index.html is stale; run `npm run site:sample --workspace @antennabench/hosted`",
      );
    }
    console.log("Canonical hosted sample matches the trusted Rust renderer");
  } else {
    process.stdout.write(result.stdout);
  }
} finally {
  if (temporaryDirectory !== undefined) {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
