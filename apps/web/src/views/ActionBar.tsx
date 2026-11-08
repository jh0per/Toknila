import { useState } from "react";
import type { Ticket, TicketDetail } from "../api/types";
import type { Intent } from "../store/msg";
import { specsFor } from "./actionLabels";
import { CancelConfirm } from "./CancelConfirm";

/// The buttons for what the daemon says the card allows now; a return asks what is wrong first.
export function ActionBar(
  { card, detail, dispatch }: { card: Ticket; detail?: TicketDetail; dispatch: (i: Intent) => void },
) {
  const [returning, setReturning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [text, setText] = useState("");
  const specs = specsFor(detail?.allowed_actions ?? []);
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
            className={`btn${a.danger ? " danger" : a.action === specs[0].action ? " primary" : ""}`}
            onClick={() => {
              if (a.action === "cancel") setCancelling(!cancelling);
              else if (a.needsText) setReturning(!returning);
              else dispatch({ type: "ticketAction", key: card.key, action: a.action });
            }}
          >
            {a.label}
          </button>
        ))}
      </div>
      {cancelling && (
        <CancelConfirm
          cardKey={card.key} onKeep={() => setCancelling(false)}
          onSend={(text) => {
            setCancelling(false);
            dispatch({ type: "ticketAction", key: card.key, action: "cancel", text });
          }}
        />
      )}
      {returning && (
        <form className="compose" onSubmit={(e) => { e.preventDefault(); if (text.trim()) send(); }}>
          <textarea aria-label="What to change" placeholder="What should the agent change?" value={text} onChange={(e) => setText(e.target.value)} />
          <button type="submit" disabled={!text.trim()}>Send back</button>
        </form>
      )}
    </div>
  );
}
