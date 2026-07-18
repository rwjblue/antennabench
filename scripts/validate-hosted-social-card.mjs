import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const sourcePath = join(
  repositoryRoot,
  "apps",
  "hosted",
  "assets-src",
  "social-card.svg",
);
const outputPath = join(
  repositoryRoot,
  "apps",
  "hosted",
  "public",
  "social-card.png",
);

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const source = readFileSync(sourcePath, "utf8");
for (const contract of [
  'width="1200" height="630"',
  'viewBox="0 0 1200 630"',
  "ANTENNABENCH",
  "Better antenna",
  "Evidence included.",
  "antennabench.com",
]) {
  invariant(source.includes(contract), `Social-card source is missing ${contract}`);
}
invariant(
  !/(?:href|url\()\s*=?\s*["']?https?:/i.test(source),
  "Social-card source must not depend on external resources",
);

const output = readFileSync(outputPath);
invariant(
  output.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])),
  "Social card must be a PNG",
);
invariant(
  output.readUInt32BE(16) === 1200 && output.readUInt32BE(20) === 630,
  "Social card must remain 1200 by 630 pixels",
);

console.log("Hosted social card source and 1200 by 630 PNG are valid");
