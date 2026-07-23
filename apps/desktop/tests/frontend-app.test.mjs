import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test, vi } from "vitest";

import { collectDesktopElements } from "../frontend/elements.mjs";

const DESKTOP_HTML = readFileSync(
  path.join(process.cwd(), "frontend", "index.html"),
  "utf8",
);
const DESKTOP_CSS = readFileSync(
  path.join(process.cwd(), "frontend", "styles.css"),
  "utf8",
);

function loadDesktopDocument() {
  document.open();
  document.write(DESKTOP_HTML);
  document.close();
  return collectDesktopElements(document);
}

function conductorView(overrides = {}) {
  return {
    sessionId: "session-1",
    revision: 1,
    actionToken: "token-1",
    lifecycle: "running",
    phase: "between_slots",
    antennaInUse: null,
    guidance: "Switch and confirm",
    secondsToTransition: null,
    now: "2026-07-18T14:00:00Z",
    currentSlot: null,
    nextSlot: null,
    nextIntent: {
      intentId: "intent-1",
      sequenceNumber: 1,
      direction: "transmit",
      antennaLabel: "DXC",
      band: "20m",
    },
    slots: [
      { slotId: "slot-1", sequenceNumber: 1, plannedAntenna: "DXC", band: "20m" },
    ],
    antennas: ["DXC", "Attic EFHW"],
    diagnostics: [],
    effectiveEvents: [],
    wsjtxRequired: false,
    ...overrides,
  };
}

test("controller manual review keeps native checkbox, label, and help semantics", () => {
  const elements = loadDesktopDocument();
  const checkbox = elements.setupForm.querySelector(
    '[data-setup-field="controllerManualReviewRequired"]',
  );
  const label = checkbox.closest("label");
  const help = label.nextElementSibling;

  assert.equal(label.className, "authority-confirmation");
  assert.match(label.textContent, /After each switch, wait for me to confirm the antenna is ready/);
  assert.equal(help.tagName, "SMALL");
  assert.equal(checkbox.getAttribute("aria-describedby"), help.id);
  assert.match(help.textContent, /Keep this checked for manual review/);
  assert.equal(checkbox.checked, true);
  label.click();
  assert.equal(checkbox.checked, false);
  checkbox.disabled = true;
  label.click();
  assert.equal(checkbox.checked, false);

  assert.match(DESKTOP_CSS, /\.field-grid label:not\(\.authority-confirmation\)/);
  assert.match(
    DESKTOP_CSS,
    /\.field-grid input:not\(\[type="checkbox"\]\):not\(\[type="radio"\]\)/,
  );
  assert.doesNotMatch(DESKTOP_CSS, /\.field-grid input, \.field-grid select/);
  assert.match(DESKTOP_CSS, /\.authority-confirmation:has\(input:focus-visible\)/);
});

test("the headless desktop relaunches into Saved sessions before creating a managed session", async () => {
  window.history.replaceState(null, "", "#saved");
  const elements = loadDesktopDocument();
  elements.setupReviewPanel.scrollIntoView = vi.fn();
  const reportDocumentUrls = [];
  Object.defineProperty(window.URL, "createObjectURL", {
    configurable: true,
    value: () => {
      const url = `blob:headless-report-${reportDocumentUrls.length + 1}`;
      reportDocumentUrls.push(url);
      return url;
    },
  });
  Object.defineProperty(window.URL, "revokeObjectURL", {
    configurable: true,
    value: vi.fn(),
  });
  vi.useFakeTimers();
  const calls = [];
  const review = {
    valid: true,
    reviewId: "review-headless",
    diagnostics: [],
    plan: {
      station: { callsign: "N1RWJ", grid: "FN42li", powerWatts: 5 },
      antennas: [
        { label: "DXC", context: "DX Commander" },
        { label: "Attic EFHW", context: "Attic end-fed half-wave" },
      ],
      mode: "tx_focused",
      goal: "general_coverage",
      wsprLiveAcquisitionEnabled: true,
      signalPlan: null,
      antennaController: {
        profileName: "Elecraft",
        invocation: "automatic",
        manualReviewRequired: true,
      },
      scheduleReview: {
        summary: "2 directed WSPR cycles; about 4 minutes of required cycle time.",
        counterbalanceExplanation: "Successive repetitions reverse the antenna order.",
        transitionSummary: "1 antenna transition.",
        transitions: [{ summary: "Change antenna" }],
      },
      capabilities: {
        canDescribe: ["Transmit coverage differences."],
        cannotEstablish: ["A universal winner."],
      },
      slots: [
        { sequenceNumber: 1, antennaLabel: "DXC", direction: "transmit", band: "20m", signal: null },
        { sequenceNumber: 2, antennaLabel: "Attic EFHW", direction: "transmit", band: "20m", signal: null },
      ],
    },
  };
  const reportHtml = "<!doctype html><meta data-antennabench-report-csp http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'sha256-YWJjZA=='; style-src-attr 'none'\"><style>body{color:#172033}</style><p>headless report</p>";
  const summaryHtml = "<!doctype html><meta data-antennabench-report-csp http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'sha256-YWJjZA=='; style-src-attr 'none'\"><style>body{color:#172033}</style><main class=\"summary\"><p>headless summary</p></main>";
  const session = {
    sessionId: "session-headless",
    bundleName: "headless.session.antennabundle",
    lifecycle: "running",
    schemaVersion: 4,
    reportAvailable: true,
    revision: 1,
  };
  let managedOpenAttempts = 0;
  let reportRefreshes = 0;
  let controllerProfileLoads = 0;
  let importedCatalog = false;
  let resolveSkipCycle;
  let controllerCatalog = {
    inputStyle: "one_line",
    profiles: [{
      profileId: "profile-1",
      revision: "revision-1",
      name: "Bench switch",
      timeoutSeconds: 12,
      switchCommand: {
        programTemplate: "/usr/local/bin/switch-antenna",
        argumentTemplates: ["--target", "{target}"],
      },
      verificationCommand: {
        programTemplate: "/usr/local/bin/check-antenna",
        argumentTemplates: ["{target}"],
      },
    }],
  };
  const existingCatalogEntry = {
    locatorId: "locator-existing",
    bundleName: "existing.session.antennabundle",
    origin: "managed",
    originLabel: "Saved by AntennaBench",
    status: "available",
    sessionId: "session-existing",
    callsign: "W1AW",
    createdAt: "2026-07-17T14:00:00Z",
    lifecycle: "ended",
    schemaVersion: 5,
    revision: 9,
    mode: "tx_focused",
    bands: ["20m"],
    antennaLabels: ["Dipole", "Vertical"],
    antennaCount: 2,
    sameSessionIdCount: 1,
    problems: [],
  };
  const importedCatalogEntry = {
    ...existingCatalogEntry,
    locatorId: "locator-imported-refreshed",
    bundleName: "imported.session.antennabundle",
    sessionId: "session-imported",
    callsign: "K1ABC",
  };
  const responses = {
    load_station_preferences: null,
    antenna_controller_profiles: () => {
      controllerProfileLoads += 1;
      if (controllerProfileLoads === 2) {
        throw new Error("catalog refresh temporarily unavailable");
      }
      return controllerCatalog;
    },
    save_antenna_controller_profile: () => {
      controllerCatalog = {
        inputStyle: "structured",
        profiles: [{
          profileId: "profile-1",
          revision: "revision-2",
          name: "Bench switch updated",
          timeoutSeconds: 15,
          switchCommand: {
            programTemplate: "/opt/antennabench/switch",
            argumentTemplates: ["--antenna", "{target}"],
          },
          verificationCommand: {
            programTemplate: "/opt/antennabench/verify",
            argumentTemplates: ["--expect", "{target}"],
          },
        }],
      };
      return controllerCatalog.profiles[0];
    },
    list_managed_sessions: () => ({
      status: "complete",
      diagnostics: [],
      entries: importedCatalog
        ? [existingCatalogEntry, importedCatalogEntry]
        : [existingCatalogEntry],
    }),
    import_managed_session: () => {
      importedCatalog = true;
      return {
        status: "imported",
        location: {
          locatorId: "locator-imported-temporary",
          bundleName: importedCatalogEntry.bundleName,
          origin: "managed",
          originLabel: "Saved by AntennaBench",
        },
      };
    },
    export_managed_session: {
      status: "exported",
      bundleName: "existing-copy.session.antennabundle",
      revision: 9,
    },
    request_station_location: {
      status: "success",
      latitude: 42.3601,
      longitude: -71.0589,
    },
    review_session_setup: review,
    create_session_from_review: {
      status: "created",
      session,
      managedLocation: {
        locatorId: "locator-headless",
        bundleName: session.bundleName,
        origin: "managed",
        originLabel: "Saved by AntennaBench",
      },
    },
    open_managed_session: (payload) => {
      if (payload.locatorId === "locator-imported-refreshed") return { status: "cancelled" };
      managedOpenAttempts += 1;
      if (managedOpenAttempts === 2) throw new Error("The saved bundle moved.");
      return {
        status: "opened",
        session: {
          ...session,
          sessionId: "session-existing",
          bundleName: "existing.session.antennabundle",
          lifecycle: "ended",
          revision: 9,
        },
      };
    },
    reveal_managed_session: undefined,
    refresh_active_session_report: (payload) => {
      if (reportRefreshes < 2) {
        assert.equal(
          payload,
          undefined,
          "the first report refresh for each newly activated session claims no prior presentation",
        );
      } else {
        assert.deepEqual(payload, { displayedPresentationId: 2 });
      }
      reportRefreshes += 1;
      return reportRefreshes === 1
        ? {
          presentationId: 1,
          reportHtml,
          summaryHtml,
          revision: 9,
          lifecycle: "ended",
          completeness: "full_detail",
        }
        : {
          presentationId: 2,
          reportHtml,
          summaryHtml,
          revision: 1,
          lifecycle: "running",
          completeness: "full_detail",
        };
    },
    export_active_session_report: ({ format }) => ({
      status: "confirmation_required",
      pendingExportId: `pending-${format}`,
      fileName: format === "summary_html" ? "existing-summary.html" : "existing-full.html",
      revision: 9,
      format,
    }),
    cancel_report_export: { status: "cancelled" },
    confirm_report_export: {
      status: "exported",
      fileName: "existing-full.html",
      revision: 9,
      format: "full_evidence_html",
    },
    active_session_conductor: conductorView({
      lifecycle: "ready",
      phase: "ready",
      wsjtxReadiness: {
        band: "20m",
        powerWatts: 5,
        wsprLiveAcquisitionEnabled: true,
        hasReceivePeriods: true,
        nextDirection: "transmit",
      },
    }),
    mutate_active_session_conductor: (payload) => payload.request.action.kind === "skip_wspr_cycle"
      ? new Promise((resolve) => { resolveSkipCycle = resolve; })
      : conductorView({ revision: 2 }),
    active_session_antenna_controller: {
      policy: "manual",
      attached: false,
      armed: false,
      targets: {},
    },
    active_session_wsjtx_status: {
      phase: "stopped",
      receivedDatagrams: 0,
      committedMutations: 0,
      ignoredDatagrams: 0,
      setupWarnings: [],
    },
    advance_active_session_wspr_live: { status: "disabled" },
  };
  const invoke = vi.fn(async (command, payload) => {
    calls.push([command, payload]);
    assert.ok(command in responses, `unexpected native command ${command}`);
    const response = responses[command];
    return typeof response === "function" ? response(payload, calls) : response;
  });
  window.__TAURI__ = { core: { invoke } };
  const uncaught = [];
  const recordError = (event) => uncaught.push(event.error ?? event.reason ?? event.message);
  window.addEventListener("error", recordError);
  window.addEventListener("unhandledrejection", recordError);
  await import("../frontend/app.mjs?headless-composition");
  try {
    await vi.waitFor(() => {
      assert.equal(document.querySelector('[data-panel="saved"]').hidden, false);
      assert.match(elements.savedCatalog.textContent, /W1AW/);
      assert.match(elements.savedCatalog.textContent, /View report/);
    });
    await vi.waitFor(() => {
      assert.ok(
        [...elements.controllerProfileSelect.options]
          .some((option) => option.value === "profile-1"),
      );
    });
    elements.controllerProfileSelect.value = "profile-1";
    window.dispatchEvent(new Event("pageshow"));
    const profileField = (name) => elements.setupForm.querySelector(
      `[data-setup-field="${name}"]`,
    );
    await vi.waitFor(() => {
      assert.equal(profileField("controllerProfileName").value, "Bench switch");
    });
    assert.equal(elements.controllerProfileSelect.value, "profile-1");
    assert.equal(profileField("controllerTimeoutSeconds").value, "12");
    assert.equal(
      profileField("controllerSwitchCommand").value,
      '"/usr/local/bin/switch-antenna" "--target" "{target}"',
    );
    assert.equal(
      profileField("controllerVerificationCommand").value,
      '"/usr/local/bin/check-antenna" "{target}"',
    );

    const preservedTarget = elements.setupForm.querySelector("[data-controller-target]");
    preservedTarget.value = "relay-preserved";
    elements.controllerProfileSelect.value = "";
    elements.controllerProfileSelect.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() => {
      assert.equal(profileField("controllerProfileName").value, "");
      assert.equal(profileField("controllerTimeoutSeconds").value, "10");
    });
    assert.equal(preservedTarget.value, "relay-preserved");
    elements.controllerProfileSelect.value = "profile-1";
    elements.controllerProfileSelect.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() => {
      assert.equal(profileField("controllerProfileName").value, "Bench switch");
    });

    profileField("controllerProfileName").value = "Unsaved local edit";
    profileField("controllerProfileName").dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(profileField("controllerProfileName").value, "Unsaved local edit");

    elements.controllerProfileSave.click();
    await vi.waitFor(() => {
      assert.equal(profileField("controllerProfileName").value, "Bench switch updated");
    });
    assert.equal(profileField("controllerTimeoutSeconds").value, "15");
    assert.equal(profileField("controllerSwitchProgram").value, "/opt/antennabench/switch");
    assert.equal(profileField("controllerSwitchArguments").value, "--antenna\n{target}");
    assert.equal(profileField("controllerVerificationProgram").value, "/opt/antennabench/verify");
    assert.equal(profileField("controllerVerificationArguments").value, "--expect\n{target}");
    assert.equal(elements.controllerProfileRefresh.hidden, false);
    assert.match(elements.controllerProfileStatus.textContent, /Profile save committed/);
    elements.controllerProfileRefresh.click();
    await vi.waitFor(() => assert.equal(elements.controllerProfileRefresh.hidden, true));
    assert.equal(
      calls.filter(([command]) => command === "save_antenna_controller_profile").length,
      1,
      "refresh recovery never resubmits the committed mutation",
    );
    elements.savedImport.click();
    await vi.waitFor(() => {
      assert.match(elements.savedImportFeedback.textContent, /imported\.session\.antennabundle was imported/);
      assert.match(elements.savedCatalog.textContent, /K1ABC/);
      assert.equal(elements.savedImportActions.hidden, false);
    });
    elements.savedImportReveal.click();
    await vi.waitFor(() => {
      assert.ok(calls.some(([command, payload]) => command === "reveal_managed_session"
        && payload.locatorId === "locator-imported-refreshed"));
    });
    elements.savedImportOpen.click();
    await vi.waitFor(() => {
      assert.ok(calls.some(([command, payload]) => command === "open_managed_session"
        && payload.locatorId === "locator-imported-refreshed"));
    });
    let exportButton = elements.savedCatalog.querySelector(
      '[data-locator-id="locator-existing"] [data-saved-action="export"]',
    );
    exportButton.click();
    await vi.waitFor(() => {
      exportButton = elements.savedCatalog.querySelector(
        '[data-locator-id="locator-existing"] [data-saved-action="export"]',
      );
      assert.equal(document.activeElement, exportButton, "row export restores keyboard focus");
      assert.match(
        elements.savedCatalog.querySelector('[data-locator-id="locator-existing"]').textContent,
        /Exported existing-copy\.session\.antennabundle/,
      );
    });
    let deleteButton = elements.savedCatalog.querySelector('[data-saved-action="delete"]');
    deleteButton.click();
    await vi.waitFor(() => assert.equal(elements.savedDeleteDialog.open, true));
    assert.equal(document.activeElement, elements.savedDeleteCancel, "Cancel receives default focus");
    assert.match(elements.savedDeleteIdentity.textContent, /W1AW.*existing\.session\.antennabundle/);
    elements.savedDeleteConfirm.focus();
    elements.savedDeleteDialog.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Tab",
      bubbles: true,
    }));
    assert.equal(document.activeElement, elements.savedDeleteCancel, "Tab stays inside the modal");
    elements.savedDeleteDialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    assert.equal(elements.savedDeleteDialog.open, false);
    assert.equal(document.activeElement.dataset.savedAction, "delete", "Escape/cancel restores row-action focus");
    assert.equal(document.activeElement.dataset.locatorId, "locator-existing");
    assert.equal(calls.some(([command]) => command === "delete_managed_session"), false);

    elements.savedCatalog.querySelector('[data-saved-action="open"][data-intent="report"]').click();
    await vi.waitFor(() => {
      assert.equal(window.location.hash, "#report");
      assert.ok(calls.some(([command, payload]) => command === "open_managed_session"
        && payload.locatorId === "locator-existing"));
      assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-1");
    });
    assert.equal(elements.mainContent.classList.contains("report-reading-active"), true);
    assert.equal(elements.reportActiveRunButton.hidden, true, "terminal sessions do not offer Active run");
    assert.equal(elements.reportSummaryModeButton.getAttribute("aria-pressed"), "true");
    elements.reportFullModeButton.click();
    await vi.waitFor(() => {
      assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-2");
      assert.equal(elements.reportFullModeButton.getAttribute("aria-pressed"), "true");
    });
    assert.deepEqual(window.URL.revokeObjectURL.mock.calls.at(-1), ["blob:headless-report-1"]);
    assert.equal(
      calls.filter(([command]) => command === "refresh_active_session_report").length,
      1,
      "mode switching performs no report refresh",
    );
    elements.reportDiagnosticsButton.click();
    await vi.waitFor(() => assert.equal(elements.reportDiagnosticsDialog.open, true));
    assert.equal(document.activeElement, elements.reportDiagnosticsClose);
    elements.reportDiagnosticsDialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    assert.equal(elements.reportDiagnosticsDialog.open, false);
    assert.equal(document.activeElement, elements.reportDiagnosticsButton);
    elements.reportExportButton.click();
    await vi.waitFor(() => assert.equal(elements.reportExportDialog.open, true));
    assert.equal(document.activeElement, elements.reportExportClose);
    elements.reportSummaryExportButton.click();
    await vi.waitFor(() => assert.equal(elements.reportReplaceDialog.open, true));
    assert.equal(document.activeElement, elements.reportReplaceCancel);
    assert.equal(elements.reportReplaceIdentity.textContent, "existing-summary.html");
    elements.reportReplaceConfirm.focus();
    elements.reportReplaceDialog.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Tab",
      bubbles: true,
    }));
    assert.equal(document.activeElement, elements.reportReplaceCancel, "report modal traps focus");
    elements.reportReplaceDialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    await vi.waitFor(() => {
      assert.equal(elements.reportReplaceDialog.open, false);
      assert.equal(document.activeElement, elements.reportSummaryExportButton);
    });
    assert.ok(calls.some(([command, payload]) => command === "cancel_report_export"
      && payload.pendingExportId === "pending-summary_html"));

    elements.reportSummaryModeButton.click();
    await vi.waitFor(() => {
      assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-3");
      assert.equal(elements.reportSummaryModeButton.getAttribute("aria-pressed"), "true");
    });
    elements.reportFullExportButton.click();
    await vi.waitFor(() => assert.equal(elements.reportReplaceDialog.open, true));
    elements.reportReplaceConfirm.click();
    elements.reportReplaceConfirm.click();
    await vi.waitFor(() => {
      assert.equal(elements.reportReplaceDialog.open, false);
      assert.equal(elements.reportExportDialog.open, false);
      assert.equal(document.activeElement, elements.reportExportButton);
      assert.match(elements.reportFeedback.textContent, /existing-full\.html/);
    });
    assert.equal(
      calls.filter(([command]) => command === "confirm_report_export").length,
      1,
      "replacement confirmation cannot be submitted twice",
    );
    elements.reportSavedButton.click();
    await vi.waitFor(() => assert.equal(window.location.hash, "#saved"));
    elements.savedCatalog.querySelector('[data-saved-action="open"][data-intent="report"]').click();
    await vi.waitFor(() => {
      assert.match(elements.savedCatalog.textContent, /saved bundle moved/);
      assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-3");
    });
    elements.savedNew.click();
    await vi.waitFor(() => {
      assert.equal(window.location.hash, "#setup");
      assert.equal(document.activeElement.id, "setup-title");
    });

    const callsign = elements.setupForm.querySelector('[data-setup-field="callsign"]');
    const grid = elements.setupForm.querySelector('[data-setup-field="grid"]');
    callsign.value = "n1rwj";
    callsign.dispatchEvent(new InputEvent("input", { bubbles: true }));
    grid.value = "fn42AB";
    grid.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(callsign.value, "N1RWJ");
    assert.equal(grid.value, "FN42ab");

    const mode = elements.setupForm.querySelector('[data-setup-field="mode"]');
    assert.match(elements.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);
    assert.match(elements.setupPlanSummary.textContent, /then a 5-minute WSPR\.live ingestion grace/);
    const wsprLive = elements.setupForm.querySelector(
      '[data-setup-field="wsprLiveAcquisitionEnabled"]',
    );
    wsprLive.checked = false;
    wsprLive.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.doesNotMatch(elements.setupPlanSummary.textContent, /ingestion grace/);
    wsprLive.checked = true;
    wsprLive.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.match(elements.setupPlanSummary.textContent, /then a 5-minute WSPR\.live ingestion grace/);
    mode.value = "tx_focused";
    mode.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.match(elements.setupPlanSummary.textContent, /8 planned WSPR cycles · about 16 minutes/);
    mode.value = "whole_station_ab";
    mode.dispatchEvent(new InputEvent("input", { bubbles: true }));

    const rounds = elements.setupForm.querySelector('[data-setup-field="rounds"]');
    rounds.value = "3";
    rounds.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.match(elements.setupPlanSummary.textContent, /12 planned WSPR cycles · about 24 minutes/);
    rounds.value = "4";
    rounds.dispatchEvent(new InputEvent("input", { bubbles: true }));

    elements.setupAddAntennaButton.click();
    assert.match(elements.setupPlanSummary.textContent, /24 planned WSPR cycles · about 48 minutes/);
    elements.setupForm.querySelectorAll("[data-remove-antenna]")[2].click();
    assert.match(elements.setupPlanSummary.textContent, /16 planned WSPR cycles · about 32 minutes/);

    const controllerEnabled = elements.setupForm.querySelector(
      '[data-setup-field="antennaControllerEnabled"]',
    );
    controllerEnabled.checked = true;
    controllerEnabled.dispatchEvent(new InputEvent("input", { bubbles: true }));
    assert.equal(elements.controllerSetupFields.hidden, false);
    assert.equal(elements.setupForm.querySelectorAll("[data-controller-target]").length, 2);
    const antennaRows = [...elements.setupForm.querySelectorAll("[data-antenna-row]")];
    antennaRows[0].querySelector('[data-antenna-field="label"]').value = "DXC";
    antennaRows[0].querySelector('[data-antenna-field="label"]').dispatchEvent(new InputEvent("input", { bubbles: true }));
    antennaRows[1].querySelector('[data-antenna-field="label"]').value = "Attic EFHW";
    antennaRows[1].querySelector('[data-antenna-field="label"]').dispatchEvent(new InputEvent("input", { bubbles: true }));
    const controllerTargets = [...elements.setupForm.querySelectorAll("[data-controller-target]")];
    controllerTargets[0].value = "2";
    controllerTargets[1].value = "1";
    elements.controllerProfileSelect.value = "";
    elements.controllerProfileSelect.dispatchEvent(new Event("change", { bubbles: true }));
    assert.equal(profileField("controllerProfileName").value, "");
    assert.equal(profileField("controllerSwitchCommand").value, "");
    assert.equal(profileField("controllerSwitchProgram").value, "");
    assert.deepEqual(controllerTargets.map((input) => input.value), ["2", "1"]);
    profileField("controllerProfileName").value = "Elecraft";
    profileField("controllerSwitchCommand").value = "switch {target}";
    profileField("controllerVerificationCommand").value = "true";
    assert.equal(controllerTargets[0].closest("label").textContent, "Controller value");
    elements.setupAddAntennaButton.click();
    assert.deepEqual(
      [...elements.setupForm.querySelectorAll("[data-controller-target]")].slice(0, 2).map((input) => input.value),
      ["2", "1"],
    );
    elements.setupForm.querySelectorAll("[data-remove-antenna]")[2].click();

    elements.useCurrentLocationButton.click();
    await vi.waitFor(() => {
      assert.equal(grid.value, "FN42li");
      assert.match(elements.locationStatus.textContent, /Estimated FN42li/);
    });

    const submit = new Event("submit", { bubbles: true, cancelable: true });
    assert.equal(elements.setupForm.dispatchEvent(submit), false);
    assert.equal(submit.defaultPrevented, true);
    await vi.waitFor(() => {
      assert.equal(elements.setupCreateButton.disabled, false);
      assert.equal(elements.setupReviewPanel.hidden, false);
    });
    assert.deepEqual(elements.setupReviewPanel.scrollIntoView.mock.calls, [[{
      behavior: "smooth",
      block: "start",
    }]]);
    assert.equal(callsign.value, "N1RWJ", "review preserves entered station values");
    const reviewCall = calls.find(([command]) => command === "review_session_setup");
    assert.equal(reviewCall[1].draft.station.callsign, "N1RWJ");
    assert.equal(reviewCall[1].draft.station.grid, "FN42li");
    assert.equal(reviewCall[1].draft.antennaController.profile.switchCommand.oneLine, "switch {target}");
    assert.equal(reviewCall[1].draft.antennaController.profile.verificationCommand.oneLine, "true");
    assert.deepEqual(reviewCall[1].draft.antennaController.targets, [
      { antennaLabel: "DXC", target: "2" },
      { antennaLabel: "Attic EFHW", target: "1" },
    ]);

    elements.setupCreateButton.click();
    await vi.waitFor(() => {
      assert.equal(window.location.hash, "#run");
      assert.equal(document.querySelector('[data-panel="run"]').hidden, false);
      assert.match(elements.setupStatus.textContent, /Session ready/);
    });
    assert.ok(calls.some(([command]) => command === "create_session_from_review"));
    assert.equal(elements.managedLocationNotice.hidden, false);
    assert.match(elements.managedLocationNotice.textContent, /Session saved in AntennaBench Sessions/);
    elements.managedLocationReveal.click();
    await vi.waitFor(() => {
      assert.ok(calls.some(([command, payload]) => command === "reveal_managed_session"
        && payload.locatorId === "locator-headless"));
    });
    assert.deepEqual(reportDocumentUrls, [
      "blob:headless-report-1",
      "blob:headless-report-2",
      "blob:headless-report-3",
      "blob:headless-report-4",
    ]);
    assert.equal(elements.reportFrame.getAttribute("src"), "blob:headless-report-4");

    const start = elements.lifecycleButtons.find(
      (button) => button.dataset.conductorAction === "start",
    );
    assert.equal(elements.wsjtxReadiness.hidden, false);
    assert.equal(start.disabled, true);
    start.click();
    assert.equal(calls.some(([command]) => command === "mutate_active_session_conductor"), false);
    elements.wsjtxReadinessAcknowledge.click();
    assert.equal(start.disabled, false);
    start.click();
    await vi.waitFor(() => {
      assert.ok(calls.some(([command]) => command === "mutate_active_session_conductor"));
      assert.equal(elements.wsjtxReadiness.hidden, true);
    });

    const skip = elements.lifecycleButtons.find(
      (button) => button.dataset.conductorAction === "skip_wspr_cycle",
    );
    const skipCalls = () => calls.filter(([command, payload]) =>
      command === "mutate_active_session_conductor"
      && payload.request.action.kind === "skip_wspr_cycle");
    skip.click();
    await vi.waitFor(() => assert.equal(elements.skipCycleDialog.open, true));
    assert.equal(document.activeElement, elements.skipCycleReason);
    assert.match(elements.skipCycleIdentity.textContent, /Cycle 1 · DXC · Transmit · 20m/);
    assert.match(elements.skipCycleDescription.textContent, /this one planned cycle/);
    assert.match(elements.skipCycleDescription.textContent, /End session/);
    elements.skipCycleConfirm.focus();
    elements.skipCycleDialog.dispatchEvent(new KeyboardEvent("keydown", {
      key: "Tab",
      bubbles: true,
    }));
    assert.equal(document.activeElement, elements.skipCycleReason, "skip modal traps focus");
    elements.skipCycleDialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    await vi.waitFor(() => {
      assert.equal(elements.skipCycleDialog.open, false);
      assert.equal(document.activeElement, skip, "Escape restores skip-action focus");
    });
    assert.equal(skipCalls().length, 0, "cancel records nothing");

    skip.click();
    await vi.waitFor(() => assert.equal(elements.skipCycleDialog.open, true));
    elements.skipCycleReason.value = "storm nearby";
    elements.skipCycleConfirm.click();
    elements.skipCycleConfirm.click();
    assert.equal(elements.skipCycleConfirm.disabled, true);
    assert.equal(elements.skipCycleConfirm.textContent, "Skipping…");
    assert.equal(elements.skipCyclePending.hidden, false);
    assert.equal(skipCalls().length, 1, "confirmation cannot submit twice");
    assert.deepEqual(skipCalls()[0][1].request, {
      actionToken: "token-1",
      expectedRevision: 2,
      action: {
        kind: "skip_wspr_cycle",
        intentId: "intent-1",
        reason: "storm nearby",
      },
    });
    resolveSkipCycle(conductorView({
      revision: 3,
      actionToken: "token-3",
      nextIntent: {
        intentId: "intent-2",
        sequenceNumber: 2,
        direction: "receive",
        antennaLabel: "Attic EFHW",
        band: "20m",
      },
    }));
    await vi.waitFor(() => {
      assert.equal(elements.skipCycleDialog.open, false);
      assert.equal(document.activeElement, skip);
      assert.match(elements.skipCycleFeedback.textContent, /Cycle skipped/);
    });
    assert.deepEqual(uncaught, []);
  } finally {
    delete window.__TAURI__;
    delete window.URL.createObjectURL;
    delete window.URL.revokeObjectURL;
    window.removeEventListener("error", recordError);
    window.removeEventListener("unhandledrejection", recordError);
    vi.clearAllTimers();
    vi.useRealTimers();
  }
}, 15_000);
