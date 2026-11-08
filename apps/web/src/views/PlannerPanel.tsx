import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { openPlannerTerminal } from "../terminal/connect";
import { LiveTerminal } from "./LiveTerminal";

/// The bottom panel of the planner entry: Start and Stop, and the planner's terminal.
export function PlannerPanel({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const planner = plannerOf(state, project.key);
  const running = planner?.running ?? false;
  // The profiles with the planner role; the built-in one is the default.
  const profiles = state.settings.profiles.filter((l) => l.profile.role === "planner").map((l) => l.profile.name);
  return (
    <section className="panel" data-testid="panel">
      <div className="tabs">
        <span className="who planner-who">
          <span className="key">Planner</span>
          <span className="muted" data-testid="planner-state">
            {planner === undefined ? "…" : running ? `running · ${planner.profile}` : "not running"}
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
              type="button" className="btn primary" data-testid="planner-start" disabled={planner === undefined}
              onClick={() => dispatch({ type: "startPlanner", project: project.key })}
            >
              Start
            </button>
          </>
        )}
      </div>
      {state.planner.error && <p className="error side-note" role="alert" data-testid="planner-error">{state.planner.error}</p>}
      <div className="tabpanel" data-tab="planner">
        <LiveTerminal
          streamKey={project.key}
          connect={(sink) => openPlannerTerminal(project.key, sink)}
          noneText="not started yet"
          placeholder="The planner has not run yet. Start it to plan this project with it."
          onEnded={() => dispatch({ type: "refreshPlanner", project: project.key })}
        />
      </div>
    </section>
  );
}
