import { Container } from "@cloudflare/containers";

import {
  HOSTED_STANDARD_V1,
  lifecycleLog,
  routeApi,
  type BindingInventory,
} from "./contracts";

export class ProcessorContainer extends Container<Env> {
  override sleepAfter = 1;
  override enableInternet = false;

  async runFoundationProbe(): Promise<number> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      await this.start({ enableInternet: false });
      const container = this.ctx.container;
      if (container === undefined) {
        throw new Error("container runtime is unavailable");
      }
      const process = await container.exec(["/app/foundation-probe"]);
      timer = setTimeout(() => process.kill(), HOSTED_STANDARD_V1.processorDeadlineMs);
      return await process.exitCode;
    } finally {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
      await this.stop();
    }
  }
}

function inventory(env: Env): BindingInventory {
  return {
    environment: env.HOSTED_ENVIRONMENT,
    profile: "hosted-standard-v1",
    admissionEnabled: false,
    available: {
      ASSETS: Boolean(env.ASSETS),
      UPLOADS_PRIVATE: Boolean(env.UPLOADS_PRIVATE),
      DERIVED_PRIVATE: Boolean(env.DERIVED_PRIVATE),
      REPORTS_PUBLIC: Boolean(env.REPORTS_PUBLIC),
      CONTROL_DB: Boolean(env.CONTROL_DB),
      PROCESSOR_QUEUE: Boolean(env.PROCESSOR_QUEUE),
      PROCESSOR: Boolean(env.PROCESSOR),
    },
  };
}

export default {
  fetch(request, env) {
    return routeApi(request, inventory(env));
  },
  async queue(batch) {
    for (const message of batch.messages) {
      console.log(JSON.stringify(lifecycleLog("queue", "deferred", "foundation_not_active")));
      message.retry({ delaySeconds: 300 });
    }
  },
} satisfies ExportedHandler<Env, ProcessorQueueMessage>;

interface ProcessorQueueMessage {
  readonly jobId: string;
  readonly expectedUploadDigest: string;
}
