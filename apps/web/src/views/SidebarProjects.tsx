import { type MouseEvent, useCallback, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, activeProjects, archivedProjects, currentProject } from "../store/state";
import { ContextMenu } from "./ContextMenu";
import { NavLink } from "./Link";
import { menuGroups } from "./menuGroups";
import { byGroup, projectMenu } from "./projectMenu";

type Menu = { x: number; y: number; project: Project | undefined };

/// The sidebar's projects: a "+" that adds one, a right-click menu (on the section and on each
/// row), and the archived projects when the person asks for them.
export function SidebarProjects({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const [menu, setMenu] = useState<Menu>();
  const close = useCallback(() => setMenu(undefined), []);
  const project = currentProject(state);
  const archived = archivedProjects(state);
  const show = state.plan.showArchived;
  const openMenu = (p: Project | undefined) => (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, project: p });
  };
  const row = (p: Project) => (
    <li key={p.id} data-project={p.key} className={p.archived ? "archived" : undefined} onContextMenu={openMenu(p)}>
      <NavLink
        route={{ name: "project", project: p.key }}
        dispatch={dispatch}
        className={`item${p.id === project?.id ? " sel" : ""}`}
      >
        <span className="key">{p.key}</span>
        <span className="t">{p.name}</span>
      </NavLink>
      {p.archived && (
        <button
          type="button" className="btn mini" data-testid={`unarchive-${p.key}`}
          onClick={() => dispatch({ type: "archiveProject", project: p.key, archived: false })}
        >
          Unarchive
        </button>
      )}
    </li>
  );
  // On the settings page the refusal shows next to its button instead.
  const refused = state.route.name === "settings" ? undefined : planError(state.plan, "archive");
  return (
    <section className="sec" data-testid="projects" onContextMenu={openMenu(undefined)}>
      <h2>
        Projects
        <button
          type="button" className="icon-btn" aria-label="Add project" title="Add project" data-testid="add-project-plus"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "addProject" } })}
        >
          +
        </button>
      </h2>
      <ul>{activeProjects(state).map(row)}</ul>
      {archived.length > 0 && (
        <button
          type="button" className="link-btn" data-testid="show-archived"
          onClick={() => dispatch({ type: "showArchived", on: !show })}
        >
          {show ? "Hide archived" : `Show archived (${archived.length})`}
        </button>
      )}
      {show && archived.length > 0 && <ul data-testid="archived-projects">{archived.map(row)}</ul>}
      {refused && <p className="error side-note" role="alert" data-testid="archive-error">{refused}</p>}
      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y} onClose={close}
          entries={menuGroups(...byGroup(projectMenu(menu.project)).map((g) => g.map((m) => ({
            label: m.label, testId: m.testId, danger: m.danger, run: () => dispatch(m.intent),
          }))))}
        />
      )}
    </section>
  );
}
