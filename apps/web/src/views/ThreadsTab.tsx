import type { Ticket } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { blocksHandIn } from "../store/threads";
import { ThreadCard } from "./ThreadCard";

/// A card without threads: where a person asks for a design (a diagram or a mockup to approve
/// before any code), or the word that one is asked for already.
export function NoThreads({ card, canAsk, dispatch }: { card: Ticket; canAsk: boolean; dispatch: (i: Intent) => void }) {
  if (canAsk) {
    return (
      <div className="no-threads" data-testid="no-threads">
        <p className="muted">
          No design yet. Ask for one and the agent first shows you a diagram or a mockup of what it will build, then waits for
          your approval before it writes any code.
        </p>
        <button className="btn primary" data-testid="ask-design" onClick={() => dispatch({ type: "ticketAction", key: card.key, action: "ask_design" })}>
          Ask for a design
        </button>
      </div>
    );
  }
  return (
    <p className="muted" data-testid="no-threads">
      {card.design_first
        ? "A design is asked for. The agent will show it here and wait for your approval before it writes any code."
        : "No threads on this card."}
    </p>
  );
}

/// The card's threads, its phase, and a notice while open threads keep it from review.
export function ThreadsTab({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const data = state.threads[card.key];
  if (!data) return <p className="muted">Loading…</p>;
  // The discussion with the assistant has its own box on the card's page; it is not a design.
  const designs = data.threads.filter((t) => t.thread.kind !== "discussion" && t.thread.state !== "withdrawn");
  const open = blocksHandIn(designs);
  return (
    <div className="threads" data-testid="threads">
      <div className="phase-row">
        <span className="muted">Phase</span>
        <span className="phase" data-testid="phase" data-phase={data.phase}>
          <span className={data.phase === "design" ? "on" : ""}>design</span>
          {" → "}
          <span className={data.phase === "implement" ? "on" : ""}>implement</span>
        </span>
      </div>
      {open > 0 && (
        <p className="note" data-testid="hand-in-notice">
          {open} open {open === 1 ? "thread blocks" : "threads block"} hand-in to review until approved.
        </p>
      )}
      {designs.length === 0 && <NoThreads card={card} canAsk={state.details[card.key]?.allowed_actions.includes("ask_design") ?? false} dispatch={dispatch} />}
      {designs.map((t) => (
        <ThreadCard key={t.thread.id} ticket={card.key} detail={t} picked={state.versionPick[t.thread.id]} dispatch={dispatch} />
      ))}
    </div>
  );
}
