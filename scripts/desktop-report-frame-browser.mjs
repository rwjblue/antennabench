import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { basename, resolve } from "node:path";

const [fullPath, compactPath] = process.argv.slice(2).map((value) => resolve(value));
assert.ok(fullPath && compactPath, "expected full and compact report HTML paths");

const reports = {
  full: readFileSync(fullPath, "utf8"),
  compact: readFileSync(compactPath, "utf8"),
};
const models = readFileSync(resolve("apps/desktop/frontend/models.mjs"), "utf8");
const embeddedStyles = {
  full: readFileSync(resolve("apps/desktop/frontend/report.css"), "utf8"),
  compact: readFileSync(resolve("apps/desktop/frontend/report-compact.css"), "utf8"),
};
const desktopConfig = JSON.parse(readFileSync(resolve("apps/desktop/tauri.conf.json"), "utf8"));
const csp = desktopConfig.app.security.csp;
const session = `antennabench-report-frame-${process.pid}`;

assert.match(csp, /style-src 'self'/);
assert.doesNotMatch(csp, /style-src[^;]*'unsafe-inline'/);
assert.match(csp, /frame-src 'self' blob:/);
for (const mode of ["full", "compact"]) {
  const styleStart = reports[mode].indexOf("<style>") + 7;
  const styleEnd = reports[mode].indexOf("</style>", styleStart);
  assert.equal(
    embeddedStyles[mode].trimEnd(),
    reports[mode].slice(styleStart, styleEnd).trimEnd(),
    `apps/desktop/frontend/report${mode === "compact" ? "-compact" : ""}.css is stale`,
  );
}

async function browser(args, { json = false } = {}) {
  const command = ["--session", session, ...(json ? ["--json"] : []), ...args];
  const child = spawn("agent-browser", command, {
    env: { ...process.env, AGENT_BROWSER_IDLE_TIMEOUT_MS: "60000" },
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
  child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
  const status = await new Promise((resolveExit, rejectExit) => {
    child.on("error", rejectExit);
    child.on("close", resolveExit);
  });
  assert.equal(
    status,
    0,
    `agent-browser ${args[0]} failed:\n${stdout}${stderr}`,
  );
  if (!json) return null;
  const output = JSON.parse(stdout);
  assert.equal(output.success, true, output.error ?? `agent-browser ${args[0]} failed`);
  return output.data;
}

async function evaluateReportFrame(pageUrl, expression) {
  const { cdpUrl } = await browser(["get", "cdp-url"], { json: true });
  const socket = new WebSocket(cdpUrl);
  await new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", rejectOpen, { once: true });
  });
  let nextId = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!message.id || !pending.has(message.id)) return;
    const { resolveCommand, rejectCommand } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) rejectCommand(new Error(message.error.message));
    else resolveCommand(message.result);
  });
  const command = (method, params = {}, sessionId = undefined) => new Promise(
    (resolveCommand, rejectCommand) => {
      nextId += 1;
      pending.set(nextId, { resolveCommand, rejectCommand });
      socket.send(JSON.stringify({ id: nextId, method, params, sessionId }));
    },
  );
  try {
    const { targetInfos } = await command("Target.getTargets");
    const target = targetInfos.find((candidate) => candidate.type === "page" && candidate.url === pageUrl);
    assert.ok(target, `browser target not found for ${pageUrl}`);
    const reportTarget = targetInfos.find(
      (candidate) => candidate.type === "iframe" && candidate.url.startsWith("blob:"),
    );
    if (reportTarget) {
      const { sessionId } = await command("Target.attachToTarget", {
        targetId: reportTarget.targetId,
        flatten: true,
      });
      const evaluation = await command("Runtime.evaluate", {
        expression,
        awaitPromise: true,
        returnByValue: true,
      }, sessionId);
      assert.equal(evaluation.exceptionDetails, undefined, evaluation.exceptionDetails?.text);
      return evaluation.result.value;
    }
    const { sessionId } = await command("Target.attachToTarget", {
      targetId: target.targetId,
      flatten: true,
    });
    const { frameTree } = await command("Page.getFrameTree", {}, sessionId);
    const reportFrame = frameTree.childFrames?.find((candidate) => candidate.frame.url.startsWith("blob:"));
    assert.ok(
      reportFrame,
      `sandboxed blob report frame was not attached: ${JSON.stringify(frameTree)}`,
    );
    const { executionContextId } = await command("Page.createIsolatedWorld", {
      frameId: reportFrame.frame.id,
      worldName: "antennabench-report-style-regression",
    }, sessionId);
    const evaluation = await command("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      contextId: executionContextId,
      returnByValue: true,
    }, sessionId);
    assert.equal(evaluation.exceptionDetails, undefined, evaluation.exceptionDetails?.text);
    return evaluation.result.value;
  } finally {
    socket.close();
  }
}

const server = createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  if (url.pathname === "/frontend/models.mjs") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(models);
    return;
  }
  if (url.pathname === "/shell.css") {
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end("html,body{margin:0}iframe{display:block;width:100%;height:800px;border:0}");
    return;
  }
  if (url.pathname === "/report.css" || url.pathname === "/report-compact.css") {
    const mode = url.pathname === "/report.css" ? "full" : "compact";
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end(embeddedStyles[mode]);
    return;
  }
  if (url.pathname === "/harness.mjs") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(`
      import { createReportDocumentUrls, updateReportFrame } from "/frontend/models.mjs";
      const reports = ${JSON.stringify(reports)};
      const mode = location.pathname.slice(1);
      const frame = document.querySelector("#report");
      const state = {
        reportPresentationId: 1,
        session: { reportHtml: reports[mode] },
      };
      const reportDocuments = createReportDocumentUrls(window);
      frame.addEventListener("load", () => { document.body.dataset.reportLoaded = "true"; }, { once: true });
      updateReportFrame(frame, state, reportDocuments);
      window.noopReportUpdate = () => updateReportFrame(frame, state, reportDocuments);
    `);
    return;
  }
  if (url.pathname === "/full" || url.pathname === "/compact") {
    response.writeHead(200, {
      "content-security-policy": csp,
      "content-type": "text/html; charset=utf-8",
    });
    response.end(`<!doctype html><html><head><meta charset="utf-8"><title>Report frame browser regression</title><link rel="stylesheet" href="/shell.css"></head>
      <body><iframe id="report" title="AntennaBench session report" sandbox="" referrerpolicy="no-referrer"></iframe>
      <script type="module" src="/harness.mjs"></script></body></html>`);
    return;
  }
  response.writeHead(404).end();
});

await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
const { port } = server.address();

try {
  await browser(["set", "viewport", "1200", "900"]);
  for (const mode of ["full", "compact"]) {
    const pageUrl = `http://127.0.0.1:${port}/${mode}`;
    await browser(["open", pageUrl], { json: true });
    await browser(["wait", "body[data-report-loaded='true']"]);
    const shell = (await browser(["eval", `(() => {
      const frame = document.querySelector("#report");
      return {
        sandbox: frame.getAttribute("sandbox"),
        source: frame.getAttribute("src"),
        srcdoc: frame.getAttribute("srcdoc"),
      };
    })()`], { json: true })).result;
    assert.equal(shell.sandbox, "");
    assert.match(shell.source, /^blob:http:\/\/127\.0\.0\.1:/);
    assert.equal(shell.srcdoc, null);

    const styles = await evaluateReportFrame(pageUrl, `(() => {
      const style = (selector) => getComputedStyle(document.querySelector(selector));
      return {
        scripts: document.scripts.length,
        bodyBackground: style("body").backgroundColor,
        bodyFont: style("body").fontFamily,
        panelBackground: style(".panel").backgroundColor,
        panelBorderStyle: style(".panel").borderTopStyle,
        tableCollapse: style("table").borderCollapse,
        heroDisplay: style(".hero").display,
      };
    })()`);
    assert.equal(styles.scripts, 0);
    assert.equal(styles.bodyBackground, "rgb(245, 247, 251)");
    assert.match(styles.bodyFont, /system-ui/);
    assert.equal(styles.panelBackground, "rgb(255, 255, 255)");
    assert.equal(styles.panelBorderStyle, "solid");
    assert.equal(styles.tableCollapse, "collapse");
    assert.equal(styles.heroDisplay, mode === "compact" ? "block" : "grid");

    await evaluateReportFrame(pageUrl, `(() => {
      const disclosure = document.querySelector("details");
      if (disclosure) disclosure.open = true;
      const focusTarget = document.querySelector("a,summary") ?? document.querySelector("main");
      focusTarget.tabIndex = -1;
      focusTarget.focus();
      scrollTo(0, Math.min(400, document.documentElement.scrollHeight - innerHeight));
    })()`);
    await browser(["wait", "500"]);
    const readerBefore = await evaluateReportFrame(pageUrl, `(() => {
      const disclosure = document.querySelector("details");
      return {
        focus: document.activeElement.tagName + ":" + document.activeElement.className,
        disclosureOpen: disclosure?.open ?? null,
        scrollY,
      };
    })()`);
    const noOp = (await browser(["eval", "window.noopReportUpdate()"], { json: true })).result;
    assert.equal(noOp, false);
    const unchangedShell = (await browser(["eval", "document.querySelector('#report').getAttribute('src')"], {
      json: true,
    })).result;
    assert.equal(unchangedShell, shell.source);
    const readerAfter = await evaluateReportFrame(pageUrl, `(() => {
      const disclosure = document.querySelector("details");
      return {
        focus: document.activeElement.tagName + ":" + document.activeElement.className,
        disclosureOpen: disclosure?.open ?? null,
        scrollY,
      };
    })()`);
    assert.deepEqual(readerAfter, readerBefore);

    await browser(["set", "viewport", "500", "900"]);
    const responsive = await evaluateReportFrame(pageUrl, `(() => ({
      tableHeadPosition: getComputedStyle(document.querySelector(".overview-table thead")).position,
      tableRowDisplay: getComputedStyle(document.querySelector(".overview-table tbody tr")).display,
      supportColumns: getComputedStyle(document.querySelector(".overview-support")).gridTemplateColumns,
    }))()`);
    assert.equal(responsive.tableHeadPosition, "absolute");
    assert.equal(responsive.tableRowDisplay, "block");
    assert.doesNotMatch(responsive.supportColumns, /\s/);
    await browser(["set", "viewport", "1200", "900"]);
  }
  process.stdout.write(
    `Embedded ${basename(fullPath)} and ${basename(compactPath)} retained report CSS under the desktop CSP.\n`,
  );
} finally {
  await browser(["close"]);
  await new Promise((resolveClose) => server.close(resolveClose));
}
