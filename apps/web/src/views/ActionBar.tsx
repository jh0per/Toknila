import { useState } from "react";
import type { Ticket } from "../api/types";
import { actionsFor } from "../store/actions";
import type { Intent } from "../store/msg";

/// The buttons for what the card's status allows; a return asks what is wrong first.
export function ActionBar({ card, dispatch }: { card: Ticket; dispatch: (i: Intent) => void }) {
  const [returning, setReturning] = useState(false);
  const [text, setText] = useState("");
  const specs = actionsFor(card);
  if (specs.length === 0) return null;
  const send = () => {
    dispatch({ type: "ticketAction", key: card.key, action: "return", text: text.trim() });
    setReturning(false);
    setText("");
  };
  return (
    <div className="actions" data-testid="actions">
      <div className="btns">
        {specs.map((a) => (
          <button
            key={a.action}
            className={a.danger ? "danger" : a.action === specs[0].action ? "primary" : ""}
            onClick={() =>
              a.needsText ? setReturning(!returning) : dispatch({ type: "ticketAction", key: card.key, action: a.action })}
          >
            {a.label}
          </button>
        ))}
      </div>
      {returning && (
        <form className="compose" onSubmit={(e) => { e.preventDefault(); if (text.trim()) send(); }}>
          <textarea aria-label="What to change" placeholder="What should the agent change?" value={text} onChange={(e) => setText(e.target.value)} />
          <button type="submit" disabled={!text.trim()}>Send back</button>
        </form>
      )}
    </div>
  );
}
