import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { suggestionOfProject } from "../store/suggestion";
import { NavLink } from "./Link";
import { PlannerPanel } from "./PlannerPanel";
import { SuggestionRow } from "./SuggestionRow";
import { SpendLine } from "./SpendLine";
import { ThemeButton } from "./ThemeButton";

/// The assistant's own screen, at /projects/KEY/planner: the chat of the panel at full width, with
/// the Terminal tab. It is the same assistant as the panel beside the roadmap and the epics; the
/// way back puts it beside the roadmap again.
export function PlannerScreen({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const next = suggestionOfProject(state, project.key);
  return (
    <>
      <header className="top">
        <div className="crumb">
          <b>{project.key}</b><i>/</i><b data-testid="planner-title">Assistant</b>
        </div>
        <SpendLine state={state} dispatch={dispatch} />
        <NavLink
          route={{ name: "project", project: project.key }} dispatch={dispatch} className="btn" testId="planner-collapse"
          title="Back to the roadmap, with the assistant beside it" also={{ type: "openAssistant" }}
        >
          ⤡ Beside the roadmap
        </NavLink>
        {state.error && <span className="error">{state.error}</span>}
        <span className="grow" />
        <ThemeButton theme={state.theme} dispatch={dispatch} />
      </header>
      {next && (
        <div className="planner-next">
          <SuggestionRow state={state} suggestion={next} dispatch={dispatch} />
        </div>
      )}
      <PlannerPanel key={project.key} project={project} state={state} dispatch={dispatch} />
    </>
  );
}
