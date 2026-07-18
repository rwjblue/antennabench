function optionalNumber(value) {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : Number(trimmed);
}

export const SETUP_QUESTION_MODES = Object.freeze({
  compare_whole_station: "whole_station_ab",
  heard_better: "tx_focused",
  hear_better: "rx_focused",
  profile_one_antenna: "single_antenna_profiling",
});

export function modeForSetupQuestion(question) {
  if (!Object.hasOwn(SETUP_QUESTION_MODES, question)) {
    throw new RangeError(`Unknown setup question: ${question}`);
  }
  return SETUP_QUESTION_MODES[question];
}

export function goalForSetupQuestion(question) {
  return modeForSetupQuestion(question) === "single_antenna_profiling"
    ? "single_antenna_profiling"
    : "general_coverage";
}

function syncGoalToMode(form, mode) {
  const goal = form.querySelector('[data-setup-field="goal"]');
  if (mode === "single_antenna_profiling") {
    goal.value = "single_antenna_profiling";
  } else if (goal.value === "single_antenna_profiling") {
    goal.value = "general_coverage";
  }
}

export function selectSetupQuestion(form, question) {
  const mode = modeForSetupQuestion(question);
  form.querySelector('[data-setup-field="mode"]').value = mode;
  form.querySelector('[data-setup-field="goal"]').value = goalForSetupQuestion(question);
  for (const choice of form.querySelectorAll("[data-setup-question]")) {
    choice.checked = choice.value === question;
  }
  return mode;
}

export function syncSetupQuestionToMode(form) {
  const mode = form.querySelector('[data-setup-field="mode"]').value;
  syncGoalToMode(form, mode);
  for (const choice of form.querySelectorAll("[data-setup-question]")) {
    choice.checked = modeForSetupQuestion(choice.value) === mode;
  }
  return mode;
}

export function syncWsprLiveForSignalPlan(form, signalPlanEnabled) {
  const wsprLive = form.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]');
  if (signalPlanEnabled) wsprLive.checked = false;
  wsprLive.disabled = signalPlanEnabled;
  return wsprLive.checked;
}

export function normalizeMaidenheadGrid(value) {
  return [...value].map((character, index) => {
    if (index < 2) return character.toUpperCase();
    if (index >= 4 && /[a-z]/i.test(character)) return character.toLowerCase();
    return character;
  }).join("");
}

export function readSignalEvidenceFields(frequency, mode, power, callsign, cadence) {
  return {
    frequencyHz: optionalNumber(frequency.value),
    mode: mode.value || null,
    powerWatts: optionalNumber(power.value),
    transmittedCallsign: callsign.value.toUpperCase(),
    cadenceFollowed: cadence.value === "" ? null : cadence.value === "true",
  };
}

export function readEvidenceAction(kind, slotId, antennaLabel, detail, signal = {}) {
  switch (kind) {
    case "confirm_antenna": return {
      kind,
      slotId,
      antennaLabel,
      note: detail,
    };
    case "confirm_signal": return {
      kind,
      slotId,
      frequencyHz: signal.frequencyHz ?? null,
      mode: signal.mode ?? null,
      powerWatts: signal.powerWatts ?? null,
      transmittedCallsign: (signal.transmittedCallsign ?? "").toUpperCase(),
      cadenceFollowed: signal.cadenceFollowed ?? null,
      note: detail,
    };
    case "mark_missed": return { kind, slotId, reason: detail };
    case "mark_bad": return { kind, slotId, reason: detail };
    case "add_note": return { kind, slotId: slotId || null, note: detail };
    default: throw new RangeError(`Unknown conductor evidence kind: ${kind}`);
  }
}

export function readEvidenceReplacement(kind, antennaLabel, detail, signal = {}) {
  switch (kind) {
    case "confirm_antenna": return { kind, antennaLabel, note: detail };
    case "confirm_signal": return {
      kind,
      frequencyHz: signal.frequencyHz ?? null,
      mode: signal.mode ?? null,
      powerWatts: signal.powerWatts ?? null,
      transmittedCallsign: (signal.transmittedCallsign ?? "").toUpperCase(),
      cadenceFollowed: signal.cadenceFollowed ?? null,
      note: detail,
    };
    case "mark_missed": return { kind, reason: detail };
    case "mark_bad": return { kind, reason: detail };
    case "add_note": return { kind, note: detail };
    default: throw new RangeError(`Unknown conductor evidence kind: ${kind}`);
  }
}

function optionalField(row, field) {
  return row.querySelector(`[data-antenna-field="${field}"]`)?.value ?? "";
}

export function readSetupDraft(form) {
  const value = (field) => form.querySelector(`[data-setup-field="${field}"]`).value;
  const signalPlanEnabled = form.querySelector('[data-setup-field="signalPlanEnabled"]').checked;
  const controllerEnabled = form.querySelector('[data-setup-field="antennaControllerEnabled"]').checked;
  const lines = (field) => value(field) === "" ? [] : value(field).split(/\r?\n/);
  const verificationOneLine = value("controllerVerificationCommand");
  const verificationProgram = value("controllerVerificationProgram");
  return {
    station: {
      callsign: value("callsign").toUpperCase(),
      grid: normalizeMaidenheadGrid(value("grid")),
      powerWatts: value("powerWatts"),
      operatorNotes: value("operatorNotes"),
    },
    antennas: [...form.querySelectorAll("[data-antenna-row]")].map((row) => ({
      label: optionalField(row, "label"),
      facets: optionalField(row, "facets"),
      heightM: optionalField(row, "heightM"),
      radialCount: optionalField(row, "radialCount"),
      radialLengthM: optionalField(row, "radialLengthM"),
      orientationDegrees: optionalField(row, "orientationDegrees"),
      tuner: optionalField(row, "tuner"),
      feedline: optionalField(row, "feedline"),
      notes: optionalField(row, "notes"),
    })),
    schedule: {
      mode: value("mode"),
      goal: value("goal"),
      band: value("band"),
      rounds: value("rounds"),
    },
    wsprLiveAcquisitionEnabled: form.querySelector('[data-setup-field="wsprLiveAcquisitionEnabled"]').checked,
    signalPlan: signalPlanEnabled ? {
      mode: value("signalMode"),
      collectionProfile: value("signalCollectionProfile"),
      plannedPowerWatts: value("signalPlannedPowerWatts"),
      transmittedCallsign: value("signalTransmittedCallsign").toUpperCase(),
      differingIdentityValidated: form.querySelector('[data-setup-field="signalDifferingIdentityValidated"]').checked,
      message: value("signalMessage"),
      repetitionCount: value("signalRepetitionCount"),
      keySpeedWpm: value("signalKeySpeedWpm"),
      transmitSeconds: value("signalTransmitSeconds"),
      intervalSeconds: value("signalIntervalSeconds"),
      frequenciesHz: value("signalFrequenciesHz"),
    } : null,
    antennaController: controllerEnabled ? {
      enabled: true,
      armForSession: form.querySelector('[data-setup-field="controllerArmForSession"]').checked,
      invocation: value("controllerInvocation"),
      manualReviewRequired: form.querySelector('[data-setup-field="controllerManualReviewRequired"]').checked,
      profile: {
        profileId: value("controllerProfileId") || null,
        name: value("controllerProfileName"),
        timeoutSeconds: Number(value("controllerTimeoutSeconds")),
        switchCommand: {
          oneLine: value("controllerSwitchCommand"),
          program: value("controllerSwitchProgram"),
          arguments: lines("controllerSwitchArguments"),
        },
        verificationCommand: (verificationOneLine || verificationProgram) ? {
          oneLine: verificationOneLine,
          program: verificationProgram,
          arguments: lines("controllerVerificationArguments"),
        } : null,
      },
      targets: [...form.querySelectorAll("[data-antenna-row]")].map((row) => ({
        antennaLabel: optionalField(row, "label"),
        target: optionalField(row, "controllerTarget"),
      })),
    } : null,
  };
}

export function applyStationPreferences(form, preferences) {
  if (!preferences) return false;
  const fields = {
    callsign: preferences.callsign ?? "",
    grid: preferences.grid ?? "",
    powerWatts: preferences.powerWatts ?? "",
    operatorNotes: preferences.operatorNotes ?? "",
  };
  const controls = Object.keys(fields).map((field) =>
    form.querySelector(`[data-setup-field="${field}"]`)
  );
  if (controls.some((control) => control.value.trim().length > 0)) return false;
  controls.forEach((control, index) => {
    const field = Object.keys(fields)[index];
    control.value = field === "callsign"
      ? fields[field].toUpperCase()
      : field === "grid"
        ? normalizeMaidenheadGrid(fields[field])
        : fields[field];
  });
  return true;
}
