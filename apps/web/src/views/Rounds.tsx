import type { TicketDetail } from "../api/types";
import { awaitingText, loopText, roundText, whenText } from "../store/review";

/// The card's review: whose verdict it waits for, the rounds of the loop used, and each round so
/// far (who, verdict, when), as the daemon holds them.
export function Rounds({ detail }: { detail?: TicketDetail }) {
  const review = detail?.review;
  if (!review || (review.rounds.length === 0 && !review.awaiting)) return null;
  const waiting = awaitingText(review);
  return (
    <section className="rounds" data-testid="rounds">
      <h4>Review</h4>
      {waiting && <p className="muted" data-testid="awaiting">{waiting}</p>}
      <p className="muted" data-testid="loop-rounds">{loopText(review)}</p>
      <ul>
        {review.rounds.map((r) => (
          <li key={r.number} data-testid="round">
            {roundText(r)} <time className="muted" dateTime={new Date(r.decided_at).toISOString()}>{whenText(r.decided_at)}</time>
          </li>
        ))}
      </ul>
    </section>
  );
}
