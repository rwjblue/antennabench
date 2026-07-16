export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

export const CONTEXT_HELP = Object.freeze({
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
    text: "One repetition tests every configured antenna in the selected direction. Both mode includes one receive and one transmit period per antenna; the estimate shows ideal WSPR time only.",
  },
  public_spots: {
    title: "Automatic WSPR spots",
    text: "AntennaBench normally gathers public reports of your transmissions from WSPR.live after completed cycles. Turn this off for an offline run; you can import saved data later.",
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
    title: "Public spot collection",
    text: "This status says whether AntennaBench is waiting, collecting, finished, off, or needs a retry. It never blocks manual operator actions or bundle export.",
  },
  wsjtx_receiver: {
    title: "WSJT-X UDP receiver",
    text: "Connect the local WSJT-X UDP feed before starting a receive-capable session. It is required for Both and RX-focused WSPR runs and optional for TX-only runs.",
  },
});

export function installContextualHelp(root) {
  const document = root.ownerDocument;
  let openDisclosure = null;

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
    const popover = document.createElement("span");
    popover.id = `context-help-${index + 1}`;
    popover.className = "context-help-popover";
    popover.setAttribute("role", "note");
    popover.textContent = help.text;
    popover.hidden = true;
    trigger.setAttribute("aria-label", `Help: ${help.title}`);
    trigger.setAttribute("aria-controls", popover.id);
    trigger.setAttribute("aria-expanded", "false");
    trigger.after(popover);
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

  return { close };
}

export function workflowFromHash(hash) {
  const workflow = hash.replace(/^#/, "");
  return WORKFLOWS.includes(workflow) ? workflow : "setup";
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

  return `${String.fromCharCode(65 + fieldLongitude)}${String.fromCharCode(65 + fieldLatitude)}${squareLongitude}${squareLatitude}${String.fromCharCode(65 + subsquareLongitude)}${String.fromCharCode(65 + subsquareLatitude)}`;
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

export function wsprRunPlanSummary(roundsValue, antennaCount, mode = "whole_station_ab") {
  const normalizedRounds = typeof roundsValue === "number"
    ? roundsValue
    : Number(String(roundsValue).trim());
  if (
    String(roundsValue).trim().length === 0
    || !Number.isSafeInteger(normalizedRounds)
    || normalizedRounds <= 0
    || !Number.isSafeInteger(antennaCount)
    || antennaCount <= 0
  ) {
    return null;
  }
  const scheduledAntennaCount = mode === "single_antenna_profiling" ? 1 : antennaCount;
  const directionCount = ["whole_station_ab", "single_antenna_profiling"].includes(mode) ? 2 : 1;
  const cycles = normalizedRounds * scheduledAntennaCount * directionCount;
  const minimumMinutes = cycles * 2;
  if (!Number.isSafeInteger(cycles) || !Number.isSafeInteger(minimumMinutes)) return null;
  return {
    rounds: normalizedRounds,
    antennaCount: scheduledAntennaCount,
    directionCount,
    cycles,
    minimumMinutes,
    text: `${cycles} WSPR ${cycles === 1 ? "cycle" : "cycles"} · at least ${minimumMinutes} ${minimumMinutes === 1 ? "minute" : "minutes"}`,
  };
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

export function updateReportFrame(reportFrame, state) {
  if (state.session === null || typeof state.session.reportHtml !== "string") return false;

  const presentationId = String(state.reportPresentationId);
  if (reportFrame.dataset.presentationId === presentationId) return false;

  reportFrame.srcdoc = state.session.reportHtml;
  reportFrame.dataset.presentationId = presentationId;
  return true;
}

export function wsprLiveAcquisitionModel(state) {
  const localTime = (value) => formatActiveRunTime(value, {
    now: state.conductor?.now,
  });
  if (state.wsprLiveAcquisitionStatus === "fetching") {
    return {
      phase: "Collecting public spots…",
      detail: "AntennaBench is checking WSPR.live now.",
      diagnostic: "",
      retry: false,
    };
  }
  if (state.wsprLiveAcquisitionError) {
    return {
      phase: "Public spots need attention",
      detail: state.wsprLiveAcquisitionError.message,
      diagnostic: "",
      retry: true,
      endWithout: state.conductor?.phase === "finalizing",
    };
  }
  const outcome = state.wsprLiveAcquisition;
  if (outcome?.status === "disabled") {
    return {
      phase: "Automatic collection is off",
      detail: "No public spots will be collected automatically. You can still import saved WSPR.live data later.",
      diagnostic: "",
      retry: false,
    };
  }
  if (!outcome || outcome.status === "dormant") {
    return {
      phase: "Waiting for the first completed cycle",
      detail: "Automatic collection will begin after a WSPR cycle completes.",
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "waiting") {
    return {
      phase: "Waiting briefly for public spots",
      detail: `Spots from the last completed cycle should be available after ${localTime(outcome.notBefore)}.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "up_to_date") {
    return {
      phase: "Public spots are up to date",
      detail: `Spots collected through ${localTime(outcome.capturedThrough)}.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "captured") {
    return {
      phase: "Public spots collected",
      detail: `${outcome.observationsCreated} new public spot(s) collected through ${localTime(outcome.capturedThrough)}.`,
      diagnostic: "",
      retry: false,
    };
  }
  if (outcome.status === "completed") {
    return {
      phase: "Final public spots collected",
      detail: `Spots collected through ${localTime(outcome.capturedThrough)}. The session ended automatically.`,
      diagnostic: "",
      retry: false,
    };
  }
  return {
    phase: "Public spots need attention",
    detail: outcome.message || "Automatic public spot collection could not finish.",
    diagnostic: "",
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


