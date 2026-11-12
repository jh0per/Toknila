import { KIND_TITLE, itemKey, itemRoute } from "../store/inbox";
import type { Intent } from "../store/msg";
import { type State, currentProject, runRoute, runsOf } from "../store/state";
import { updateReady } from "../store/updates";
import { NavLink } from "./Link";
import { SidebarProjects } from "./SidebarProjects";
import { SlotStepper } from "./SlotStepper";

export function Sidebar({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const waiting = state.inbox;
  const running = runsOf(state);
  return (
    <nav className="sidebar" aria-label="Sidebar" data-link={state.link}>
      <div className="brand">
        <h1><NavLink route={{ name: "chains" }} dispatch={dispatch}>Toknila</NavLink></h1>
        {/* Nothing to say while the connection works; only losing it is news. */}
        {state.link === "offline" && (
          <span className="link l-offline" role="status" data-testid="link" title="The window lost the daemon and is reconnecting">
            Offline
          </span>
        )}
      </div>

      <SidebarProjects state={state} dispatch={dispatch} />

      <section className="sec">
        <h2>
          <NavLink route={{ name: "inbox" }} dispatch={dispatch} testId="inbox-link">Inbox</NavLink>
          <span className={`badge${waiting.length === 0 ? " zero" : ""}`} data-testid="inbox-count">{waiting.length}</span>
        </h2>
        <ul>
          {waiting.map((item) => {
            const route = itemRoute(state, item);
            return (
              <li key={itemKey(item)}>
                {route && (
                  <NavLink route={route} dispatch={dispatch} className="item">
                    <span className="dot" style={{ background: "var(--block)" }} />
                    <span className="key">{item.ticket}</span>
                    <span className="t">{KIND_TITLE[item.kind]}</span>
                  </NavLink>
                )}
              </li>
            );
          })}
        </ul>
        {waiting.length === 0 && <p className="muted side-note">Nothing waiting.</p>}
      </section>

      <section className="sec" data-testid="running">
        <h2>Agents</h2>
        {project && <SlotStepper project={project} state={state} dispatch={dispatch} />}
        {running.length === 0 && <p className="muted side-note">None running.</p>}
        <ul>
          {running.map((run) => {
            const route = runRoute(state, run);
            const body = run.card ? (
              <>
                <span className="dot pulse" />
                <span className="key">{run.card.key}</span>
                <span className="t">{run.card.title}</span>
                <span className="n" data-testid="run-role">{run.role}</span>
              </>
            ) : (
              <>
                <span className="dot pulse" />
                <span className="t">Assistant</span>
              </>
            );
            return (
              <li key={run.card ? `${run.card.id}:${run.role}` : "assistant"} data-testid={run.card ? "run-card" : "run-assistant"}>
                {route ? <NavLink route={route} dispatch={dispatch} className="item">{body}</NavLink> : <span className="item">{body}</span>}
              </li>
            );
          })}
        </ul>
      </section>

      <div className="side-foot">
        <NavLink route={{ name: "usage" }} dispatch={dispatch} className={`item${state.route.name === "usage" ? " sel" : ""}`} testId="usage-link">
          <span className="t">Usage</span>
        </NavLink>
        <NavLink route={{ name: "settings", project: project?.key }} dispatch={dispatch} className={`item${state.route.name === "settings" ? " sel" : ""}`} testId="settings-link">
          <span className="t">Settings</span>
          {updateReady(state.updates) && (
            <span className="update-dot" data-testid="settings-dot" role="img" aria-label="An update is ready" title="An update is ready" />
          )}
        </NavLink>
        <button type="button" className="item shortcuts-link" data-testid="shortcuts-link" onClick={() => dispatch({ type: "toggleShortcuts" })}>
          <span className="t">Keyboard shortcuts</span>
          <kbd>?</kbd>
        </button>
      </div>
    </nav>
  );
}
