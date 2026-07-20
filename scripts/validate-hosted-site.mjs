import {
  accessSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import { dirname, extname, join, normalize, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const hostedRoot = join(repositoryRoot, "apps", "hosted");
const outputRoot = join(hostedRoot, "dist", "site");

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function read(path) {
  return readFileSync(join(repositoryRoot, path), "utf8");
}

function readJson(path) {
  return JSON.parse(read(path));
}

function filesBelow(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

function outputPathForUrl(url, sourceFile) {
  const clean = url.split(/[?#]/, 1)[0];
  if (clean === "") {
    return sourceFile;
  }
  const base = clean.startsWith("/")
    ? join(outputRoot, clean.slice(1))
    : resolve(dirname(sourceFile), clean);
  if (extname(base) !== "") {
    return base;
  }
  if (base.endsWith(sep) || statMaybeDirectory(base)) {
    return join(base, "index.html");
  }
  return `${base}.html`;
}

function statMaybeDirectory(path) {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

function relativeLuminance(hex) {
  const channels = hex
    .slice(1)
    .match(/../g)
    .map((value) => Number.parseInt(value, 16) / 255)
    .map((value) => value <= 0.04045
      ? value / 12.92
      : ((value + 0.055) / 1.055) ** 2.4);
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}

function contrastRatio(foreground, background) {
  const first = relativeLuminance(foreground);
  const second = relativeLuminance(background);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

function assertInternalLinks(htmlFiles) {
  for (const file of htmlFiles) {
    const html = readFileSync(file, "utf8");
    for (const match of html.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
      const url = match[1];
      if (
        url.startsWith("#") ||
        url.startsWith("mailto:") ||
        url.startsWith("https://") ||
        url.startsWith("http://") ||
        url.startsWith("data:")
      ) {
        continue;
      }
      const target = normalize(outputPathForUrl(url, file));
      invariant(
        target.startsWith(`${outputRoot}${sep}`) || target === outputRoot,
        `${relative(outputRoot, file)} links outside the site output: ${url}`,
      );
      try {
        accessSync(target);
      } catch {
        throw new Error(`${relative(outputRoot, file)} has a broken internal link: ${url}`);
      }
    }
  }
}

for (const expected of [
  "index.html",
  "404.html",
  "how-it-works/index.html",
  "why-wspr/index.html",
  "sample-report/index.html",
  "robots.txt",
  "sitemap.xml",
  "favicon.svg",
  "mark.svg",
  "social-card.png",
  "_headers",
]) {
  accessSync(join(outputRoot, expected));
}

const outputFiles = filesBelow(outputRoot);
const htmlFiles = outputFiles.filter((path) => path.endsWith(".html"));
const marketingHtml = htmlFiles.filter(
  (path) => !relative(outputRoot, path).startsWith(`sample-report${sep}`),
);
for (const file of marketingHtml) {
  const html = readFileSync(file, "utf8");
  invariant(!/<script\b/i.test(html), `${relative(outputRoot, file)} contains client JavaScript`);
  invariant(
    /<meta name="description" content="[^"]+"/.test(html),
    `${relative(outputRoot, file)} is missing its description`,
  );
  invariant(
    /<link rel="canonical" href="https:\/\/antennabench\.com\//.test(html),
    `${relative(outputRoot, file)} is missing its apex canonical URL`,
  );
  invariant(
    /<meta property="og:image" content="https:\/\/antennabench\.com\/social-card\.png"/.test(html),
    `${relative(outputRoot, file)} is missing the social image`,
  );
  invariant(
    !/\b(?:src|poster)="https?:\/\//i.test(html),
    `${relative(outputRoot, file)} loads an external runtime resource`,
  );
  invariant(
    !/<link\b[^>]*rel="stylesheet"[^>]*href="https?:\/\//i.test(html),
    `${relative(outputRoot, file)} loads an external stylesheet`,
  );
  for (const image of html.matchAll(/<img\b[^>]*>/g)) {
    invariant(/\balt="[^"]*"/.test(image[0]), `${relative(outputRoot, file)} has an image without alt text`);
  }
  for (const frame of html.matchAll(/<iframe\b[^>]*>/g)) {
    invariant(/\btitle="[^"]+"/.test(frame[0]), `${relative(outputRoot, file)} has an iframe without a title`);
  }
}
invariant(
  outputFiles.every((path) => !path.endsWith(".js") && !path.endsWith(".mjs")),
  "Static marketing output unexpectedly contains JavaScript",
);

for (const file of outputFiles.filter((path) => path.endsWith(".css"))) {
  invariant(
    !/https?:\/\//.test(readFileSync(file, "utf8")),
    `${relative(outputRoot, file)} loads an external runtime resource`,
  );
}
assertInternalLinks(htmlFiles);

const home = readFileSync(join(outputRoot, "index.html"), "utf8");
invariant(home.includes('class="skip-link"'), "Home page is missing its keyboard skip link");
invariant(home.includes('aria-label="Main navigation"'), "Home page is missing its navigation label");
invariant(home.includes('title="AntennaBench canonical sample evidence report"'), "Report preview is missing its accessible title");
invariant(home.includes('href="/why-wspr/"'), "Home page is missing the WSPR and RBN explanation link");

const whyWspr = readFileSync(join(outputRoot, "why-wspr", "index.html"), "utf8");
for (const networkChoiceContract of [
  "Why AntennaBench starts with WSPR",
  "four-character Maidenhead grids",
  "Use WSPR to understand the setup",
  "Confirm the live result with RBN",
  "A missing spot is still not a zero.",
]) {
  invariant(
    whyWspr.includes(networkChoiceContract),
    `WSPR and RBN explanation is missing its content contract: ${networkChoiceContract}`,
  );
}
const whyWsprMain = whyWspr.match(/<main\b[^>]*>([\s\S]*?)<\/main>/i)?.[1];
invariant(whyWsprMain !== undefined, "WSPR and RBN explanation is missing its main content");
invariant(!whyWsprMain.includes("—"), "WSPR and RBN article contains an em dash");
invariant(
  !whyWspr.includes("The snapshot is bounded, checked in, and reproducible."),
  "WSPR and RBN explanation still contains the removed generic reproducibility section",
);
const footprintMaps = whyWspr.match(/class="receiver-footprint-map"/g)?.length ?? 0;
invariant(footprintMaps === 3, `Expected three band footprint maps, found ${footprintMaps}`);
const occupiedGridCells = whyWspr.match(/class="receiver-grid-cell"/g)?.length ?? 0;
invariant(
  occupiedGridCells >= 800,
  `Expected the footprint maps to render the checked-in occupied grids, found ${occupiedGridCells}`,
);
const rbnNodeMarkers = whyWspr.match(/class="receiver-rbn-node"/g)?.length ?? 0;
invariant(
  rbnNodeMarkers >= 400,
  `Expected the footprint maps to render the checked-in RBN nodes, found ${rbnNodeMarkers}`,
);

const stylesheet = outputFiles
  .filter((path) => path.endsWith(".css"))
  .map((path) => readFileSync(path, "utf8"))
  .join("\n");
for (const accessibilityContract of [
  ":focus-visible",
  "prefers-reduced-motion:reduce",
  "width<=700px",
]) {
  invariant(stylesheet.includes(accessibilityContract), `Styles are missing ${accessibilityContract}`);
}
for (const [foreground, background, name] of [
  ["#102b2b", "#f7f1df", "primary text on paper"],
  ["#1f756c", "#f7f1df", "teal text on paper"],
  ["#fffefa", "#1f756c", "primary button text"],
  ["#70c5b8", "#102b2b", "bright teal on dark ink"],
]) {
  invariant(contrastRatio(foreground, background) >= 4.5, `${name} is below WCAG AA contrast`);
}

const headers = read("apps/hosted/public/_headers");
for (const header of [
  "Content-Security-Policy:",
  "Cross-Origin-Opener-Policy:",
  "Permissions-Policy:",
  "Referrer-Policy:",
  "X-Content-Type-Options: nosniff",
  "X-Frame-Options: SAMEORIGIN",
  "Cache-Control:",
]) {
  invariant(headers.includes(header), `Static asset headers are missing ${header}`);
}
for (const directive of [
  "connect-src 'none'",
  "object-src 'none'",
  "script-src 'none'",
  "frame-ancestors 'self'",
]) {
  invariant(headers.includes(directive), `CSP is missing ${directive}`);
}

const siteConfig = readJson("apps/hosted/wrangler.site.jsonc");
invariant(siteConfig.assets.directory === "./dist/site", "Site deployment must serve Astro output");
for (const forbidden of [
  "main",
  "r2_buckets",
  "d1_databases",
  "queues",
  "durable_objects",
  "containers",
  "vars",
]) {
  invariant(!(forbidden in siteConfig), `Site-only deployment must not declare ${forbidden}`);
}

const foundationConfig = readJson("apps/hosted/wrangler.jsonc");
invariant(
  foundationConfig.assets.directory === "./dist/site" &&
    foundationConfig.env.preview.assets.directory === "./dist/site" &&
    foundationConfig.env.production.assets.directory === "./dist/site",
  "Every future hosted profile must reuse Astro output",
);
invariant(
  foundationConfig.assets.run_worker_first.includes("/api/*"),
  "The future same-origin API seam must remain explicit",
);

const hostedPackage = readJson("apps/hosted/package.json");
const allDependencies = {
  ...hostedPackage.dependencies,
  ...hostedPackage.devDependencies,
};
invariant(allDependencies.astro === "7.1.1", "Astro must remain exactly pinned");
invariant(
  Object.keys(allDependencies).every((name) => name !== "react" && name !== "react-dom"),
  "React is reserved for the later authenticated application",
);

const canonicalSample = read("apps/hosted/public/sample-report/index.html");
for (const reportContract of [
  "What did the run show?",
  "Answered by this run: Observed reach",
  "No same-path SNR comparison",
  "Observed reach",
  "Run quality and answerability",
  "does not select an antenna winner",
]) {
  invariant(
    canonicalSample.includes(reportContract),
    `Canonical sample is missing the report contract: ${reportContract}`,
  );
}
invariant(
  !canonicalSample.includes('id="same-path-signal"') &&
    !canonicalSample.includes('href="#same-path-signal"'),
  "Unavailable shared-path signal must not dominate canonical sample navigation",
);
invariant(!/<script\b/i.test(canonicalSample), "Canonical sample must remain standalone and script-free");

const deployedMark = read("apps/hosted/public/mark.svg");
const desktopMark = read("apps/desktop/frontend/mark.svg");
invariant(deployedMark === desktopMark, "Hosted and desktop marks have drifted");

const socialCard = readFileSync(join(outputRoot, "social-card.png"));
invariant(
  socialCard.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])),
  "Social card must be a PNG",
);
invariant(
  socialCard.readUInt32BE(16) === 1200 && socialCard.readUInt32BE(20) === 630,
  "Social card must remain 1200 by 630 pixels",
);

console.log(
  `Hosted site validation passed: ${marketingHtml.length} pages, ${outputFiles.length} generated files, no client JavaScript`,
);
