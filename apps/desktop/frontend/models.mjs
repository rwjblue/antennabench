export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

export const CONTEXT_HELP = Object.freeze({
  setup_question: {
    title: "Question-first setup",
    text: "Choose the routine question closest to what you want to learn; it selects an existing experiment mode without adding evidence or predicting a winner. The mode and goal remain visible and editable in the run plan.",
  },
  station_location: {
    title: "Grid and current location",
    text: "A Maidenhead grid is a short code for your station area. Use current location asks macOS for one location estimate; you can always type the grid instead.",
  },
  transmit_power: {
    title: "Transmit power",
    text: "Enter the transmitter power used for the session in watts. It gives the report useful context but does not control your radio.",
  },
  antennas: {
    title: "Antenna descriptions",
    text: "Use a clear label and short description so the antennas stay recognizable in prompts and reports. Optional height, orientation, feedline, tuner, radial, and installation details add context without changing the schedule.",
  },
  run_plan: {
    title: "Experiment plan",
    text: "Mode describes what part of the station you are comparing, goal describes the kind of coverage you care about, and band selects the WSPR band. These choices organize the evidence; they do not declare a winner.",
  },
  rounds: {
    title: "Rounds and cycles",
    text: "One repetition tests every configured antenna in the selected direction. Both mode includes one receive and one transmit period per antenna; the estimate shows required WSPR cycle time only.",
  },
  antenna_controller: {
    title: "Antenna switching assistant",
    text: "AntennaBench can run a program already installed on this computer to request each antenna change and optionally verify it. Profiles are saved only in local application data, and opening an imported session never grants permission to run them.",
  },
  public_spots: {
    title: "Automatic bidirectional WSPR spots",
    text: "AntennaBench collects delayed public reports for both transmissions and receptions from WSPR.live on a best-effort basis; enable WSJT-X Upload spots and keep it online. It retains the rows returned for configured request windows, the upstream mirror does not provide an independent completeness guarantee, and you can turn this off for an offline run.",
  },
  controlled_signal: {
    title: "Controlled CW or RTTY plan",
    text: "Use this advanced plan when you will manually transmit an exact callsign, message, cadence, power, and frequency list for CW or RTTY. The evidence profile says how observations will be collected, while repetitions and interval fields define the balanced sequence.",
  },
  countdown: {
    title: "Countdown",
    text: "The countdown shows time until the current transmission ends or the armed cycle starts. Rust owns the actual timing; this display refreshes locally between checks.",
  },
  current_cycle: {
    title: "Current or last cycle",
    text: "This card shows the cycle that is transmitting now, or the most recently completed cycle between actions. The evidence line says what antenna state is actually supported.",
  },
  next_cycle: {
    title: "Next cycle",
    text: "This card shows the next planned antenna and band. Its exact start time appears only after you switch antennas and press the named ready button.",
  },
  skip_cycle: {
    title: "Skip cycle",
    text: "Skip records that this planned cycle was missed and moves to the next antenna. Use it when you cannot conduct the cycle; the record can be corrected later.",
  },
  notes_corrections: {
    title: "Notes and corrections",
    text: "Add note records useful context, usually on the current or last cycle. Corrections append a visible replacement or retraction instead of erasing history.",
  },
  session_controls: {
    title: "Pause, end, and abandon",
    text: "Pause keeps the session available to resume, while End closes it normally. Abandon is terminal and marks the run as intentionally discontinued; existing evidence remains.",
  },
  wspr_live_status: {
    title: "Delayed/public WSPR.live collection",
    text: "This default online source collects both TX and RX rows after WSPRnet/WSPR.live ingestion; Upload spots and network access are required. AntennaBench retains rows returned for configured request windows; the upstream mirror does not provide an independent completeness guarantee.",
  },
  wsjtx_receiver: {
    title: "Local/offline WSJT-X receiver",
    text: "This direct UDP source is required before a receive-capable run only when WSPR.live is off. It can remain off for the default online path, or run alongside WSPR.live as separately attributed evidence.",
  },
});

export function installContextualHelp(root) {
  const document = root.ownerDocument ?? root;
  let openDisclosure = null;

  const position = () => {
    if (!openDisclosure) return;
    const { trigger, popover } = openDisclosure;
    const triggerRect = trigger.getBoundingClientRect();
    const popoverRect = popover.getBoundingClientRect();
    const viewportWidth = document.defaultView?.innerWidth ?? document.documentElement.clientWidth;
    const viewportHeight = document.defaultView?.innerHeight ?? document.documentElement.clientHeight;
    const margin = 12;
    const gap = 8;
    const width = popoverRect.width || Math.min(360, viewportWidth - (margin * 2));
    const height = popoverRect.height || 150;
    const left = Math.min(
      Math.max(margin, triggerRect.left + (triggerRect.width / 2) - (width / 2)),
      Math.max(margin, viewportWidth - width - margin),
    );
    const fitsBelow = triggerRect.bottom + gap + height <= viewportHeight - margin;
    const top = fitsBelow
      ? triggerRect.bottom + gap
      : Math.max(margin, triggerRect.top - height - gap);
    popover.style.left = `${Math.round(left)}px`;
    popover.style.top = `${Math.round(top)}px`;
  };

  const close = (restoreFocus = false) => {
    if (!openDisclosure) return;
    const { trigger, popover } = openDisclosure;
    popover.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
    trigger.removeAttribute("aria-describedby");
    openDisclosure = null;
    if (restoreFocus) trigger.focus();
  };

  [...root.querySelectorAll("[data-help-trigger]")].forEach((trigger, index) => {
    const help = CONTEXT_HELP[trigger.dataset.helpTrigger];
    if (!help) return;
    const popover = document.createElement("div");
    popover.id = `context-help-${index + 1}`;
    popover.className = "context-help-popover";
    popover.setAttribute("role", "note");
    const title = document.createElement("strong");
    title.id = `${popover.id}-title`;
    title.textContent = help.title;
    const text = document.createElement("p");
    text.textContent = help.text;
    popover.append(title, text);
    popover.hidden = true;
    trigger.setAttribute("aria-label", `Help: ${help.title}`);
    trigger.setAttribute("aria-controls", popover.id);
    trigger.setAttribute("aria-expanded", "false");
    popover.setAttribute("aria-labelledby", title.id);
    (document.body ?? document.documentElement).append(popover);
    trigger.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const wasOpen = openDisclosure?.trigger === trigger;
      close();
      if (wasOpen) return;
      popover.hidden = false;
      trigger.setAttribute("aria-expanded", "true");
      trigger.setAttribute("aria-describedby", popover.id);
      openDisclosure = { trigger, popover };
      position();
    });
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") close(true);
  });
  document.addEventListener("click", (event) => {
    if (
      openDisclosure
      && event.target !== openDisclosure.trigger
      && !openDisclosure.popover.contains(event.target)
    ) {
      close();
    }
  });
  document.defaultView?.addEventListener("resize", position);
  document.addEventListener("scroll", position, true);

  return { close };
}

export function workflowFromHash(hash) {
  const workflow = hash.replace(/^#/, "");
  return WORKFLOWS.includes(workflow) ? workflow : "setup";
}

export function createWorkflowScrollMemory(initialWorkflow) {
  let activeWorkflow = initialWorkflow;
  const positions = new Map([[activeWorkflow, 0]]);
  return {
    transition(nextWorkflow, currentScrollTop) {
      if (nextWorkflow === activeWorkflow) return null;
      positions.set(activeWorkflow, currentScrollTop);
      activeWorkflow = nextWorkflow;
      return positions.get(activeWorkflow) ?? 0;
    },
  };
}

export function maidenheadGrid(latitude, longitude) {
  if (!Number.isFinite(latitude) || !Number.isFinite(longitude)) {
    throw new TypeError("Location coordinates must be finite numbers.");
  }
  if (latitude < -90 || latitude > 90 || longitude < -180 || longitude > 180) {
    throw new RangeError("Location coordinates are outside the supported range.");
  }

  const boundedLatitude = latitude === 90 ? 90 - 1e-9 : latitude;
  const boundedLongitude = longitude === 180 ? 180 - 1e-9 : longitude;
  const shiftedLatitude = boundedLatitude + 90;
  const shiftedLongitude = boundedLongitude + 180;
  const fieldLongitude = Math.floor(shiftedLongitude / 20);
  const fieldLatitude = Math.floor(shiftedLatitude / 10);
  const squareLongitude = Math.floor((shiftedLongitude % 20) / 2);
  const squareLatitude = Math.floor(shiftedLatitude % 10);
  const subsquareLongitude = Math.floor((shiftedLongitude % 2) * 12);
  const subsquareLatitude = Math.floor((shiftedLatitude % 1) * 24);

  return `${String.fromCharCode(65 + fieldLongitude)}${String.fromCharCode(65 + fieldLatitude)}${squareLongitude}${squareLatitude}${String.fromCharCode(97 + subsquareLongitude)}${String.fromCharCode(97 + subsquareLatitude)}`;
}

export function locationLookupMessage(outcome) {
  switch (outcome?.status) {
    case "denied":
      return "Location access is off for AntennaBench. Allow it in System Settings > Privacy & Security > Location Services, or enter the grid manually.";
    case "restricted":
      return "Location access is restricted by macOS settings. Enter the grid manually instead.";
    case "timeout":
      return "The macOS location request timed out. Enter the grid manually or try again.";
    case "unavailable":
      return "macOS location services are unavailable. Enter the grid manually instead.";
    default:
      return "The station location could not be determined. Enter the grid manually instead.";
  }
}

export function focusSetupOutcome(
  state,
  reviewPanel,
  diagnostics,
  form = null,
  scrollBehavior = "auto",
) {
  if (state.setupStatus === "reviewed") {
    reviewPanel.focus({ preventScroll: true });
    reviewPanel.scrollIntoView?.({ behavior: scrollBehavior, block: "start" });
    return "review";
  }
  if (state.setupStatus === "invalid") {
    const invalidField = form?.querySelector?.("[data-setup-invalid]");
    if (invalidField) {
      invalidField.focus();
      return "field";
    }
    diagnostics.focus();
    return "diagnostics";
  }
  return null;
}

export function setupPlanEstimate({
  mode,
  rounds,
  antennaCount,
  signalPlanEnabled = false,
  frequenciesHz = "",
}) {
  const parsedRounds = Number(rounds);
  if (!Number.isSafeInteger(parsedRounds) || parsedRounds < 1 || antennaCount < 1) {
    return "Enter the round count and antennas to see the planned run size.";
  }
  const scheduledAntennaCount = mode === "single_antenna_profiling" ? 1 : antennaCount;
  const roundsLabel = `${parsedRounds} ${parsedRounds === 1 ? "round" : "rounds"}`;
  if (signalPlanEnabled) {
    const frequencies = frequenciesHz.split(",").map((value) => value.trim());
    const validFrequencies = frequencies.length > 0 && frequencies.every((value) => /^\d+$/.test(value) && Number(value) > 0);
    if (!validFrequencies) {
      return `${roundsLabel} · enter the exact frequencies to see the controlled-signal slot count.`;
    }
    const frequencyCount = new Set(frequencies.map((value) => BigInt(value).toString())).size;
    const slotCount = parsedRounds * scheduledAntennaCount * frequencyCount;
    return `${roundsLabel} · ${slotCount} controlled-signal ${slotCount === 1 ? "slot" : "slots"}. Timing follows the configured operator cadence.`;
  }
  const directionCount = ["whole_station_ab", "single_antenna_profiling"].includes(mode) ? 2 : 1;
  const cycleCount = parsedRounds * scheduledAntennaCount * directionCount;
  const requiredCycleMinutes = cycleCount * 2;
  return `${roundsLabel} · ${cycleCount} planned WSPR ${cycleCount === 1 ? "cycle" : "cycles"} · about ${requiredCycleMinutes} minutes of required cycle time.`;
}

export function conductorActionAvailable(view, action) {
  if (action === "arm_wspr_cycle") {
    return view.lifecycle === "running"
      && view.nextIntent !== null
      && ["between_slots", "switching"].includes(view.phase);
  }
  if (action === "skip_wspr_cycle") {
    return view.lifecycle === "running"
      && view.nextIntent !== null
      && ["between_slots", "switching"].includes(view.phase);
  }
  return lifecycleActionAvailability(view.lifecycle).has(action)
    && !(view.phase === "finalizing" && action === "end");
}

function wsjtxReadinessKey(view) {
  if (!view?.wsjtxReadiness || !["ready", "interrupted"].includes(view.lifecycle)) return null;
  return `${view.sessionId}:${view.revision}:${view.lifecycle}`;
}

export function wsjtxReadinessModel(state) {
  const readiness = state.conductor?.wsjtxReadiness ?? null;
  const key = wsjtxReadinessKey(state.conductor);
  if (!readiness || !key) return { visible: false, acknowledged: false, key: null, items: [] };
  const band = readiness.band.replace(/^(\d+)m$/, "$1 m");
  const direction = readiness.nextDirection === "transmit" ? "transmit" : "receive";
  const items = [
    `Band: ${band}.`,
    "Mode: WSPR.",
    readiness.powerWatts === null
      ? "Transmit power: power was not recorded."
      : `Transmit power: ${readiness.powerWatts} W.`,
    "Tx Pct: 100%.",
  ];
  if (readiness.hasReceivePeriods) items.push("Monitor: On for receive periods.");
  items.push(`Enable Tx: ${direction === "transmit" ? "On" : "Off"} for the next ${direction} period.`);
  if (readiness.wsprLiveAcquisitionEnabled) {
    items.push("Upload spots: On, with WSJT-X online for automatic WSPR.live collection.");
  }
  return {
    visible: true,
    acknowledged: state.wsjtxReadinessAcknowledgement === key,
    key,
    items,
  };
}

export function createCountdownAnchor(view, sampledAtMilliseconds) {
  if (view?.secondsToTransition === null || view?.secondsToTransition === undefined) return null;
  const seconds = Math.max(0, Math.floor(Number(view.secondsToTransition)));
  const sampledAt = Number(sampledAtMilliseconds);
  if (!Number.isFinite(seconds) || !Number.isFinite(sampledAt)) return null;
  return {
    key: [
      view.sessionId,
      view.revision,
      view.actionToken,
      view.lifecycle,
      view.phase,
      view.currentSlot?.slotId ?? "",
      view.nextSlot?.slotId ?? "",
      seconds,
    ].join(":"),
    seconds,
    sampledAtMilliseconds: sampledAt,
  };
}

export function projectCountdown(anchor, nowMilliseconds) {
  if (!anchor) return null;
  const now = Number(nowMilliseconds);
  if (!Number.isFinite(now)) return anchor.seconds;
  const elapsedSeconds = Math.floor(Math.max(0, now - anchor.sampledAtMilliseconds) / 1000);
  return Math.max(0, anchor.seconds - elapsedSeconds);
}

export function formatActiveRunTime(value, options = {}) {
  const instant = new Date(value);
  const now = new Date(options.now ?? Date.now());
  const locale = options.locale;
  const timeZone = options.timeZone;
  const dayFormatter = new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    timeZone,
  });
  const sameDay = dayFormatter.format(instant) === dayFormatter.format(now);
  return new Intl.DateTimeFormat(locale, sameDay
    ? { hour: "numeric", minute: "2-digit", timeZone }
    : { month: "short", day: "numeric", hour: "numeric", minute: "2-digit", timeZone }
  ).format(instant);
}

export function recommendedNoteTarget(view) {
  if (!view) return "";
  if (view.phase === "awaiting_slot" && view.nextSlot) return view.nextSlot.slotId;
  if (["active", "guard"].includes(view.phase) && view.currentSlot) {
    return view.currentSlot.slotId;
  }
  if (
    ["between_slots", "switching", "interrupted", "finalizing", "complete", "ended", "abandoned"]
      .includes(view.phase)
  ) {
    const now = new Date(view.now).getTime();
    const completed = (view.slots ?? [])
      .filter((slot) => new Date(slot.endsAt).getTime() <= now)
      .sort((left, right) => new Date(right.endsAt) - new Date(left.endsAt))[0];
    return completed?.slotId ?? view.currentSlot?.slotId ?? "";
  }
  return "";
}

export function viewModel(state) {
  return WORKFLOWS.map((workflow) => ({
    workflow,
    active: workflow === state.activeWorkflow,
  }));
}

export function createReportDocumentUrls(browserWindow = globalThis) {
  return {
    create(reportHtml) {
      const compact = reportHtml.includes('<main class="compact-summary');
      const stylesheetUrl = new browserWindow.URL(
        compact ? "report-compact.css" : "report.css",
        browserWindow.location.href,
      ).href.replaceAll("&", "&amp;").replaceAll('"', "&quot;");
      const styleStart = reportHtml.indexOf("<style>");
      const styleEnd = reportHtml.indexOf("</style>", styleStart);
      if (styleStart === -1 || styleEnd === -1) {
        throw new Error("The report document is missing its standalone stylesheet.");
      }
      const embeddedHtml = `${reportHtml.slice(0, styleStart)}<link rel="stylesheet" href="${stylesheetUrl}">${reportHtml.slice(styleEnd + 8)}`
        .replace("style-src 'unsafe-inline'", "style-src 'self'");
      const document = new browserWindow.Blob([embeddedHtml], {
        type: "text/html;charset=utf-8",
      });
      return browserWindow.URL.createObjectURL(document);
    },
    revoke(url) {
      browserWindow.URL.revokeObjectURL(url);
    },
  };
}

export function releaseReportFrame(reportFrame, reportDocuments) {
  const currentUrl = reportFrame.dataset.reportDocumentUrl;
  if (currentUrl) reportDocuments.revoke(currentUrl);
  delete reportFrame.dataset.reportDocumentUrl;
  delete reportFrame.dataset.presentationId;
  reportFrame.removeAttribute?.("src");
}

export function updateReportFrame(reportFrame, state, reportDocuments) {
  if (state.session === null || typeof state.session.reportHtml !== "string") return false;

  const presentationId = String(state.reportPresentationId);
  if (reportFrame.dataset.presentationId === presentationId) return false;

  const previousUrl = reportFrame.dataset.reportDocumentUrl;
  const nextUrl = reportDocuments.create(state.session.reportHtml);
  try {
    reportFrame.removeAttribute?.("srcdoc");
    reportFrame.src = nextUrl;
  } catch (error) {
    reportDocuments.revoke(nextUrl);
    throw error;
  }
  reportFrame.dataset.reportDocumentUrl = nextUrl;
  reportFrame.dataset.presentationId = presentationId;
  if (previousUrl) reportDocuments.revoke(previousUrl);
  return true;
}

export function wsprLiveAcquisitionModel(state) {
  const localTime = (value) => formatActiveRunTime(value, {
    now: state.conductor?.now,
  });
  if (state.wsprLiveAcquisitionStatus === "fetching") {
    return {
      compact: { kind: "checking", text: "WSPR.live · Checking for public spots now…" },
      phase: "Collecting best-effort public spots…",
      detail: "Delayed/public active · AntennaBench is checking WSPR.live for TX and RX rows in the configured request window now.",
      diagnostic: "",
      retry: false,
    };
  }
  if (state.wsprLiveAcquisitionError) {
    return {
      compact: { kind: "error", text: "WSPR.live · Collection needs attention" },
      phase: "Public collection needs attention",
      detail: state.wsprLiveAcquisitionError.message,
      diagnostic: state.wsprLiveAcquisitionError.detail ?? "",
      retry: true,
      endWithout: state.conductor?.phase === "finalizing",
    };
  }
  const outcome = state.wsprLiveAcquisition;
  if (outcome?.status === "disabled") {
    return {
      compact: { kind: "offline", text: "WSPR.live · Automatic collection off; manual run available" },
      phase: "Automatic collection is off",
      detail: "Delayed/public inactive · no WSPR.live spots will be collected automatically. Receive-capable runs require the direct/local UDP source.",
      diagnostic: "",
      retry: false,
    };
  }
  if (!outcome || outcome.status === "dormant") {
    return {
      compact: { kind: "waiting", text: "WSPR.live · Waiting for the first completed cycle" },
      phase: "Waiting for the first completed cycle",
      detail: "Delayed/public active · collection begins after a confirmed WSPR receive or transmit cycle completes.",
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "waiting") {
    return {
      compact: { kind: "waiting", text: `WSPR.live · Waiting for ingestion until ${localTime(outcome.notBefore)}` },
      phase: "Waiting briefly for public spots",
      detail: `Delayed/public active · TX and RX spots from the last completed cycle may be available after ${localTime(outcome.notBefore)}.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "up_to_date") {
    return {
      compact: { kind: "success", text: `WSPR.live · Last configured-window check succeeded through ${localTime(outcome.capturedThrough)}` },
      phase: "Best-effort public collection completed",
      detail: `Delayed/public active · AntennaBench retained the rows returned for configured request windows through ${localTime(outcome.capturedThrough)}. The upstream mirror does not provide an independent completeness guarantee.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "captured") {
    const checkedAt = localTime(outcome.checkedAt ?? outcome.capturedThrough);
    const compact = outcome.observationsCreated > 0
      ? { kind: "success", text: `WSPR.live · ${outcome.observationsCreated} new spot(s) retained · checked ${checkedAt}` }
      : { kind: "success", text: `WSPR.live · Last check succeeded; no new matching spots yet · ${checkedAt}` };
    return {
      compact,
      phase: "Best-effort public collection completed",
      detail: `Delayed/public active · ${outcome.observationsCreated} new TX/RX spot(s) were retained from configured request windows through ${localTime(outcome.capturedThrough)}. The upstream mirror does not provide an independent completeness guarantee.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "completed") {
    return {
      compact: { kind: "success", text: `WSPR.live · Final configured-window check complete through ${localTime(outcome.capturedThrough)}` },
      phase: "Best-effort public collection completed",
      detail: `TX and RX spots returned for configured request windows through ${localTime(outcome.capturedThrough)} were retained. The upstream mirror does not provide an independent completeness guarantee. The session ended automatically.`,
      diagnostic: "",
      retry: false,
    };
  }
  return {
    compact: { kind: "error", text: "WSPR.live · Collection needs attention" },
    phase: "Public collection needs attention",
    detail: outcome.message || "Automatic public spot collection could not finish.",
    diagnostic: outcome.detail ?? "",
    retry: true,
    endWithout: state.conductor?.phase === "finalizing",
  };
}

function lifecycleActionAvailability(lifecycle) {
  switch (lifecycle) {
    case "ready": return new Set(["start", "abandon"]);
    case "running": return new Set(["interrupt", "end", "abandon"]);
    case "interrupted": return new Set(["resume", "end", "abandon"]);
    default: return new Set();
  }
}
