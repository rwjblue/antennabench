export function createReportTransitionCoordinator({
  document,
  browserWindow,
  getActiveWorkflow,
}) {
  let activeTransition = null;
  let generation = 0;

  const cancel = () => {
    generation += 1;
    activeTransition?.skipTransition?.();
    activeTransition = null;
    delete document.documentElement.dataset.reportTransition;
  };

  const navigate = async (workflow, update) => {
    const fromWorkflow = getActiveWorkflow();
    const direction = fromWorkflow !== "report" && workflow === "report"
      ? "enter"
      : fromWorkflow === "report" && workflow !== "report"
        ? "exit"
        : null;
    cancel();
    const reducedMotion = browserWindow.matchMedia?.("(prefers-reduced-motion: reduce)").matches
      === true;
    if (
      !direction
      || reducedMotion
      || typeof document.startViewTransition !== "function"
    ) {
      return update();
    }

    generation += 1;
    const transitionGeneration = generation;
    document.documentElement.dataset.reportTransition = direction;
    let operation = Promise.resolve();
    const transition = document.startViewTransition(() => {
      operation = Promise.resolve(update());
    });
    activeTransition = transition;
    try {
      await Promise.resolve(transition.updateCallbackDone).catch(() => {});
      await Promise.all([
        operation,
        Promise.resolve(transition.finished).catch(() => {}),
      ]);
    } finally {
      if (transitionGeneration === generation) {
        activeTransition = null;
        delete document.documentElement.dataset.reportTransition;
      }
    }
    return undefined;
  };

  return { cancel, navigate };
}
