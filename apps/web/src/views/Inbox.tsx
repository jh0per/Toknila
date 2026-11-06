import { groupInbox } from "../store/inbox";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { InboxRow } from "./InboxRow";

/// Everything waiting on a person, grouped by kind, newest first inside a group.
export function Inbox({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const groups = groupInbox(state.inbox);
  return (
    <div className="inbox" data-testid="inbox">
      <h2>Inbox</h2>
      {state.error && <p className="error">{state.error}</p>}
      {groups.length === 0 && <p className="muted">Nothing needs a person.</p>}
      {groups.map((g) => (
        <section key={g.kind} data-testid={`inbox-${g.kind}`}>
          <h3>{g.title} <span className="muted">{g.items.length}</span></h3>
          <ul>
            {g.items.map((item) => (
              <InboxRow key={`${item.ticket}:${item.thread ?? ""}`} state={state} item={item} dispatch={dispatch} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
