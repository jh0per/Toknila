import { type MouseEvent, useCallback, useState } from "react";
import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, activeProjects, archivedProjects, currentProject } from "../store/state";
import { useChainMenu } from "./ChainMenu";
import { ContextMenu } from "./ContextMenu";
import { menuGroups } from "./menuGroups";
import { byGroup, projectMenu } from "./projectMenu";
import { SidebarProject } from "./SidebarProject";
import { useTree } from "./useTree";

type Menu = { x: number; y: number; project: Project | undefined };

/// The sidebar's tree: a "+" that adds a project, one node per project (see `SidebarProject`), a
/// right-click menu (on the section, on each project and on each chain), and the archived projects
/// when the person asks for them.
export function SidebarProjects({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const [menu, setMenu] = useState<Menu>();
  const close = useCallback(() => setMenu(undefined), []);
  const chainMenu = useChainMenu(dispatch);
  const tree = useTree<HTMLElement>();
  // The inbox and the usage page are about every project, so none is on the screen there.
  const global = (state.route.name === "inbox" && !state.route.project) || state.route.name === "usage";
  const project = global ? undefined : currentProject(state);
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
  // On the settings page the refusal shows next to its button instead.
  const refused = state.route.name === "settings" ? undefined : planError(state.plan, "archive");
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
      <ul role="tree" aria-label="Projects" className="tree">{activeProjects(state).map(node)}</ul>
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
