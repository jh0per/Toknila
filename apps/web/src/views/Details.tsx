import type { Ticket } from "../api/types";
import { reasonText } from "../format";
import type { State } from "../store/state";

/// Prompt, acceptance, subtasks and comments of a card.
export function Details({ state, card }: { state: State; card: Ticket }) {
  const detail = state.details[card.key];
  return (
    <div className="details" data-testid="details">
      <h3>{card.key} {card.title}</h3>
      <p className="muted">
        {card.status}
        {card.paused ? " · paused" : ""}
        {card.status_reason ? ` · ${reasonText(card.status_reason)}` : ""}
      </p>
      <h4>Prompt</h4>
      <pre className="prompt">{card.prompt || "(empty)"}</pre>
      <h4>Acceptance</h4>
      {card.acceptance.length === 0 ? <p className="muted">None.</p> : (
        <ul>{card.acceptance.map((a, i) => <li key={i}>{a}</li>)}</ul>
      )}
      <h4>Subtasks</h4>
      {!detail ? <p className="muted">Loading…</p> : detail.subtasks.length === 0 ? <p className="muted">None.</p> : (
        <ul>
          {detail.subtasks.map((s) => (
            <li key={s.id}><input type="checkbox" checked={s.done} readOnly aria-label={s.text} /> {s.text}</li>
          ))}
        </ul>
      )}
      <h4>Comments</h4>
      {!detail ? <p className="muted">Loading…</p> : detail.comments.length === 0 ? <p className="muted">None.</p> : (
        <ul className="comments">
          {detail.comments.map((c) => (
            <li key={c.id}>
              <b>{c.body.gist}</b> <span className="muted">{c.author}</span>
              {c.body.what && <div>{c.body.what}</div>}
              {c.body.why && <div className="muted">{c.body.why}</div>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
