import {
  invokeActiveSessionAntennaController,
  invokeActiveSessionConductor,
  invokeActiveSessionWsjtxStatus,
  invokeAdvanceSessionWsprLive,
  invokeAntennaControllerProfiles,
  invokeAttachSessionAntennaController,
  invokeCreateSessionFromReview,
  invokeDeleteAntennaControllerProfile,
  invokeExportActiveSessionReport,
  invokeExportSession,
  invokeImportActiveSessionRbn,
  invokeImportActiveSessionWsprLive,
  invokeLoadStationPreferences,
  invokeMutateSessionConductor,
  invokeOpenSession,
  invokeRefreshActiveSessionReport,
  invokeReviewSessionSetup,
  invokeRunSessionAntennaController,
  invokeSaveAntennaControllerProfile,
  invokeStartSessionWsjtx,
  invokeStationLocation,
  invokeStopSessionWsjtx,
} from "./bridge.mjs";
import { projectCountdown } from "./models.mjs";
import {
  antennaControllerActionFailed,
  antennaControllerCatalogSucceeded,
  antennaControllerProfileSucceeded,
  antennaControllerProfileActionFailed,
  antennaControllerRunSucceeded,
  antennaControllerViewSucceeded,
  beginAntennaControllerAction,
  beginConductorLoad,
  beginConductorMutation,
  beginExportSession,
  beginOpenSession,
  beginReportExport,
  beginReportRefresh,
  beginRbnImport,
  beginSetupCreation,
  beginSetupReview,
  beginWsjtxAction,
  beginWsprLiveAcquisition,
  beginWsprLiveImport,
  conductorLoadSucceeded,
  conductorMutationFailed,
  conductorPollSucceeded,
  editSessionSetup,
  exportSessionCancelled,
  exportSessionFailed,
  exportSessionSucceeded,
  initialState,
  openSessionCancelled,
  openSessionFailed,
  openSessionSucceeded,
  rbnImportCancelled,
  rbnImportFailed,
  rbnImportSucceeded,
  reportExportCancelled,
  reportExportFailed,
  reportExportSucceeded,
  reportRefreshFailed,
  reportRefreshSucceeded,
  selectWorkflow,
  setWsjtxReadinessAcknowledged,
  setupCreationCancelled,
  setupCreationFailed,
  setupCreationSucceeded,
  setupReviewFailed,
  setupReviewSucceeded,
  wsjtxActionFailed,
  wsjtxActionSucceeded,
  wsprLiveAcquisitionFailed,
  wsprLiveAcquisitionSucceeded,
  wsprLiveImportCancelled,
  wsprLiveImportFailed,
  wsprLiveImportSucceeded,
} from "./state.mjs";

const bridgeUnavailable = () => new Error("The native desktop bridge is unavailable.");
const unexpectedResponse = () => new Error("The desktop command returned an unexpected response.");

export function createDesktopController(options = {}) {
  const effects = {
    invoke: options.invoke,
    render: options.render ?? (() => {}),
    navigate: options.navigate ?? (() => {}),
    monotonicNow: options.monotonicNow ?? (() => Date.now()),
    setInterval: options.setInterval ?? (() => null),
    clearInterval: options.clearInterval ?? (() => {}),
    onFocus: options.onFocus ?? (() => () => {}),
    onVisibilityChange: options.onVisibilityChange ?? (() => () => {}),
    onHashChange: options.onHashChange ?? (() => () => {}),
    isVisible: options.isVisible ?? (() => true),
    prompt: options.prompt ?? (() => null),
    confirm: options.confirm ?? (() => false),
    getCountdownAnchor: options.getCountdownAnchor ?? (() => null),
    renderCountdown: options.renderCountdown ?? (() => {}),
  };
  let state = options.state ?? initialState(options.initialWorkflow);
  let transitionRefreshKey = null;
  let conductorPollInFlight = false;
  let started = false;
  let disposed = false;
  const cleanups = [];

  const commit = (nextState) => {
    state = nextState;
    effects.render(state);
    return state;
  };
  const invoke = () => {
    if (typeof effects.invoke !== "function") throw bridgeUnavailable();
    return effects.invoke;
  };

  const controller = {
    get state() {
      return state;
    },

    render() {
      effects.render(state);
    },

    editSetup() {
      if (!["reviewing", "creating"].includes(state.setupStatus)) {
        commit(editSessionSetup(state));
      }
    },

    setWsjtxReadinessAcknowledged(acknowledged) {
      commit(setWsjtxReadinessAcknowledged(state, acknowledged));
    },

    async reviewSetup(draft) {
      if (["reviewing", "creating"].includes(state.setupStatus)) return state;
      commit(beginSetupReview(state));
      try {
        commit(setupReviewSucceeded(state, await invokeReviewSessionSetup(invoke(), draft)));
      } catch (error) {
        commit(setupReviewFailed(state, error));
      }
      return state;
    },

    async loadAntennaControllerProfiles() {
      if (state.antennaControllerStatus === "loading") return state;
      commit(beginAntennaControllerAction(state));
      try {
        commit(antennaControllerCatalogSucceeded(
          state,
          await invokeAntennaControllerProfiles(invoke()),
        ));
      } catch (error) {
        commit(antennaControllerActionFailed(state, error));
      }
      return state;
    },

    async refreshAntennaController() {
      if (!state.session || state.antennaControllerStatus === "running") return state;
      commit(beginAntennaControllerAction(state));
      try {
        commit(antennaControllerViewSucceeded(
          state,
          await invokeActiveSessionAntennaController(invoke()),
        ));
      } catch (error) {
        commit(antennaControllerActionFailed(state, error));
      }
      return state;
    },

    async attachAntennaController(request) {
      if (!state.session || state.antennaControllerStatus === "running") return state;
      commit(beginAntennaControllerAction(state, "attaching"));
      try {
        commit(antennaControllerViewSucceeded(
          state,
          await invokeAttachSessionAntennaController(invoke(), request),
        ));
      } catch (error) {
        commit(antennaControllerActionFailed(state, error));
      }
      return state;
    },

    async saveAntennaControllerProfile(draft) {
      commit(beginAntennaControllerAction(state, "saving"));
      try {
        const savedProfile = await invokeSaveAntennaControllerProfile(invoke(), draft);
        commit(antennaControllerProfileSucceeded(
          state,
          await invokeAntennaControllerProfiles(invoke()),
          { kind: "saved", profileId: savedProfile.profileId },
        ));
        if (state.session) await controller.refreshAntennaController();
        return savedProfile;
      } catch (error) {
        commit(antennaControllerProfileActionFailed(state, error));
        return null;
      }
    },

    async deleteAntennaControllerProfile(profileId) {
      commit(beginAntennaControllerAction(state, "deleting"));
      try {
        await invokeDeleteAntennaControllerProfile(invoke(), profileId);
        commit(antennaControllerProfileSucceeded(
          state,
          await invokeAntennaControllerProfiles(invoke()),
          { kind: "deleted", profileId: "" },
        ));
        if (state.session) await controller.refreshAntennaController();
        return true;
      } catch (error) {
        commit(antennaControllerProfileActionFailed(state, error));
        return false;
      }
    },

    async runAntennaController() {
      if (!state.conductor?.nextIntent || state.antennaControllerStatus === "running") return state;
      const request = {
        actionToken: state.conductor.actionToken,
        expectedRevision: state.conductor.revision,
        intentId: state.conductor.nextIntent.intentId,
      };
      commit(beginAntennaControllerAction(state, "running"));
      try {
        commit(antennaControllerRunSucceeded(
          state,
          await invokeRunSessionAntennaController(invoke(), request),
        ));
        await controller.refreshConductor(false);
        await controller.refreshAntennaController();
        await controller.refreshReport();
      } catch (error) {
        commit(antennaControllerActionFailed(state, error));
      }
      return state;
    },

    async createSession() {
      const reviewId = state.setupReview?.reviewId;
      if (!reviewId || state.setupStatus === "creating") return state;
      commit(beginSetupCreation(state));
      try {
        const outcome = await invokeCreateSessionFromReview(invoke(), reviewId);
        if (outcome.status === "cancelled") {
          commit(setupCreationCancelled(state));
        } else if (outcome.status === "created" && outcome.session) {
          commit(setupCreationSucceeded(state, outcome.session));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(setupCreationFailed(state, error));
      }
      if (state.setupStatus === "created") {
        effects.navigate("run");
        await controller.refreshReport();
        await controller.refreshConductor();
      }
      return state;
    },

    async openSession() {
      if (state.openStatus === "loading") return state;
      commit(beginOpenSession(state));
      try {
        const outcome = await invokeOpenSession(invoke());
        if (outcome.status === "cancelled") {
          commit(openSessionCancelled(state));
        } else if (outcome.status === "opened" && outcome.session) {
          commit(openSessionSucceeded(state, outcome.session));
          effects.navigate("report");
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(openSessionFailed(state, error));
      }
      if (state.openStatus === "ready") await controller.refreshReport();
      return state;
    },

    async exportSession() {
      if (state.exportStatus === "loading") return state;
      commit(beginExportSession(state));
      try {
        const outcome = await invokeExportSession(invoke());
        if (outcome.status === "cancelled") {
          commit(exportSessionCancelled(state));
        } else if (outcome.status === "exported" && outcome.bundleName) {
          commit(exportSessionSucceeded(state, outcome.bundleName));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(exportSessionFailed(state, error));
      }
      return state;
    },

    async importWsprLive() {
      if (state.session?.lifecycle !== "running" || state.importStatus === "loading") return state;
      commit(beginWsprLiveImport(state));
      try {
        const outcome = await invokeImportActiveSessionWsprLive(invoke());
        if (outcome.status === "cancelled") {
          commit(wsprLiveImportCancelled(state));
        } else if (outcome.status === "imported" && outcome.session) {
          commit(wsprLiveImportSucceeded(state, outcome));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(wsprLiveImportFailed(state, error));
      }
      if (state.importStatus === "ready") await controller.refreshReport();
      return state;
    },

    async importRbn() {
      const eligible = state.session?.schemaVersion === 3
        && !["draft", "ready"].includes(state.session?.lifecycle);
      if (!eligible || state.importStatus === "loading") return state;
      commit(beginRbnImport(state));
      try {
        const outcome = await invokeImportActiveSessionRbn(invoke());
        if (outcome.status === "cancelled") {
          commit(rbnImportCancelled(state));
        } else if (outcome.status === "imported" && outcome.session) {
          commit(rbnImportSucceeded(state, outcome));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(rbnImportFailed(state, error));
      }
      if (state.importStatus === "ready") await controller.refreshReport();
      return state;
    },

    async refreshReport() {
      if (!state.session || state.reportStatus === "refreshing" || state.reportExportStatus === "loading") {
        return state;
      }
      commit(beginReportRefresh(state));
      try {
        commit(reportRefreshSucceeded(state, await invokeRefreshActiveSessionReport(invoke())));
      } catch (error) {
        commit(reportRefreshFailed(state, error));
      }
      return state;
    },

    async exportReport(format = "full_evidence_html", controllerEvidence = "complete") {
      if (!state.session?.reportHtml || state.reportExportStatus === "loading") return state;
      commit(beginReportExport(state));
      try {
        const outcome = await invokeExportActiveSessionReport(invoke(), format, controllerEvidence);
        commit(outcome.status === "cancelled"
          ? reportExportCancelled(state)
          : reportExportSucceeded(state, outcome));
      } catch (error) {
        commit(reportExportFailed(state, error));
      }
      return state;
    },

    async refreshWsjtxStatus() {
      if (["starting", "stopping"].includes(state.wsjtxStatus)) return state;
      commit(beginWsjtxAction(state));
      try {
        commit(wsjtxActionSucceeded(state, await invokeActiveSessionWsjtxStatus(invoke())));
      } catch (error) {
        commit(wsjtxActionFailed(state, error));
      }
      return state;
    },

    async startWsjtx(request) {
      if (["starting", "stopping"].includes(state.wsjtxStatus)) return state;
      commit(beginWsjtxAction(state, "starting"));
      try {
        commit(wsjtxActionSucceeded(state, await invokeStartSessionWsjtx(invoke(), request)));
      } catch (error) {
        commit(wsjtxActionFailed(state, error));
      }
      return state;
    },

    async stopWsjtx() {
      if (["starting", "stopping"].includes(state.wsjtxStatus)) return state;
      commit(beginWsjtxAction(state, "stopping"));
      try {
        commit(wsjtxActionSucceeded(state, await invokeStopSessionWsjtx(invoke())));
      } catch (error) {
        commit(wsjtxActionFailed(state, error));
      }
      return state;
    },

    async advanceWsprLive(retry = false) {
      if (state.conductor?.lifecycle !== "running" || state.wsprLiveAcquisitionStatus === "fetching") {
        return state;
      }
      commit(beginWsprLiveAcquisition(state));
      try {
        const outcome = await invokeAdvanceSessionWsprLive(invoke(), retry);
        commit(wsprLiveAcquisitionSucceeded(state, outcome));
        if (["captured", "completed"].includes(outcome.status)) {
          await controller.refreshConductor(false);
          await controller.refreshReport();
        }
      } catch (error) {
        commit(wsprLiveAcquisitionFailed(state, error));
      }
      return state;
    },

    async refreshConductor(advanceAcquisition = true, silent = false) {
      if (["loading", "refreshing", "mutating"].includes(state.conductorStatus)
        || conductorPollInFlight) return state;
      if (silent) conductorPollInFlight = true;
      else commit(beginConductorLoad(state));
      try {
        const conductor = await invokeActiveSessionConductor(invoke());
        commit(silent
          ? conductorPollSucceeded(state, conductor)
          : conductorLoadSucceeded(state, conductor));
      } catch (error) {
        if (!silent) commit(conductorMutationFailed(state, error));
      } finally {
        conductorPollInFlight = false;
      }
      if (state.conductor) {
        await controller.refreshAntennaController();
        await controller.refreshWsjtxStatus();
        if (advanceAcquisition && state.conductor.lifecycle === "running") {
          await controller.advanceWsprLive();
        }
      }
      return state;
    },

    async submitConductorAction(action) {
      if (!state.conductor || state.conductorStatus === "mutating") return state;
      const request = {
        actionToken: state.conductor.actionToken,
        expectedRevision: state.conductor.revision,
        action,
      };
      commit(beginConductorMutation(state, action.kind));
      try {
        commit(conductorLoadSucceeded(state, await invokeMutateSessionConductor(invoke(), request)));
      } catch (error) {
        commit(conductorMutationFailed(state, error));
      }
      if (state.conductorStatus === "ready") {
        await controller.refreshWsjtxStatus();
        await controller.advanceWsprLive();
        await controller.refreshReport();
      }
      return state;
    },

    async selectWorkflow(workflow) {
      commit(selectWorkflow(state, workflow));
      effects.navigate(state.activeWorkflow);
      if (state.activeWorkflow === "run" && state.session) await controller.refreshConductor();
      if (state.activeWorkflow === "report" && state.session) await controller.refreshReport();
      return state;
    },

    async routeWorkflow(workflow) {
      commit(selectWorkflow(state, workflow));
      if (state.activeWorkflow === "run" && state.session) await controller.refreshConductor();
      if (state.activeWorkflow === "report" && state.session) await controller.refreshReport();
      return state;
    },

    async requestStationLocation() {
      return invokeStationLocation(invoke());
    },

    async loadStationPreferences() {
      return invokeLoadStationPreferences(invoke());
    },

    prompt(message, initial = "") {
      return effects.prompt(message, initial);
    },

    confirm(message) {
      return effects.confirm(message);
    },

    refreshOnReturn() {
      if (effects.isVisible() && state.activeWorkflow === "run" && state.session) {
        void controller.refreshConductor(true, true);
      }
    },

    periodicRefresh() {
      if (state.activeWorkflow === "run"
        && state.conductorStatus === "ready"
        && state.conductor?.lifecycle === "running") {
        void controller.refreshConductor(true, true);
      }
      if (state.activeWorkflow === "report" && state.session) void controller.refreshReport();
    },

    tickCountdown() {
      if (state.activeWorkflow !== "run"
        || state.conductorStatus !== "ready"
        || state.conductor?.lifecycle !== "running") return;
      const anchor = effects.getCountdownAnchor();
      const projectedSeconds = projectCountdown(anchor, effects.monotonicNow());
      if (projectedSeconds === 0 && transitionRefreshKey === anchor?.key) return;
      effects.renderCountdown(projectedSeconds);
      if (projectedSeconds === 0 && anchor?.seconds > 0 && transitionRefreshKey !== anchor.key) {
        transitionRefreshKey = anchor.key;
        void controller.refreshConductor(true, true);
      }
    },

    start() {
      if (disposed || started) return controller;
      started = true;
      const refreshTimer = effects.setInterval(() => controller.periodicRefresh(), 5000);
      const countdownTimer = effects.setInterval(() => controller.tickCountdown(), 1000);
      cleanups.push(() => effects.clearInterval(refreshTimer));
      cleanups.push(() => effects.clearInterval(countdownTimer));
      cleanups.push(effects.onFocus(() => controller.refreshOnReturn()));
      cleanups.push(effects.onVisibilityChange(() => controller.refreshOnReturn()));
      cleanups.push(effects.onHashChange((workflow) => void controller.routeWorkflow(workflow)));
      return controller;
    },

    dispose() {
      if (disposed) return;
      disposed = true;
      while (cleanups.length > 0) cleanups.pop()?.();
    },
  };

  return controller;
}
