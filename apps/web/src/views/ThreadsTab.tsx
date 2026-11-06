import type { Ticket } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { blocksHandIn } from "../store/threads";
import { ThreadCard } from "./ThreadCard";

/// The card's threads, its phase, and a notice while open threads keep it from review.
export function ThreadsTab({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const data = state.threads[card.key];
  if (!data) return <p className="muted">Loading…</p>;
  const open = blocksHandIn(data.threads);
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
      {data.threads.length === 0 && <p className="muted">No threads on this card.</p>}
      {data.threads.map((t) => (
        <ThreadCard key={t.thread.id} ticket={card.key} detail={t} picked={state.versionPick[t.thread.id]} dispatch={dispatch} />
      ))}
    </div>
  );
}
