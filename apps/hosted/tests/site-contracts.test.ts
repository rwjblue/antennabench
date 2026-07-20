import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

function read(relativePath: string): string {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

describe("public project site contracts", () => {
  it("uses static Astro in the existing hosted workspace without React", () => {
    const packageJson = JSON.parse(read("../package.json"));
    const dependencies = {
      ...packageJson.dependencies,
      ...packageJson.devDependencies,
    };
    expect(dependencies.astro).toBe("7.1.1");
    expect(dependencies).not.toHaveProperty("react");
    expect(dependencies).not.toHaveProperty("react-dom");

    const astroConfig = read("../astro.config.mjs");
    expect(astroConfig).toContain('site: "https://antennabench.com"');
    expect(astroConfig).toContain('output: "static"');
    expect(astroConfig).toContain('outDir: "./dist/site"');
    expect(astroConfig).not.toMatch(/adapter|server|hybrid/);
  });

  it("keeps site deployment independent of unfinished hosted services", () => {
    const siteConfig = JSON.parse(read("../wrangler.site.jsonc"));
    expect(siteConfig).toMatchObject({
      name: "antennabench-site",
      workers_dev: false,
      preview_urls: false,
      assets: {
        directory: "./dist/site",
        html_handling: "auto-trailing-slash",
        not_found_handling: "404-page",
      },
    });
    for (const forbidden of [
      "main",
      "r2_buckets",
      "d1_databases",
      "queues",
      "durable_objects",
      "containers",
      "vars",
    ]) {
      expect(siteConfig).not.toHaveProperty(forbidden);
    }
  });

  it("preserves the future same-origin API and React extension seams", () => {
    const foundationConfig = JSON.parse(read("../wrangler.jsonc"));
    for (const profile of [
      foundationConfig,
      foundationConfig.env.preview,
      foundationConfig.env.production,
    ]) {
      expect(profile.assets.directory).toBe("./dist/site");
      expect(profile.assets.run_worker_first).toEqual(["/api/*"]);
    }
    const decision = read("../../../docs/decisions/0023-use-static-astro-for-the-project-site.md");
    expect(decision).toMatch(/`\/app` may\s+later host/);
    expect(decision).toContain("`/api/*`");
    expect(decision).toContain("separate report origin");
  });

  it("ships explicit same-origin framing and privacy headers", () => {
    const headers = read("../public/_headers");
    expect(headers).toContain("script-src 'none'");
    expect(headers).toContain("connect-src 'none'");
    expect(headers).toContain("frame-ancestors 'self'");
    expect(headers).toContain("X-Frame-Options: SAMEORIGIN");
    expect(headers).toContain("Referrer-Policy: no-referrer");
    expect(headers).toContain("Permissions-Policy:");
    expect(headers).not.toMatch(/analytics|google|segment|sentry/i);
  });

  it("deploys only reviewed main history through the production environment", () => {
    const workflow = read("../../../.github/workflows/hosted-site-deploy.yml");
    expect(workflow).toContain("branches: [main]");
    expect(workflow).toContain("environment:");
    expect(workflow).toContain("name: production");
    expect(workflow).toContain("git merge-base --is-ancestor");
    expect(workflow).toContain("secrets.CLOUDFLARE_ACCOUNT_ID");
    expect(workflow).toContain("secrets.CLOUDFLARE_API_TOKEN");
    expect(workflow).not.toContain("pull_request:");
  });

  it("publishes the WSPR and RBN choice as user-facing site guidance", () => {
    const page = read("../src/pages/why-wspr.astro");
    const maps = read("../src/components/ReceiverFootprintMaps.astro");
    const header = read("../src/components/SiteHeader.astro");
    const footer = read("../src/components/SiteFooter.astro");
    const sitemap = read("../src/pages/sitemap.xml.ts");
    const doc = read("../../../docs/why-not-just-use-rbn.md");

    expect(page).toContain("receiver-census-summary.json");
    expect(page).toContain("stronger signals");
    expect(page).toContain("Use WSPR to understand the setup");
    expect(page).toContain("Confirm the live result with RBN");
    expect(page).not.toContain("—");
    expect(page).not.toContain("The snapshot is bounded, checked in, and reproducible.");
    expect(maps).toContain("wspr-receivers-by-band.csv?raw");
    expect(maps).toContain("rbn-active-nodes-reduced.csv?raw");
    expect(maps).toContain("world-outline-natural-earth.geojson?raw");
    expect(maps).toContain("receiver-grid-cell");
    expect(maps).toContain("receiver-rbn-node");
    expect(maps).toContain("four-character Maidenhead grid");
    expect(header).toContain('href="/why-wspr/"');
    expect(footer).toContain('href="/why-wspr/"');
    expect(sitemap).toContain('"/why-wspr/"');
    expect(doc).toContain("https://antennabench.com/why-wspr/");
    expect(doc).toContain("BEGIN GENERATED RECEIVER SNAPSHOT");
    expect(doc).toContain("END GENERATED RECEIVER SNAPSHOT");
  });
});
