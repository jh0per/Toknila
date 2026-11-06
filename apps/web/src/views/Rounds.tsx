import { reviewRounds } from "../store/rounds";
import { type State, ticketEvents } from "../store/state";
import type { Ticket } from "../api/types";

/// "Round 2: returned — comment" for each time the person sent the card back from review.
export function Rounds({ state, card }: { state: State; card: Ticket }) {
  const rounds = reviewRounds(ticketEvents(state, card.id), card.id);
  if (rounds.length === 0) return null;
  return (
    <section className="rounds" data-testid="rounds">
      <h4>Review rounds</h4>
      <ul>
        {rounds.map((r) => (
          <li key={r.seq}>Round {r.round}: returned{r.comment ? ` — ${r.comment}` : ""}</li>
        ))}
      </ul>
    </section>
  );
}
