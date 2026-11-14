import { type MouseEvent, type ReactNode, useCallback, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, archivedProjects, currentProject } from "../store/state";
import { sidebarProjects } from "../store/switcher";
import { agentsOf, waitingIn } from "../store/tree";
import { useChainMenu } from "./ChainMenu";
import { ContextMenu } from "./ContextMenu";
import { menuGroups } from "./menuGroups";
import { byGroup, projectMenu } from "./projectMenu";
import { HiddenProjectRow, SidebarProject, runningDot } from "./SidebarProject";
import { useTree } from "./useTree";

type Menu = { x: number; y: number; project: Project | undefined };

/// The sidebar's tree: a "+" that adds a project, one node per listed project (see
/// `SidebarProject`), a right-click menu (on the section, on each project and on each chain), and
/// the hidden and the archived projects when the person asks for them.
export function SidebarProjects({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const [menu, setMenu] = useState<Menu>();
  const close = useCallback(() => setMenu(undefined), []);
  const chainMenu = useChainMenu(dispatch);
  const tree = useTree<HTMLElement>();
  // The inbox and the usage page are about every project, so none is on the screen there.
  const global = (state.route.name === "inbox" && !state.route.project) || state.route.name === "usage";
  const project = global ? undefined : currentProject(state);
  // The open project's tree and the other listed projects as collapsed rows; past three others,
  // the rest wait behind one "+ N more" row that opens the switcher. The daemon's order stays. A
  // hidden project is in the list only while it is on the screen; the others wait under the list.
  const { rows: listed, more, hidden } = sidebarProjects(state, project);
  const archived = archivedProjects(state);
  const show = state.plan.showArchived;
  const openMenu = (p: Project | undefined) => (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, project: p });
  };
  const node = (p: Project) => (
    <SidebarProject
      key={p.id} project={p} state={state} dispatch={dispatch} onScreen={p.id === project?.id}
      onMenu={openMenu(p)} onChainMenu={chainMenu.open}
    />
  );
  // On the settings page Archive's refusal shows next to its button instead; Hide has no button
  // there, so its refusal always shows here.
  const refused = (state.route.name === "settings" ? undefined : planError(state.plan, "archive")) ?? planError(state.plan, "hide");
  return (
    <section className="sec" data-testid="projects" onContextMenu={openMenu(undefined)} {...tree}>
      <h2>
        Projects
        <button
          type="button" className="icon-btn" aria-label="Add project" title="Add project" data-testid="add-project-plus"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "addProject" } })}
        >
          +
        </button>
      </h2>
      <ul role="tree" aria-label="Projects" className="tree">
        {listed.map(node)}
        {more > 0 && (
          <li role="none" className="more-projects">
            <button type="button" className="item" data-testid="sidebar-more" onClick={() => dispatch({ type: "toggleSwitcher" })}>
              <span className="t">+ {more} more project{more === 1 ? "" : "s"}</span>
            </button>
          </li>
        )}
      </ul>
      <HiddenProjects
        hidden={hidden} state={state} dispatch={dispatch}
        row={(p) => <HiddenProjectRow key={p.id} project={p} state={state} dispatch={dispatch} onMenu={openMenu(p)} />}
      />
      {archived.length > 0 && (
        <button
          type="button" className="link-btn" data-testid="show-archived"
          onClick={() => dispatch({ type: "showArchived", on: !show })}
        >
          {show ? "Hide archived" : `Show archived (${archived.length})`}
        </button>
      )}
      {show && archived.length > 0 && (
        <ul role="tree" aria-label="Archived projects" className="tree" data-testid="archived-projects">{archived.map(node)}</ul>
      )}
      {refused && <p className="error side-note" role="alert" data-testid="archive-error">{refused}</p>}
      {chainMenu.menu}
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

/// "Show hidden (N)" under the list, and the hidden projects as rows once opened. Closed, the row
/// says what waits in them and whether an agent of one runs, so a hidden project never hides a
/// question; opened, each row says its own.
function HiddenProjects({ hidden, state, dispatch, row }: {
  hidden: Project[];
  state: State;
  dispatch: (i: Intent) => void;
  row: (p: Project) => ReactNode;
}) {
  if (hidden.length === 0) return null;
  const open = state.plan.showHidden;
  const waiting = hidden.reduce((n, p) => n + waitingIn(state, p), 0);
  const running = hidden.some((p) => agentsOf(state, p).runs.length > 0);
  return (
    <>
      <button
        type="button" className="link-btn hidden-toggle" data-testid="show-hidden" aria-expanded={open}
        onClick={() => dispatch({ type: "showHidden", on: !open })}
      >
        <span>{open ? "Close hidden" : `Show hidden (${hidden.length})`}</span>
        {!open && running && runningDot("An agent is running in a hidden project", "hidden-running")}
        {!open && waiting > 0 && (
          <span className="badge" title={`${waiting} waiting for you in hidden projects`} data-testid="hidden-waiting">{waiting}</span>
        )}
      </button>
      {open && <ul role="tree" aria-label="Hidden projects" className="tree" data-testid="hidden-projects">{hidden.map(row)}</ul>}
    </>
  );
}
