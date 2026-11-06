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
      <div className="brand">
        <h1><NavLink route={{ name: "chains" }} dispatch={dispatch}>Toknila</NavLink></h1>
        <span className={`link l-${state.link}`} data-testid="link">{state.link}</span>
      </div>

      <section className="sec" data-testid="projects">
        <h2>Projects</h2>
        <ul>
          {state.projects.map((p) => (
            <li key={p.id}>
              <NavLink
                route={{ name: "project", project: p.key }}
                dispatch={dispatch}
                className={`item${p.id === project?.id ? " sel" : ""}`}
              >
                <span className="key">{p.key}</span>
                <span className="t">{p.name}</span>
              </NavLink>
            </li>
          ))}
        </ul>
      </section>

      <section className="sec" data-testid="chains">
        <h2>Chains</h2>
        <ul>
          {chainsOf(state, project).map((c) => (
            <li key={c.chain.id}>
              <NavLink
                route={{ name: "projectChain", project: project?.key ?? "", chain: c.chain.name }}
                dispatch={dispatch}
                className={`item${c.chain.id === chain?.chain.id ? " sel" : ""}`}
              >
                <span className="t key">{c.chain.name}</span>
                <span className="n">{c.summary.done}/{c.summary.total}</span>
              </NavLink>
            </li>
          ))}
        </ul>
      </section>

      <section className="sec">
        <h2>
          <NavLink route={{ name: "inbox" }} dispatch={dispatch} testId="inbox-link">Inbox</NavLink>
          <span className={`badge${waiting.length === 0 ? " zero" : ""}`} data-testid="inbox-count">{waiting.length}</span>
        </h2>
        <ul>
          {waiting.map(({ ticket, chain: c }) => (
            <li key={ticket.id}>
              <NavLink route={cardRoute(state, c, ticket)} dispatch={dispatch} className="item">
                <span className="dot" style={{ background: "var(--block)" }} />
                <span className="key">{ticket.key}</span>
                <span className="t">{ticket.status}</span>
              </NavLink>
            </li>
          ))}
        </ul>
        {waiting.length === 0 && <p className="muted side-note">Nothing waiting.</p>}
      </section>

      <section className="sec" data-testid="running">
        <h2>Agents</h2>
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
        <span className="item" title="Settings will live here">
          <span className="t">Settings</span>
        </span>
      </div>
    </nav>
  );
}
