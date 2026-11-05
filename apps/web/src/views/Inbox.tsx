import type { Intent } from "../store/msg";
import { type State, cardRoute, inbox } from "../store/state";
import { NavLink } from "./Link";

/// Everything waiting on a person, across chains.
export function Inbox({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const items = inbox(state);
  return (
    <div className="inbox" data-testid="inbox">
      <h2>Inbox</h2>
      {items.length === 0 && <p className="muted">Nothing needs a person.</p>}
      <ul>
        {items.map(({ ticket, chain }) => (
          <li key={ticket.id} data-key={ticket.key}>
            <NavLink route={cardRoute(state, chain, ticket)} dispatch={dispatch}>{ticket.key}</NavLink>{" "}
            {ticket.title} <span className="muted">{ticket.status} · {chain.chain.name}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
