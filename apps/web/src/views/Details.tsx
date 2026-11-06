import type { Ticket } from "../api/types";
import { reasonText } from "../format";
import type { State } from "../store/state";
import { DetailsMeta } from "./DetailsMeta";

/// Prompt, acceptance, subtasks, ship and comments of a card.
export function Details({ state, card }: { state: State; card: Ticket }) {
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
      <section className="comments">
        <h4>Comments</h4>
        {!detail ? <p className="muted">Loading…</p> : detail.comments.length === 0 ? <p className="muted">None.</p> : (
          <ul>
            {detail.comments.map((c) => (
              <li key={c.id}>
                <b>{c.body.gist}</b> <span className="muted">{c.author}</span>
                {c.body.what && <div>{c.body.what}</div>}
                {c.body.why && <div className="muted">{c.body.why}</div>}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
