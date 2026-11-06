import type { Ticket } from "../api/types";
import { reasonText } from "../format";
import type { Intent } from "../store/msg";
import { type State, queuePosition } from "../store/state";
import { Comments } from "./Comments";
import { DetailsMeta } from "./DetailsMeta";
import { Rounds } from "./Rounds";

/// Prompt, acceptance, subtasks, ship and comments of a card.
export function Details({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const detail = state.details[card.key];
  return (
    <div className="details" data-testid="details">
      <div className="details-head">
        <span className="key">{card.key}</span>
        <h3>{card.title}</h3>
      </div>
      <p className="muted">
        {card.status}
        {card.paused ? " · paused" : ""}
        {queuePosition(state, card.id) !== undefined ? ` · queued #${queuePosition(state, card.id)}` : ""}
        {card.status_reason ? ` · ${reasonText(card.status_reason)}` : ""}
      </p>
      <DetailsMeta card={card} ship={detail?.ship} />
      <div className="cols">
        <section>
          <h4>Prompt</h4>
          <pre className="prompt">{card.prompt || "(empty)"}</pre>
        </section>
        <section>
          <h4>Acceptance</h4>
          {card.acceptance.length === 0 ? <p className="muted">None.</p> : (
            <ul className="ck">{card.acceptance.map((a, i) => <li key={i}>{a}</li>)}</ul>
          )}
        </section>
        <section>
          <h4>Subtasks</h4>
          {!detail ? <p className="muted">Loading…</p> : detail.subtasks.length === 0 ? <p className="muted">None.</p> : (
            <ul className="ck">
              {detail.subtasks.map((s) => (
                <li key={s.id} className={s.done ? "d" : ""}>{s.text}</li>
              ))}
            </ul>
          )}
        </section>
      </div>
      <Rounds state={state} card={card} />
      <Comments key={card.key} card={card} detail={detail} dispatch={dispatch} />
    </div>
  );
}
