import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const source = join(repositoryRoot, "apps", "hosted", "assets-src", "social-card.svg");
const committedOutput = join(repositoryRoot, "apps", "hosted", "public", "social-card.png");
const check = process.argv.slice(2).includes("--check");
const temporaryDirectory = check
  ? mkdtempSync(join(tmpdir(), "antennabench-social-card-"))
  : undefined;
const output = temporaryDirectory === undefined
  ? committedOutput
  : join(temporaryDirectory, "social-card.png");

try {
  await sharp(source, { density: 144 })
    .resize(1200, 630, { fit: "fill" })
    .png({ compressionLevel: 9, adaptiveFiltering: false })
    .toFile(output);

  if (check) {
    const generated = readFileSync(output);
    const committed = readFileSync(committedOutput);
    if (!generated.equals(committed)) {
      throw new Error(
        "apps/hosted/public/social-card.png is stale; run `npm run site:social --workspace @antennabench/hosted`",
      );
    }
    console.log("Hosted social card matches its deterministic SVG source");
  } else {
    console.log(`wrote ${committedOutput}`);
  }
} finally {
  if (temporaryDirectory !== undefined) {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
