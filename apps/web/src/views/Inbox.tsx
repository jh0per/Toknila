import { groupInbox } from "../store/inbox";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { suggestionsOf } from "../store/suggestion";
import { inboxOf } from "../store/tree";
import { InboxRow } from "./InboxRow";
import { NavLink } from "./Link";
import { SuggestionRow } from "./SuggestionRow";

/// The planner's next steps first (one per project that has one), then everything waiting on a
/// person, grouped by kind, newest first inside a group. A project's route shows that project only
/// and says which.
export function Inbox({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const key = state.route.name === "inbox" ? state.route.project : undefined;
  const project = key ? state.projects.find((p) => p.key === key) : undefined;
  const items = project ? inboxOf(state, project) : key ? [] : state.inbox;
  const groups = groupInbox(items);
  const next = suggestionsOf(state).filter((s) => !key || s.project === key);
  return (
    <div className="inbox" data-testid="inbox">
      <h2>{key ? <>Inbox · <span data-testid="inbox-project">{project?.name ?? key}</span></> : "Inbox"}</h2>
      {key && (
        <p className="muted">
          Only this project. <NavLink route={{ name: "inbox" }} dispatch={dispatch} testId="inbox-all">All projects</NavLink>
        </p>
      )}
      {state.error && <p className="error">{state.error}</p>}
      {next.map((s) => <SuggestionRow key={s.project} state={state} suggestion={s} dispatch={dispatch} />)}
      {groups.length === 0 && next.length === 0 && <p className="muted">Nothing needs a person.</p>}
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
