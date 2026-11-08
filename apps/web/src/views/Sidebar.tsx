import { KIND_TITLE, itemKey, itemRoute } from "../store/inbox";
import type { Intent } from "../store/msg";
import { type State, cardRoute, chainsOf, currentChain, currentProject, runningAgents } from "../store/state";
import { useChainMenu } from "./ChainMenu";
import { NavLink } from "./Link";
import { SidebarProjects } from "./SidebarProjects";
import { SlotStepper } from "./SlotStepper";

export function Sidebar({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const chain = currentChain(state);
  const waiting = state.inbox;
  const running = runningAgents(state);
  const chainMenu = useChainMenu(dispatch);
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

      <section className="sec" data-testid="chains">
        <h2>
          Chains
          {project && (
            <span className="h-btns">
              <button
                type="button" className="icon-btn" aria-label="Import a plan" title="Import a plan" data-testid="import-plus"
                onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
              >
                ↓
              </button>
              <button
                type="button" className="icon-btn" aria-label="New chain" title="New chain" data-testid="new-chain-plus"
                onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: project.key } })}
              >
                +
              </button>
            </span>
          )}
        </h2>
        <ul>
          {project && (
            <li key="planner" data-testid="planner-entry">
              <NavLink
                route={{ name: "planner", project: project.key }}
                dispatch={dispatch}
                className={`item${state.route.name === "planner" ? " sel" : ""}`}
                testId="planner-link"
              >
                <span className="t key">Planner</span>
                <span className="n" title="The project's main agent: it plans, you start the cards">✦</span>
              </NavLink>
            </li>
          )}
          {chainsOf(state, project).map((c) => (
            <li key={c.chain.id} data-chain-item={c.chain.name} onContextMenu={chainMenu.open(c)}>
              <NavLink
                route={{ name: "projectChain", project: project?.key ?? "", chain: c.chain.name }}
                dispatch={dispatch}
                className={`item${c.chain.id === chain?.chain.id && state.route.name !== "planner" ? " sel" : ""}`}
              >
                <span className="t key">{c.chain.name}</span>
                <span className="n">{c.summary.done}/{c.summary.total}</span>
              </NavLink>
            </li>
          ))}
        </ul>
        {chainMenu.menu}
      </section>

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
          {running.map(({ ticket, chain: c }) => (
            <li key={ticket.id}>
              <NavLink route={cardRoute(state, c, ticket)} dispatch={dispatch} className="item">
                <span className="dot pulse" />
                <span className="key">{ticket.key}</span>
                <span className="t">{ticket.title}</span>
              </NavLink>
            </li>
          ))}
        </ul>
      </section>

      <div className="side-foot">
        <NavLink route={{ name: "usage" }} dispatch={dispatch} className={`item${state.route.name === "usage" ? " sel" : ""}`} testId="usage-link">
          <span className="t">Usage</span>
        </NavLink>
        <NavLink route={{ name: "settings", project: project?.key }} dispatch={dispatch} className={`item${state.route.name === "settings" ? " sel" : ""}`} testId="settings-link">
          <span className="t">Settings</span>
        </NavLink>
      </div>
    </nav>
  );
}
