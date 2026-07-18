import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const command = process.argv[2];
if (command !== "build") {
  throw new Error("usage: run-hosted-astro.mjs build");
}

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const hostedRoot = join(repositoryRoot, "apps", "hosted");
const astroCli = join(repositoryRoot, "node_modules", "astro", "bin", "astro.mjs");
const result = spawnSync(process.execPath, [astroCli, command], {
  cwd: hostedRoot,
  encoding: "utf8",
  env: {
    ...process.env,
    ASTRO_TELEMETRY_DISABLED: "1",
  },
});

process.stdout.write(result.stdout);
process.stderr.write(result.stderr);
process.exit(result.status ?? 1);
