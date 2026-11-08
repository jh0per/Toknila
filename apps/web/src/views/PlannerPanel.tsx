import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { DEFAULT_PLANNER_PROFILE, plannerOf, showsTerminal } from "../store/planner";
import type { State } from "../store/state";
import { openPlannerTerminal } from "../terminal/connect";
import { AskBox } from "./AskBox";
import { LiveTerminal } from "./LiveTerminal";

/// The bottom panel of the assistant's screen. Until it has run there is no terminal to look at:
/// a centred box asks what to build or change, and sending starts it. Once it runs, its terminal
/// is the record of the session (and keeps working for whoever types in it), and the same box sits
/// under it as the way to talk to it.
export function PlannerPanel({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const planner = plannerOf(state, project.key);
  const running = planner?.running ?? false;
  const terminal = showsTerminal(state, project.key);
  // The profiles with the planner role; the built-in one is the default.
  const profiles = state.settings.profiles.filter((l) => l.profile.role === "planner").map((l) => l.profile.name);
  const named = planner !== undefined && planner.profile !== DEFAULT_PLANNER_PROFILE ? ` · ${planner.profile}` : "";
  return (
    <section className="panel" data-testid="panel">
      <div className="tabs">
        <span className="who planner-who">
          <span className="key">Assistant</span>
          <span className="muted" data-testid="planner-state">
            {planner === undefined ? "…" : running ? `running${named}` : "not running"}
          </span>
        </span>
        <span className="grow" />
        {running ? (
          <button type="button" className="btn" data-testid="planner-stop" onClick={() => dispatch({ type: "stopPlanner", project: project.key })}>
            Stop
          </button>
        ) : (
          <>
            {profiles.length > 1 && (
              <select
                aria-label="Planner profile" data-testid="planner-profile" value={state.planner.profile}
                onChange={(e) => dispatch({ type: "pickPlannerProfile", profile: e.target.value })}
              >
                {profiles.map((p) => <option key={p} value={p}>{p}</option>)}
              </select>
            )}
            <button
              type="button" className="btn" data-testid="planner-start" disabled={planner === undefined}
              title="Start the assistant without a message"
              onClick={() => dispatch({ type: "startPlanner", project: project.key })}
            >
              Start
            </button>
          </>
        )}
      </div>
      {state.planner.error && <p className="error side-note" role="alert" data-testid="planner-error">{state.planner.error}</p>}
      {terminal ? (
        <div className="tabpanel" data-tab="planner">
          <LiveTerminal
            streamKey={project.key}
            connect={(sink) => openPlannerTerminal(project.key, sink)}
            noneText="nothing yet"
            placeholder="The assistant has not written anything yet."
            onEnded={() => dispatch({ type: "refreshPlanner", project: project.key })}
          />
          <AskBox
            project={project.key} state={state} dispatch={dispatch} testId="planner-ask" compact
            label="Message to the assistant" placeholder="Tell the assistant what to change; Enter sends"
            button="Send"
          />
        </div>
      ) : (
        <div className="tabpanel assist-idle" data-tab="assistant" data-testid="planner-idle">
          <h3>Tell the assistant what you want to build or change</h3>
          <AskBox
            project={project.key} state={state} dispatch={dispatch} testId="planner-ask" rows={3}
            label="Tell the assistant what you want to build or change"
            placeholder="For example: add a settings page where people change their name"
            button="Send to the assistant"
          />
          <p className="muted assist-note" data-testid="placeholder">
            The assistant reads your project and writes the plan as cards. You start the cards and accept the work.
          </p>
        </div>
      )}
    </section>
  );
}
