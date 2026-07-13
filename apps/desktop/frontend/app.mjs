export const WORKFLOWS = Object.freeze(["setup", "run", "transfer", "report"]);

export function initialState(workflow = "setup") {
  return selectWorkflow({ activeWorkflow: "setup" }, workflow);
}

export function selectWorkflow(state, workflow) {
  if (!WORKFLOWS.includes(workflow)) {
    throw new RangeError(`Unknown desktop workflow: ${workflow}`);
  }

  if (state.activeWorkflow === workflow) {
    return state;
  }

  return { ...state, activeWorkflow: workflow };
}

export function workflowFromHash(hash) {
  const workflow = hash.replace(/^#/, "");
  return WORKFLOWS.includes(workflow) ? workflow : "setup";
}

export function viewModel(state) {
  return WORKFLOWS.map((workflow) => ({
    workflow,
    active: workflow === state.activeWorkflow,
  }));
}

function mount(root, browserWindow) {
  let state = initialState(workflowFromHash(browserWindow.location.hash));
  const navigation = [...root.querySelectorAll("[data-workflow]")];
  const panels = [...root.querySelectorAll("[data-panel]")];

  const render = () => {
    for (const item of viewModel(state)) {
      const button = navigation.find(
        (candidate) => candidate.dataset.workflow === item.workflow,
      );
      const panel = panels.find(
        (candidate) => candidate.dataset.panel === item.workflow,
      );

      button.classList.toggle("active", item.active);
      button.setAttribute("aria-current", item.active ? "page" : "false");
      panel.hidden = !item.active;
    }
  };

  for (const button of navigation) {
    button.addEventListener("click", () => {
      state = selectWorkflow(state, button.dataset.workflow);
      browserWindow.history.replaceState(null, "", `#${state.activeWorkflow}`);
      render();
      root.querySelector("main").focus({ preventScroll: true });
    });
  }

  browserWindow.addEventListener("hashchange", () => {
    state = selectWorkflow(
      state,
      workflowFromHash(browserWindow.location.hash),
    );
    render();
  });

  render();
}

if (typeof document !== "undefined") {
  mount(document, window);
}
