import { discussionOf, entries } from "../store/discussion";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { DiffView } from "./DiffView";

const OUTCOME = { accepted: "Accepted: the card has it now.", rejected: "Rejected: the card is unchanged." } as const;

/// The changes the assistant proposed to the open card (its prompt and acceptance) as bubbles in
/// the panel's chat: each a diff with Accept and Reject, and what became of it once decided.
export function CardProposals({ state, card, dispatch }: { state: State; card: string; dispatch: (i: Intent) => void }) {
  const thread = discussionOf(state.threads[card]);
  const proposals = entries(thread).flatMap((i) => (i.type === "edit" ? [i] : []));
  if (proposals.length === 0) return null;
  const decide = (id: string, action: "accept_edit" | "reject_edit") =>
    dispatch({ type: "threadAction", key: card, thread: String(thread?.thread.number ?? 1), action, text: id });
  return (
    <ol className="chat-proposals" data-testid="chat-proposals">
      {proposals.map((i) => (
        <li key={i.id} className="bubble agent proposal" data-testid="discuss-edit" data-state={i.state}>
          <div className="bubble-head"><b>Assistant</b><span className="kind">proposed a change to {card}</span></div>
          <DiffView proposal={i.proposal} />
          {i.state === "pending" ? (
            <div className="row">
              <button type="button" className="btn primary" data-testid="edit-accept" onClick={() => decide(i.id, "accept_edit")}>Accept</button>
              <button type="button" className="btn" data-testid="edit-reject" onClick={() => decide(i.id, "reject_edit")}>Reject</button>
            </div>
          ) : (
            <p className="muted" data-testid="edit-outcome">{OUTCOME[i.state]}</p>
          )}
        </li>
      ))}
    </ol>
  );
}
