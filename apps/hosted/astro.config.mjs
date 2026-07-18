import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://antennabench.com",
  output: "static",
  outDir: "./dist/site",
  build: {
    assets: "assets",
  },
  compressHTML: true,
});
