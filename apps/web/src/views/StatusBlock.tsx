import { useEffect, useState } from "react";
import type { Ticket, TicketDetail } from "../api/types";
import { statusLine } from "../store/cardStatus";
import type { Intent } from "../store/msg";
import { type State, currentChain, queuePosition } from "../store/state";
import { Answer } from "./Answer";

/// What a card's Details lead with: one sentence about where the card stands and the next step,
/// and, when the card asks a question, the answer box right here.
export function StatusBlock({ state, card, detail, dispatch }: {
  state: State;
  card: Ticket;
  detail: TicketDetail | undefined;
  dispatch: (i: Intent) => void;
}) {
  const chain = currentChain(state);
  // A running check shows how long it has run, so the sentence is read again every few seconds.
  const [now, setNow] = useState(() => Date.now());
  const running = detail?.review.check?.phase === "running";
  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => setNow(Date.now()), 5000);
    return () => clearInterval(id);
  }, [running]);
  const line = statusLine(card, detail, { queue: queuePosition(state, card.id), autostart: chain?.chain.autostart ?? false, now });
  // The question as the daemon offers it (its answers included), when the card waits on one.
  const asked = card.status === "waiting" ? state.inbox.find((i) => i.ticket === card.key && i.kind === "question") : undefined;
  return (
    <section className={`status-block tone-${line.tone}`} data-testid="status-block" data-status={card.status}>
      <p className="status-sentence" data-testid="status-sentence">{line.text}</p>
      {line.next && !asked && <p className="muted status-next" data-testid="status-next">{line.next}</p>}
      {line.tab && !asked && (
        <button type="button" className="btn primary" data-testid="status-tab-link" onClick={() => dispatch({ type: "selectTab", tab: line.tab!.tab })}>
          {line.tab.label}
        </button>
      )}
      {asked && <Answer item={asked} dispatch={dispatch} />}
      {card.status === "waiting" && !asked && <p className="muted">Write to the agent under Comments below.</p>}
    </section>
  );
}
