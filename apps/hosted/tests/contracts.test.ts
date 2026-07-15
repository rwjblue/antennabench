import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

import {
  HOSTED_BINDING_ROLES,
  HOSTED_STANDARD_V1,
  lifecycleLog,
  routeApi,
  type BindingInventory,
} from "../src/contracts";

const FAKE_BINDINGS: BindingInventory = {
  environment: "development",
  profile: "hosted-standard-v1",
  admissionEnabled: false,
  available: Object.fromEntries(HOSTED_BINDING_ROLES.map((role) => [role, true])) as Record<
    (typeof HOSTED_BINDING_ROLES)[number],
    boolean
  >,
};

describe("hosted foundation contracts", () => {
  it("keeps every remote environment binding explicit and non-overlapping", () => {
    const config = JSON.parse(readFileSync(new URL("../wrangler.jsonc", import.meta.url), "utf8"));
    const environments = [
      ["development", config],
      ["preview", config.env.preview],
      ["production", config.env.production],
    ] as const;
    const resourceNames = new Set<string>();
    for (const [name, environment] of environments) {
      expect(environment.name).toBe(`antennabench-hosted-${name}`);
      expect(environment.vars).toEqual({
        HOSTED_ENVIRONMENT: name,
        HOSTED_PROFILE: "hosted-standard-v1",
        ADMISSION_ENABLED: "false",
      });
      expect(environment.r2_buckets.map((binding: { binding: string }) => binding.binding)).toEqual([
        "UPLOADS_PRIVATE",
        "DERIVED_PRIVATE",
        "REPORTS_PUBLIC",
      ]);
      expect(environment.d1_databases[0].binding).toBe("CONTROL_DB");
      expect(environment.queues.producers[0].binding).toBe("PROCESSOR_QUEUE");
      expect(environment.durable_objects.bindings[0].name).toBe("PROCESSOR");
      expect(environment.containers[0]).toMatchObject({
        instance_type: "basic",
        max_instances: 2,
        ssh: { enabled: false },
      });
      expect(environment.queues.consumers[0]).toMatchObject({
        max_batch_size: 1,
        max_concurrency: 2,
      });
      expect(environment.observability.logs.invocation_logs).toBe(false);
      expect(environment.preview_urls).toBe(false);
      expect(environment.workers_dev).toBe(name === "preview");
      for (const resource of [
        ...environment.r2_buckets.map((binding: { bucket_name: string }) => binding.bucket_name),
        environment.d1_databases[0].database_name,
        environment.queues.producers[0].queue,
        environment.queues.consumers[0].dead_letter_queue,
      ]) {
        expect(resource).toContain(`-${name}-`);
        expect(resourceNames.has(resource)).toBe(false);
        resourceNames.add(resource);
      }
    }
  });

  it("keeps the production resource and processor limits fixed in code", () => {
    expect(HOSTED_STANDARD_V1.profile).toBe("hosted-standard-v1");
    expect(HOSTED_STANDARD_V1.compressedArchiveBytes).toBe(32 * 1024 * 1024);
    expect(HOSTED_STANDARD_V1.processorDeadlineMs).toBe(120_000);
    expect(HOSTED_STANDARD_V1.processorInstanceType).toBe("basic");
    expect(HOSTED_STANDARD_V1.processorMaxInstances).toBe(2);
    expect(HOSTED_STANDARD_V1.processorInternetEgress).toBe(false);
  });

  it("reports typed fake bindings without enabling admission", async () => {
    const response = routeApi(new Request("https://local.invalid/api/health"), FAKE_BINDINGS);
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      service: "antennabench-hosted",
      environment: "development",
      profile: "hosted-standard-v1",
      admissionEnabled: false,
      bindings: FAKE_BINDINGS.available,
    });
    expect(response.headers.get("cache-control")).toBe("no-store");
  });

  it("has no upload or publication API in the foundation", async () => {
    for (const [method, path] of [
      ["POST", "/api/uploads"],
      ["PUT", "/api/reports/example"],
      ["GET", "/api/reports/example"],
    ]) {
      const response = routeApi(new Request(`https://local.invalid${path}`, { method }), FAKE_BINDINGS);
      expect(response.status).toBe(404);
      expect(await response.json()).toEqual({ error: "not_found" });
    }
  });

  it("allows only redacted structured lifecycle fields", () => {
    expect(lifecycleLog("processor", "stopped", "job_complete")).toEqual({
      event: "hosted_lifecycle",
      stage: "processor",
      outcome: "stopped",
      code: "job_complete",
    });
    expect(JSON.stringify(lifecycleLog("worker", "ready", "health"))).not.toMatch(
      /callsign|grid|location|notes|token|capability|url/i,
    );
  });
});
