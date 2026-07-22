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
    relativeOutput: join("sample-report", "summary", "index.html"),
    rendererArguments: ["--summary"],
  },
  {
    relativeOutput: join("sample-report", "inconclusive", "index.html"),
    rendererArguments: ["--bundle", inconclusiveFixture],
  },
];
const check = process.argv.slice(2).includes("--check");
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
        throw new Error(
          `${sample.relativeOutput} is stale; run \`npm run site:sample --workspace @antennabench/hosted\``,
        );
      }
    } else {
      process.stdout.write(result.stdout);
    }
  }
  if (check) {
    console.log("Hosted samples match the trusted Rust renderer");
  }
} finally {
  if (temporaryDirectory !== undefined) {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
