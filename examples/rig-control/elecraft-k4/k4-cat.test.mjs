import assert from "node:assert/strict";
import { createServer } from "node:net";
import test from "node:test";

import {
  expectedState,
  K4ControlError,
  parseOptions,
  switchAntenna,
  verifyAntenna,
} from "./k4-cat.mjs";

function options(...args) {
  return parseOptions([
    "--target",
    "2",
    "--mode",
    "whole_station_ab",
    "--direction",
    "receive",
    ...args,
  ]);
}

async function startFakeK4(
  initial = { transmit: 1, receive: 5 },
  { respondToQueries = true } = {},
) {
  const state = { ...initial };
  const commands = [];
  const server = createServer((socket) => {
    let buffer = "";
    // Deadline-driven clients may close while the fake is still flushing a split reply.
    socket.on("error", () => {});
    socket.on("data", (chunk) => {
      buffer += chunk.toString("ascii");
      const frames = buffer.split(";");
      buffer = frames.pop() ?? "";
      for (const frame of frames) {
        if (!frame) continue;
        commands.push(frame);
        if (frame === "AI0") continue;
        if (frame === "AN") {
          if (respondToQueries) socket.write(`AN${state.transmit};`, "ascii");
        } else if (frame === "AR") {
          // Split the response to exercise stream framing rather than packet assumptions.
          if (respondToQueries) {
            socket.write(`AR${state.receive}`, "ascii", () => socket.write(";", "ascii"));
          }
        } else if (/^AN[1-3]$/.test(frame)) {
          state.transmit = Number(frame.slice(2));
        } else if (/^AR[0-7]$/.test(frame)) {
          state.receive = Number(frame.slice(2));
        } else {
          socket.write(`${frame}?;`, "ascii");
        }
      }
    });
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert(address && typeof address === "object");

  return {
    state,
    commands,
    host: "127.0.0.1",
    port: address.port,
    async close() {
      await new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      );
    },
  };
}

test("durable experiment mode selects the required K4 paths", () => {
  assert.deepEqual(expectedState(options()), { transmit: 2, receive: 6 });
  assert.deepEqual(
    expectedState(
      parseOptions([
        "--target",
        "1",
        "--mode",
        "tx_focused",
        "--direction",
        "transmit",
      ]),
    ),
    { transmit: 1, receive: undefined },
  );
  assert.deepEqual(
    expectedState(
      parseOptions([
        "--target",
        "2",
        "--mode",
        "rx_focused",
        "--direction",
        "receive",
      ]),
    ),
    { transmit: undefined, receive: 6 },
  );
});

test("mode and intention direction must agree", () => {
  assert.throws(
    () =>
      parseOptions([
        "--target",
        "1",
        "--mode",
        "tx_focused",
        "--direction",
        "receive",
      ]),
    new K4ControlError("mode tx_focused cannot have a receive intention"),
  );
});

test("whole-station switching sets and reads back TX and main RX paths", async (t) => {
  const fake = await startFakeK4();
  t.after(() => fake.close());
  const configured = options(
    "--host",
    fake.host,
    "--port",
    String(fake.port),
    "--settle-ms",
    "0",
  );

  const result = await switchAntenna(configured);

  assert.deepEqual(result.actual, { transmit: 2, receive: 6 });
  assert.deepEqual(fake.state, { transmit: 2, receive: 6 });
  assert(fake.commands.includes("AN2"));
  assert(fake.commands.includes("AR6"));
  assert(fake.commands.includes("AN"));
  assert(fake.commands.includes("AR"));
});

test("focused modes leave the unrelated antenna path unchanged", async (t) => {
  const fake = await startFakeK4({ transmit: 1, receive: 5 });
  t.after(() => fake.close());

  await switchAntenna(
    parseOptions([
      "--host",
      fake.host,
      "--port",
      String(fake.port),
      "--target",
      "2",
      "--mode",
      "tx_focused",
      "--direction",
      "transmit",
      "--settle-ms",
      "0",
    ]),
  );
  assert.deepEqual(fake.state, { transmit: 2, receive: 5 });

  await switchAntenna(
    parseOptions([
      "--host",
      fake.host,
      "--port",
      String(fake.port),
      "--target",
      "2",
      "--mode",
      "rx_focused",
      "--direction",
      "receive",
      "--settle-ms",
      "0",
    ]),
  );
  assert.deepEqual(fake.state, { transmit: 2, receive: 6 });
});

test("switching never changes hardware when the bridge cannot read antenna state", async (t) => {
  const fake = await startFakeK4(
    { transmit: 1, receive: 5 },
    { respondToQueries: false },
  );
  t.after(() => fake.close());

  await assert.rejects(
    switchAntenna(
      parseOptions([
        "--host",
        fake.host,
        "--port",
        String(fake.port),
        "--target",
        "2",
        "--mode",
        "tx_focused",
        "--direction",
        "transmit",
        "--timeout-ms",
        "150",
        "--settle-ms",
        "0",
      ]),
    ),
    /timed out waiting for K4 antenna state/,
  );

  assert.deepEqual(fake.state, { transmit: 1, receive: 5 });
  assert(!fake.commands.includes("AN2"));
});

test("verification fails non-ambiguously when read-back never matches", async (t) => {
  const fake = await startFakeK4({ transmit: 1, receive: 5 });
  t.after(() => fake.close());
  const configured = options(
    "--host",
    fake.host,
    "--port",
    String(fake.port),
    "--timeout-ms",
    "150",
  );

  await assert.rejects(
    verifyAntenna(configured),
    /expected \{"transmit":"AN2","receive":"AR6"\}, got \{"transmit":"AN1","receive":"AR5"\}/,
  );
});
