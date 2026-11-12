import { type ReactNode, useState } from "react";
import type { Project } from "../api/types";
import { ESTIMATE_NOTE, costText } from "../format";
import { runningSpend, todayOf } from "../store/spend";
import { KIND_TITLE, itemKey, itemRoute } from "../store/inbox";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { agentsOf, inboxOf, runRouteIn } from "../store/tree";
import { NavLink } from "./Link";
import { NavIcon } from "./NavIcon";
import { SlotStepper } from "./SlotStepper";
import { Chev, indent } from "./TreeRow";

type Props = { project: Project; state: State; dispatch: (i: Intent) => void; onScreen: boolean };

/// A node of a project that opens to its own rows. Until the person touches it, it follows the
/// project: open while the project is on the screen.
function Node({ id, open, onToggle, head, children, testId }: {
  id: string;
  open: boolean;
  onToggle: () => void;
  head: ReactNode;
  children: ReactNode;
  testId?: string;
}) {
  return (
    <li role="treeitem" aria-expanded={open} data-testid={testId} data-node={id}>
      <div className="row" style={indent(1)}>
        <button
          type="button" className="chev-btn" tabIndex={-1} data-toggle aria-label={open ? `Close ${id}` : `Open ${id}`}
          onClick={onToggle}
        >
          <Chev open={open} />
        </button>
        {head}
      </div>
      {open && <ul role="group">{children}</ul>}
    </li>
  );
}

const leafRow = (children: ReactNode) => (
  <li role="treeitem"><div className="row" style={indent(2)}><Chev />{children}</div></li>
);

/// The Inbox node of a project: the count of what waits for the person here; open, the items, each
/// a link to where it is answered. The row itself opens the inbox page of this project.
export function InboxNode({ project, state, dispatch, onScreen }: Props) {
  const [touched, setTouched] = useState<boolean>();
  const open = touched ?? onScreen;
  const items = inboxOf(state, project);
  const sel = state.route.name === "inbox" && state.route.project === project.key;
  return (
    <Node
      id="Inbox" open={open} onToggle={() => setTouched(!open)} testId={`inbox-node-${project.key}`}
      head={
        <NavLink
          route={{ name: "inbox", project: project.key }} dispatch={dispatch} row className={`item${sel ? " sel" : ""}`}
          testId="project-inbox-link"
        >
          <NavIcon name="inbox" />
          <span className="t">Inbox</span>
          <span className={`badge${items.length === 0 ? " zero" : ""}`} data-testid="project-inbox-count">{items.length}</span>
        </NavLink>
      }
    >
      {items.length === 0 && leafRow(<span className="muted side-note">Nothing waiting.</span>)}
      {items.map((item) => {
        const route = itemRoute(state, item);
        return route && (
          <li role="treeitem" key={itemKey(item)} data-testid="project-inbox-item">
            <div className="row" style={indent(2)}>
              <Chev />
              <NavLink route={route} dispatch={dispatch} row className="item">
                <span className="dot" style={{ background: "var(--block)" }} />
                <span className="key">{item.ticket}</span>
                <span className="t">{KIND_TITLE[item.kind]}</span>
              </NavLink>
            </div>
          </li>
        );
      })}
    </Node>
  );
}

/// The Agents node of a project: how many agents run out of how many slots; open, the running
/// cards with their pulsing dot (the assistant first) and the slot stepper of this project.
export function AgentsNode({ project, state, dispatch, onScreen }: Props) {
  const [touched, setTouched] = useState<boolean>();
  const open = touched ?? onScreen;
  const { runs, used, limit } = agentsOf(state, project);
  const spent = runningSpend(todayOf(state, project.key), runs);
  return (
    <Node
      id="Agents" open={open} onToggle={() => setTouched(!open)} testId={onScreen ? "running" : `agents-node-${project.key}`}
      head={
        <button type="button" className="item group-toggle" data-row data-toggle onClick={() => setTouched(!open)}>
          <NavIcon name="agents" />
          <span className="t">Agents</span>
          <span className="n" data-testid="project-slots" title="Agents running out of slots">{used} / {limit}</span>
          {spent !== undefined && (
            <span className="side-spend" data-testid="agents-spend" title={`Spent today by the agents running now. ${ESTIMATE_NOTE}`}>
              est. {costText(spent)}
            </span>
          )}
        </button>
      }
    >
      {runs.length === 0 && leafRow(<span className="muted side-note">None running.</span>)}
      {runs.map((run) => {
        const route = runRouteIn(state, project, run);
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
          <li
            role="treeitem" key={run.card ? `${run.card.id}:${run.role}` : "assistant"}
            data-testid={run.card ? "run-card" : "run-assistant"}
          >
            <div className="row" style={indent(2)}>
              <Chev />
              {route ? <NavLink route={route} dispatch={dispatch} row className="item">{body}</NavLink> : <span className="item">{body}</span>}
            </div>
          </li>
        );
      })}
      {leafRow(<div className="item"><SlotStepper project={project} state={state} dispatch={dispatch} /></div>)}
    </Node>
  );
}
