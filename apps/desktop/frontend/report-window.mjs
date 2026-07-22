import { invokeReportWindowDocument } from "./bridge.mjs";
import { createReportDocumentUrls, releaseReportFrame } from "./models.mjs";

const frame = document.querySelector("[data-report-window-frame]");
const error = document.querySelector("[data-report-window-error]");
const errorDetail = document.querySelector("[data-report-window-error-detail]");
const reportDocuments = createReportDocumentUrls(window);

function humanize(value, fallback) {
  if (!value) return fallback;
  return value.replaceAll("_", " ").replace(/\b\w/gu, (letter) => letter.toUpperCase());
}

function release() {
  releaseReportFrame(frame, reportDocuments);
}

window.addEventListener("pagehide", release, { once: true });
window.addEventListener("beforeunload", release, { once: true });

try {
  const invoke = window.__TAURI__?.core?.invoke;
  if (typeof invoke !== "function") throw new Error("The restricted native bridge is unavailable.");
  const report = await invokeReportWindowDocument(invoke);
  const kind = report.documentKind === "summary" ? "Summary" : "Full evidence";
  document.title = `AntennaBench ${kind} · revision ${report.revision ?? "legacy"}`;
  document.querySelector("[data-report-window-kind]").textContent = kind;
  document.querySelector("[data-report-window-session]").textContent = report.bundleName;
  document.querySelector("[data-report-window-session]").title = report.sessionId;
  document.querySelector("[data-report-window-lifecycle]").textContent = humanize(
    report.lifecycle,
    "Static",
  );
  document.querySelector("[data-report-window-revision]").textContent = `Revision ${report.revision ?? "legacy"}`;
  frame.title = `AntennaBench ${kind} report · revision ${report.revision ?? "legacy"}`;
  const url = reportDocuments.create(report.html);
  frame.dataset.reportDocumentUrl = url;
  frame.dataset.presentationId = String(report.presentationId);
  frame.dataset.reportMode = report.documentKind;
  frame.src = url;
} catch (failure) {
  frame.hidden = true;
  error.hidden = false;
  errorDetail.textContent = failure?.detail ?? failure?.message ?? "Close this window and retry from Local report.";
}
