import { type MouseEvent, type ReactNode, useEffect, useState } from "react";
import type { ChainDetail, Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { Route } from "../store/route";
import { type State, chainsOf, currentChain } from "../store/state";
import { agentsOf, projectOpen, readProjectOpen, storeProjectOpen, waitingIn } from "../store/tree";
import { NavLink } from "./Link";
import { SidebarChains } from "./SidebarChains";
import { AgentsNode, InboxNode } from "./SidebarNodes";
import { type IconName, NavIcon } from "./NavIcon";
import { Chev, indent } from "./TreeRow";

/// The letters in a project's badge: the first two of its key.
const initials = (p: Project) => p.key.slice(0, 2).toUpperCase();

/// A project's badge: its initials in a small square (the sidebar's rows and the switcher share it).
export const projectBadge = (p: Project) => (
  <span className="pbadge" aria-hidden="true" data-testid="project-badge" data-initials={initials(p)} />
);

/// What the Assistant row says it is: one assistant, at the roadmap and inside an epic.
export const ASSISTANT_HINT = "plans epics on the roadmap, stories inside an epic";

type Props = {
  project: Project;
  state: State;
  dispatch: (i: Intent) => void;
  /// The project is the one on the screen: its node is open and stays so.
  onScreen: boolean;
  onMenu: (e: MouseEvent) => void;
  onChainMenu: (c: ChainDetail) => (e: MouseEvent) => void;
};

/// One project of the tree: a row that opens and closes (the open state is remembered per project),
/// and under it the Roadmap, the Assistant, the Chains with their New chain and Import actions,
/// the project's Designs and its Settings. A closed project shows a pulsing dot while an agent runs
/// in it and a badge with what waits for the person.
export function SidebarProject({ project, state, dispatch, onScreen, onMenu, onChainMenu }: Props) {
  const key = project.key;
  const [stored, setStored] = useState(() => readProjectOpen(key));
  useEffect(() => setStored(readProjectOpen(key)), [key]);
  const [chainsOpen, setChainsOpen] = useState(true);
  const open = projectOpen(stored, onScreen);
  const toggle = () => {
    if (onScreen) return;
    const next = !open;
    setStored(next);
    storeProjectOpen(key, next);
  };
  const route = state.route;
  const chains = chainsOf(state, project);
  const chainRoute = route.name === "chain" || route.name === "projectChain" || route.name === "card";
  const selected = onScreen && chainRoute ? currentChain(state)?.chain.id : undefined;
  const running = agentsOf(state, project).runs.length;
  const waiting = waitingIn(state, project);
  const leaf = (label: string, icon: IconName, to: Route, sel: boolean, extra?: ReactNode, testId?: string, hint?: string, also?: Intent) => (
    <li role="treeitem" aria-selected={sel} data-testid={testId?.replace("-link", "-entry")}>
      <div className="row" style={indent(1)}>
        <Chev />
        <NavLink route={to} dispatch={dispatch} row className={`item${sel ? " sel" : ""}`} testId={testId} title={hint} also={also}>
          <NavIcon name={icon} />
          <span className="t">{label}</span>
          {extra}
        </NavLink>
      </div>
    </li>
  );
  return (
    <li
      role="treeitem" aria-expanded={open} data-project={key} className={project.archived ? "archived" : undefined}
    >
      <div className="row proj" style={indent(0)} onContextMenu={onMenu}>
        <button
          type="button" className="chev-btn" tabIndex={-1} data-toggle aria-label={open ? `Close ${project.name}` : `Open ${project.name}`}
          onClick={toggle}
        >
          <Chev open={open} />
        </button>
        <NavLink route={{ name: "project", project: key }} dispatch={dispatch} row title={project.name} className={`item${onScreen ? " cur" : ""}`}>
          {projectBadge(project)}
          <span className="t">{project.name}</span>
          {!open && running > 0 && <span className="dot pulse" role="img" aria-label="An agent is running" title="An agent is running" />}
          {!open && waiting > 0 && <span className="badge" title={`${waiting} waiting for you`} data-testid="project-waiting">{waiting}</span>}
        </NavLink>
        {project.archived && (
          <button
            type="button" className="btn mini" data-testid={`unarchive-${key}`}
            onClick={() => dispatch({ type: "archiveProject", project: key, archived: false })}
          >
            Unarchive
          </button>
        )}
      </div>
      {open && (
        <ul role="group">
          {leaf("Roadmap", "roadmap", { name: "project", project: key }, onScreen && route.name === "project", undefined, "roadmap-link")}
          {leaf("Assistant", "assistant", { name: "project", project: key }, onScreen && route.name === "planner", <span className="n" aria-hidden="true">✦</span>, "planner-link", ASSISTANT_HINT, { type: "openAssistant" })}
          <InboxNode project={project} state={state} dispatch={dispatch} onScreen={onScreen} />
          <AgentsNode project={project} state={state} dispatch={dispatch} onScreen={onScreen} />
          <li role="treeitem" aria-expanded={chainsOpen} data-testid="chains" data-project-chains={key}>
            <div className="row" style={indent(1)}>
              <Chev open={chainsOpen} />
              <button type="button" className="item group-toggle" data-row data-toggle onClick={() => setChainsOpen(!chainsOpen)}>
                <NavIcon name="chains" />
                <span className="t">Epics</span>
              </button>
              <span className="h-btns">
                <button
                  type="button" className="icon-btn" aria-label="Import a plan" title="Import a plan" data-testid="import-plus"
                  onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: key } })}
                >
                  ↓
                </button>
                <button
                  type="button" className="icon-btn" aria-label="New epic" title="New epic" data-testid="new-chain-plus"
                  onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: key } })}
                >
                  +
                </button>
              </span>
            </div>
            {chainsOpen && (
              <ul role="group">
                <SidebarChains chains={chains} project={key} selected={selected} dispatch={dispatch} onMenu={onChainMenu} />
              </ul>
            )}
          </li>
          {leaf("Designs", "designs", { name: "settings", project: key, section: "designs" }, false)}
          {leaf("Settings", "settings", { name: "settings", project: key }, false)}
        </ul>
      )}
    </li>
  );
}
