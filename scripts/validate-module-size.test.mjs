import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  countNonTestSourceLines,
  validateModuleSizes,
  validateRepository,
} from "./validate-module-size.mjs";

const BASE_CONFIG = Object.freeze({
  version: 1,
  default_budget: 3,
  allowlist_slack: 1,
  allowlist: {},
});

test("counts only lines before the first test configuration marker", () => {
  assert.equal(countNonTestSourceLines("one\ntwo\n#[cfg(test)]\nmod tests {}\n"), 2);
  assert.equal(countNonTestSourceLines("one\ntwo\nthree\n"), 3);
  assert.equal(countNonTestSourceLines("one\r\ntwo\r\n"), 2);
});

test("rejects an unlisted module over the default budget", () => {
  withRepository({ "crates/example/src/lib.rs": lines(4) }, (root) => {
    const { errors } = validateModuleSizes(root, BASE_CONFIG);
    assert.equal(errors.length, 1);
    assert.match(errors[0], /crates\/example\/src\/lib\.rs/);
    assert.match(errors[0], /measured 4/);
    assert.match(errors[0], /effective budget 3/);
    assert.match(errors[0], /Decompose the module/);
    assert.match(errors[0], /raise its allowlist entry/);
  });
});

test("rejects an allowlist entry that no longer ratchets closely to the file", () => {
  withRepository({ "crates/example/src/lib.rs": lines(4) }, (root) => {
    const config = {
      ...BASE_CONFIG,
      allowlist: { "crates/example/src/lib.rs": 6 },
    };
    const { errors } = validateModuleSizes(root, config);
    assert.equal(errors.length, 1);
    assert.match(errors[0], /stale allowlist budget 6/);
    assert.match(errors[0], /measured 4/);
    assert.match(errors[0], /1-line slack/);
  });
});

test("excludes inline test modules from the measured source size", () => {
  const source = `${lines(3)}#[cfg(test)]\nmod tests {\n${lines(20)}}\n`;
  withRepository({ "apps/desktop/src/session.rs": source }, (root) => {
    const { errors, measurements } = validateModuleSizes(root, BASE_CONFIG);
    assert.deepEqual(errors, []);
    assert.equal(measurements[0].measured, 3);
  });
});

test("accepts a passing default file and a tightly seeded exception", () => {
  withRepository(
    {
      "crates/example/src/lib.rs": lines(3),
      "apps/desktop/src/large.rs": lines(5),
    },
    (root) => {
      const config = {
        ...BASE_CONFIG,
        allowlist: { "apps/desktop/src/large.rs": 5 },
      };
      assert.deepEqual(validateModuleSizes(root, config).errors, []);
    },
  );
});

test("the committed repository baseline satisfies the module-size policy", () => {
  assert.deepEqual(validateRepository(process.cwd()).errors, []);
});

function withRepository(files, run) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "antennabench-module-size-"));
  try {
    for (const [file, content] of Object.entries(files)) {
      const absolute = path.join(root, file);
      fs.mkdirSync(path.dirname(absolute), { recursive: true });
      fs.writeFileSync(absolute, content);
    }
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function lines(count) {
  return Array.from({ length: count }, (_, index) => `line ${index + 1}`).join("\n") + "\n";
}
