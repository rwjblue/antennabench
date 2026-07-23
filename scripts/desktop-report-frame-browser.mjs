import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { basename, resolve } from "node:path";

const [fullPath, summaryPath] = process.argv.slice(2).map((value) => resolve(value));
assert.ok(fullPath && summaryPath, "expected Full evidence and Summary HTML paths");

const reports = {
  full: readFileSync(fullPath, "utf8"),
  summary: readFileSync(summaryPath, "utf8"),
};
const desktopHtml = readFileSync(resolve("apps/desktop/frontend/index.html"), "utf8");
const desktopStyles = readFileSync(resolve("apps/desktop/frontend/styles.css"), "utf8");
const models = readFileSync(resolve("apps/desktop/frontend/models.mjs"), "utf8");
const bridge = readFileSync(resolve("apps/desktop/frontend/bridge.mjs"), "utf8");
const reportWindowHtml = readFileSync(resolve("apps/desktop/frontend/report-window.html"), "utf8");
const reportWindowScript = readFileSync(resolve("apps/desktop/frontend/report-window.mjs"), "utf8");
const reportWindowStyles = readFileSync(resolve("apps/desktop/frontend/report-window.css"), "utf8");
const embeddedStyles = {
  full: readFileSync(resolve("apps/desktop/frontend/report.css"), "utf8"),
  summary: readFileSync(resolve("apps/desktop/frontend/report-summary.css"), "utf8"),
};
const desktopConfig = JSON.parse(readFileSync(resolve("apps/desktop/tauri.conf.json"), "utf8"));
const csp = desktopConfig.app.security.csp;
const session = `antennabench-report-frame-${process.pid}`;

assert.match(csp, /style-src 'self'/);
assert.doesNotMatch(csp, /style-src[^;]*'unsafe-inline'/);
assert.match(csp, /frame-src 'self' blob:/);
for (const mode of ["full", "summary"]) {
  const styleStart = reports[mode].indexOf("<style>") + 7;
  const styleEnd = reports[mode].indexOf("</style>", styleStart);
  assert.equal(
    embeddedStyles[mode].trimEnd(),
    reports[mode].slice(styleStart, styleEnd).trimEnd(),
    `apps/desktop/frontend/report${mode === "summary" ? "-summary" : ""}.css is stale`,
  );
}

const geometryFixture = `<section data-geometry-regression aria-hidden="true">
  <div class="path-strip">
    <span class="path-strip-track"><span class="path-strip-dot geometry-left g200" data-geometry="negative"></span></span>
    <span class="path-strip-track"><span class="path-strip-dot geometry-left g500" data-geometry="zero"></span></span>
    <span class="path-strip-track"><span class="path-strip-dot geometry-left g800" data-geometry="positive"></span></span>
    <span class="path-strip-track"><span class="path-strip-median geometry-left g650" data-geometry="median"></span></span>
  </div>
  <div class="chart">
    <div class="chart-row"><span>Width</span><span class="bar-track"><span class="bar usable geometry-width g250" data-geometry="proportional-width"></span></span><span>25%</span></div>
    <div class="chart-row"><span>Position</span><span class="azimuth-track"><span class="azimuth-marker geometry-left g750" data-geometry="other-position"></span></span><span>75%</span></div>
    <div class="chart-row"><span>Range</span><span class="snr-track"><span class="snr-range-position geometry-left g100" data-geometry="range-position"><span class="snr-range geometry-width g600" data-geometry="range-width"></span></span></span><span>10–70%</span></div>
  </div>
</section>`;
for (const mode of ["full", "summary"]) {
  reports[mode] = reports[mode].replace("</main>", `${geometryFixture}</main>`);
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
  if (!json) return stdout;
  const output = JSON.parse(stdout);
  assert.equal(output.success, true, output.error ?? `agent-browser ${args[0]} failed`);
  return output.data;
}

async function reportWorkspaceGeometry() {
  return (await browser(["eval", `(() => {
    const rect = (selector) => {
      const element = document.querySelector(selector);
      const bounds = element.getBoundingClientRect();
      return {
        top: bounds.top,
        bottom: bounds.bottom,
        height: bounds.height,
        clientHeight: element.clientHeight,
        scrollHeight: element.scrollHeight,
      };
    };
    const frame = document.querySelector("[data-report-frame]");
    const frameDocument = frame.contentDocument;
    const notices = document.querySelector(".report-notices");
    return {
      viewport: { width: innerWidth, height: innerHeight },
      document: {
        clientHeight: document.documentElement.clientHeight,
        scrollHeight: document.documentElement.scrollHeight,
        scrollTop: document.scrollingElement.scrollTop,
      },
      appShell: rect(".app-shell"),
      workspace: rect(".workspace"),
      content: rect(".content"),
      panel: rect('[data-panel="report"]'),
      viewer: rect(".report-viewer"),
      viewerTracks: getComputedStyle(document.querySelector(".report-viewer")).gridTemplateRows,
      toolbar: rect(".report-toolbar"),
      notices: {
        ...rect(".report-notices"),
        visible: notices.getClientRects().length > 0,
      },
      frame: {
        ...rect("[data-report-frame]"),
        computedHeight: getComputedStyle(frame).height,
        alignSelf: getComputedStyle(frame).alignSelf,
        documentClientHeight: frameDocument?.documentElement.clientHeight ?? 0,
        documentScrollHeight: frameDocument?.documentElement.scrollHeight ?? 0,
      },
    };
  })()`], { json: true })).result;
}

function assertBoundedReportWorkspace(geometry, { noticesVisible }) {
  const tolerance = 2;
  assert.equal(geometry.document.clientHeight, geometry.viewport.height);
  assert.equal(geometry.document.scrollHeight, geometry.document.clientHeight);
  assert.equal(geometry.document.scrollTop, 0);
  for (const [name, box] of Object.entries({
    appShell: geometry.appShell,
    workspace: geometry.workspace,
    content: geometry.content,
    panel: geometry.panel,
    viewer: geometry.viewer,
  })) {
    assert.ok(box.height > 0, `${name} must have a definite positive height`);
    assert.ok(
      box.scrollHeight <= box.clientHeight + tolerance,
      `${name} became an outer scroll owner (${box.scrollHeight}px > ${box.clientHeight}px)`,
    );
  }
  assert.ok(
    Math.abs(geometry.panel.height - geometry.content.clientHeight) <= tolerance,
    `report panel ${geometry.panel.height}px did not fill content ${geometry.content.clientHeight}px`,
  );
  assert.ok(
    Math.abs(geometry.viewer.height - geometry.panel.clientHeight) <= tolerance,
    `report viewer ${geometry.viewer.height}px did not fill panel ${geometry.panel.clientHeight}px`,
  );
  assert.ok(
    Math.abs(geometry.frame.bottom - geometry.viewer.bottom) <= tolerance,
    `report frame ended at ${geometry.frame.bottom}px before viewer ${geometry.viewer.bottom}px `
      + `(tracks ${geometry.viewerTracks}, computed height ${geometry.frame.computedHeight}, `
      + `align-self ${geometry.frame.alignSelf})`,
  );
  assert.ok(geometry.frame.height > 0, "report frame must consume remaining reader height");
  assert.ok(
    geometry.toolbar.top >= geometry.viewer.top - tolerance
      && geometry.toolbar.bottom <= geometry.viewer.bottom + tolerance,
    "report toolbar must remain inside the bounded viewer",
  );
  assert.equal(geometry.notices.visible, noticesVisible);
  if (noticesVisible) {
    assert.ok(
      geometry.notices.top >= geometry.toolbar.bottom - tolerance
        && geometry.notices.bottom <= geometry.frame.top + tolerance,
      "report notices must remain visible between toolbar and frame",
    );
  }
  assert.ok(
    geometry.frame.documentScrollHeight > geometry.frame.documentClientHeight,
    "the iframe document must remain the sole routine report scroll owner",
  );
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
  if (url.pathname === "/models.mjs") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(models);
    return;
  }
  if (url.pathname === "/bridge.mjs") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(bridge);
    return;
  }
  if (url.pathname === "/report-window.mjs") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(reportWindowScript);
    return;
  }
  if (url.pathname === "/report-window.css") {
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end(reportWindowStyles);
    return;
  }
  if (url.pathname === "/report-window-mock.mjs") {
    const kind = url.searchParams.get("kind") === "full_evidence" ? "full_evidence" : "summary";
    const revision = Number(url.searchParams.get("revision") ?? "16");
    const fail = url.searchParams.get("fail") === "1";
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(`
      const originalRevoke = URL.revokeObjectURL.bind(URL);
      window.__reportWindowRevoked = [];
      URL.revokeObjectURL = (url) => {
        window.__reportWindowRevoked.push(url);
        originalRevoke(url);
      };
      window.__TAURI__ = { core: { invoke(command) {
        if (command !== "report_window_document") throw new Error("restricted mock command denied");
        if (${JSON.stringify(fail)}) return Promise.reject({ detail: "bounded native reader failure" });
        return Promise.resolve({
          windowLabel: "report-browser-${kind}-${revision}",
          presentationId: ${revision},
          sessionId: "session-${revision}",
          bundleName: "canonical-${revision}.session.wsprabundle",
          revision: ${revision},
          lifecycle: "ended",
          documentKind: ${JSON.stringify(kind)},
          html: ${JSON.stringify(reports[kind === "summary" ? "summary" : "full"])},
        });
      } } };
    `);
    return;
  }
  if (url.pathname === "/report-window.html") {
    response.writeHead(200, {
      "content-security-policy": csp,
      "content-type": "text/html; charset=utf-8",
    });
    response.end(reportWindowHtml.replace(
      '<script type="module" src="report-window.mjs"></script>',
      `<script type="module" src="/report-window-mock.mjs${url.search}"></script><script type="module" src="report-window.mjs"></script>`,
    ));
    return;
  }
  if (url.pathname === "/report-window-comparison") {
    response.writeHead(200, {
      "content-security-policy": csp,
      "content-type": "text/html; charset=utf-8",
    });
    response.end(`<!doctype html><html><head><meta charset="utf-8"><title>Immutable report-window comparison</title><link rel="stylesheet" href="/comparison.css"></head><body><iframe title="Summary revision 16" src="/report-window.html?kind=summary&revision=16"></iframe><iframe title="Full evidence revision 17" src="/report-window.html?kind=full_evidence&revision=17"></iframe></body></html>`);
    return;
  }
  if (url.pathname === "/comparison.css") {
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end("html,body{width:100%;height:100%;margin:0;overflow:hidden}body{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:12px;background:#dfe6e1}iframe{width:100%;height:100%;min-width:0;border:1px solid #afbeb6;border-radius:12px;background:#fff}");
    return;
  }
  if (url.pathname === "/shell.css") {
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end("html,body{margin:0}iframe{display:block;width:100%;height:800px;border:0}");
    return;
  }
  if (url.pathname === "/styles.css") {
    response.writeHead(200, { "content-type": "text/css; charset=utf-8" });
    response.end(desktopStyles);
    return;
  }
  if (url.pathname === "/desktop") {
    response.writeHead(200, {
      "content-security-policy": csp,
      "content-type": "text/html; charset=utf-8",
    });
    response.end(desktopHtml);
    return;
  }
  if (url.pathname === "/report.css" || url.pathname === "/report-summary.css") {
    const mode = url.pathname === "/report.css" ? "full" : "summary";
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
        reportMode: mode === "full" ? "full_evidence" : "summary",
        session: { reportHtml: reports.full, summaryHtml: reports.summary },
      };
      const reportDocuments = createReportDocumentUrls(window);
      frame.addEventListener("load", () => { document.body.dataset.reportLoaded = "true"; }, { once: true });
      updateReportFrame(frame, state, reportDocuments);
      window.noopReportUpdate = () => updateReportFrame(frame, state, reportDocuments);
      window.switchReportMode = (reportMode) => {
        document.body.dataset.reportLoaded = "false";
        frame.addEventListener("load", () => {
          document.body.dataset.reportLoaded = "true";
        }, { once: true });
        state.reportMode = reportMode;
        return updateReportFrame(frame, state, reportDocuments);
      };
    `);
    return;
  }
  if (url.pathname === "/standalone-full" || url.pathname === "/standalone-summary") {
    const mode = url.pathname.endsWith("summary") ? "summary" : "full";
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(reports[mode]);
    return;
  }
  if (url.pathname === "/full" || url.pathname === "/summary") {
    response.writeHead(200, {
      "content-security-policy": csp,
      "content-type": "text/html; charset=utf-8",
    });
    response.end(`<!doctype html><html><head><meta charset="utf-8"><title>Report frame browser regression</title><link rel="stylesheet" href="/shell.css"></head>
      <body><iframe id="report" title="AntennaBench session report" sandbox="allow-same-origin" referrerpolicy="no-referrer"></iframe>
      <script type="module" src="/harness.mjs"></script></body></html>`);
    return;
  }
  response.writeHead(404).end();
});

await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
const { port } = server.address();

try {
  await browser(["set", "viewport", "1200", "900"]);
  const desktopPageUrl = `http://127.0.0.1:${port}/desktop`;
  await browser(["open", desktopPageUrl], { json: true });
  await browser(["wait", "body"]);
  await browser(["eval", `(() => {
    const checkbox = document.querySelector('[data-setup-field="controllerManualReviewRequired"]');
    if (!checkbox) throw new Error("controller manual-review checkbox not found");
    for (let element = checkbox.parentElement; element; element = element.parentElement) {
      element.hidden = false;
    }
    checkbox.scrollIntoView({ block: "center" });
  })()`], { json: true });
  const accessibilitySnapshot = await browser(["snapshot", "-i"]);
  assert.match(
    accessibilitySnapshot,
    /checkbox "After each switch, wait for me to confirm the antenna is ready"/,
  );
  for (const width of [1200, 500]) {
    await browser(["set", "viewport", String(width), "900"]);
    const controls = (await browser(["eval", `(() => {
      const checkbox = document.querySelector('[data-setup-field="controllerManualReviewRequired"]');
      const label = checkbox.closest("label");
      const help = label.nextElementSibling;
      const inputRect = checkbox.getBoundingClientRect();
      const labelText = [...label.childNodes].find(
        (node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim(),
      );
      const textRange = document.createRange();
      textRange.selectNode(labelText);
      const textRect = textRange.getBoundingClientRect();
      const confirmationWidths = [...document.querySelectorAll(".authority-confirmation input")]
        .filter((input) => input.getClientRects().length > 0)
        .map((input) => input.getBoundingClientRect().width);
      return {
        checkboxWidth: inputRect.width,
        checkboxHeight: inputRect.height,
        labelDisplay: getComputedStyle(label).display,
        labelText: label.textContent.trim(),
        inlineOverlap: Math.min(inputRect.bottom, textRect.bottom) - Math.max(inputRect.top, textRect.top),
        checkboxBeforeText: inputRect.right <= textRect.left,
        helpImmediatelyBelow: help === label.nextElementSibling && help.tagName === "SMALL",
        helpId: help.id,
        describedBy: checkbox.getAttribute("aria-describedby"),
        confirmationWidths,
      };
    })()`], { json: true })).result;
    assert.ok(controls.checkboxWidth > 0 && controls.checkboxWidth <= 24, `${width}px checkbox width`);
    assert.ok(controls.checkboxHeight > 0 && controls.checkboxHeight <= 24, `${width}px checkbox height`);
    assert.equal(controls.labelDisplay, "flex");
    assert.match(controls.labelText, /After each switch, wait for me to confirm the antenna is ready/);
    assert.ok(controls.inlineOverlap > 0, `${width}px checkbox and label text do not share a line`);
    assert.equal(controls.checkboxBeforeText, true);
    assert.equal(controls.helpImmediatelyBelow, true);
    assert.equal(controls.describedBy, controls.helpId);
    assert.ok(controls.confirmationWidths.length >= 2);
    assert.ok(controls.confirmationWidths.every((checkboxWidth) => checkboxWidth <= 24));
  }
  const toggled = (await browser(["eval", `(() => {
    const checkbox = document.querySelector('[data-setup-field="controllerManualReviewRequired"]');
    const label = checkbox.closest("label");
    const initiallyChecked = checkbox.checked;
    label.click();
    const afterLabelClick = checkbox.checked;
    checkbox.focus();
    return { initiallyChecked, afterLabelClick };
  })()`], { json: true })).result;
  assert.deepEqual(toggled, { initiallyChecked: true, afterLabelClick: false });
  await browser(["press", "Space"]);
  const keyboardState = (await browser(["eval", `(() => {
    const checkbox = document.querySelector('[data-setup-field="controllerManualReviewRequired"]');
    const label = checkbox.closest("label");
    return {
      checked: checkbox.checked,
      focusOutline: getComputedStyle(label).outlineStyle,
    };
  })()`], { json: true })).result;
  assert.equal(keyboardState.checked, true);
  assert.equal(keyboardState.focusOutline, "solid");

  await browser(["set", "viewport", "1120", "760"]);
  const desktopShell = (await browser(["eval", `(() => {
    const content = document.querySelector(".content");
    const workspace = document.querySelector(".workspace").getBoundingClientRect();
    const sidebar = document.querySelector(".sidebar").getBoundingClientRect();
    const brand = document.querySelector(".sidebar-brand");
    const brandRect = brand.getBoundingClientRect();
    content.scrollTop = 0;
    content.focus();
    return {
      documentClientHeight: document.documentElement.clientHeight,
      documentScrollHeight: document.documentElement.scrollHeight,
      documentScrollTop: document.scrollingElement.scrollTop,
      contentClientHeight: content.clientHeight,
      contentScrollHeight: content.scrollHeight,
      contentOverflow: getComputedStyle(content).overflowY,
      topbarCount: document.querySelectorAll(".topbar").length,
      workspace: { top: workspace.top, bottom: workspace.bottom },
      sidebar: { top: sidebar.top, bottom: sidebar.bottom },
      brand: {
        top: brandRect.top,
        bottom: brandRect.bottom,
        display: getComputedStyle(brand).display,
        href: brand.getAttribute("href"),
      },
    };
  })()`], { json: true })).result;
  assert.equal(desktopShell.documentScrollHeight, desktopShell.documentClientHeight);
  assert.equal(desktopShell.documentScrollTop, 0);
  assert.ok(desktopShell.contentScrollHeight > desktopShell.contentClientHeight * 2);
  assert.equal(desktopShell.contentOverflow, "auto");
  assert.equal(desktopShell.topbarCount, 0);
  assert.ok(desktopShell.workspace.top >= 0 && desktopShell.workspace.bottom <= 760);
  assert.ok(desktopShell.sidebar.top >= 0 && desktopShell.sidebar.bottom <= 760);
  assert.equal(desktopShell.brand.display, "grid");
  assert.equal(desktopShell.brand.href, "#saved");
  assert.ok(desktopShell.brand.top > desktopShell.sidebar.top);
  assert.ok(desktopShell.brand.bottom <= desktopShell.sidebar.bottom);

  const reportHierarchy = (await browser(["eval", `(() => {
    const content = document.querySelector(".content");
    const workspace = document.querySelector(".workspace");
    const viewer = document.querySelector("[data-report-viewer]");
    document.querySelector("[data-report-placeholder]").hidden = true;
    viewer.hidden = false;
    const history = document.querySelector("[data-operational-history]");
    const diagnostics = document.querySelector("[data-report-diagnostics-dialog]");
    const alert = document.querySelector("[data-operational-history-alert]");
    const frame = document.querySelector("[data-report-frame]");
    window.__setReportWorkspaceActive = (active) => {
      for (const panel of document.querySelectorAll("[data-panel]")) {
        panel.hidden = panel.dataset.panel !== (active ? "report" : "setup");
      }
      document.querySelector("[data-report-panel-heading]").hidden = active;
      content.classList.toggle("report-reading-active", active);
      workspace.classList.toggle("report-reading-active", active);
      document.querySelector(".app-shell").classList.toggle("report-reading-active", active);
    };
    window.__setReportWorkspaceActive(true);
    document.querySelector("[data-report-bundle]").textContent = "canonical-sample.session.wsprabundle";
    document.querySelector("[data-report-revision]").textContent = "Revision 16 · ended";
    const status = document.querySelector("[data-report-status]");
    status.textContent = "Full detail · revision 16";
    status.classList.remove("muted");
    document.querySelector("[data-report-summary]").textContent = "K1ABC · FN42 · 2 antennas · 16 slots · 1,025 observations";
    history.hidden = false;
    alert.hidden = true;
    frame.hidden = false;
    frame.src = "/standalone-summary";
    content.scrollTop = 0;
    const contentRect = content.getBoundingClientRect();
    const frameRect = frame.getBoundingClientRect();
    const toolbarRect = document.querySelector(".report-toolbar").getBoundingClientRect();
    const modeControl = document.querySelector(".report-mode-control").getBoundingClientRect();
    return {
      nativeDialog: diagnostics.tagName,
      open: diagnostics.open,
      alertVisible: alert.getClientRects().length > 0,
      detailVisible: history.getClientRects().length > 0,
      frameBeginsInReadingPath: frameRect.top >= contentRect.top && frameRect.top < contentRect.bottom,
      frameTop: frameRect.top,
      contentBottom: contentRect.bottom,
      contentOverflow: getComputedStyle(content).overflowY,
      contentScrollHeight: content.scrollHeight,
      contentClientHeight: content.clientHeight,
      toolbarVisible: toolbarRect.top >= contentRect.top && toolbarRect.bottom <= contentRect.bottom,
      modeControlVisible: modeControl.left >= toolbarRect.left && modeControl.right <= toolbarRect.right,
      summarySelected: document.querySelector('[data-report-mode="summary"]')
        .getAttribute("aria-pressed"),
      fullSelected: document.querySelector('[data-report-mode="full_evidence"]')
        .getAttribute("aria-pressed"),
      sidebarVisible: document.querySelector(".sidebar").getClientRects().length > 0,
    };
  })()`], { json: true })).result;
  assert.equal(reportHierarchy.nativeDialog, "DIALOG");
  assert.equal(reportHierarchy.open, false);
  assert.equal(reportHierarchy.alertVisible, false);
  assert.equal(reportHierarchy.detailVisible, false);
  assert.equal(reportHierarchy.contentOverflow, "hidden");
  assert.equal(reportHierarchy.contentScrollHeight, reportHierarchy.contentClientHeight);
  assert.equal(reportHierarchy.toolbarVisible, true);
  assert.equal(reportHierarchy.modeControlVisible, true);
  assert.equal(reportHierarchy.summarySelected, "true");
  assert.equal(reportHierarchy.fullSelected, "false");
  assert.equal(reportHierarchy.sidebarVisible, false);
  assert.equal(reportHierarchy.frameBeginsInReadingPath, true,
    `report frame began at ${reportHierarchy.frameTop}px after the ${reportHierarchy.contentBottom}px reading path`);
  await browser(["wait", "200"]);
  const standardReportGeometry = await reportWorkspaceGeometry();
  assert.deepEqual(standardReportGeometry.viewport, { width: 1120, height: 760 });
  assertBoundedReportWorkspace(standardReportGeometry, { noticesVisible: false });
  const normalTransitions = (await browser(["eval", `(async () => {
    if (typeof document.startViewTransition !== "function") {
      return { supported: false };
    }
    const run = async (direction, active) => {
      document.documentElement.dataset.reportTransition = direction;
      const transition = document.startViewTransition(
        () => window.__setReportWorkspaceActive(active),
      );
      await transition.ready;
      const animations = document.getAnimations()
        .map((animation) => ({
          duration: Number(animation.effect?.getTiming?.().duration ?? 0),
          pseudo: animation.effect?.target?.pseudoElement ?? "",
        }));
      await transition.finished;
      return animations;
    };
    const exitAnimations = await run("exit", false);
    const enterAnimations = await run("enter", true);
    delete document.documentElement.dataset.reportTransition;
    return {
      supported: true,
      exitAnimations,
      enterAnimations,
      activePanel: document.querySelector("[data-panel]:not([hidden])")?.dataset.panel,
      transitionStateCleared: !("reportTransition" in document.documentElement.dataset),
    };
  })()`], { json: true })).result;
  assert.equal(normalTransitions.supported, true);
  assert.ok(normalTransitions.exitAnimations.some(({ duration }) => duration >= 120));
  assert.ok(normalTransitions.enterAnimations.some(({ duration }) => duration >= 120));
  assert.equal(
    [...normalTransitions.exitAnimations, ...normalTransitions.enterAnimations]
      .some(({ pseudo }) => pseudo.includes("report-document")),
    false,
    "the sandboxed report document must not participate in shell animation",
  );
  assert.equal(normalTransitions.activePanel, "report");
  assert.equal(normalTransitions.transitionStateCleared, true);
  assertBoundedReportWorkspace(await reportWorkspaceGeometry(), { noticesVisible: false });

  const interruptedTransitions = (await browser(["eval", `(async () => {
    document.documentElement.dataset.reportTransition = "exit";
    const exiting = document.startViewTransition(
      () => window.__setReportWorkspaceActive(false),
    );
    await exiting.updateCallbackDone;
    exiting.skipTransition();
    document.documentElement.dataset.reportTransition = "enter";
    const entering = document.startViewTransition(
      () => window.__setReportWorkspaceActive(true),
    );
    await Promise.allSettled([exiting.finished, entering.finished]);
    delete document.documentElement.dataset.reportTransition;
    return {
      activePanel: document.querySelector("[data-panel]:not([hidden])")?.dataset.panel,
      visiblePanels: [...document.querySelectorAll("[data-panel]")]
        .filter((panel) => panel.getClientRects().length > 0).length,
      classes: {
        appShell: document.querySelector(".app-shell").classList.contains("report-reading-active"),
        workspace: document.querySelector(".workspace").classList.contains("report-reading-active"),
        content: document.querySelector(".content").classList.contains("report-reading-active"),
      },
      transitionStateCleared: !("reportTransition" in document.documentElement.dataset),
    };
  })()`], { json: true })).result;
  assert.deepEqual(interruptedTransitions, {
    activePanel: "report",
    visiblePanels: 1,
    classes: { appShell: true, workspace: true, content: true },
    transitionStateCleared: true,
  });
  assertBoundedReportWorkspace(await reportWorkspaceGeometry(), { noticesVisible: false });

  await browser(["set", "media", "light", "reduced-motion"]);
  const reducedTransition = (await browser(["eval", `(async () => {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    document.documentElement.dataset.reportTransition = "exit";
    const transition = document.startViewTransition(
      () => window.__setReportWorkspaceActive(false),
    );
    await transition.ready;
    const durations = document.getAnimations()
      .map((animation) => Number(animation.effect?.getTiming?.().duration ?? 0));
    await transition.finished;
    window.__setReportWorkspaceActive(true);
    delete document.documentElement.dataset.reportTransition;
    return { reduced, durations };
  })()`], { json: true })).result;
  assert.equal(reducedTransition.reduced, true);
  assert.ok(
    reducedTransition.durations.every((duration) => duration <= 1),
    `reduced-motion transition durations were ${reducedTransition.durations.join(", ")}`,
  );
  await browser(["set", "media", "light"]);
  assertBoundedReportWorkspace(await reportWorkspaceGeometry(), { noticesVisible: false });
  const pendingUpdate = (await browser(["eval", `(() => {
    const frame = document.querySelector("[data-report-frame]");
    const sourceBefore = frame.getAttribute("src");
    const control = document.querySelector("[data-report-update]");
    document.querySelector("[data-report-update-revision]").textContent = "Revision 17";
    control.hidden = false;
    const toolbarRect = document.querySelector(".report-toolbar").getBoundingClientRect();
    const controlRect = control.getBoundingClientRect();
    return {
      sourceBefore,
      sourceAfter: frame.getAttribute("src"),
      controlVisible: control.getClientRects().length > 0,
      controlWithinToolbar: controlRect.left >= toolbarRect.left
        && controlRect.right <= toolbarRect.right
        && controlRect.top >= toolbarRect.top
        && controlRect.bottom <= toolbarRect.bottom,
      revision: document.querySelector("[data-report-update-revision]").textContent,
      action: document.querySelector("[data-report-update-button]").textContent.trim(),
      summarySelected: document.querySelector('[data-report-mode="summary"]')
        .getAttribute("aria-pressed"),
      fullSelected: document.querySelector('[data-report-mode="full_evidence"]')
        .getAttribute("aria-pressed"),
    };
  })()`], { json: true })).result;
  assert.equal(pendingUpdate.sourceAfter, pendingUpdate.sourceBefore);
  assert.equal(pendingUpdate.controlVisible, true);
  assert.equal(pendingUpdate.controlWithinToolbar, true);
  assert.equal(pendingUpdate.revision, "Revision 17");
  assert.equal(pendingUpdate.action, "Update report");
  assert.equal(pendingUpdate.summarySelected, "true");
  assert.equal(pendingUpdate.fullSelected, "false");
  const pendingReportGeometry = await reportWorkspaceGeometry();
  assertBoundedReportWorkspace(pendingReportGeometry, { noticesVisible: false });
  assert.ok(
    pendingReportGeometry.frame.height < standardReportGeometry.frame.height,
    "pending-update toolbar row must recompute the remaining iframe height",
  );
  await browser(["screenshot", resolve("target/desktop-report-browser/report-pending-update-1120x760.png")]);
  await browser(["eval", `document.querySelector("[data-report-update]").hidden = true`], { json: true });
  await browser(["wait", "1000"]);
  await browser(["eval", `(() => {
    document.querySelector("[data-report-diagnostics-dialog]").showModal();
    document.querySelector("[data-report-diagnostics-close]").focus();
  })()`], { json: true });
  const expandedHistory = (await browser(["eval", `(() => {
    const history = document.querySelector("[data-operational-history]");
    const diagnostics = document.querySelector("[data-report-diagnostics-dialog]");
    return {
      open: diagnostics.open,
      detailVisible: history.querySelector(".operational-history-detail").getClientRects().length > 0,
      focused: document.activeElement === document.querySelector("[data-report-diagnostics-close]"),
    };
  })()`], { json: true })).result;
  assert.deepEqual(expandedHistory, { open: true, detailVisible: true, focused: true });
  await browser(["screenshot", resolve("target/desktop-report-browser/report-diagnostics-1120x760.png")]);
  await browser(["eval", `document.querySelector("[data-report-diagnostics-dialog]").close()`], { json: true });
  await browser(["screenshot", resolve("target/desktop-report-browser/report-workspace-1120x760.png")]);
  const materialWarning = (await browser(["eval", `(() => {
    const alert = document.querySelector("[data-operational-history-alert]");
    document.querySelector("[data-operational-history]").dataset.state = "persistence_gap";
    alert.hidden = false;
    alert.querySelector("strong").textContent = "Operational history has a known persistence gap";
    alert.querySelector("span").textContent = "Open Diagnostics for detail.";
    return {
      historyOpen: document.querySelector("[data-report-diagnostics-dialog]").open,
      alertVisible: alert.getClientRects().length > 0,
      alertRole: alert.getAttribute("role"),
      alertText: alert.textContent.trim(),
      nestedOverflow: getComputedStyle(document.querySelector(".report-notices")).overflowY,
    };
  })()`], { json: true })).result;
  assert.equal(materialWarning.historyOpen, false);
  assert.equal(materialWarning.alertVisible, true);
  assert.equal(materialWarning.alertRole, "alert");
  assert.match(materialWarning.alertText, /known persistence gap/);
  assert.notEqual(materialWarning.nestedOverflow, "scroll");
  const warningReportGeometry = await reportWorkspaceGeometry();
  assertBoundedReportWorkspace(warningReportGeometry, { noticesVisible: true });
  assert.ok(
    warningReportGeometry.frame.height < standardReportGeometry.frame.height,
    "material warning must recompute the remaining iframe height",
  );
  await browser(["screenshot", resolve("target/desktop-report-browser/report-material-warning-1120x760.png")]);
  const exportDialog = (await browser(["eval", `(() => {
    const dialog = document.querySelector("[data-report-export-dialog]");
    dialog.showModal();
    document.querySelector("[data-report-export-close]").focus();
    return {
      open: dialog.open,
      labelledBy: dialog.getAttribute("aria-labelledby"),
      summaryAction: document.querySelector("[data-report-export-summary]").textContent.trim(),
      fullAction: document.querySelector("[data-report-export-full]").textContent.trim(),
      focused: document.activeElement === document.querySelector("[data-report-export-close]"),
    };
  })()`], { json: true })).result;
  assert.deepEqual(exportDialog, {
    open: true,
    labelledBy: "report-export-title",
    summaryAction: "Save Summary HTML",
    fullAction: "Save Full evidence HTML",
    focused: true,
  });
  await browser(["screenshot", resolve("target/desktop-report-browser/report-export-1120x760.png")]);
  await browser(["eval", `(() => {
    document.querySelector("[data-report-export-dialog]").close();
    document.querySelector("[data-operational-history-alert]").hidden = true;
    document.querySelector("[data-report-update]").hidden = false;
  })()`], { json: true });
  const minimumWindow = desktopConfig.app.windows[0];
  assert.deepEqual(
    { width: minimumWindow.minWidth, height: minimumWindow.minHeight },
    { width: 820, height: 620 },
    "update the minimum report geometry fixture when the desktop window contract changes",
  );
  await browser([
    "set", "viewport", String(minimumWindow.minWidth), String(minimumWindow.minHeight),
  ]);
  const compactPendingUpdate = (await browser(["eval", `(() => {
    const control = document.querySelector("[data-report-update]").getBoundingClientRect();
    const toolbar = document.querySelector(".report-toolbar").getBoundingClientRect();
    return {
      withinToolbar: control.left >= toolbar.left
        && control.right <= toolbar.right
        && control.top >= toolbar.top
        && control.bottom <= toolbar.bottom,
      withinViewport: control.left >= 0 && control.right <= innerWidth,
      summarySelected: document.querySelector('[data-report-mode="summary"]')
        .getAttribute("aria-pressed"),
    };
  })()`], { json: true })).result;
  assert.deepEqual(compactPendingUpdate, {
    withinToolbar: true,
    withinViewport: true,
    summarySelected: "true",
  });
  const minimumPendingGeometry = await reportWorkspaceGeometry();
  assert.deepEqual(minimumPendingGeometry.viewport, { width: 820, height: 620 });
  assertBoundedReportWorkspace(minimumPendingGeometry, { noticesVisible: false });
  await browser(["screenshot", resolve("target/desktop-report-browser/report-pending-update-820x620.png")]);
  await browser(["eval", `document.querySelector("[data-report-update]").hidden = true`], { json: true });
  await browser(["screenshot", resolve("target/desktop-report-browser/report-workspace-820x620.png")]);
  const narrowHistory = (await browser(["eval", `(() => {
    const diagnostics = document.querySelector("[data-report-diagnostics-dialog]");
    const history = document.querySelector("[data-operational-history]");
    diagnostics.showModal();
    const rect = history.getBoundingClientRect();
    return {
      overflowX: getComputedStyle(history).overflowX,
      overflowY: getComputedStyle(history).overflowY,
      withinDocumentWidth: rect.right <= document.documentElement.scrollWidth,
      detailVisible: history.querySelector(".operational-history-detail").getClientRects().length > 0,
    };
  })()`], { json: true })).result;
  assert.notEqual(narrowHistory.overflowX, "scroll");
  assert.notEqual(narrowHistory.overflowY, "scroll");
  assert.equal(narrowHistory.withinDocumentWidth, true);
  assert.equal(narrowHistory.detailVisible, true);
  await browser(["set", "viewport", "1120", "760"]);
  await browser(["eval", `(() => {
    document.querySelector("[data-report-diagnostics-dialog]").close();
    document.querySelector(".app-shell").classList.remove("report-reading-active");
    document.querySelector(".workspace").classList.remove("report-reading-active");
    document.querySelector(".content").classList.remove("report-reading-active");
    for (const panel of document.querySelectorAll("[data-panel]")) panel.hidden = panel.dataset.panel !== "setup";
    const content = document.querySelector(".content");
    content.scrollTop = 0;
    content.focus();
  })()`], { json: true });
  await browser(["press", "PageDown"]);
  await browser(["wait", "200"]);
  const pagedShell = (await browser(["eval", `(() => {
    const content = document.querySelector(".content");
    const workspace = document.querySelector(".workspace").getBoundingClientRect();
    const sidebar = document.querySelector(".sidebar").getBoundingClientRect();
    const brand = document.querySelector(".sidebar-brand").getBoundingClientRect();
    return {
      contentScrollTop: content.scrollTop,
      documentScrollTop: document.scrollingElement.scrollTop,
      workspace: { top: workspace.top, bottom: workspace.bottom },
      sidebar: { top: sidebar.top, bottom: sidebar.bottom },
      brand: { top: brand.top, bottom: brand.bottom },
    };
  })()`], { json: true })).result;
  assert.ok(pagedShell.contentScrollTop > 0);
  assert.equal(pagedShell.documentScrollTop, 0);
  assert.deepEqual(pagedShell.workspace, desktopShell.workspace);
  assert.deepEqual(pagedShell.sidebar, desktopShell.sidebar);
  assert.deepEqual(pagedShell.brand, {
    top: desktopShell.brand.top,
    bottom: desktopShell.brand.bottom,
  });

  const focusedReview = (await browser(["eval", `(() => {
    const content = document.querySelector(".content");
    const review = document.querySelector("[data-setup-review]");
    review.hidden = false;
    content.scrollTop = 0;
    review.focus({ preventScroll: true });
    review.scrollIntoView({ block: "start" });
    const contentRect = content.getBoundingClientRect();
    const reviewRect = review.getBoundingClientRect();
    const guidanceRect = review.querySelector(".eyebrow").getBoundingClientRect();
    return {
      contentScrollTop: content.scrollTop,
      documentScrollTop: document.scrollingElement.scrollTop,
      contentPaddingTop: Number.parseFloat(getComputedStyle(content).paddingTop),
      reviewOffset: reviewRect.top - contentRect.top,
      guidanceVisible: guidanceRect.top >= contentRect.top && guidanceRect.bottom < contentRect.bottom,
      visibleBottom: reviewRect.top < contentRect.bottom,
      createAfterCycleTable: review.querySelector(".review-cycle-detail")
        .compareDocumentPosition(review.querySelector("[data-create-session]"))
        & Node.DOCUMENT_POSITION_FOLLOWING,
    };
  })()`], { json: true })).result;
  assert.ok(focusedReview.contentScrollTop > 0);
  assert.equal(focusedReview.documentScrollTop, 0);
  assert.ok(
    focusedReview.reviewOffset >= focusedReview.contentPaddingTop - 2,
    `review offset ${focusedReview.reviewOffset}px crossed ${focusedReview.contentPaddingTop}px content padding`,
  );
  assert.equal(focusedReview.guidanceVisible, true);
  assert.equal(focusedReview.visibleBottom, true);
  assert.ok(focusedReview.createAfterCycleTable > 0);

  await browser(["set", "viewport", "900", "760"]);
  const compactShell = (await browser(["eval", `(() => ({
    documentClientHeight: document.documentElement.clientHeight,
    documentScrollHeight: document.documentElement.scrollHeight,
    contentOverflow: getComputedStyle(document.querySelector(".content")).overflowY,
    workspaceColumns: getComputedStyle(document.querySelector(".workspace")).gridTemplateColumns,
    navigationColumns: getComputedStyle(document.querySelector(".sidebar nav")).gridTemplateColumns,
    brandDisplay: getComputedStyle(document.querySelector(".sidebar-brand")).display,
    topbarCount: document.querySelectorAll(".topbar").length,
  }))()`], { json: true })).result;
  assert.ok(compactShell.documentScrollHeight > compactShell.documentClientHeight);
  assert.equal(compactShell.contentOverflow, "visible");
  assert.doesNotMatch(compactShell.workspaceColumns, /\s/);
  assert.equal(compactShell.navigationColumns.trim().split(/\s+/).length, 4);
  assert.equal(compactShell.brandDisplay, "none");
  assert.equal(compactShell.topbarCount, 0);

  const summaryReaderUrl = `http://127.0.0.1:${port}/report-window.html?kind=summary&revision=16`;
  await browser(["set", "viewport", "980", "780"]);
  await browser(["open", summaryReaderUrl], { json: true });
  await browser(["wait", "iframe[src^='blob:']"]);
  const immutableReader = (await browser(["eval", `(() => {
    const frame = document.querySelector("[data-report-window-frame]");
    const embedded = frame.contentDocument;
    return {
      title: document.title,
      kind: document.querySelector("[data-report-window-kind]").textContent,
      session: document.querySelector("[data-report-window-session]").textContent,
      lifecycle: document.querySelector("[data-report-window-lifecycle]").textContent,
      revision: document.querySelector("[data-report-window-revision]").textContent,
      sandbox: frame.getAttribute("sandbox"),
      source: frame.getAttribute("src"),
      embeddedScripts: embedded.scripts.length,
      embeddedSummary: embedded.querySelector(".summary") !== null,
      topLevelOverflow: getComputedStyle(document.body).overflow,
    };
  })()`], { json: true })).result;
  assert.equal(immutableReader.title, "AntennaBench Summary · revision 16");
  assert.equal(immutableReader.kind, "Summary");
  assert.equal(immutableReader.session, "canonical-16.session.wsprabundle");
  assert.equal(immutableReader.lifecycle, "Ended");
  assert.equal(immutableReader.revision, "Revision 16");
  assert.equal(immutableReader.sandbox, "allow-same-origin");
  assert.match(immutableReader.source, /^blob:http:\/\/127\.0\.0\.1:/u);
  assert.equal(immutableReader.embeddedScripts, 0);
  assert.equal(immutableReader.embeddedSummary, true);
  assert.equal(immutableReader.topLevelOverflow, "hidden");
  await browser(["screenshot", resolve("target/desktop-report-browser/report-window-summary-980x780.png")]);

  await browser(["set", "viewport", "700", "620"]);
  const compactReader = (await browser(["eval", `(() => {
    const identity = document.querySelector(".reader-identity").getBoundingClientRect();
    const frame = document.querySelector("[data-report-window-frame]").getBoundingClientRect();
    return {
      identityWithinViewport: identity.left >= 0 && identity.right <= innerWidth,
      frameWithinViewport: frame.left >= 0 && frame.right <= innerWidth && frame.bottom <= innerHeight,
      metadataColumns: getComputedStyle(document.querySelector(".reader-identity dl")).gridTemplateColumns,
    };
  })()`], { json: true })).result;
  assert.equal(compactReader.identityWithinViewport, true);
  assert.equal(compactReader.frameWithinViewport, true);
  assert.equal(compactReader.metadataColumns.trim().split(/\s+/u).length, 2);

  await browser(["set", "viewport", "1600", "900"]);
  await browser(["open", `http://127.0.0.1:${port}/report-window-comparison`], { json: true });
  await browser(["wait", "1500"]);
  const comparison = (await browser(["eval", `(() => [...document.querySelectorAll("body > iframe")].map((reader) => {
    const shell = reader.contentDocument;
    const report = shell.querySelector("[data-report-window-frame]");
    return {
      outerTitle: reader.title,
      kind: shell.querySelector("[data-report-window-kind]").textContent,
      revision: shell.querySelector("[data-report-window-revision]").textContent,
      source: report.getAttribute("src"),
      scripts: report.contentDocument.scripts.length,
    };
  }))()`], { json: true })).result;
  assert.deepEqual(comparison.map(({ kind, revision }) => ({ kind, revision })), [
    { kind: "Summary", revision: "Revision 16" },
    { kind: "Full evidence", revision: "Revision 17" },
  ]);
  assert.ok(comparison.every(({ source }) => source.startsWith("blob:http://127.0.0.1:")));
  assert.ok(comparison.every(({ scripts }) => scripts === 0));
  await browser(["screenshot", resolve("target/desktop-report-browser/report-windows-side-by-side-1600x900.png")]);

  await browser(["open", summaryReaderUrl], { json: true });
  await browser(["wait", "iframe[src^='blob:']"]);
  const released = (await browser(["eval", `(() => {
    window.dispatchEvent(new Event("pagehide"));
    return {
      revoked: window.__reportWindowRevoked.length,
      source: document.querySelector("[data-report-window-frame]").getAttribute("src"),
    };
  })()`], { json: true })).result;
  assert.deepEqual(released, { revoked: 1, source: null });

  await browser(["open", `${summaryReaderUrl}&fail=1`], { json: true });
  await browser(["wait", "[data-report-window-error]:not([hidden])"]);
  const readerFailure = (await browser(["eval", `(() => ({
    detail: document.querySelector("[data-report-window-error-detail]").textContent,
    frameHidden: document.querySelector("[data-report-window-frame]").hidden,
  }))()`], { json: true })).result;
  assert.deepEqual(readerFailure, { detail: "bounded native reader failure", frameHidden: true });

  await browser(["set", "viewport", "1200", "900"]);
  for (const mode of ["full", "summary"]) {
    const standaloneUrl = `http://127.0.0.1:${port}/standalone-${mode}`;
    await browser(["open", standaloneUrl], { json: true });
    await browser(["wait", ".panel"]);
    const standalone = (await browser(["eval", `(() => {
      const ratio = (selector, dimension) => {
        const element = document.querySelector(selector);
        const track = element.parentElement;
        const value = dimension === "left"
          ? Number.parseFloat(getComputedStyle(element).left)
          : element.getBoundingClientRect().width;
        return value / track.getBoundingClientRect().width;
      };
      const style = (selector) => getComputedStyle(document.querySelector(selector));
      return {
        scripts: document.scripts.length,
        inlineStyles: document.querySelectorAll("[style]").length,
        bodyBackground: style("body").backgroundColor,
        panelBackground: style(".panel").backgroundColor,
        heroDisplay: style(".hero").display,
        negative: ratio('[data-geometry="negative"]', "left"),
        proportionalWidth: ratio('[data-geometry="proportional-width"]', "width"),
      };
    })()`], { json: true })).result;
    assert.equal(standalone.scripts, 0);
    assert.equal(standalone.inlineStyles, 0);
    assert.equal(standalone.bodyBackground, "rgb(245, 247, 251)");
    assert.equal(standalone.panelBackground, "rgb(255, 255, 255)");
    assert.equal(standalone.heroDisplay, mode === "summary" ? "block" : "grid");
    assert.ok(Math.abs(standalone.negative - 0.2) < 0.01);
    assert.ok(Math.abs(standalone.proportionalWidth - 0.25) < 0.01);
    if (mode === "summary") {
      const focusOrder = (await browser(["eval", `(() => {
        const selector = [
          'a[href]',
          'button:not([disabled])',
          'input:not([disabled])',
          'select:not([disabled])',
          'textarea:not([disabled])',
          'summary',
          '[tabindex="0"]',
        ].join(',');
        return [...document.querySelectorAll(selector)]
          .filter((element) => element.getClientRects().length > 0)
          .map((element) => element.tagName);
      })()`], { json: true })).result;
      assert.deepEqual(focusOrder, [
        "A",
        "SUMMARY",
        "SUMMARY",
        "SUMMARY",
        "A",
        "A",
        "A",
        "SUMMARY",
        "SUMMARY",
      ]);

      const pdfPath = resolve("target/desktop-report-browser/summary-us-letter.pdf");
      await browser(["pdf", pdfPath]);
      const pdf = readFileSync(pdfPath).toString("latin1");
      const printPageCount = pdf.match(/\/Type\s*\/Page\b/g)?.length ?? 0;
      assert.equal(printPageCount, 2, `canonical Summary US Letter page baseline changed`);
      assert.ok(printPageCount <= 2, `Summary printed to ${printPageCount} pages`);
    }
  }

  for (const mode of ["full", "summary"]) {
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
    assert.equal(shell.sandbox, "allow-same-origin");
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
        summaryOpening: ${mode === "summary"} ? (() => {
          const overview = document.querySelector(".summary-overview");
          const limitation = document.querySelector(".summary-principal-limitation");
          return {
            overviewTop: overview.getBoundingClientRect().top,
            limitationBottom: limitation.getBoundingClientRect().bottom,
            viewportHeight: innerHeight,
            findingCount: document.querySelectorAll(".summary-finding").length,
            populationCount: document.querySelectorAll(".summary-finding-population").length,
            exactDetailOpen: document.querySelector(".summary-condition-detail").open,
            methodsOpen: document.querySelector(".answerability-disclosure").open,
            primarySectionCount: document.querySelectorAll("main.summary > section.panel").length,
            aggregateCount: document.querySelectorAll(".summary-path-aggregate svg").length,
            rawTabStopCount: document.querySelectorAll('[tabindex="0"]').length,
            auditVisualCount: document.querySelectorAll([
              ".path-distribution-dot-group",
              ".common-opportunity-rate-cell",
              ".coverage-world",
              ".coverage-polar",
            ].join(",")).length,
            readingRuleCount: document.querySelectorAll(".summary-reading-rules li").length,
            disclosureCount: document.querySelectorAll("details").length,
            provenanceOpen: document.querySelector(".summary-reference").open,
          };
        })() : null,
      };
    })()`);
    assert.equal(styles.scripts, 0);
    assert.equal(styles.bodyBackground, "rgb(245, 247, 251)");
    assert.match(styles.bodyFont, /system-ui/);
    assert.equal(styles.panelBackground, "rgb(255, 255, 255)");
    assert.equal(styles.panelBorderStyle, "solid");
    assert.equal(styles.tableCollapse, "collapse");
    assert.equal(styles.heroDisplay, mode === "summary" ? "block" : "grid");
    if (mode === "summary") {
      assert.equal(styles.summaryOpening.findingCount, 3);
      assert.equal(styles.summaryOpening.populationCount, 3);
      assert.equal(styles.summaryOpening.exactDetailOpen, false);
      assert.equal(styles.summaryOpening.methodsOpen, false);
      assert.ok(styles.summaryOpening.primarySectionCount <= 4);
      assert.ok(styles.summaryOpening.aggregateCount > 0);
      assert.equal(styles.summaryOpening.rawTabStopCount, 0);
      assert.equal(styles.summaryOpening.auditVisualCount, 0);
      assert.equal(styles.summaryOpening.readingRuleCount, 2);
      assert.equal(styles.summaryOpening.disclosureCount, 5);
      assert.equal(styles.summaryOpening.provenanceOpen, false);
      assert.ok(styles.summaryOpening.overviewTop >= 0);
      assert.ok(
        styles.summaryOpening.limitationBottom <= styles.summaryOpening.viewportHeight,
        `Summary limitation missed the initial viewport: ${JSON.stringify(styles.summaryOpening)}`,
      );
    } else {
      assert.equal(styles.summaryOpening, null);
    }

    const geometry = await evaluateReportFrame(pageUrl, `(() => {
      const ratio = (selector, dimension) => {
        const element = document.querySelector(selector);
        const track = element.parentElement;
        const value = dimension === "left"
          ? Number.parseFloat(getComputedStyle(element).left)
          : element.getBoundingClientRect().width;
        return value / track.getBoundingClientRect().width;
      };
      return {
        inlineStyles: document.querySelectorAll("[style]").length,
        negative: ratio('[data-geometry="negative"]', "left"),
        zero: ratio('[data-geometry="zero"]', "left"),
        positive: ratio('[data-geometry="positive"]', "left"),
        median: ratio('[data-geometry="median"]', "left"),
        proportionalWidth: ratio('[data-geometry="proportional-width"]', "width"),
        otherPosition: ratio('[data-geometry="other-position"]', "left"),
        rangePosition: ratio('[data-geometry="range-position"]', "left"),
        rangeWidth: ratio('[data-geometry="range-width"]', "width"),
      };
    })()`);
    assert.equal(geometry.inlineStyles, 0);
    for (const [name, expected] of Object.entries({
      negative: 0.2,
      zero: 0.5,
      positive: 0.8,
      median: 0.65,
      proportionalWidth: 0.25,
      otherPosition: 0.75,
      rangePosition: 0.1,
      rangeWidth: 0.6,
    })) {
      assert.ok(Math.abs(geometry[name] - expected) < 0.01, `${mode} ${name}: ${geometry[name]}`);
    }

    const navigationTargets = await evaluateReportFrame(pageUrl, `Array.from(
      document.querySelectorAll('.question-nav a[href^="#"]'),
      (link) => link.getAttribute("href").slice(1),
    )`);
    assert.deepEqual(
      navigationTargets,
      mode === "full"
          ? [
            "what-run-show",
            "same-path-signal",
            "reach-unique-paths",
            "distance-direction",
            "coverage-overlap",
            "run-quality",
            "audit-appendix",
          ]
        : [
            "what-run-show",
            "same-path-signal",
            "observed-footprint",
          ],
    );
    const outerScrollBefore = (await browser([
      "eval",
      "document.scrollingElement.scrollTop",
    ], { json: true })).result;
    for (const targetId of navigationTargets) {
      const prepared = await evaluateReportFrame(pageUrl, `(() => {
        const link = document.querySelector('a[href="#${targetId}"]');
        const target = document.querySelector("#${targetId}");
        link.focus();
        return {
          linkFound: Boolean(link),
          targetFound: Boolean(target),
          linkFocused: document.activeElement === link,
          linkOutline: getComputedStyle(link).outlineStyle,
        };
      })()`);
      assert.deepEqual(prepared, {
        linkFound: true,
        targetFound: true,
        linkFocused: true,
        linkOutline: "solid",
      });
      await browser(["press", "Enter"]);
      await browser(["wait", "1500"]);
      const navigated = await evaluateReportFrame(pageUrl, `(() => {
        const target = document.querySelector("#${targetId}");
        const targetRect = target.getBoundingClientRect();
        return {
          hash: location.hash,
          activeId: document.activeElement.id,
          targetTop: targetRect.top,
          targetBottom: targetRect.bottom,
          targetOutline: getComputedStyle(target).outlineStyle,
          viewportHeight: innerHeight,
        };
      })()`);
      assert.equal(navigated.hash, `#${targetId}`);
      assert.equal(navigated.activeId, targetId);
      assert.equal(navigated.targetOutline, "solid");
      assert.ok(
        navigated.targetTop < navigated.viewportHeight && navigated.targetBottom > 0,
        `${mode} ${targetId}: ${JSON.stringify(navigated)}`,
      );
    }
    const outerScrollAfter = (await browser([
      "eval",
      "document.scrollingElement.scrollTop",
    ], { json: true })).result;
    assert.equal(outerScrollAfter, outerScrollBefore);

    await evaluateReportFrame(pageUrl, `(() => {
      const disclosure = document.querySelector("details");
      if (disclosure) disclosure.open = true;
      const focusTarget = document.querySelector("a,summary") ?? document.querySelector("main");
      focusTarget.tabIndex = -1;
      focusTarget.focus();
      scrollTo({
        top: Math.min(400, document.documentElement.scrollHeight - innerHeight),
        behavior: "instant",
      });
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

    await evaluateReportFrame(pageUrl, `(() => {
      const main = document.querySelector("main");
      main.tabIndex = -1;
      main.focus();
      scrollTo({ top: 0, behavior: "instant" });
    })()`);
    await browser(["press", "End"]);
    await browser(["wait", "200"]);
    const endScroll = await evaluateReportFrame(pageUrl, "scrollY");
    assert.ok(endScroll > 0, `${mode} End did not scroll the report document`);
    await browser(["press", "Home"]);
    await browser(["wait", "200"]);
    assert.equal(await evaluateReportFrame(pageUrl, "scrollY"), 0);
    await browser(["press", "PageDown"]);
    await browser(["wait", "200"]);
    const pageDownScroll = await evaluateReportFrame(pageUrl, "scrollY");
    assert.ok(pageDownScroll > 0, `${mode} Page Down did not scroll the report document`);
    await browser(["press", "PageUp"]);
    await browser(["wait", "200"]);
    assert.ok(
      await evaluateReportFrame(pageUrl, "scrollY") < pageDownScroll,
      `${mode} Page Up did not move back through the report document`,
    );

    await browser(["set", "viewport", "500", "900"]);
    const responsive = await evaluateReportFrame(pageUrl, `(() => ({
      tableHeadPosition: getComputedStyle(document.querySelector(".overview-table thead")).position,
      tableRowDisplay: getComputedStyle(document.querySelector(".overview-table tbody tr")).display,
      supportColumns: getComputedStyle(document.querySelector(
        ${JSON.stringify(mode === "summary" ? ".summary-populations" : ".overview-support")},
      )).gridTemplateColumns,
    }))()`);
    assert.equal(responsive.tableHeadPosition, "absolute");
    assert.equal(responsive.tableRowDisplay, "block");
    assert.doesNotMatch(responsive.supportColumns, /\s/);
    await browser(["set", "viewport", "1200", "900"]);

    const alternateMode = mode === "summary" ? "full_evidence" : "summary";
    const switched = (await browser([
      "eval",
      `window.switchReportMode(${JSON.stringify(alternateMode)})`,
    ], { json: true })).result;
    assert.equal(switched, true);
    await browser(["wait", "body[data-report-loaded='true']"]);
    const alternateHero = await evaluateReportFrame(
      pageUrl,
      "getComputedStyle(document.querySelector('.hero')).display",
    );
    assert.equal(alternateHero, alternateMode === "summary" ? "block" : "grid");
  }
  process.stdout.write(
    `Standalone and embedded ${basename(fullPath)} and ${basename(summaryPath)} retained report CSS under exact CSP boundaries.\n`,
  );
} finally {
  await browser(["close"]);
  await new Promise((resolveClose) => server.close(resolveClose));
}
