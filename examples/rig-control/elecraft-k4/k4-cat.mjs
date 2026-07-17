import { createConnection } from "node:net";

const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 9299;
const DEFAULT_TIMEOUT_MS = 2_000;
const DEFAULT_SETTLE_MS = 150;
const POLL_INTERVAL_MS = 100;
const MAX_RESPONSE_BYTES = 4_096;

const MODE_SCOPES = Object.freeze({
  whole_station_ab: "both",
  tx_focused: "transmit",
  rx_focused: "receive",
  single_antenna_profiling: "both",
});

export class K4ControlError extends Error {}

function requiredValue(argv, index, option) {
  const value = argv[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new K4ControlError(`${option} requires a value`);
  }
  return value;
}

function boundedInteger(value, option, minimum, maximum) {
  if (!/^\d+$/.test(value)) {
    throw new K4ControlError(`${option} must be an integer`);
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new K4ControlError(`${option} must be between ${minimum} and ${maximum}`);
  }
  return parsed;
}

export function parseOptions(argv) {
  const raw = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const option = argv[index];
    if (!option?.startsWith("--")) {
      throw new K4ControlError(`unexpected argument: ${option ?? "<missing>"}`);
    }
    if (raw.has(option)) {
      throw new K4ControlError(`duplicate option: ${option}`);
    }
    raw.set(option, requiredValue(argv, index, option));
  }

  const allowed = new Set([
    "--host",
    "--port",
    "--target",
    "--mode",
    "--direction",
    "--timeout-ms",
    "--settle-ms",
  ]);
  for (const option of raw.keys()) {
    if (!allowed.has(option)) {
      throw new K4ControlError(`unknown option: ${option}`);
    }
  }

  const target = raw.get("--target");
  if (target !== "1" && target !== "2") {
    throw new K4ControlError("--target must be 1 or 2");
  }

  const mode = raw.get("--mode");
  const scope = MODE_SCOPES[mode];
  if (!scope) {
    throw new K4ControlError(
      "--mode must be whole_station_ab, tx_focused, rx_focused, or single_antenna_profiling",
    );
  }

  const direction = raw.get("--direction");
  if (direction !== "receive" && direction !== "transmit") {
    throw new K4ControlError("--direction must be receive or transmit");
  }
  if (scope !== "both" && direction !== scope) {
    throw new K4ControlError(`mode ${mode} cannot have a ${direction} intention`);
  }

  return {
    host: raw.get("--host") ?? DEFAULT_HOST,
    port: boundedInteger(raw.get("--port") ?? String(DEFAULT_PORT), "--port", 1, 65_535),
    target: Number(target),
    mode,
    direction,
    scope,
    timeoutMs: boundedInteger(
      raw.get("--timeout-ms") ?? String(DEFAULT_TIMEOUT_MS),
      "--timeout-ms",
      100,
      9_000,
    ),
    settleMs: boundedInteger(
      raw.get("--settle-ms") ?? String(DEFAULT_SETTLE_MS),
      "--settle-ms",
      0,
      5_000,
    ),
  };
}

export function expectedState(options) {
  return {
    transmit: options.scope === "receive" ? undefined : options.target,
    receive: options.scope === "transmit" ? undefined : options.target + 4,
  };
}

function setCommands(expected) {
  return [
    expected.transmit === undefined ? "" : `AN${expected.transmit};`,
    expected.receive === undefined ? "" : `AR${expected.receive};`,
  ].join("");
}

function queryCommands(expected) {
  return [
    "AI0;",
    expected.transmit === undefined ? "" : "AN;",
    expected.receive === undefined ? "" : "AR;",
  ].join("");
}

function matchesExpected(actual, expected) {
  return (
    (expected.transmit === undefined || actual.transmit === expected.transmit) &&
    (expected.receive === undefined || actual.receive === expected.receive)
  );
}

function stateDescription(state) {
  return JSON.stringify({
    transmit: state.transmit === undefined ? null : `AN${state.transmit}`,
    receive: state.receive === undefined ? null : `AR${state.receive}`,
  });
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function sendCommands(options, commands) {
  await new Promise((resolve, reject) => {
    const socket = createConnection({ host: options.host, port: options.port });
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      if (error) reject(error);
      else resolve();
    };

    socket.setTimeout(options.timeoutMs, () =>
      finish(new K4ControlError("timed out while sending K4 CAT commands")),
    );
    socket.on("error", (error) =>
      finish(new K4ControlError(`K4 CAT connection failed: ${error.message}`)),
    );
    socket.on("connect", () => {
      socket.end(`AI0;${commands}`, "ascii", (error) => {
        if (error) finish(new K4ControlError(`K4 CAT write failed: ${error.message}`));
        else finish();
      });
    });
  });
}

async function queryState(options, expected, timeoutMs) {
  return await new Promise((resolve, reject) => {
    const socket = createConnection({ host: options.host, port: options.port });
    let buffer = "";
    let responseBytes = 0;
    let settled = false;
    const actual = { transmit: undefined, receive: undefined };

    const finish = (error) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      if (error) reject(error);
      else resolve(actual);
    };

    socket.setTimeout(timeoutMs, () =>
      finish(new K4ControlError("timed out waiting for K4 antenna state")),
    );
    socket.on("error", (error) =>
      finish(new K4ControlError(`K4 CAT query failed: ${error.message}`)),
    );
    socket.on("end", () => {
      const complete =
        (expected.transmit === undefined || actual.transmit !== undefined) &&
        (expected.receive === undefined || actual.receive !== undefined);
      if (!complete) finish(new K4ControlError("K4 CAT connection ended before read-back completed"));
    });
    socket.on("connect", () => socket.write(queryCommands(expected), "ascii"));
    socket.on("data", (chunk) => {
      responseBytes += chunk.length;
      if (responseBytes > MAX_RESPONSE_BYTES) {
        finish(new K4ControlError("K4 CAT response exceeded 4096 bytes"));
        return;
      }

      buffer += chunk.toString("ascii");
      const frames = buffer.split(";");
      buffer = frames.pop() ?? "";
      for (const frame of frames) {
        if (frame.includes("?")) {
          finish(new K4ControlError(`K4 rejected CAT command: ${frame};`));
          return;
        }
        const transmit = /^AN([1-3])$/.exec(frame);
        if (transmit) actual.transmit = Number(transmit[1]);
        const receive = /^AR([0-7])$/.exec(frame);
        if (receive) actual.receive = Number(receive[1]);
      }

      const complete =
        (expected.transmit === undefined || actual.transmit !== undefined) &&
        (expected.receive === undefined || actual.receive !== undefined);
      if (complete) finish();
    });
  });
}

async function waitForExpectedState(options, expected) {
  const deadline = Date.now() + options.timeoutMs;
  let lastState;
  let lastError;
  while (Date.now() < deadline) {
    const remaining = Math.max(1, deadline - Date.now());
    try {
      lastState = await queryState(options, expected, Math.min(remaining, 1_000));
      if (matchesExpected(lastState, expected)) return lastState;
      lastError = undefined;
    } catch (error) {
      lastError = error;
    }
    if (Date.now() < deadline) await delay(Math.min(POLL_INTERVAL_MS, deadline - Date.now()));
  }

  if (lastState) {
    throw new K4ControlError(
      `K4 antenna state did not match: expected ${stateDescription(expected)}, got ${stateDescription(lastState)}`,
    );
  }
  throw lastError ?? new K4ControlError("K4 antenna state could not be read");
}

export async function switchAntenna(options) {
  const expected = expectedState(options);
  await sendCommands(options, setCommands(expected));
  if (options.settleMs > 0) await delay(options.settleMs);
  const actual = await waitForExpectedState(options, expected);
  return { expected, actual };
}

export async function verifyAntenna(options) {
  const expected = expectedState(options);
  const actual = await waitForExpectedState(options, expected);
  return { expected, actual };
}

export async function runCli(action, argv) {
  try {
    const options = parseOptions(argv);
    const result =
      action === "switch" ? await switchAntenna(options) : await verifyAntenna(options);
    process.stdout.write(
      `${JSON.stringify({
        ok: true,
        action,
        target: options.target,
        mode: options.mode,
        direction: options.direction,
        expected: result.expected,
        actual: result.actual,
      })}\n`,
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${JSON.stringify({ ok: false, action, error: message })}\n`);
    process.exitCode = 1;
  }
}
