import { mkdtempSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const publicRoot = join(repositoryRoot, "apps", "hosted", "public");
const inconclusiveFixture = join(
  repositoryRoot,
  "fixtures",
  "session-bundles",
  "inconclusive-sample-report.session.wsprabundle",
);
const samples = [
  {
    relativeOutput: join("sample-report", "index.html"),
    rendererArguments: [],
  },
  {
    relativeOutput: join("sample-report", "compact", "index.html"),
    rendererArguments: ["--compact-summary"],
  },
  {
    relativeOutput: join("sample-report", "inconclusive", "index.html"),
    rendererArguments: ["--bundle", inconclusiveFixture],
  },
];
const check = process.argv.slice(2).includes("--check");
let stale = false;
const temporaryDirectory = check
  ? mkdtempSync(join(tmpdir(), "antennabench-hosted-sample-"))
  : undefined;

try {
  for (const sample of samples) {
    const committedOutput = join(publicRoot, sample.relativeOutput);
    const output = temporaryDirectory === undefined
      ? committedOutput
      : join(temporaryDirectory, sample.relativeOutput);
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
        ...sample.rendererArguments,
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
        stale = true;
        const limit = Math.min(generated.length, committed.length);
        let first = 0;
        while (first < limit && generated[first] === committed[first]) first += 1;
        let generatedEnd = generated.length;
        let committedEnd = committed.length;
        while (
          generatedEnd > first
          && committedEnd > first
          && generated[generatedEnd - 1] === committed[committedEnd - 1]
        ) {
          generatedEnd -= 1;
          committedEnd -= 1;
        }
        console.error(JSON.stringify({
          sample: sample.relativeOutput,
          generatedLength: generated.length,
          committedLength: committed.length,
          first,
          generatedEnd,
          committedEnd,
          generated: generated.subarray(Math.max(0, first - 120), Math.min(generated.length, generatedEnd + 120)).toString("base64"),
          committed: committed.subarray(Math.max(0, first - 120), Math.min(committed.length, committedEnd + 120)).toString("base64"),
        }));
      }
    } else {
      process.stdout.write(result.stdout);
    }
  }
  if (check) {
    if (stale) {
      throw new Error("Hosted samples are stale; run the repository sample generator");
    }
    console.log("Hosted samples match the trusted Rust renderer");
  }
} finally {
  if (temporaryDirectory !== undefined) {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
