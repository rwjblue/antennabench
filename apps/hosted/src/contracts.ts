export const HOSTED_STANDARD_V1 = Object.freeze({
  profile: "hosted-standard-v1",
  compressedArchiveBytes: 32 * 1024 * 1024,
  expandedArchiveBytes: 128 * 1024 * 1024,
  archiveEntries: 1_024,
  archiveDepth: 8,
  expansionRatio: 100,
  rootJsonBytes: 4 * 1024 * 1024,
  jsonlLineBytes: 256 * 1024,
  jsonlRecordsTotal: 200_000,
  reportRows: 25_000,
  reportModelBytes: 8 * 1024 * 1024,
  reportHtmlBytes: 16 * 1024 * 1024,
  processorDeadlineMs: 120_000,
  processorInstanceType: "basic",
  processorMaxInstances: 2,
  processorInternetEgress: false,
} as const);

export const HOSTED_BINDING_ROLES = Object.freeze([
  "ASSETS",
  "UPLOADS_PRIVATE",
  "DERIVED_PRIVATE",
  "REPORTS_PUBLIC",
  "CONTROL_DB",
  "PROCESSOR_QUEUE",
  "PROCESSOR",
] as const);

export type HostedBindingRole = (typeof HOSTED_BINDING_ROLES)[number];
export type HostedEnvironment = "development" | "preview" | "production";

export interface BindingInventory {
  readonly environment: HostedEnvironment;
  readonly profile: "hosted-standard-v1";
  readonly admissionEnabled: false;
  readonly available: Readonly<Record<HostedBindingRole, boolean>>;
}

export function healthResponse(inventory: BindingInventory): Response {
  return Response.json(
    {
      service: "antennabench-hosted",
      environment: inventory.environment,
      profile: inventory.profile,
      admissionEnabled: inventory.admissionEnabled,
      bindings: inventory.available,
    },
    {
      headers: {
        "cache-control": "no-store",
        "content-security-policy": "default-src 'none'; frame-ancestors 'none'",
        "referrer-policy": "no-referrer",
        "x-content-type-options": "nosniff",
      },
    },
  );
}

export function routeApi(request: Request, inventory: BindingInventory): Response {
  const url = new URL(request.url);
  if (request.method === "GET" && url.pathname === "/api/health") {
    return healthResponse(inventory);
  }
  return Response.json(
    { error: "not_found" },
    { status: 404, headers: { "cache-control": "no-store" } },
  );
}

export type LifecycleStage = "queue" | "processor" | "worker";
export type LifecycleOutcome = "deferred" | "failed" | "ready" | "stopped";
export type LifecycleCode =
  | "foundation_not_active"
  | "foundation_probe"
  | "health"
  | "job_complete";

export function lifecycleLog(
  stage: LifecycleStage,
  outcome: LifecycleOutcome,
  code: LifecycleCode,
): Readonly<Record<string, string>> {
  return Object.freeze({ event: "hosted_lifecycle", stage, outcome, code });
}
