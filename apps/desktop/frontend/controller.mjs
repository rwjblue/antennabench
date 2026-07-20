import {
  invokeActiveSessionAntennaController,
  invokeActiveSessionConductor,
  invokeActiveSessionWsjtxStatus,
  invokeAdvanceSessionWsprLive,
  invokeAntennaControllerProfiles,
  invokeAttachSessionAntennaController,
  invokeCreateSessionFromReview,
  invokeCancelReportExport,
  invokeConfirmReportExport,
  invokeDeleteManagedSession,
  invokeExportManagedSession,
  invokeDeleteAntennaControllerProfile,
  invokeExportActiveSessionReport,
  invokeImportManagedSession,
  invokeImportActiveSessionRbn,
  invokeImportActiveSessionWsprLive,
  invokeLoadStationPreferences,
  invokeListManagedSessions,
  invokeMutateSessionConductor,
  invokeOpenManagedSession,
  invokeRefreshActiveSessionReport,
  invokeRevealManagedSession,
  invokeRevealManagedSessionsDirectory,
  invokeReviewSessionSetup,
  invokeRunSessionAntennaController,
  invokeSaveAntennaControllerProfile,
  invokeStartSessionWsjtx,
  invokeStationLocation,
  invokeStopSessionWsjtx,
} from "./bridge.mjs";
import { OPEN_INTENTS, projectCountdown, sessionOpenDestination } from "./models.mjs";
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
  beginManagedExport,
  beginManagedImport,
  beginManagedCatalogLoad,
  beginManagedDelete,
  beginManagedReveal,
  beginOpenSession,
  beginReportExport,
  beginReportExportCancellation,
  beginReportReplacement,
  beginReportRefresh,
  beginSkipCycleMutation,
  beginSupportSummaryCopy,
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
  initialState,
  managedCatalogLoadFailed,
  managedCatalogLoadSucceeded,
  managedExportCancelled,
  managedExportFailed,
  managedExportSucceeded,
  managedImportCancelled,
  managedImportFailed,
  managedImportSucceeded,
  managedDeleteFailed,
  managedDeleteSucceeded,
  managedRevealFailed,
  managedRevealSucceeded,
  cancelManagedDelete,
  cancelSkipCycle,
  openSessionCancelled,
  openSessionFailed,
  openSessionSucceeded,
  requestManagedDelete,
  rbnImportCancelled,
  rbnImportFailed,
  rbnImportSucceeded,
  reportExportCancelled,
  reportExportConfirmationRequired,
  reportExportFailed,
  reportExportSucceeded,
  reportRefreshFailed,
  reportRefreshSucceeded,
  requestSkipCycle,
  selectWorkflow,
  setWsjtxReadinessAcknowledged,
  setupCreationCancelled,
  setupCreationFailed,
  setupCreationSucceeded,
  setupReviewFailed,
  setupReviewSucceeded,
  skipCycleMutationFailed,
  skipCycleMutationSucceeded,
  supportSummaryCopyFailed,
  supportSummaryCopySucceeded,
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
const WSPR_LIVE_ACQUISITION_WATCHDOG_MS = 60_000;

export function createDesktopController(options = {}) {
  const effects = {
    invoke: options.invoke,
    render: options.render ?? (() => {}),
    navigate: options.navigate ?? (() => {}),
    monotonicNow: options.monotonicNow ?? (() => Date.now()),
    setInterval: options.setInterval ?? (() => null),
    clearInterval: options.clearInterval ?? (() => {}),
    setTimeout: options.setTimeout ?? globalThis.setTimeout,
    clearTimeout: options.clearTimeout ?? globalThis.clearTimeout,
    onFocus: options.onFocus ?? (() => () => {}),
    onVisibilityChange: options.onVisibilityChange ?? (() => () => {}),
    onHashChange: options.onHashChange ?? (() => () => {}),
    isVisible: options.isVisible ?? (() => true),
    prompt: options.prompt ?? (() => null),
    confirm: options.confirm ?? (() => false),
    copyText: options.copyText ?? (() => Promise.reject(new Error("Clipboard access is unavailable."))),
    getCountdownAnchor: options.getCountdownAnchor ?? (() => null),
    renderCountdown: options.renderCountdown ?? (() => {}),
    onDispose: options.onDispose ?? (() => {}),
  };
  let state = options.state ?? initialState(options.initialWorkflow);
  let transitionRefreshKey = null;
  let conductorPollInFlight = false;
  let reportPollInFlight = null;
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
  const loadWorkSession = async () => {
    await controller.refreshReport();
    const reportRevision = state.session?.revision;
    const reportLifecycle = state.session?.lifecycle;
    await controller.refreshConductor();
    if (
      state.session
      && (state.session.revision !== reportRevision || state.session.lifecycle !== reportLifecycle)
    ) {
      await controller.refreshReport();
    }
  };
  const openSession = async ({ source, intent = null, locatorId = null, open }) => {
    if (
      state.openStatus === "loading"
      || state.reportStatus === "refreshing"
      || ["loading", "refreshing", "mutating"].includes(state.conductorStatus)
      || reportPollInFlight
      || conductorPollInFlight
    ) return state;
    commit(beginOpenSession(state, source, intent, locatorId));
    let destination = null;
    try {
      const outcome = await open();
      if (outcome.status === "cancelled") {
        commit(openSessionCancelled(state));
      } else if (outcome.status === "opened" && outcome.session) {
        destination = sessionOpenDestination(outcome.session, intent);
        commit(openSessionSucceeded(
          state,
          outcome.session,
          destination.workflow,
          destination.redirected ? "work_redirected" : null,
          destination.intent,
        ));
        effects.navigate(destination.workflow);
      } else {
        throw unexpectedResponse();
      }
    } catch (error) {
      commit(openSessionFailed(state, error));
    }
    if (destination?.workflow === "run") await loadWorkSession();
    if (destination?.workflow === "report") await controller.refreshReport();
    return state;
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
          commit(setupCreationSucceeded(state, outcome.session, outcome.managedLocation ?? null));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(setupCreationFailed(state, error));
      }
      if (state.setupStatus === "created") {
        effects.navigate("run");
        await loadWorkSession();
      }
      return state;
    },

    async openManagedSession(locatorId, intent = null) {
      if (intent !== null && !OPEN_INTENTS.includes(intent)) {
        throw new RangeError(`Unknown session opening intent: ${intent}`);
      }
      return openSession({
        source: "managed",
        intent,
        locatorId,
        open: () => invokeOpenManagedSession(invoke(), locatorId),
      });
    },

    async loadManagedSessions() {
      if (
        ["loading", "refreshing"].includes(state.catalogStatus)
        || state.catalogImportStatus === "loading"
      ) return state;
      commit(beginManagedCatalogLoad(state));
      try {
        commit(managedCatalogLoadSucceeded(state, await invokeListManagedSessions(invoke())));
      } catch (error) {
        commit(managedCatalogLoadFailed(state, error));
      }
      return state;
    },

    async revealManagedSessionsDirectory() {
      if (state.catalogRowOperation || state.catalogImportStatus === "loading") return state;
      commit(beginManagedReveal(state));
      try {
        await invokeRevealManagedSessionsDirectory(invoke());
        commit(managedRevealSucceeded(state));
      } catch (error) {
        commit(managedRevealFailed(state, error));
      }
      return state;
    },

    async revealManagedSession(locatorId) {
      if (state.catalogRowOperation || state.catalogImportStatus === "loading") return state;
      commit(beginManagedReveal(state, locatorId));
      try {
        await invokeRevealManagedSession(invoke(), locatorId);
        commit(managedRevealSucceeded(state));
      } catch (error) {
        commit(managedRevealFailed(state, error));
      }
      return state;
    },

    requestManagedSessionDeletion(entry) {
      commit(requestManagedDelete(state, entry));
      return state;
    },

    cancelManagedSessionDeletion() {
      commit(cancelManagedDelete(state));
      return state;
    },

    async deleteManagedSession() {
      const locatorId = state.catalogDeleteTarget?.locatorId;
      if (!locatorId || state.catalogDeleteStatus === "deleting") return state;
      commit(beginManagedDelete(state));
      try {
        const outcome = await invokeDeleteManagedSession(invoke(), locatorId);
        if (outcome.status !== "trashed" || !outcome.bundleName) throw unexpectedResponse();
        commit(managedDeleteSucceeded(state, outcome));
        await controller.loadManagedSessions();
      } catch (error) {
        commit(managedDeleteFailed(state, error));
      }
      return state;
    },

    async importManagedSession() {
      if (
        state.catalogImportStatus === "loading"
        || state.catalogRowOperation
        || ["loading", "refreshing"].includes(state.catalogStatus)
      ) return state;
      commit(beginManagedImport(state));
      try {
        const outcome = await invokeImportManagedSession(invoke());
        if (outcome.status === "cancelled") {
          commit(managedImportCancelled(state));
        } else if (outcome.status === "imported" && outcome.location?.bundleName) {
          commit(managedImportSucceeded(state, outcome.location));
          await controller.loadManagedSessions();
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(managedImportFailed(state, error));
      }
      return state;
    },

    async exportManagedSession(locatorId) {
      if (!locatorId || state.catalogRowOperation || state.catalogImportStatus === "loading") {
        return state;
      }
      commit(beginManagedExport(state, locatorId));
      try {
        const outcome = await invokeExportManagedSession(invoke(), locatorId);
        if (outcome.status === "cancelled") {
          commit(managedExportCancelled(state));
        } else if (outcome.status === "exported" && outcome.bundleName) {
          commit(managedExportSucceeded(state, outcome.bundleName));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(managedExportFailed(state, error));
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

    async refreshReport(silent = false) {
      if (
        !state.session
        || state.openStatus === "loading"
        || state.reportStatus === "refreshing"
        || ["loading", "confirming", "replacing", "cancelling"].includes(
          state.reportExportStatus,
        )
      ) {
        return state;
      }
      if (reportPollInFlight) {
        if (silent) return state;
        await reportPollInFlight;
        return controller.refreshReport(false);
      }
      if (!silent) commit(beginReportRefresh(state));
      const sessionAtStart = state.session;
      const previousPresentationId = state.reportPresentationId;
      const poll = (async () => {
        try {
          const presentation = await invokeRefreshActiveSessionReport(invoke());
          if (state.session !== sessionAtStart) return;
          const changed = String(presentation.presentationId) !== String(previousPresentationId);
          if (!silent || changed) commit(reportRefreshSucceeded(state, presentation));
        } catch (error) {
          if (state.session === sessionAtStart) commit(reportRefreshFailed(state, error));
        }
      })();
      reportPollInFlight = poll;
      try {
        await poll;
      } finally {
        if (reportPollInFlight === poll) reportPollInFlight = null;
      }
      return state;
    },

    async exportReport(
      format = "full_evidence_html",
      controllerEvidence = "complete",
      operationalHistory = "omitted",
    ) {
      if (
        !state.session?.reportHtml
        || ["loading", "confirming", "replacing", "cancelling"].includes(
          state.reportExportStatus,
        )
      ) return state;
      commit(beginReportExport(state));
      try {
        const outcome = await invokeExportActiveSessionReport(
          invoke(), format, controllerEvidence, operationalHistory,
        );
        if (outcome.status === "cancelled") {
          commit(reportExportCancelled(state));
        } else if (outcome.status === "exported" && outcome.fileName) {
          commit(reportExportSucceeded(state, outcome));
        } else if (
          outcome.status === "confirmation_required"
          && outcome.pendingExportId
          && outcome.fileName
        ) {
          commit(reportExportConfirmationRequired(state, outcome));
        } else {
          throw unexpectedResponse();
        }
      } catch (error) {
        commit(reportExportFailed(state, error));
      }
      return state;
    },

    async confirmReportReplacement() {
      const pendingExportId = state.reportExportPending?.pendingExportId;
      if (!pendingExportId || state.reportExportStatus === "replacing") return state;
      commit(beginReportReplacement(state));
      try {
        const outcome = await invokeConfirmReportExport(invoke(), pendingExportId);
        if (outcome.status !== "exported" || !outcome.fileName) throw unexpectedResponse();
        commit(reportExportSucceeded(state, outcome));
      } catch (error) {
        commit(reportExportFailed(state, error));
      }
      return state;
    },

    async cancelReportReplacement() {
      const pendingExportId = state.reportExportPending?.pendingExportId;
      if (!pendingExportId || state.reportExportStatus === "replacing") return state;
      commit(beginReportExportCancellation(state));
      try {
        const outcome = await invokeCancelReportExport(invoke(), pendingExportId);
        if (outcome.status !== "cancelled") throw unexpectedResponse();
        commit(reportExportCancelled(state));
      } catch (error) {
        commit(reportExportFailed(state, error));
      }
      return state;
    },

    async copySupportSummary() {
      const summary = state.session?.operationalHistory?.supportSummary;
      if (typeof summary !== "string" || state.supportCopyStatus === "copying") return state;
      commit(beginSupportSummaryCopy(state));
      try {
        await effects.copyText(summary);
        commit(supportSummaryCopySucceeded(state));
      } catch (error) {
        commit(supportSummaryCopyFailed(state, error));
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
      let watchdog;
      try {
        const timeout = new Promise((_, reject) => {
          watchdog = effects.setTimeout(() => reject({
            kind: "resource",
            message: "WSPR.live collection took too long. Retry when the provider is responsive.",
            detail: "the 60-second acquisition watchdog expired before the desktop command returned",
          }), WSPR_LIVE_ACQUISITION_WATCHDOG_MS);
        });
        const outcome = await Promise.race([
          invokeAdvanceSessionWsprLive(invoke(), retry),
          timeout,
        ]);
        commit(wsprLiveAcquisitionSucceeded(state, outcome));
        if (["captured", "completed"].includes(outcome.status)) {
          await controller.refreshConductor(false);
          await controller.refreshReport();
        }
      } catch (error) {
        commit(wsprLiveAcquisitionFailed(state, error));
      } finally {
        effects.clearTimeout(watchdog);
      }
      return state;
    },

    async refreshConductor(advanceAcquisition = true, silent = false) {
      if (state.openStatus === "loading"
        || ["loading", "refreshing", "mutating"].includes(state.conductorStatus)
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

    requestSkipCycle() {
      return commit(requestSkipCycle(state));
    },

    cancelSkipCycle() {
      return commit(cancelSkipCycle(state));
    },

    async submitSkipCycle(reason = "") {
      const presented = state.skipCycleDialog;
      if (!presented || state.skipCycleStatus === "submitting") return state;
      const request = {
        actionToken: presented.actionToken,
        expectedRevision: presented.expectedRevision,
        action: {
          kind: "skip_wspr_cycle",
          intentId: presented.intentId,
          reason,
        },
      };
      commit(beginSkipCycleMutation(state));
      try {
        commit(skipCycleMutationSucceeded(
          state,
          await invokeMutateSessionConductor(invoke(), request),
        ));
      } catch (error) {
        commit(skipCycleMutationFailed(state, error));
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
      if (state.activeWorkflow === "saved") await controller.loadManagedSessions();
      if (state.activeWorkflow === "run" && state.session) await controller.refreshConductor();
      if (state.activeWorkflow === "report" && state.session) await controller.refreshReport(true);
      return state;
    },

    async routeWorkflow(workflow) {
      const destination = !state.session && ["run", "report"].includes(workflow)
        ? "saved"
        : workflow;
      commit(selectWorkflow(state, destination));
      if (state.activeWorkflow === "saved") await controller.loadManagedSessions();
      if (state.activeWorkflow === "run" && state.session) await controller.refreshConductor();
      if (state.activeWorkflow === "report" && state.session) await controller.refreshReport(true);
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
      if (effects.isVisible() && state.activeWorkflow === "saved") {
        void controller.loadManagedSessions();
      }
      if (effects.isVisible() && state.activeWorkflow === "run" && state.session) {
        void controller.refreshConductor(true, true);
      }
      if (effects.isVisible() && state.activeWorkflow === "report" && state.session) {
        void controller.refreshReport(true);
      }
    },

    periodicRefresh() {
      if (state.activeWorkflow === "run"
        && state.conductorStatus === "ready"
        && state.conductor?.lifecycle === "running") {
        void controller.refreshConductor(true, true);
      }
      if (state.activeWorkflow === "report"
        && state.session?.lifecycle === "running") void controller.refreshReport(true);
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
      cleanups.push(effects.onHashChange((workflow) => controller.routeWorkflow(workflow)));
      return controller;
    },

    dispose() {
      if (disposed) return;
      disposed = true;
      while (cleanups.length > 0) cleanups.pop()?.();
      effects.onDispose();
    },
  };

  return controller;
}
