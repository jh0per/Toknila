import { useState } from "react";
import type { ChainDetail, Place, Ticket, TicketDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import { type State, ticketsInOrder } from "../store/state";

type Props = { state: State; card: Ticket; detail: TicketDetail | undefined; chain: ChainDetail | undefined; dispatch: (i: Intent) => void };

/// Editing and moving a card, shown only when core allows them (`can_edit`, `can_move`).
export function CardPlan({ state, card, detail, chain, dispatch }: Props) {
  if (!detail) return null;
  const p = state.plan;
  if (p.editing === card.key && detail.can_edit) return <CardEditor state={state} card={card} dispatch={dispatch} />;
  if (p.moving === card.key && detail.can_move && chain) {
    return <CardMover state={state} card={card} chain={chain} dispatch={dispatch} />;
  }
  if (!detail.can_edit && !detail.can_move) return null;
  return (
    <div className="plan-btns">
      {detail.can_edit && (
        <button type="button" className="btn" data-testid="edit-card" onClick={() => dispatch({ type: "editCard", key: card.key })}>
          Edit
        </button>
      )}
      {detail.can_move && chain && chain.tickets.length > 1 && (
        <button type="button" className="btn" data-testid="move-card" onClick={() => dispatch({ type: "pickPlace", key: card.key })}>
          Move
        </button>
      )}
    </div>
  );
}

/// The title and the description (the agent's prompt, Markdown) of a card, open for changes.
function CardEditor({ state, card, dispatch }: { state: State; card: Ticket; dispatch: (i: Intent) => void }) {
  const [title, setTitle] = useState(card.title);
  const [prompt, setPrompt] = useState(card.prompt);
  const refused = planError(state.plan, "edit") ?? planError(state.plan, "edit", "title");
  return (
    <form
      className="plan-form" data-testid="card-editor"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "saveCard", key: card.key, patch: { title: title.trim(), prompt } });
      }}
    >
      <label className="field">
        <span>Title</span>
        <input data-testid="edit-title" value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
      </label>
      <label className="field">
        <span>Description (Markdown)</span>
        <textarea data-testid="edit-prompt" rows={6} value={prompt} onChange={(e) => setPrompt(e.target.value)} />
      </label>
      {refused && <p className="error" role="alert" data-testid="edit-error">{refused}</p>}
      <div className="row">
        <button className="btn primary" type="submit" disabled={state.plan.busy} data-testid="save-card">Save</button>
        <button className="btn" type="button" data-testid="edit-close" onClick={() => dispatch({ type: "editCard", key: undefined })}>
          Close without saving
        </button>
      </div>
    </form>
  );
}

/// A new place for a card: directly before or after another card of its chain.
function CardMover({ state, card, chain, dispatch }: { state: State; card: Ticket; chain: ChainDetail; dispatch: (i: Intent) => void }) {
  const others = ticketsInOrder(chain).filter((t) => t.id !== card.id);
  const [place, setPlace] = useState<Place>("before");
  const [anchor, setAnchor] = useState(others[0]?.key ?? "");
  const refused = planError(state.plan, "move");
  return (
    <form
      className="plan-form" data-testid="card-mover"
      onSubmit={(e) => {
        e.preventDefault();
        if (anchor) dispatch({ type: "moveCard", chain: chain.chain.id, key: card.key, anchor, place });
      }}
    >
      <div className="row">
        <span>Move {card.key}</span>
        <select aria-label="Place" data-testid="move-place" value={place} onChange={(e) => setPlace(e.target.value as Place)}>
          <option value="before">before</option>
          <option value="after">after</option>
        </select>
        <select aria-label="Card" data-testid="move-anchor" value={anchor} onChange={(e) => setAnchor(e.target.value)}>
          {others.map((t) => <option key={t.id} value={t.key}>{t.key} · {t.title}</option>)}
        </select>
      </div>
      {refused && <p className="error" role="alert" data-testid="move-error">{refused}</p>}
      <div className="row">
        <button className="btn primary" type="submit" disabled={state.plan.busy || !anchor} data-testid="move-submit">Move</button>
        <button className="btn" type="button" data-testid="move-close" onClick={() => dispatch({ type: "pickPlace", key: undefined })}>
          Close
        </button>
      </div>
    </form>
  );
}
