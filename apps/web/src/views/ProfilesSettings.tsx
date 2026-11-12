import type { Intent } from "../store/msg";
import { listProfile } from "../store/profiles";
import type { State } from "../store/state";
import { currentProject } from "../store/state";
import { NavLink } from "./Link";

const ROLE: Record<string, string> = { worker: "Works on stories", reviewer: "Reviews finished stories", planner: "Plans the project" };

/// Agents: the agents Toknila can start (the profiles), which one plans, and where to add more.
export function ProfilesSettings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { profiles, profilesFolder } = state.settings;
  const project = currentProject(state);
  const shown = profiles.filter((l) => listProfile(l.profile.name, project?.default_profile, import.meta.env.DEV));
  const planners = profiles.filter((l) => l.profile.role === "planner").map((l) => l.profile.name);
  return (
    <div className="page-section">
      <section className="card-block" data-testid="profiles">
        <p className="muted">
          An agent says how a coding agent is started and what it may do. To add one, put a <code>.toml</code> file in the
          agent files folder; it applies at once.
        </p>
        <ul className="profiles">
          {shown.map((l) => (
            <li key={l.profile.name} data-profile={l.profile.name}>
              <b className="key">{l.profile.name}</b>
              <span className="muted">{ROLE[l.profile.role] ?? l.profile.role}</span>
              <span className="muted" data-testid="profile-source">{l.file ? l.file : "built in"}</span>
              <details className="fold inline">
                <summary>Command</summary>
                <code className="cmd">{l.profile.command.length ? l.profile.command.join(" ") : "starts no process"}</code>
              </details>
            </li>
          ))}
        </ul>
        <dl className="facts">
          <dt>Agent files folder</dt>
          <dd data-testid="profiles-folder"><code>{profilesFolder || "…"}</code></dd>
        </dl>
      </section>
      <section className="card-block">
        <h2>Planner</h2>
        <label className="field">
          <span className="field-label">Agent that plans</span>
          <span className="field-hint">Used when you start the planner. It applies at once.</span>
          <select
            data-testid="settings-planner-profile" value={state.planner.profile}
            onChange={(e) => dispatch({ type: "pickPlannerProfile", profile: e.target.value })}
          >
            {(planners.includes(state.planner.profile) ? planners : [state.planner.profile, ...planners]).map((n) => <option key={n}>{n}</option>)}
          </select>
        </label>
      </section>
      <p className="muted">
        The Claude model of each agent is set per project, under{" "}
        <NavLink route={{ name: "settings", project: project?.key, section: "advanced" }} dispatch={dispatch} className="inline-link">
          Projects, Advanced
        </NavLink>
        .
      </p>
    </div>
  );
}
