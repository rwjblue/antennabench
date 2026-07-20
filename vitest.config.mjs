import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "desktop-node",
          environment: "node",
          include: [
            "apps/desktop/tests/frontend-controller.test.mjs",
            "apps/desktop/tests/frontend-state.test.mjs",
          ],
        },
      },
      {
        extends: true,
        test: {
          name: "desktop-dom",
          environment: "jsdom",
          testTimeout: 15_000,
          include: [
            "apps/desktop/tests/frontend-app.test.mjs",
            "apps/desktop/tests/frontend-renderers.test.mjs",
          ],
        },
      },
      {
        extends: true,
        test: {
          name: "hosted-worker",
          environment: "node",
          include: ["apps/hosted/tests/**/*.test.ts"],
        },
      },
    ],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      reportsDirectory: "coverage/desktop",
      include: ["apps/desktop/frontend/**/*.mjs"],
    },
  },
});
