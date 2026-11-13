import { useEffect, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { DEFAULT_PLANNER_PROFILE, plannerOf } from "../store/planner";
import type { State } from "../store/state";
import { openPlannerTerminal } from "../terminal/connect";
import { ChatPane } from "./ChatPane";
import { LiveTerminal } from "./LiveTerminal";

type PlannerTab = "chat" | "terminal";

/// The bottom panel of the assistant's screen: two tabs. Chat (the one it opens on) is the
/// conversation, with the record of what was said and done and the box to say more; Terminal is
/// the planner's own screen with the Start, Stop and profile controls, for whoever wants it.
export function PlannerPanel({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const [tab, setTab] = useState<PlannerTab>(state.planner.terminalWanted ? "terminal" : "chat");
  // A link in the chat ("Assistant → Terminal") opens the Terminal tab, also when this screen was
  // not open yet.
  useEffect(() => {
    if (!state.planner.terminalWanted) return;
    setTab("terminal");
    dispatch({ type: "plannerTerminalShown" });
  }, [state.planner.terminalWanted]);
  const planner = plannerOf(state, project.key);
  const running = planner?.running ?? false;
  // The profiles with the planner role; the built-in one is the default.
  const profiles = state.settings.profiles.filter((l) => l.profile.role === "planner").map((l) => l.profile.name);
  const named = planner !== undefined && planner.profile !== DEFAULT_PLANNER_PROFILE ? ` · ${planner.profile}` : "";
  return (
    <section className="panel" data-testid="panel">
      <div role="tablist" aria-label="Assistant" className="tabs">
        {(["chat", "terminal"] as const).map((t) => (
          <button
            key={t} type="button" role="tab" id={`planner-tab-${t}`} aria-selected={tab === t} aria-controls="planner-tabpanel"
            className={tab === t ? "sel" : ""} data-testid={`planner-tab-${t}`} onClick={() => setTab(t)}
          >
            {t === "chat" ? "Chat" : "Terminal"}
          </button>
        ))}
        <span className="who planner-who">
          <span className="key">Assistant</span>
          <span className="muted" data-testid="planner-state">
            {planner === undefined ? "…" : running ? `running${named}` : "not running"}
          </span>
        </span>
      </div>
      {state.planner.error && <p className="error side-note" role="alert" data-testid="planner-error">{state.planner.error}</p>}
      <div role="tabpanel" id="planner-tabpanel" aria-labelledby={`planner-tab-${tab}`} className="planner-tabpanel" data-tab={tab}>
        {tab === "chat" ? (
          <ChatPane project={project} state={state} dispatch={dispatch} />
        ) : (
          <>
            <div className="term-controls">
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
            <LiveTerminal
              streamKey={project.key}
              connect={(sink) => openPlannerTerminal(project.key, sink)}
              noneText="nothing yet"
              placeholder="The assistant has not run yet. Start it here, or send it a message in the chat."
              onEnded={() => dispatch({ type: "refreshPlanner", project: project.key })}
            />
          </>
        )}
      </div>
    </section>
  );
}
