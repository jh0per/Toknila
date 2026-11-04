import type { Intent } from "../store/msg";
import { type State, cardRoute, chainsOf, currentChain, currentProject, inbox, runningAgents } from "../store/state";
import { NavLink } from "./Link";

export function Sidebar({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const chain = currentChain(state);
  const waiting = inbox(state);
  const running = runningAgents(state);
  return (
    <nav className="sidebar" aria-label="Sidebar">
      <h1><NavLink route={{ name: "chains" }} dispatch={dispatch}>Toknila</NavLink></h1>
      <span className={`link l-${state.link}`} data-testid="link">{state.link}</span>

      <section data-testid="projects">
        <h2>Projects</h2>
        <ul>
          {state.projects.map((p) => (
            <li key={p.id} className={p.id === project?.id ? "sel" : ""}>
              <NavLink route={{ name: "project", project: p.key }} dispatch={dispatch}>{p.name}</NavLink>
            </li>
          ))}
        </ul>
      </section>

      <section data-testid="chains">
        <h2>Chains</h2>
        <ul>
          {chainsOf(state, project).map((c) => (
            <li key={c.chain.id} className={c.chain.id === chain?.chain.id ? "sel" : ""}>
              <NavLink
                route={{ name: "projectChain", project: project?.key ?? "", chain: c.chain.name }}
                dispatch={dispatch}
              >
                {c.chain.name}
              </NavLink>
              <span className="muted"> {c.summary.done}/{c.summary.total}</span>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>
          <NavLink route={{ name: "inbox" }} dispatch={dispatch} testId="inbox-link">Inbox</NavLink>{" "}
          <span className="count" data-testid="inbox-count">{waiting.length}</span>
        </h2>
        <ul>
          {waiting.map(({ ticket, chain: c }) => (
            <li key={ticket.id}>
              <NavLink route={cardRoute(state, c, ticket)} dispatch={dispatch}>{ticket.key}</NavLink>
              <span className="muted"> {ticket.status}</span>
            </li>
          ))}
        </ul>
      </section>

      <section data-testid="running">
        <h2>Running agents</h2>
        {running.length === 0 && <p className="muted">None.</p>}
        <ul>
          {running.map(({ ticket, chain: c }) => (
            <li key={ticket.id}>
              <NavLink route={cardRoute(state, c, ticket)} dispatch={dispatch}>{ticket.key}</NavLink>
              <span className="muted"> {ticket.title}</span>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>Settings</h2>
        <p className="muted">Settings will live here.</p>
      </section>
    </nav>
  );
}
