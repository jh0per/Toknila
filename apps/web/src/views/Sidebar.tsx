import type { Intent } from "../store/msg";
import { type State, currentProject } from "../store/state";
import { updateReady } from "../store/updates";
import { NavLink } from "./Link";
import { NavIcon } from "./NavIcon";
import { SidebarProjects } from "./SidebarProjects";

export function Sidebar({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const waiting = state.inbox.length;
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

      {/* The one row for everything: what waits in a closed project is never unseen. */}
      <section className="sec">
        <h2>
          <NavLink route={{ name: "inbox" }} dispatch={dispatch} testId="inbox-link">Inbox</NavLink>
          <span className={`badge${waiting === 0 ? " zero" : ""}`} data-testid="inbox-count">{waiting}</span>
        </h2>
      </section>

      <div className="side-foot">
        <NavLink route={{ name: "usage" }} dispatch={dispatch} className={`item${state.route.name === "usage" ? " sel" : ""}`} testId="usage-link">
          <NavIcon name="usage" />
          <span className="t">Usage</span>
        </NavLink>
        <NavLink route={{ name: "settings", project: project?.key }} dispatch={dispatch} className={`item${state.route.name === "settings" ? " sel" : ""}`} testId="settings-link">
          <NavIcon name="settings" />
          <span className="t">Settings</span>
          {updateReady(state.updates) && (
            <span className="update-dot" data-testid="settings-dot" role="img" aria-label="An update is ready" title="An update is ready" />
          )}
        </NavLink>
        <NavLink route={{ name: "guide" }} dispatch={dispatch} className={`item${state.route.name === "guide" ? " sel" : ""}`} testId="guide-link">
          <NavIcon name="guide" />
          <span className="t">Guide</span>
        </NavLink>
        <button type="button" className="item shortcuts-link" data-testid="shortcuts-link" onClick={() => dispatch({ type: "toggleShortcuts" })}>
          <NavIcon name="shortcuts" />
          <span className="t">Keyboard shortcuts</span>
          <kbd>?</kbd>
        </button>
      </div>
    </nav>
  );
}
