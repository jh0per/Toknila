import type { TicketDetail } from "../api/types";
import { awaitingText, roundText } from "../store/review";

/// The card's review: who reviews it, whose verdict it waits for, and each round so far, as the
/// daemon holds them.
export function Rounds({ detail }: { detail?: TicketDetail }) {
  const review = detail?.review;
  if (!review || (review.rounds.length === 0 && !review.awaiting)) return null;
  const waiting = awaitingText(review);
  return (
    <section className="rounds" data-testid="rounds">
      <h4>Review</h4>
      {waiting && <p className="muted" data-testid="awaiting">{waiting}</p>}
      <ul>
        {review.rounds.map((r) => (
          <li key={r.number}>{roundText(r)}</li>
        ))}
      </ul>
    </section>
  );
}
